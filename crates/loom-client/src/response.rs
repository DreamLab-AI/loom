//! What came back, including the façade's own account of how it answered.

use serde_json::Value;

/// How the façade produced the answer, from the `loom.served_mode` telemetry.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServedMode {
    /// Delegated to the model with the ontology scaffold injected.
    Grounded,
    /// Answered from ontology retrieval alone, with no model call.
    Verbatim,
    /// Forwarded to the model unchanged (ADR-139).
    Passthrough,
    /// The façade sent no telemetry — it is an older build, or a plain
    /// OpenAI-compatible server standing in for one.
    Unreported,
    /// A `served_mode` this crate does not know.
    Other(String),
}

impl ServedMode {
    /// The wire spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Grounded => "grounded",
            Self::Verbatim => "verbatim",
            Self::Passthrough => "passthrough",
            Self::Unreported => "unreported",
            Self::Other(s) => s,
        }
    }

    fn parse(raw: Option<&str>) -> Self {
        match raw {
            None => Self::Unreported,
            Some("grounded") => Self::Grounded,
            Some("verbatim") => Self::Verbatim,
            Some("passthrough") => Self::Passthrough,
            Some(other) => Self::Other(other.to_owned()),
        }
    }
}

impl std::fmt::Display for ServedMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Token counts, when the façade reported them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    /// Tokens in the prompt.
    pub prompt_tokens: Option<u64>,
    /// Tokens generated.
    pub completion_tokens: Option<u64>,
}

/// A successful answer, with enough context to audit how it was produced.
#[derive(Clone, Debug)]
pub struct ChatOutcome {
    /// The assistant's visible text.
    pub content: String,
    /// The model's reasoning trace, when it emitted one separately. Excluded
    /// from [`content`](Self::content) — reasoning models put their working
    /// here and only the conclusion in `content`.
    pub reasoning: Option<String>,
    /// The model id the façade reported, which may differ from the one asked
    /// for: the point of the façade is that the model can be swapped behind it.
    pub model: Option<String>,
    /// How the façade answered.
    pub served_mode: ServedMode,
    /// `loom.grounding.status`, when reported.
    pub grounding_status: Option<String>,
    /// Token counts, when reported.
    pub usage: Usage,
    /// `choices[0].finish_reason`.
    pub finish_reason: Option<String>,
    /// How many attempts this answer took (1 when it succeeded first time).
    pub attempts: u32,
    /// The token budget of the successful attempt, after flooring and any
    /// truncation doubling.
    pub max_tokens: u64,
    /// The whole response body, for callers needing a field this type omits.
    pub raw: Value,
}

impl ChatOutcome {
    /// Whether the façade grounded this answer in the ontology.
    #[must_use]
    pub fn was_grounded(&self) -> bool {
        matches!(
            self.served_mode,
            ServedMode::Grounded | ServedMode::Verbatim
        )
    }
}

/// Parse the `loom` telemetry block, absent on a plain `OpenAI` server.
pub(crate) fn served_mode(body: &Value) -> ServedMode {
    ServedMode::parse(
        body.get("loom")
            .and_then(|l| l.get("served_mode"))
            .and_then(Value::as_str),
    )
}

pub(crate) fn grounding_status(body: &Value) -> Option<String> {
    body.get("loom")
        .and_then(|l| l.get("grounding"))
        .and_then(|g| g.get("status"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

pub(crate) fn usage(body: &Value) -> Usage {
    let get = |k: &str| {
        body.get("usage")
            .and_then(|u| u.get(k))
            .and_then(Value::as_u64)
    };
    Usage {
        prompt_tokens: get("prompt_tokens"),
        completion_tokens: get("completion_tokens"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_telemetry_reads_as_unreported_not_as_a_failure() {
        // A plain llama.cpp server sends no `loom` block; that is not an error.
        assert_eq!(
            served_mode(&json!({ "choices": [] })),
            ServedMode::Unreported
        );
    }

    #[test]
    fn known_modes_parse() {
        for (raw, want) in [
            ("grounded", ServedMode::Grounded),
            ("verbatim", ServedMode::Verbatim),
            ("passthrough", ServedMode::Passthrough),
        ] {
            assert_eq!(
                served_mode(&json!({ "loom": { "served_mode": raw } })),
                want
            );
        }
    }

    #[test]
    fn an_unknown_mode_is_preserved_rather_than_dropped() {
        let m = served_mode(&json!({ "loom": { "served_mode": "future-mode" } }));
        assert_eq!(m, ServedMode::Other("future-mode".into()));
        assert_eq!(m.as_str(), "future-mode");
    }

    #[test]
    fn usage_is_optional_field_by_field() {
        let u = usage(&json!({ "usage": { "prompt_tokens": 10 } }));
        assert_eq!(u.prompt_tokens, Some(10));
        assert_eq!(u.completion_tokens, None);
    }

    #[test]
    fn grounding_status_is_read_from_the_nested_block() {
        assert_eq!(
            grounding_status(&json!({ "loom": { "grounding": { "status": "ok" } } })),
            Some("ok".to_owned())
        );
        assert_eq!(grounding_status(&json!({ "loom": {} })), None);
    }
}
