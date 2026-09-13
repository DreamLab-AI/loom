//! The client itself: one call, the retries it is worth making, and the
//! success-shaped failures it refuses to return as answers.

use std::time::Duration;

use serde_json::Value;

use crate::error::{truncate, Error, Result};
use crate::request::ChatRequest;
use crate::response::{grounding_status, served_mode, usage, ChatOutcome, ServedMode};
use crate::tokens::{apply_token_floor, DEFAULT_MIN_MAX_TOKENS};

/// Default per-request timeout. Grounded reasoning over a long prompt runs to
/// minutes; 180s was producing spurious read timeouts in production.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

/// Default attempt ceiling, counting the first try.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Default pause before retrying a transient failure.
pub const DEFAULT_RETRY_BACKOFF: Duration = Duration::from_secs(2);

/// The marker a façade puts in `content` when it answered from retrieval and
/// never called the model. Matched as a substring of the assistant text.
const NO_GENERATION_MARKER: &str = "no model generation was performed";

/// Bytes of an error body kept for logging.
const ERROR_BODY_LIMIT: usize = 500;

/// A client bound to one façade base URL.
///
/// Cloning is cheap: the inner `reqwest::Client` is `Arc`-backed, so clones
/// share a connection pool.
///
/// ```no_run
/// # async fn demo() -> Result<(), loom_client::Error> {
/// use loom_client::{ChatRequest, LoomClient, LoomOptions, Message};
///
/// let client = LoomClient::builder("http://loom:8080/v1").build();
/// let answer = client
///     .chat(
///         ChatRequest::new("qwen3.8-27B", vec![Message::user("Summarise the scaffold.")])
///             .temperature(0.2)
///             .options(LoomOptions::declining_verbatim()),
///     )
///     .await?;
/// println!("{}", answer.content);
/// # Ok(()) }
/// ```
#[derive(Clone, Debug)]
pub struct LoomClient {
    http: reqwest::Client,
    base: String,
    timeout: Duration,
    min_max_tokens: u64,
    max_attempts: u32,
    retry_backoff: Duration,
}

/// Builder for [`LoomClient`].
#[derive(Clone, Debug)]
pub struct LoomClientBuilder {
    base: String,
    timeout: Duration,
    min_max_tokens: u64,
    max_attempts: u32,
    retry_backoff: Duration,
    http: Option<reqwest::Client>,
}

impl LoomClientBuilder {
    /// Per-request timeout. Default [`DEFAULT_TIMEOUT`].
    #[must_use]
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// Token floor. Default [`DEFAULT_MIN_MAX_TOKENS`]; `0` disables it.
    #[must_use]
    pub fn min_max_tokens(mut self, n: u64) -> Self {
        self.min_max_tokens = n;
        self
    }

    /// Attempt ceiling, counting the first try. Default
    /// [`DEFAULT_MAX_ATTEMPTS`]; `0` and `1` both mean a single attempt.
    #[must_use]
    pub fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n.max(1);
        self
    }

    /// Pause before retrying a transient failure. Default
    /// [`DEFAULT_RETRY_BACKOFF`]. Raise it for a call that crosses a gateway
    /// prone to 52x, where an immediate retry just fails again.
    #[must_use]
    pub fn retry_backoff(mut self, d: Duration) -> Self {
        self.retry_backoff = d;
        self
    }

    /// Supply the underlying HTTP client, to share a pool or set proxies.
    #[must_use]
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Finish the client.
    #[must_use]
    pub fn build(self) -> LoomClient {
        LoomClient {
            http: self.http.unwrap_or_default(),
            base: self.base,
            timeout: self.timeout,
            min_max_tokens: self.min_max_tokens,
            max_attempts: self.max_attempts,
            retry_backoff: self.retry_backoff,
        }
    }
}

impl LoomClient {
    /// Start building a client for `base`, e.g. `http://loom:8080/v1`.
    ///
    /// A trailing `/` is stripped so path joins stay canonical.
    #[must_use]
    pub fn builder(base: impl Into<String>) -> LoomClientBuilder {
        LoomClientBuilder {
            base: base.into().trim_end_matches('/').to_owned(),
            timeout: DEFAULT_TIMEOUT,
            min_max_tokens: DEFAULT_MIN_MAX_TOKENS,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            retry_backoff: DEFAULT_RETRY_BACKOFF,
            http: None,
        }
    }

    /// A client on all defaults.
    #[must_use]
    pub fn new(base: impl Into<String>) -> Self {
        Self::builder(base).build()
    }

    /// A client from `LOOM_BASE_URL`, falling back to `http://loom:8080/v1`.
    ///
    /// Callers with their own precedence chain should read it themselves and
    /// pass the winner to [`LoomClient::builder`] — a library guessing at
    /// env-var priority is a library that surprises somebody.
    #[must_use]
    pub fn from_env() -> Self {
        let base = std::env::var("LOOM_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://loom:8080/v1".to_owned());
        Self::new(base)
    }

