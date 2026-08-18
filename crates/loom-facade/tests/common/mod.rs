//! Shared test scaffolding for the loom-facade oneshot router tests: an
//! in-memory `AppState` built from the golden fixture + a tempdir, plus port
//! stubs (a call-counting `VectorIndex`, a canned/erroring `EmbeddingProvider`).
//!
//! Every EXP file drives the real `build_router` through
//! `tower::ServiceExt::oneshot` — no socket, no network (except EXP-006's
//! wiremock backend, which is a localhost mock).

#![allow(dead_code)] // each integration test compiles this module independently

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use loom_backend_openai::OpenAiBackend;
use loom_domain::{
    ConceptMatch, EmbeddingProvider, Generation, GenerationId, GenerationSource, Iri, LexicalIndex,
    LoomError, MatchProvenance, ModelBackend, VectorIndex,
};
use loom_facade::mirror::MirrorStore;
use loom_facade::state::AppState;
use loom_facade::{build_router, Config};
use loom_graph_oxigraph::OxigraphStore;
use loom_scaffold::policy::InjectionPolicy;
use loom_scaffold::LexicalRetriever;

/// The 7-class golden fixture (EXP-002 anchor), embedded at compile time.
pub const FIXTURE: &str = include_str!("../../../../tests/golden-python/fixture.json");

/// A `VectorIndex` stub: canned hits, a configurable generation + readiness, and
/// a call counter so a test can PROVE the hot path never touched it.
pub struct StubVector {
    pub ready: bool,
    pub generation: Generation,
    pub hits: Vec<ConceptMatch>,
    pub calls: Arc<AtomicUsize>,
}

impl StubVector {
    pub fn new(ready: bool, generation: Generation, hits: Vec<ConceptMatch>) -> Self {
        Self {
            ready,
            generation,
            hits,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl VectorIndex for StubVector {
    async fn nearest(&self, _q: &[f32], _k: usize) -> Result<Vec<ConceptMatch>, LoomError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.hits.clone())
    }
    fn is_ready(&self) -> bool {
        self.ready
    }
    fn generation(&self) -> Generation {
        self.generation.clone()
    }
}

/// An `EmbeddingProvider` stub: returns a canned 384-vec, or an error to model a
/// disabled/unreachable embedder (the fusion degrade path).
pub struct StubEmbed {
    pub fail: bool,
}

#[async_trait]
impl EmbeddingProvider for StubEmbed {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, LoomError> {
        if self.fail {
            Err(LoomError::Embed("stub embedder disabled".to_owned()))
        } else {
            Ok(vec![0.0_f32; 384])
        }
    }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LoomError> {
        if self.fail {
            return Err(LoomError::Embed("stub embedder disabled".to_owned()));
        }
        Ok(texts.iter().map(|_| vec![0.0_f32; 384]).collect())
    }
    #[allow(clippy::unnecessary_literal_bound)]
    fn model_id(&self) -> &str {
        "bge-small-en-v1.5"
    }
    fn dimensions(&self) -> usize {
        384
    }
}

/// Build a `ConceptMatch` from a fixture slug + cosine score (semantic provenance).
pub fn semantic_hit(slug: &str, score: f32) -> ConceptMatch {
    ConceptMatch {
        iri: Iri::from_slug(slug),
        score,
        provenance: MatchProvenance::SemanticHnsw,
    }
}

/// A generation descriptor with a given id (for the parity-guard tests).
pub fn generation_with_id(id: &str) -> Generation {
    Generation {
        id: GenerationId(id.to_owned()),
        source: GenerationSource::MirrorManifest,
        generated_at: Some(id.to_owned()),
        commit_sha: None,
        promoted_at: None,
        cluster_span_seconds: None,
        artifacts: Vec::new(),
        verified_single_generation: true,
        class_count: None,
    }
}

/// A fully-wired in-memory façade for a test. Owns the `TempDir` so the mirror /
/// graph data outlives the router.
pub struct TestEnv {
    pub state: AppState,
    pub lexical_generation_id: String,
    _dir: TempDir,
    pub vector_calls: Arc<AtomicUsize>,
}

/// Builder for a `TestEnv` — sensible defaults (fixture index, not-ready vector,
/// working embedder, retrieval-only backend, graph over an empty dir); override
/// the pieces a given EXP needs.
#[allow(clippy::struct_excessive_bools)] // a test builder of independent knobs
pub struct TestEnvBuilder {
    ttl: Option<String>,
    backend: Option<OpenAiBackend>,
    vector: Option<Arc<StubVector>>,
    embed_fail: bool,
    semantic_fallback: bool,
    semantic_min_inject: Option<f64>,
    semantic_debug_surface: bool,
    verbatim_mode: bool,
    verbatim_threshold: f64,
    exposure_append: bool,
    backend_no_think: bool,
    think_token_floor: u64,
}

impl TestEnvBuilder {
    pub fn new() -> Self {
        Self {
            ttl: None,
            backend: None,
            vector: None,
            embed_fail: false,
            semantic_fallback: false,
            semantic_min_inject: None,
            semantic_debug_surface: false,
            verbatim_mode: false,
            verbatim_threshold: 8.0,
            exposure_append: false,
            backend_no_think: false,
            think_token_floor: 0, // matches Config::default — F3 off unless a test opts in
        }
    }

