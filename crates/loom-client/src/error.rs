//! The failure taxonomy of a façade call.
//!
//! The variants are shaped by what a caller can actually *do* about each one.
//! [`Error::ScaffoldOnly`] and [`Error::PassthroughRefused`] look like success
//! on the wire — HTTP 200, a well-formed body — and are errors here because
//! treating them as answers is what has historically gone wrong (see their
//! documentation for the incidents).

/// A façade call that did not produce a usable model answer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The request never completed: DNS, connect, TLS, timeout, or a dropped
    /// connection. Retryable — [`crate::LoomClient`] already retries these once.
    #[error("transport failure talking to {base}: {source}")]
    Transport {
        /// The base URL that was being called.
        base: String,
        /// The underlying `reqwest` failure.
        #[source]
        source: reqwest::Error,
    },

    /// The façade answered with a non-2xx status. `body` is truncated to a few
    /// hundred bytes: it goes into logs, and a model error body can be enormous.
    #[error("HTTP {status} from {base}: {body}")]
    Http {
        /// The base URL that was being called.
        base: String,
        /// The HTTP status code.
        status: u16,
        /// The response body, truncated for logging.
        body: String,
    },

    /// A 2xx whose body was not JSON, or was JSON of the wrong shape.
    #[error("malformed response from {base}: {detail}")]
    Malformed {
        /// The base URL that was being called.
        base: String,
        /// What specifically was wrong.
        detail: String,
    },

    /// A well-formed response carrying no assistant content: no `choices`, an
    /// empty `choices`, or a `choices[0].message.content` of `null`.
    #[error("empty response from {base}: the model returned no content")]
    Empty {
        /// The base URL that was being called.
        base: String,
    },

    /// The façade served ontology retrieval *instead of* calling the model.
    ///
    /// This arrives as HTTP 200 with prose in `content`, so a client that does
    /// not look for it will parse scaffold markdown as if it were the model's
    /// answer. Two nights of dream-engine verdicts were derived from this text
    /// before it was detected (2026-09-01), which is why it is an error and not
    /// a flag on the response.
    ///
    /// Send [`crate::LoomOptions::declining_verbatim`] to force the delegate path.
    #[error("{base} served scaffold retrieval without model generation \
             (façade in verbatim mode, or the model backend is down)")]
    ScaffoldOnly {
        /// The base URL that was being called.
        base: String,
    },

    /// Passthrough was requested (`loom_options.scaffold = false`) but the
    /// façade reported a different `served_mode`.
    ///
    /// The usual cause is a façade predating ADR-139, which does not know the
    /// option and grounds the request anyway. For a subject the ontology does
    /// not cover — a codebase, an arbitrary document — that grounding is
    /// actively wrong: a packet about `pnpm verify` came back reasoning about
    /// the blockchain sense of "Node" (2026-09-09).
    #[error("{base} did not pass the request through (served_mode={served_mode}); \
             it needs the ADR-139 build")]
    PassthroughRefused {
        /// The base URL that was being called.
        base: String,
        /// The `served_mode` the façade actually reported.
        served_mode: String,
    },

    /// Every attempt stopped at the token budget (`finish_reason = "length"`).
    ///
    /// A truncated answer is never returned: for reasoning models the visible
    /// content can be empty while the whole budget went to `reasoning_content`,
    /// so a truncated body is not a short answer but an absent one.
    #[error("{base}: still truncated after {attempts} attempts (final budget {budget} tokens)")]
    Truncated {
        /// The base URL that was being called.
        base: String,
        /// How many attempts were made.
        attempts: u32,
        /// The token budget on the final attempt.
        budget: u64,
    },
}

impl Error {
    /// Whether retrying this call unchanged could plausibly succeed.
    ///
    /// Transport failures, 5xx (including Cloudflare's 52x) and empty bodies
    /// are worth one retry. A 4xx is the caller's fault and will not improve,
    /// and the semantic refusals are decisions rather than glitches.
    ///
    /// ```
    /// # use loom_client::Error;
    /// let e = Error::Http { base: "http://loom:8080/v1".into(), status: 503, body: String::new() };
    /// assert!(e.is_transient());
    /// let e = Error::Http { base: "http://loom:8080/v1".into(), status: 400, body: String::new() };
    /// assert!(!e.is_transient());
    /// ```
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Transport { .. } | Self::Empty { .. } => true,
            Self::Http { status, .. } => (500..600).contains(status),
            Self::Malformed { .. }
            | Self::ScaffoldOnly { .. }
            | Self::PassthroughRefused { .. }
            | Self::Truncated { .. } => false,
        }
    }
}

/// Trim `body` to `limit` bytes on a character boundary, for error messages.
pub(crate) fn truncate(body: &str, limit: usize) -> String {
    if body.len() <= limit {
        return body.to_owned();
    }
    let mut end = limit;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = body[..end].to_owned();
    out.push('…');
    out
}

/// Result alias for façade calls.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_bodies_whole() {
        assert_eq!(truncate("short", 100), "short");
    }

    #[test]
    fn truncate_does_not_split_a_multibyte_character() {
        // "é" is two bytes; cutting at 3 would land mid-character and panic on
        // a naive slice.
        let out = truncate("aaé", 3);
        assert!(out.starts_with("aa"), "got {out:?}");
        assert!(out.ends_with('…'));
    }

    #[test]
    fn five_hundreds_are_transient_and_four_hundreds_are_not() {
        let t = |status| Error::Http { base: String::new(), status, body: String::new() };
        assert!(t(500).is_transient());
        assert!(t(524).is_transient());
        assert!(!t(404).is_transient());
        assert!(!t(422).is_transient());
    }

    #[test]
    fn semantic_refusals_are_not_retried() {
        assert!(!Error::ScaffoldOnly { base: String::new() }.is_transient());
        assert!(!Error::PassthroughRefused { base: String::new(), served_mode: "grounded".into() }
            .is_transient());
    }
}
