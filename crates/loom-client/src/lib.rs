//! A client for an **Ontology Loom** façade.
//!
//! The Loom is an OpenAI-compatible chat endpoint that grounds each request in
//! a formal ontology before delegating to a language model. The point of the
//! façade is that it is a *stable door*: consumers hold its URL, and the model
//! behind it can be swapped with no consumer change. This crate is the client
//! side of that arrangement.
//!
//! It speaks plain `/v1/chat/completions`, so it also works against any
//! OpenAI-compatible server — a llama.cpp instance, say — with the
//! Loom-specific behaviour degrading quietly to nothing.
//!
//! # Why not just use an HTTP client
//!
//! Because three of the failure modes look exactly like success, and each one
//! has already cost real work:
//!
//! - **The façade answers from retrieval without calling the model.** HTTP 200,
//!   well-formed body, prose in `content` — and it is ontology markdown, not an
//!   answer. Two nights of automated verdicts were derived from that text
//!   before anyone noticed. Here it is [`Error::ScaffoldOnly`].
//! - **The model is truncated at the token budget.** For a reasoning model the
//!   budget goes to `reasoning_content` first, so a truncated response has
//!   *empty* visible content rather than a short answer. Below roughly 1536
//!   tokens this happens silently, which is why [`raise_integer_token_floor`]
//!   exists and why truncation is retried on a doubled budget rather than
//!   returned.
//! - **Grounding is applied to a subject the ontology does not cover.** Asking
//!   about a codebase produced reasoning about the blockchain sense of "Node".
//!   [`LoomOptions::passthrough`] declines grounding; if the façade grounds
//!   anyway, that is [`Error::PassthroughRefused`] rather than an answer.
//!
//! # Getting started
//!
//! ```no_run
//! # async fn demo() -> Result<(), loom_client::Error> {
//! use loom_client::{ChatRequest, LoomClient, LoomOptions, Message};
//!
//! let client = LoomClient::builder("http://loom:8080/v1")
//!     .timeout(std::time::Duration::from_secs(300))
//!     .build();
//!
//! let answer = client
//!     .chat(
//!         ChatRequest::new("qwen3.8-27B", vec![
//!             Message::system("Return only JSON."),
//!             Message::user("What does the ontology say about grounding?"),
//!         ])
//!         .temperature(0.2)
//!         .max_tokens(4096)
//!         // The subject IS in the ontology: ground it, but never answer from
//!         // retrieval alone.
//!         .options(LoomOptions::declining_verbatim()),
//!     )
//!     .await?;
//!
//! println!("{} (served {})", answer.content, answer.served_mode);
//! # Ok(()) }
//! ```
//!
//! # Choosing the right switch
//!
//! [`LoomOptions`] carries two independent controls, and picking the wrong one
//! is the most common way to get a disappointing answer:
//!
//! - Subject **is** in the ontology → [`LoomOptions::declining_verbatim`].
//!   Grounded generation, with the retrieval-only short-circuit declined.
//! - Subject is **not** in the ontology → [`LoomOptions::passthrough`]. The
//!   façade becomes a plain proxy for that request, and you still hold the
//!   stable door rather than reaching around it to the model's own port.
//!
//! # Failure handling
//!
//! [`LoomClient::chat`] retries a truncated answer on a doubled budget and a
//! transient failure after a pause, up to an attempt ceiling. Everything else
//! fails immediately. [`Error::is_transient`] is public so callers doing their
//! own scheduling can reuse the judgement.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod client;
mod error;
mod request;
mod response;
mod tokens;

pub use client::{
    resolve_base, LoomClient, LoomClientBuilder, DEFAULT_MAX_ATTEMPTS, DEFAULT_RETRY_BACKOFF,
    DEFAULT_TIMEOUT,
};
pub use error::{Error, Result};
pub use request::{ChatRequest, LoomOptions, Message};
pub use response::{ChatOutcome, ServedMode, Usage};
pub use tokens::{apply_token_floor, raise_integer_token_floor, DEFAULT_MIN_MAX_TOKENS};