    /// F1: turn on verbatim serving with an explicit top-score threshold.
    pub fn with_verbatim(mut self, enabled: bool, threshold: f64) -> Self {
        self.verbatim_mode = enabled;
        self.verbatim_threshold = threshold;
        self
    }

    /// F2: append the `Not covered above` line to answer content on drops.
    pub fn with_exposure_append(mut self, enabled: bool) -> Self {
        self.exposure_append = enabled;
        self
    }

    /// F3: no-think + think-token floor knobs.
    pub fn with_thinking(mut self, no_think: bool, think_floor: u64) -> Self {
        self.backend_no_think = no_think;
        self.think_token_floor = think_floor;
        self
    }

    /// Load a small ontology into the graph store (writes `ontology.ttl`, which
    /// is on the store's allowlist), making the graph AVAILABLE.
    pub fn with_graph_ttl(mut self, ttl: &str) -> Self {
        self.ttl = Some(ttl.to_owned());
        self
    }

    pub fn with_backend(mut self, backend: OpenAiBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn with_vector(mut self, vector: Arc<StubVector>) -> Self {
        self.vector = Some(vector);
        self
    }

    pub fn with_embed_fail(mut self, fail: bool) -> Self {
        self.embed_fail = fail;
        self
    }

    pub fn with_semantic_fallback(mut self, enabled: bool, min_inject: Option<f64>) -> Self {
        self.semantic_fallback = enabled;
        self.semantic_min_inject = min_inject;
        self
    }

    /// Turn on the `/loom/search/semantic` debug surface (default-off, audit
    /// finding 1). Off ⇒ the route answers 404.
    pub fn with_semantic_debug_surface(mut self, enabled: bool) -> Self {
        self.semantic_debug_surface = enabled;
        self
    }

    pub fn build(self) -> TestEnv {
        let dir = TempDir::new().expect("tempdir");
        let index_path = dir.path().join("scaffold-index.json");
        std::fs::write(&index_path, FIXTURE).expect("write fixture index");
        if let Some(ttl) = &self.ttl {
            std::fs::write(dir.path().join("ontology.ttl"), ttl).expect("write ttl");
        }

        let retriever = LexicalRetriever::from_json_str(FIXTURE).expect("fixture retriever");
        let lexical_generation_id = retriever.generation().id.0.clone();

        let vector: Arc<StubVector> = self.vector.unwrap_or_else(|| {
            Arc::new(StubVector::new(false, generation_with_id("none"), vec![]))
        });
        let vector_calls = Arc::clone(&vector.calls);

        let backend = self
            .backend
            .unwrap_or_else(|| OpenAiBackend::new("", Duration::from_secs(5), 1536));

        let config = Config {
            index_path: index_path.to_string_lossy().into_owned(),
            backend_url: backend.endpoint().to_owned(),
            semantic_fallback: self.semantic_fallback,
            semantic_min_inject: self.semantic_min_inject,
            semantic_debug_surface: self.semantic_debug_surface,
            verbatim_mode: self.verbatim_mode,
            verbatim_threshold: self.verbatim_threshold,
            exposure_append: self.exposure_append,
            backend_no_think: self.backend_no_think,
            think_token_floor: self.think_token_floor,
            ..Config::default()
        };

        let graph = OxigraphStore::load(dir.path());
        let embedder = StubEmbed {
            fail: self.embed_fail,
        };
        let generation = MirrorStore::new(&config.index_path);

        let state = AppState::new(
            Arc::new(retriever),
            vector,
            Arc::new(graph),
            Arc::new(embedder),
            Arc::new(backend),
            Arc::new(generation),
            InjectionPolicy::default(),
            config,
        );

        TestEnv {
            state,
            lexical_generation_id,
            _dir: dir,
            vector_calls,
        }
    }
}

impl TestEnv {
    pub fn router(&self) -> Router {
        build_router(self.state.clone())
    }

    pub fn vector_call_count(&self) -> usize {
        self.vector_calls.load(Ordering::SeqCst)
    }
}

/// Fire a request at the router and return `(status, json_body)`. A non-JSON
/// body parses to `Value::Null` so callers can still assert the status.
pub async fn call(
    router: Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    let request = if let Some(b) = body {
        req = req.header("content-type", "application/json");
        req.body(Body::from(serde_json::to_vec(&b).unwrap()))
            .unwrap()
    } else {
        req.body(Body::empty()).unwrap()
    };
    let resp = router.oneshot(request).await.expect("router oneshot");
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}
