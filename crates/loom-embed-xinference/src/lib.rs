//! `loom-embed-xinference` — the `EmbeddingProvider` port over Xinference.
//!
//! A thin OpenAI-embeddings client to `XINFERENCE_URL/embeddings`. The model is
//! LOCKED to `bge-small-en-v1.5`/384 (RUST-ARCHITECTURE §11.3): a different
//! embedding model silently invalidates the HNSW artifact, so `model_id()` and
//! `dimensions()` are compile-time constants and every returned vector is
//! length-checked against 384 — a mismatch is `LoomError::Dimension`, not a
//! quietly-wrong answer.
//!
//! Two call sites only (§11.3): build-time embed-on-promote and query-time OOV
//! embed on a lexical miss. Never on the augmentation read path otherwise.

// Mirror the domain crate's pedantic posture: this adapter's public surface is
// getter-heavy and names product terms (Xinference, HNSW, bge) that would each
// need backticks. The safety-bearing lints (unsafe, correctness) stay on.
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// Re-exported so downstream (and this crate's integration tests) can name the
// port and error without a direct dependency on loom-domain.
pub use loom_domain::{EmbeddingProvider, LoomError};

/// The LOCKED embedding model — the ops-law lock (§10, §11.3). Not configurable.
pub const MODEL_ID: &str = "bge-small-en-v1.5";

/// The LOCKED embedding width. Every returned vector must be exactly this long.
pub const DIMENSIONS: usize = 384;

/// Default endpoint when `XINFERENCE_URL` is unset (§10 config table).
const DEFAULT_BASE_URL: &str = "http://xinference:9997/v1";

/// Default request timeout in seconds when `XINFERENCE_TIMEOUT_SECS` is unset.
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// A thin, connection-pooled OpenAI-embeddings client to Xinference.
///
/// `reqwest` with rustls (no OpenSSL system dep — keeps the musl static build
/// clean, §11.3). The model is fixed at [`MODEL_ID`]; only the endpoint and
/// timeout are configurable.
pub struct XinferenceEmbedder {
    client: reqwest::Client,
    /// Fully-formed `{base}/embeddings` POST target.
    endpoint: String,
}

impl XinferenceEmbedder {
    /// Build against an explicit base URL (e.g. `http://xinference:9997/v1`) and
    /// timeout. The `/embeddings` suffix is appended; a trailing slash on
    /// `base_url` is tolerated.
    pub fn new(base_url: &str, timeout: Duration) -> Self {
        let endpoint = format!("{}/embeddings", base_url.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client with rustls should always build");
        Self { client, endpoint }
    }

    /// Construct from the environment: `XINFERENCE_URL`
    /// (default `http://xinference:9997/v1`) and `XINFERENCE_TIMEOUT_SECS`
    /// (default 60). Infallible by design — a TLS-backend init failure is an
    /// unrecoverable startup fault, and the design's call sites treat this as a
    /// plain constructor (§8.4 recall-gate wiring).
    pub fn from_env() -> Self {
        let base_url =
            std::env::var("XINFERENCE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        let timeout_secs = std::env::var("XINFERENCE_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);
        Self::new(&base_url, Duration::from_secs(timeout_secs))
    }

    /// The resolved POST target — surfaced for `/health` and diagnostics.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Startup probe for the facade to call: embed the literal `"probe"` and
    /// assert a 384-wide vector comes back. Proves the endpoint is live AND the
    /// model behind it is the locked 384-dim one before serving begins.
    pub async fn verify(&self) -> Result<(), LoomError> {
        self.embed("probe").await.map(|_| ())
    }
}

// --- wire types -------------------------------------------------------------

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Deserialize)]
struct EmbedData {
    index: usize,
    embedding: Vec<f32>,
}

/// Map a transport-layer `reqwest` failure to a labelled `LoomError::Embed`,
/// preserving the coarse failure kind (connect / timeout / decode) for triage.
fn transport_error(e: &reqwest::Error) -> LoomError {
    let kind = if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_decode() {
        "decode"
    } else {
        "request"
    };
    LoomError::Embed(format!("xinference {kind}: {e}"))
}

/// Keep an error body readable in logs without dumping a full response.
fn truncate(s: &str) -> String {
    const CAP: usize = 200;
    if s.len() <= CAP {
        s.to_owned()
    } else {
        format!("{}…", &s[..CAP])
    }
}

#[async_trait]
impl EmbeddingProvider for XinferenceEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, LoomError> {
        // Delegate to the batch path of one — single source of request/parse
        // logic. `swap_remove(0)` avoids re-shifting the one-element vec.
        let mut vecs = self.embed_batch(&[text.to_owned()]).await?;
        Ok(vecs.swap_remove(0))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LoomError> {
        // No inputs ⇒ no request; the API would reject an empty batch anyway.
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let request = EmbedRequest {
            model: MODEL_ID,
            input: texts,
        };
        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| transport_error(&e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LoomError::Embed(format!(
                "xinference http {}: {}",
                status.as_u16(),
                truncate(&body)
            )));
        }