    /// The base URL this client is bound to.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The façade root: the base with a trailing `/v1` removed.
    ///
    /// `/health` lives at the root, not under `/v1`.
    ///
    /// ```
    /// # use loom_client::LoomClient;
    /// assert_eq!(LoomClient::new("http://loom:8080/v1").root(), "http://loom:8080");
    /// assert_eq!(LoomClient::new("http://loom:8080").root(), "http://loom:8080");
    /// ```
    #[must_use]
    pub fn root(&self) -> &str {
        self.base.strip_suffix("/v1").unwrap_or(&self.base)
    }

    /// Is the façade up? `GET {root}/health`, true when it answers 2xx with a
    /// JSON body whose `ok` is not `false`.
    ///
    /// A body without an `ok` field counts as healthy: a plain
    /// OpenAI-compatible server standing in for a façade has no such field,
    /// and calling that unhealthy would be wrong.
    pub async fn healthy(&self, timeout: Duration) -> bool {
        let Ok(resp) = self
            .http
            .get(format!("{}/health", self.root()))
            .timeout(timeout)
            .send()
            .await
        else {
            return false;
        };
        if !resp.status().is_success() {
            return false;
        }
        match resp.json::<Value>().await {
            Ok(body) => body.get("ok").and_then(Value::as_bool).unwrap_or(true),
            Err(_) => false,
        }
    }

    /// The first model id the façade advertises, for callers that want
    /// whatever is deployed rather than a pinned name.
    ///
    /// Reads `data[0].id`, then `models[0].name`, covering both the `OpenAI`
    /// shape and the llama.cpp one.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] or [`Error::Http`] if `/models` cannot be read, and
    /// [`Error::Malformed`] if it lists no model under either shape.
    pub async fn first_model_id(&self) -> Result<String> {
        let body = self.models().await?;
        body.get("data")
            .and_then(|d| d.get(0))
            .and_then(|m| m.get("id"))
            .or_else(|| {
                body.get("models")
                    .and_then(|m| m.get(0))
                    .and_then(|m| m.get("name"))
            })
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::Malformed {
                base: self.base.clone(),
                detail: "/models listed no model (no data[0].id, no models[0].name)".into(),
            })
    }

    /// The raw `GET {base}/models` body.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] if the request fails, [`Error::Http`] on a non-2xx,
    /// [`Error::Malformed`] if the body is not JSON.
    pub async fn models(&self) -> Result<Value> {
        let resp = self
            .http
            .get(format!("{}/models", self.base))
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|source| Error::Transport { base: self.base.clone(), source })?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|source| Error::Transport { base: self.base.clone(), source })?;
        if !(200..300).contains(&status) {
            return Err(Error::Http {
                base: self.base.clone(),
                status,
                body: truncate(&text, ERROR_BODY_LIMIT),
            });
        }
        serde_json::from_str(&text).map_err(|e| Error::Malformed {
            base: self.base.clone(),
            detail: format!("/models was not JSON: {e}"),
        })
    }

    /// Send a chat request, retrying where retrying is worthwhile.
    ///
    /// Up to [`max_attempts`](LoomClientBuilder::max_attempts) attempts:
    ///
    /// - **truncated** (`finish_reason = "length"`) — the budget doubles and it
    ///   tries again immediately. A truncated reasoning answer can be empty
    ///   rather than short, so it is never returned.
    /// - **transient** (transport failure, 5xx, empty body) — it waits
    ///   [`retry_backoff`](LoomClientBuilder::retry_backoff) and tries again on
    ///   the same budget.
    /// - anything else fails immediately.
    ///
    /// Two success-shaped failures are rejected rather than returned:
    /// [`Error::ScaffoldOnly`] and [`Error::PassthroughRefused`].
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] or [`Error::Http`] when the call fails and retrying
    /// did not help; [`Error::Empty`] or [`Error::Malformed`] on an unusable
    /// body; [`Error::Truncated`] when every attempt hit the budget;
    /// [`Error::ScaffoldOnly`] when the façade answered without calling the
    /// model; [`Error::PassthroughRefused`] when passthrough was asked for and
    /// the façade grounded the request anyway.
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatOutcome> {
        let mut budget = request.requested_max_tokens().unwrap_or(0).max(self.min_max_tokens);
        let mut last_transient: Option<Error> = None;

        for attempt in 1..=self.max_attempts {
            match self.attempt(&request, budget, attempt).await {
                Ok(Attempt::Answer(outcome)) => return Ok(*outcome),
                Ok(Attempt::Truncated) => {
                    budget = budget.saturating_mul(2);
                }
                Err(e) if e.is_transient() && attempt < self.max_attempts => {
                    last_transient = Some(e);
                    tokio::time::sleep(self.retry_backoff).await;
                }
                Err(e) => return Err(e),
            }
        }

        // The loop ended without an answer: either every attempt truncated, or
        // the final attempt was transient. Report whichever actually happened.
        Err(last_transient.unwrap_or(Error::Truncated {
            base: self.base.clone(),
            attempts: self.max_attempts,
            budget,
        }))
    }

    /// One attempt, with no retry logic of its own.
    async fn attempt(
        &self,
        request: &ChatRequest,
        budget: u64,
        attempt: u32,
    ) -> Result<Attempt> {
        let mut body = request.body();
        if let Some(map) = body.as_object_mut() {
            if budget > 0 {
                map.insert("max_tokens".to_owned(), Value::from(budget));
            }
            apply_token_floor(map, self.min_max_tokens);
        }

        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base))
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(|source| Error::Transport { base: self.base.clone(), source })?;

        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .map_err(|source| Error::Transport { base: self.base.clone(), source })?;

        if !(200..300).contains(&status) {
            return Err(Error::Http {
                base: self.base.clone(),
                status,
                body: truncate(&text, ERROR_BODY_LIMIT),
            });
        }

        let parsed: Value = serde_json::from_str(&text).map_err(|e| Error::Malformed {
            base: self.base.clone(),
            detail: format!("response was not JSON: {e}"),
        })?;

        // An OpenAI-shaped error can ride inside a 2xx.
        if let Some(err) = parsed.get("error").filter(|e| !e.is_null()) {
            return Err(Error::Malformed {
                base: self.base.clone(),
                detail: format!("body carried an error object: {err}"),
            });
        }

        let choice = parsed
            .get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| Error::Empty { base: self.base.clone() })?;
        let finish_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        if finish_reason.as_deref() == Some("length") {
            return Ok(Attempt::Truncated);
        }

        let message = choice.get("message");
        let content = message
            .and_then(|m| m.get("content"))
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Empty { base: self.base.clone() })?;

        if content.contains(NO_GENERATION_MARKER) {
            return Err(Error::ScaffoldOnly { base: self.base.clone() });
        }

        let mode = served_mode(&parsed);
        // Only assert passthrough when the façade actually reported a mode: a
        // plain OpenAI server sends no telemetry and is passthrough by nature.
        if request.loom_options().wants_passthrough()
            && mode != ServedMode::Passthrough
            && mode != ServedMode::Unreported
        {
            return Err(Error::PassthroughRefused {
                base: self.base.clone(),
                served_mode: mode.as_str().to_owned(),
            });
        }

        Ok(Attempt::Answer(Box::new(ChatOutcome {
            content: content.to_owned(),
            reasoning: message
                .and_then(|m| m.get("reasoning_content"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            model: parsed.get("model").and_then(Value::as_str).map(ToOwned::to_owned),
            served_mode: mode,
            grounding_status: grounding_status(&parsed),
            usage: usage(&parsed),
            finish_reason,
            attempts: attempt,
            max_tokens: budget.max(self.min_max_tokens),
            raw: parsed,
        })))
    }
}

