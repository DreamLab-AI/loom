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
        let err = self.0;
        // The message is always the Display form; the code follows §7.
        let msg = err.to_string();
        match err {
            // 400 — a genuinely malformed client request.
            LoomError::BadQuery(_) => reply(StatusCode::BAD_REQUEST, "bad_query", &msg),

            // 503 — retrieval-only node asked to delegate.
            LoomError::NoBackend => reply(StatusCode::SERVICE_UNAVAILABLE, "no_backend", &msg),

            // 502 — the model's failure, propagated but labelled (§7 folds both
            // transport failure and upstream non-2xx into 502).
            LoomError::BackendUnreachable(_) => {
                reply(StatusCode::BAD_GATEWAY, "backend_unreachable", &msg)
            }
            LoomError::BackendHttp { status, body } => {
                // Attach the upstream status + body so the failure stays auditable.
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": "backend_http",
                        "upstream_status": status,
                        "detail": body,
                    })),
                )
                    .into_response()
            }

            // 502 — embedder trouble on an explicit embed surface. (On the
            // fusion path the pipeline degrades to NoMatch and never reaches here.)
            LoomError::Embed(_) | LoomError::Dimension { .. } => {
                reply(StatusCode::BAD_GATEWAY, "embed_error", &msg)
            }

            // NOT a client error — the accelerator is gone; degrade and label.
            // A 200 keeps the "never a 4xx/5xx for a missing accelerator" rule.
            LoomError::GraphUnavailable(_) | LoomError::SemanticUnready(_) => (
                StatusCode::OK,
                Json(json!({ "degraded": true, "reason": msg })),
            )
                .into_response(),

            // 500 — the lexical index is the floor; if it is gone the node
            // cannot serve its purpose. Everything else is an internal fault.
            LoomError::IndexUnavailable(_)
            | LoomError::GenerationDrift(_)
            | LoomError::Attest(_)
            | LoomError::Io(_)
            | LoomError::Json(_) => reply(StatusCode::INTERNAL_SERVER_ERROR, "internal", &msg),
        }
    }
}

fn reply(code: StatusCode, kind: &str, detail: &str) -> Response {
    (code, Json(json!({ "error": kind, "detail": detail }))).into_response()
}
