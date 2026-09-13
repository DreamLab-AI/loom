//! Building a chat request, including the two Loom-private switches.

use serde::Serialize;
use serde_json::{Map, Value};

/// One chat message.
#[derive(Clone, Debug, Serialize)]
pub struct Message {
    /// `system`, `user` or `assistant`.
    pub role: String,
    /// The message text.
    pub content: String,
}

impl Message {
    /// A `system` message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: content.into() }
    }

    /// A `user` message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }

    /// An `assistant` message.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// The façade's two per-request switches, sent as `loom_options`.
///
/// They are **independent controls**, and conflating them is the mistake this
/// type exists to prevent:
///
/// | | `verbatim` | `scaffold` |
/// |---|---|---|
/// | what `false` declines | answering from retrieval alone, with no model call | grounding the request in the ontology at all |
/// | what still happens | the ontology scaffold is still injected | nothing: the façade is a plain proxy |
/// | when you want it | the subject *is* in the ontology and you want grounded generation | the subject is **not** in the ontology (a codebase, an arbitrary document) |
///
/// A plain `OpenAI` server ignores the field, and the façade strips it before
/// delegating, so sending it is safe against either.
///
/// `None` leaves the façade's configured behaviour untouched; only an explicit
/// `Some(false)` opts out. This mirrors the façade, which acts on
/// present-and-false and ignores every other value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LoomOptions {
    /// `Some(false)` declines a verbatim serve, forcing the delegate path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbatim: Option<bool>,
    /// `Some(false)` declines scaffold injection: the model alone (ADR-139).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaffold: Option<bool>,
}

impl LoomOptions {
    /// Decline the verbatim short-circuit; keep ontology grounding.
    ///
    /// The right default for generative work on a subject the ontology covers.
    /// Without it, a prompt whose wording happens to match an ontology class
    /// title is answered from retrieval with no model call at all.
    #[must_use]
    pub fn declining_verbatim() -> Self {
        Self { verbatim: Some(false), scaffold: None }
    }

    /// Decline grounding entirely: the façade becomes a plain proxy (ADR-139).
    ///
    /// Also declines verbatim, which grounding-off implies — sending only
    /// `scaffold: false` to a façade that predates ADR-139 would leave the
    /// verbatim short-circuit live.
    #[must_use]
    pub fn passthrough() -> Self {
        Self { verbatim: Some(false), scaffold: Some(false) }
    }

    /// True when this asked the façade to pass the request through unchanged.
    #[must_use]
    pub fn wants_passthrough(self) -> bool {
        self.scaffold == Some(false)
    }

    /// True when no switch is set, so the field need not be sent at all.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.verbatim.is_none() && self.scaffold.is_none()
    }
}

/// A chat completion request.
///
/// Construct with [`ChatRequest::new`] and adjust with the builder methods;
/// anything without a method goes through [`ChatRequest::extra`], which is
/// merged into the body verbatim.
///
/// ```
/// # use loom_client::{ChatRequest, LoomOptions, Message};
/// let req = ChatRequest::new("qwen3.8-27B", vec![Message::user("Explain the scaffold.")])
///     .temperature(0.2)
///     .max_tokens(4096)
///     .options(LoomOptions::declining_verbatim());
/// assert_eq!(req.body()["temperature"], 0.2);
/// ```
#[derive(Clone, Debug)]
pub struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    top_k: Option<u64>,
    max_tokens: Option<u64>,
    options: LoomOptions,
    extra: Map<String, Value>,
}