/// What one attempt produced. `ChatOutcome` carries the whole response body,
/// so it is boxed rather than widening every `Result` on the path.
enum Attempt {
    Answer(Box<ChatOutcome>),
    Truncated,
}

/// The first façade in `candidates` that answers [`LoomClient::healthy`],
/// or `None` when none does.
///
/// The canonical address and its fallbacks serve the same façade by different
/// routes — a LAN address through a NAT, and a direct rail address that still
/// works when the NAT is down. Callers should cache the winner for the process
/// rather than probing per call; this crate deliberately keeps no global state.
///
/// ```no_run
/// # async fn demo() {
/// let base = loom_client::resolve_base(
///     &["http://192.168.2.132:8084/v1".to_string(), "http://10.10.10.1:8084/v1".to_string()],
///     std::time::Duration::from_secs(5),
/// ).await;
/// # let _ = base;
/// # }
/// ```
pub async fn resolve_base(candidates: &[String], timeout: Duration) -> Option<String> {
    for candidate in candidates {
        if LoomClient::new(candidate.clone()).healthy(timeout).await {
            return Some(candidate.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_loses_a_trailing_slash() {
        assert_eq!(LoomClient::new("http://loom:8080/v1/").base(), "http://loom:8080/v1");
    }

    #[test]
    fn root_strips_only_a_v1_suffix() {
        assert_eq!(LoomClient::new("http://loom:8080/v1").root(), "http://loom:8080");
        assert_eq!(LoomClient::new("http://loom:8080").root(), "http://loom:8080");
        // Not a suffix: a host that merely contains "/v1" keeps it.
        assert_eq!(LoomClient::new("http://h/v1/x").root(), "http://h/v1/x");
    }

    #[test]
    fn max_attempts_is_never_zero() {
        let c = LoomClient::builder("http://x").max_attempts(0).build();
        assert_eq!(c.max_attempts, 1);
    }
}
