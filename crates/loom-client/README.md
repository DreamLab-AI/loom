# loom-client

A Rust client for an **Ontology Loom** façade — an OpenAI-compatible chat
endpoint that grounds each request in a formal ontology before delegating to a
language model.

The façade exists to be a *stable door*: consumers hold its URL, and the model
behind it can be swapped with no consumer change. This crate is the client side
of that arrangement. It speaks plain `/v1/chat/completions`, so it also works
against any OpenAI-compatible server — llama.cpp, vLLM — with the
Loom-specific behaviour degrading quietly to nothing.

```toml
[dependencies]
loom-client = "0.1"
```

```rust
use loom_client::{ChatRequest, LoomClient, LoomOptions, Message};

let client = LoomClient::builder("http://loom:8080/v1").build();

let answer = client
    .chat(
        ChatRequest::new("qwen3.8-27B", vec![Message::user("What grounds this claim?")])
            .temperature(0.2)
            .max_tokens(4096)
            .options(LoomOptions::declining_verbatim()),
    )
    .await?;

println!("{} (served {})", answer.content, answer.served_mode);
```

## Why not just use an HTTP client

Because three of the failure modes arrive as a well-formed HTTP 200, and each
one has already cost real work:

**The façade answers from retrieval without calling the model.** Status 200,
prose in `content` — and it is ontology markdown, not an answer. Two nights of
automated verdicts were derived from that text before anyone noticed. Here it
is `Error::ScaffoldOnly`, never a return value.

**The model is truncated at the token budget.** For a reasoning model the
budget is spent on `reasoning_content` first, so a truncated response has
*empty* visible content rather than a short answer. Below roughly 1536 tokens
this happens silently. The client floors sub-1536 budgets, and retries
truncation on a doubled budget rather than returning it.

**Grounding is applied to a subject the ontology does not cover.** A question
about a codebase came back reasoning about the blockchain sense of "Node".
`LoomOptions::passthrough()` declines grounding; if the façade grounds anyway,
that is `Error::PassthroughRefused`.

## Choosing the right switch

`LoomOptions` carries two independent controls. Picking the wrong one is the
commonest way to get a disappointing answer.

| | `declining_verbatim()` | `passthrough()` |
|---|---|---|
| sends | `{"verbatim": false}` | `{"verbatim": false, "scaffold": false}` |
| declines | answering from retrieval alone | grounding the request at all |
| still happens | the ontology scaffold is injected | nothing — a plain proxy |
| use when | the subject **is** in the ontology | the subject is **not** (a codebase, a document) |

Sending neither leaves the façade's configured behaviour untouched.

## Retries

`chat()` makes up to three attempts:

- **truncated** (`finish_reason = "length"`) — the budget doubles, retried at once.
- **transient** (transport failure, 5xx including Cloudflare 52x, empty body) —
  retried after a pause.
- anything else fails immediately.

`Error::is_transient()` is public, for callers running their own scheduling.

## Licence

MIT OR Apache-2.0, at your option.
