//! loom-backend-openai — the `ModelBackend` adapter: OpenAI-compatible chat
//! delegation to `DISTILL_BACKEND_URL`, the model-swap seam (ADR-135 D1.2).
//!
//! Ports the backend-delegation semantics of `app/loom_facade.py::_backend`
//! (and `_probe_backend`): the `max_tokens` floor, `stream` stripping, the
//! `/models` passthrough and the 5s reachability probe. Python semantics win;
//! divergences from the stdlib façade are noted inline.
//!
//! Model identity is NEVER encoded in `endpoint()` — it rides in the response
//! body, so swapping Qwen3.8-27B for the next model is one env-var change with
//! zero consumer change.

use std::time::Duration;

use loom_domain::{BackendResponse, LoomError, ModelBackend};
use serde_json::Value;

/// `LOOM_MIN_MAX_TOKENS` default — reasoning backends spend their budget in
/// `reasoning_content` and emit EMPTY final content when `max_tokens` is too
/// small (verified: 400 → empty; ≥1536 is safe). `0` disables the floor.
const DEFAULT_MIN_MAX_TOKENS: u64 = 1536;

/// `LOOM_TIMEOUT` default (seconds) — distillation is slow by design.
const DEFAULT_TIMEOUT_SECS: f64 = 600.0;

/// Fixed 5s probe timeout for `reachable()` (mirrors `_probe_backend`).
const PROBE_TIMEOUT_SECS: u64 = 5;

/// `ModelBackend` over a pooled rustls `reqwest::Client` bound to
/// `DISTILL_BACKEND_URL`. Cloneable — the inner client is `Arc`-backed and
/// connection-pooled, so clones share the pool.
#[derive(Clone, Debug)]
pub struct OpenAiBackend {
    client: reqwest::Client,
    /// The raw `DISTILL_BACKEND_URL`, trailing `/` stripped. May be empty:
    /// an empty endpoint is a retrieval-only node, and `chat`/`models` surface
    /// `LoomError::NoBackend` (503 at the façade). `endpoint()` still returns
    /// this raw string — model identity is never encoded here.
    endpoint: String,
    /// Delegate timeout for `chat`/`models` (`LOOM_TIMEOUT`).
    timeout: Duration,
    /// `max_tokens` floor (`LOOM_MIN_MAX_TOKENS`); `0` disables flooring.
    min_max_tokens: u64,
}

impl OpenAiBackend {
    /// Explicit constructor — used by `from_env` and by tests (which inject a
    /// mock-server URL and an explicit floor, avoiding process-env races).
    ///
    /// `endpoint` has any trailing `/` stripped so path joins stay canonical.
    #[must_use]
    pub fn new(endpoint: impl Into<String>, timeout: Duration, min_max_tokens: u64) -> Self {
        let endpoint = endpoint.into().trim_end_matches('/').to_owned();
        // A pooled client on the default (rustls) config; per-request
        // `.timeout()` carries the delegate vs probe distinction, so no global
        // timeout is set here.
        let client = reqwest::Client::new();
        Self {
            client,
            endpoint,
            timeout,
            min_max_tokens,
        }
    }