impl ChatRequest {
    /// A request for `model` with `messages`, and no sampling overrides.
    #[must_use]
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            options: LoomOptions::default(),
            extra: Map::new(),
        }
    }

    /// Sampling temperature.
    #[must_use]
    pub fn temperature(mut self, t: f64) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Nucleus sampling cutoff.
    #[must_use]
    pub fn top_p(mut self, p: f64) -> Self {
        self.top_p = Some(p);
        self
    }

    /// Top-k sampling cutoff.
    #[must_use]
    pub fn top_k(mut self, k: u64) -> Self {
        self.top_k = Some(k);
        self
    }

    /// Token budget. Raised to the client's floor if it is below it, and used
    /// as the starting budget for truncation retries, which double it.
    #[must_use]
    pub fn max_tokens(mut self, n: u64) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// The Loom-private switches.
    #[must_use]
    pub fn options(mut self, options: LoomOptions) -> Self {
        self.options = options;
        self
    }

    /// Set an arbitrary top-level body field, e.g. `chat_template_kwargs`.
    ///
    /// ```
    /// # use loom_client::{ChatRequest, Message};
    /// # use serde_json::json;
    /// let req = ChatRequest::new("m", vec![Message::user("hi")])
    ///     .extra("chat_template_kwargs", json!({ "enable_thinking": false }));
    /// assert_eq!(req.body()["chat_template_kwargs"]["enable_thinking"], false);
    /// ```
    #[must_use]
    pub fn extra(mut self, key: impl Into<String>, value: Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }

    /// The switches this request carries.
    #[must_use]
    pub fn loom_options(&self) -> LoomOptions {
        self.options
    }

    /// The requested token budget, before any floor is applied.
    #[must_use]
    pub fn requested_max_tokens(&self) -> Option<u64> {
        self.max_tokens
    }

    /// Render the JSON body, without the token floor (the client applies that
    /// when it sends, so a retry can raise the budget independently).
    ///
    /// `stream` is never set: the client reads whole responses, and the façade
    /// strips the field before delegating in any case.
    #[must_use]
    pub fn body(&self) -> Value {
        let mut map = Map::new();
        map.insert("model".into(), Value::String(self.model.clone()));
        map.insert(
            "messages".into(),
            serde_json::to_value(&self.messages).unwrap_or(Value::Null),
        );
        if let Some(t) = self.temperature {
            map.insert("temperature".into(), Value::from(t));
        }
        if let Some(p) = self.top_p {
            map.insert("top_p".into(), Value::from(p));
        }
        if let Some(k) = self.top_k {
            map.insert("top_k".into(), Value::from(k));
        }
        if let Some(n) = self.max_tokens {
            map.insert("max_tokens".into(), Value::from(n));
        }
        if !self.options.is_empty() {
            map.insert(
                "loom_options".into(),
                serde_json::to_value(self.options).unwrap_or(Value::Null),
            );
        }
        for (k, v) in &self.extra {
            map.insert(k.clone(), v.clone());
        }
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn declining_verbatim_keeps_the_scaffold() {
        let o = LoomOptions::declining_verbatim();
        assert_eq!(serde_json::to_value(o).unwrap(), json!({ "verbatim": false }));
        assert!(!o.wants_passthrough());
    }

    #[test]
    fn passthrough_declines_both() {
        let o = LoomOptions::passthrough();
        assert_eq!(
            serde_json::to_value(o).unwrap(),
            json!({ "verbatim": false, "scaffold": false })
        );
        assert!(o.wants_passthrough());
    }

    #[test]
    fn unset_options_are_omitted_entirely() {
        let body = ChatRequest::new("m", vec![Message::user("hi")]).body();
        assert!(
            body.get("loom_options").is_none(),
            "an empty options object would read as a deliberate instruction"
        );
    }

    #[test]
    fn body_carries_sampling_and_messages() {
        let body = ChatRequest::new("qwen", vec![Message::system("s"), Message::user("u")])
            .temperature(0.2)
            .top_p(0.95)
            .top_k(20)
            .max_tokens(4096)
            .body();
        assert_eq!(body["model"], "qwen");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "u");
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["top_p"], 0.95);
        assert_eq!(body["top_k"], 20);
        assert_eq!(body["max_tokens"], 4096);
    }

    #[test]
    fn stream_is_never_sent() {
        let body = ChatRequest::new("m", vec![Message::user("hi")]).body();
        assert!(body.get("stream").is_none());
    }
}
