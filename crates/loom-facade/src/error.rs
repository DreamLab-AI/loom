//! `impl IntoResponse for LoomError` — the §7 error→HTTP table, at the router
//! edge. Fail-open on the channel, fail-labelled on the payload (ADR-135): the
//! **lexical index is the hard floor** (its loss is a real 500); the graph store
//! and the semantic index are ACCELERATORS, so their absence is a degrade-and-
//! report (a labelled 200), NEVER a client error.
//!
//! DIVERGENCE from the Python façade (documented, doc-table wins): Python's
//! `_backend` propagates the upstream HTTP status verbatim (`e.code`). The §7
//! table folds every backend failure — unreachable OR non-2xx — into 502. This
//! file follows the §7 table (the mission's stated source of truth), so a
//! backend 500 surfaces to the client as 502 with the upstream body attached.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use loom_domain::LoomError;
use serde_json::json;

/// A thin newtype so `LoomError` (a foreign type) can carry an axum
/// `IntoResponse` impl without violating orphan rules.
#[derive(Debug)]
pub struct ApiError(pub LoomError);

impl From<LoomError> for ApiError {
    fn from(e: LoomError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, body) = api_error_parts(self.0);
        (code, Json(body)).into_response()
    }
}

/// The §7 error→HTTP mapping as DATA rather than a finished response.
///
/// Split out (ADR-138 closeout) because the chat path must now attach the
/// grounding contract to a FAILURE as well as to a success: a consumer has to be
/// able to tell "the corpus had nothing to say" from "the model was unreachable",
/// and only a grounding block on the failure body can carry that distinction.
/// `IntoResponse` above is the same mapping with nothing attached.
#[must_use]
pub fn api_error_parts(err: LoomError) -> (StatusCode, serde_json::Value) {
    // The message is always the Display form; the code follows §7.
    let msg = err.to_string();
    match err {
        // 400 — a genuinely malformed client request.
        LoomError::BadQuery(_) => (StatusCode::BAD_REQUEST, body("bad_query", &msg)),

        // 503 — retrieval-only node asked to delegate.
        LoomError::NoBackend => (StatusCode::SERVICE_UNAVAILABLE, body("no_backend", &msg)),

        // 502 — the model's failure, propagated but labelled (§7 folds both
        // transport failure and upstream non-2xx into 502).
        LoomError::BackendUnreachable(_) => {
            (StatusCode::BAD_GATEWAY, body("backend_unreachable", &msg))
        }
        LoomError::BackendHttp { status, body: b } => (
            StatusCode::BAD_GATEWAY,
            json!({
                "error": "backend_http",
                "upstream_status": status,
                "detail": b,
            }),
        ),

        // 502 — embedder trouble on an explicit embed surface. (On the fusion
        // path the pipeline degrades to NoMatch and never reaches here.)
        LoomError::Embed(_) | LoomError::Dimension { .. } => {
            (StatusCode::BAD_GATEWAY, body("embed_error", &msg))
        }

        // NOT a client error — the accelerator is gone; degrade and label. A 200
        // keeps the "never a 4xx/5xx for a missing accelerator" rule. The
        // unqualified-artefact rejection joins them: a semantic artefact that
        // failed its contract is an absent accelerator, not a fault.
        LoomError::GraphUnavailable(_) | LoomError::SemanticUnready(_) | LoomError::Artefact(_) => (
            StatusCode::OK,
            json!({ "degraded": true, "reason": msg }),
        ),

        // 500 — the lexical index is the floor; if it is gone the node cannot
        // serve its purpose. A bundle that failed activation is the same class of
        // fault: the node has no verified content to serve.
        LoomError::IndexUnavailable(_)
        | LoomError::GenerationDrift(_)
        | LoomError::Bundle(_)
        | LoomError::Attest(_)
        | LoomError::Io(_)
        | LoomError::Json(_) => (StatusCode::INTERNAL_SERVER_ERROR, body("internal", &msg)),
    }
}

fn body(kind: &str, detail: &str) -> serde_json::Value {
    json!({ "error": kind, "detail": detail })
}
