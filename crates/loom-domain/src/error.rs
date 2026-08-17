//! The one typed error the whole system speaks (§7). Mapped to HTTP at the
//! router edge. Fail-open on the channel, fail-labelled on the payload
//! (ADR-135 liveness rule): a missing accelerator degrades the answer and is
//! reported — it does not 500 the request. The lexical index is the hard floor.

#[derive(thiserror::Error, Debug)]
pub enum LoomError {
    #[error("scaffold index unavailable: {0}")]
    IndexUnavailable(String),

    #[error("graph store unavailable: {0}")]
    GraphUnavailable(String), // fail-open → lexical

    #[error("bad sparql: {0}")]
    BadQuery(String), // 400

    #[error("semantic index not ready: {0}")]
    SemanticUnready(String), // fail-open → lexical

    #[error("embedder error: {0}")]
    Embed(String), // 502 on /embed; skip on fallback

    #[error("embedding dimension mismatch: got {got}, want {want}")]
    Dimension { got: usize, want: usize },

    #[error("no DISTILL_BACKEND_URL configured")]
    NoBackend, // 503

    #[error("backend unreachable: {0}")]
    BackendUnreachable(String), // 502

    #[error("backend http {status}: {body}")]
    BackendHttp { status: u16, body: String },

    #[error("generation not atomic: {0}")]
    GenerationDrift(String), // mirror reject

    #[error("attestation failed: {0}")]
    Attest(String), // build/CI only

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