        let parsed: EmbedResponse = response.json().await.map_err(|e| transport_error(&e))?;

        // The response must carry one vector per input; anything else is a
        // malformed reply, not a partial answer we can trust.
        let mut data = parsed.data;
        if data.len() != texts.len() {
            return Err(LoomError::Embed(format!(
                "xinference returned {} vectors for {} inputs",
                data.len(),
                texts.len()
            )));
        }

        // Xinference orders `data` by request index, but we sort defensively so
        // the returned order matches the input order regardless (parity with the
        // Python `sorted(data, key=index)` in tools/ingest/embed_and_stage.py).
        data.sort_by_key(|d| d.index);

        let mut out = Vec::with_capacity(data.len());
        for d in data {
            if d.embedding.len() != DIMENSIONS {
                return Err(LoomError::Dimension {
                    got: d.embedding.len(),
                    want: DIMENSIONS,
                });
            }
            out.push(d.embedding);
        }
        Ok(out)
    }

    fn model_id(&self) -> &str {
        MODEL_ID
    }

    fn dimensions(&self) -> usize {
        DIMENSIONS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A `DIMENSIONS`-wide embedding filled with `fill` — the shape Xinference
    /// returns, sized to the lock so the length check passes.
    fn embedding(fill: f64) -> Vec<f64> {
        vec![fill; DIMENSIONS]
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[tokio::test]
    async fn happy_path_returns_vectors_in_input_order() {
        let server = MockServer::start().await;
        // Response deliberately SHUFFLED (index 1 before index 0) to prove we
        // re-order by `.index`, not by array position.
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "model": MODEL_ID,
                "data": [
                    { "index": 1, "object": "embedding", "embedding": embedding(0.20) },
                    { "index": 0, "object": "embedding", "embedding": embedding(0.10) }
                ]
            })))
            .mount(&server)
            .await;

        let embedder = XinferenceEmbedder::new(&server.uri(), Duration::from_secs(5));
        let out = embedder
            .embed_batch(&["alpha".to_owned(), "beta".to_owned()])
            .await
            .expect("happy path should succeed");

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].len(), DIMENSIONS);
        assert_eq!(out[1].len(), DIMENSIONS);
        // input[0] ("alpha", index 0) → the 0.10 vector; input[1] → 0.20.
        assert!(approx(out[0][0], 0.10), "index-0 vector out of order");
        assert!(approx(out[1][0], 0.20), "index-1 vector out of order");
    }

    #[tokio::test]
    async fn single_embed_delegates_and_checks_dimension() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [ { "index": 0, "embedding": embedding(0.5) } ]
            })))
            .mount(&server)
            .await;

        let embedder = XinferenceEmbedder::new(&server.uri(), Duration::from_secs(5));
        let v = embedder.embed("rgb protocol").await.expect("embed");
        assert_eq!(v.len(), DIMENSIONS);
        assert!(approx(v[0], 0.5));
    }

    #[tokio::test]
    async fn wrong_width_is_dimension_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [ { "index": 0, "embedding": vec![0.1_f64; 383] } ]
            })))
            .mount(&server)
            .await;

        let embedder = XinferenceEmbedder::new(&server.uri(), Duration::from_secs(5));
        let err = embedder.embed("x").await.expect_err("383 dims must reject");
        match err {
            LoomError::Dimension { got, want } => {
                assert_eq!(got, 383);
                assert_eq!(want, DIMENSIONS);
            }
            other => panic!("expected Dimension, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn server_500_is_embed_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(500).set_body_string("model overloaded"))
            .mount(&server)
            .await;

        let embedder = XinferenceEmbedder::new(&server.uri(), Duration::from_secs(5));
        let err = embedder.embed("x").await.expect_err("500 must reject");
        match err {
            LoomError::Embed(detail) => assert!(detail.contains("500"), "detail: {detail}"),
            other => panic!("expected Embed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn connection_refused_is_embed_error() {
        // Port 1 is privileged and unbound in the test sandbox ⇒ connect refused.
        let embedder = XinferenceEmbedder::new("http://127.0.0.1:1", Duration::from_secs(2));
        let err = embedder.embed("x").await.expect_err("no server must reject");
        assert!(
            matches!(err, LoomError::Embed(_)),
            "expected Embed, got {err:?}"
        );
    }

    #[tokio::test]
    async fn empty_batch_short_circuits() {
        let embedder = XinferenceEmbedder::new("http://127.0.0.1:1", Duration::from_secs(2));
        let out = embedder.embed_batch(&[]).await.expect("empty batch is Ok");
        assert!(out.is_empty());
    }

    #[test]
    fn model_and_dimensions_are_locked() {
        let embedder = XinferenceEmbedder::new("http://unused", Duration::from_secs(1));
        assert_eq!(embedder.model_id(), "bge-small-en-v1.5");
        assert_eq!(embedder.dimensions(), 384);
    }
}
