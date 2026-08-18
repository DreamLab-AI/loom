//! loom-facade — the composition root (RUST-ARCHITECTURE §9/§10). Binds one
//! concrete adapter per hexagonal port, sequences the fusion pipeline (§6), and
//! serves the axum router. This library target exists so the router can be
//! exercised in-memory (`tower::ServiceExt::oneshot`) with a fixture `AppState`;
//! `main.rs` is the thin binary over it.

// Port errors are documented on the domain traits (§4); prose mentions env-var
// and Python identifiers verbatim, which would otherwise trip `doc_markdown`.
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]

pub mod config;
pub mod error;
pub mod fusion;
pub mod mirror;
pub mod routes;
pub mod serving;
pub mod state;

use std::sync::Arc;

pub use config::Config;
pub use routes::build_router;
pub use state::AppState;

use loom_domain::{GraphStore, LexicalIndex, VectorIndex};
use loom_scaffold::policy::InjectionPolicy;
use loom_scaffold::LexicalRetriever;

use loom_backend_openai::OpenAiBackend;
use loom_embed_xinference::XinferenceEmbedder;
use loom_graph_oxigraph::OxigraphStore;
use loom_vector_ruvector::HnswIndex;

use crate::mirror::MirrorStore;

/// An empty scaffold index — the fail-open floor when `ONTOLOGY_INDEX` cannot be
/// loaded. Every query then returns no seeds (a NoMatch scaffold), so the node
/// still answers `/health` and delegates chat, degraded and honest.
const EMPTY_INDEX_JSON: &str = r#"{"version":1,"generated":"","classes":{}}"#;

/// Build the full `AppState` from the process environment — the composition
/// root. Each adapter loads fail-open with a `tracing` status line; the
/// embedder's startup `verify()` is spawned as a non-fatal probe (warn-only), so
/// a cold/absent Xinference never blocks the bind.
///
/// # Panics
/// Never — the empty-index fallback guarantees a usable lexical floor.
#[must_use]
pub fn app_state_from_env() -> AppState {
    let config = Config::from_env();

    // --- lexical (the hard floor) — load index + prose, else an empty floor.
    let retriever = match LexicalRetriever::load(Some(&config.index_path)) {
        Ok(r) => {
            tracing::info!(
                classes = r.class_count(),
                path = %config.index_path,
                "lexical index loaded"
            );
            r
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                path = %config.index_path,
                "lexical index NOT loaded; serving on an empty floor (degraded)"
            );
            LexicalRetriever::from_json_str(EMPTY_INDEX_JSON).expect("static empty index parses")
        }
    };

    // --- graph (accelerator) — fail-open; report availability.
    let graph = OxigraphStore::load(config.data_dir());
    {
        let s = graph.status();
        if s.available {
            tracing::info!(triples = s.triples, files = ?s.loaded_files, "graph store loaded");
        } else {
            tracing::warn!(error = ?s.error, "graph store DISABLED (fail-open → lexical)");
        }
    }

    // --- semantic (accelerator, gated) — fail-open; report readiness.
    let semantic = HnswIndex::open(&config.hnsw_artifact);
    if semantic.is_ready() {
        tracing::info!(
            artifact = %config.hnsw_artifact,
            enabled = config.semantic_fallback,
            "HNSW artifact ready"
        );
    } else {
        tracing::warn!(
            artifact = %config.hnsw_artifact,
            "HNSW artifact not ready (semantic fallback unavailable — fail-open)"
        );
    }

    // --- embedder — non-fatal startup probe (spawned, never blocks the bind).
    let embedder = XinferenceEmbedder::from_env();
    {
        let probe = XinferenceEmbedder::from_env();
        tokio::spawn(async move {
            match probe.verify().await {
                Ok(()) => tracing::info!("embedder verified (bge-small-en-v1.5/384)"),
                Err(e) => tracing::warn!(error = %e, "embedder verify failed (non-fatal)"),
            }
        });
    }

    // --- backend (the model-swap seam).
    let backend = OpenAiBackend::from_env();
    if config.backend_url.is_empty() {
        tracing::info!("no DISTILL_BACKEND_URL — retrieval-only node");
    } else {
        tracing::info!(endpoint = %config.backend_url, "backend seam configured");
    }

    let generation = MirrorStore::new(&config.index_path);
    let policy = InjectionPolicy::from_env();

    AppState::new(
        Arc::new(retriever),
        Arc::new(semantic),
        Arc::new(graph),
        Arc::new(embedder),
        Arc::new(backend),
        Arc::new(generation),
        policy,
        config,
    )
}