    /// Build from the environment surface (RUST-ARCHITECTURE §10):
    /// `DISTILL_BACKEND_URL` (empty ⇒ retrieval-only node), `LOOM_TIMEOUT`
    /// (default 600s), `LOOM_MIN_MAX_TOKENS` (default 1536; `0` disables).
    #[must_use]
    pub fn from_env() -> Self {
        let endpoint = std::env::var("DISTILL_BACKEND_URL").unwrap_or_default();
        let timeout_secs = std::env::var("LOOM_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|s| *s > 0.0)
            .unwrap_or(DEFAULT_TIMEOUT_SECS);
        let min_max_tokens = std::env::var("LOOM_MIN_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_MIN_MAX_TOKENS);
        Self::new(endpoint, Duration::from_secs_f64(timeout_secs), min_max_tokens)
    }

    /// True when no `DISTILL_BACKEND_URL` is configured.
    #[must_use]
    fn is_retrieval_only(&self) -> bool {
        self.endpoint.is_empty()
    }

    /// Apply the `max_tokens` floor and strip `stream`, in place, mirroring the
    /// façade's pre-delegation rewrite (`app/loom_facade.py:204-209`).
    ///
    /// Python floors ONLY fields that are JSON integers (`isinstance(v, int)`)
    /// via `max(v, MIN)` — which raises sub-floor values (incl. negatives:
    /// `max(-1, 1536) == 1536`) and leaves a higher ask untouched. Everything
    /// else — strings, floats, `u64`-overflow numbers, `null` — is left EXACTLY
    /// as sent (audit finding 2: the old code coerced any non-`u64` value to the
    /// floor, which could LOWER a larger string-typed ask). Insertion of
    /// `max_tokens = floor` happens only when BOTH keys are absent. A floor of
    /// `0` disables all of this. `stream` is always popped.
    fn normalise_body(&self, body: &mut Value) {
        let Some(map) = body.as_object_mut() else {
            return;
        };

        // Streams are disabled — parity with the façade, which pops `stream`.
        map.remove("stream");

        if self.min_max_tokens == 0 {
            return;
        }
        let floor = i128::from(self.min_max_tokens);

        // Insertion guard is key-presence (Python `field in j`), independent of
        // the value's type.
        let mut saw_any = false;
        for field in ["max_tokens", "max_completion_tokens"] {
            let Some(slot) = map.get_mut(field) else {
                continue;
            };
            saw_any = true;
            // Only JSON integers are floored. `as_i64`/`as_u64` succeed exactly
            // for serde integers; strings/floats/overflow numbers/null yield
            // None and pass through verbatim.
            let current = slot
                .as_i64()
                .map(i128::from)
                .or_else(|| slot.as_u64().map(i128::from));
            if let Some(current) = current {
                if current < floor {
                    *slot = Value::from(self.min_max_tokens);
                }
            }
        }
        if !saw_any {
            map.insert("max_tokens".to_owned(), Value::from(self.min_max_tokens));
        }
    }

    /// Join the endpoint with a `/v1`-relative sub-path. `DISTILL_BACKEND_URL`
    /// already carries the `/v1` suffix (e.g. `…:8085/v1`), so the façade's
    /// `path[len('/v1'):]` slice reduces to appending the bare sub-path.
    fn url(&self, sub: &str) -> String {
        format!("{}{sub}", self.endpoint)
    }
}

#[async_trait::async_trait]
impl ModelBackend for OpenAiBackend {
    async fn chat(&self, mut body: Value) -> Result<BackendResponse, LoomError> {
        if self.is_retrieval_only() {
            // Façade maps this to 503; the adapter speaks the typed error.
            return Err(LoomError::NoBackend);
        }
        self.normalise_body(&mut body);

        let resp = self
            .client
            .post(self.url("/chat/completions"))
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| LoomError::BackendUnreachable(e.to_string()))?;

        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|e| LoomError::BackendUnreachable(e.to_string()))?;

        if !(200..300).contains(&status) {
            // Propagate the model's labelled failure (502 at the façade).
            return Err(LoomError::BackendHttp { status, body: text });
        }

        // OpenAI-compatible bodies are JSON; keep a raw-string fallback rather
        // than failing a 2xx the model considered a success.
        let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
        Ok(BackendResponse {
            status,
            body: value,
        })
    }

    async fn models(&self) -> Result<Value, LoomError> {
        if self.is_retrieval_only() {
            return Err(LoomError::NoBackend);
        }
        let resp = self
            .client
            .get(self.url("/models"))
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| LoomError::BackendUnreachable(e.to_string()))?;

        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|e| LoomError::BackendUnreachable(e.to_string()))?;

        if !(200..300).contains(&status) {
            return Err(LoomError::BackendHttp { status, body: text });
        }
        serde_json::from_str::<Value>(&text).map_err(LoomError::from)
    }

    async fn reachable(&self) -> bool {
        if self.is_retrieval_only() {
            return false;
        }
        // Mirror `_probe_backend`: GET `{BACKEND}/models`, 5s, success == 2xx.
        // (Python's `urlopen` raises on non-2xx and on transport error → False.)
        // NOTE divergence: the port doc-comment says `/health`; Python probes
        // `/models`, and the mission pins Python semantics — so `/models` it is.
        self.client
            .get(self.url("/models"))
            .timeout(Duration::from_secs(PROBE_TIMEOUT_SECS))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }
}
