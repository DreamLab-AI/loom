# ADR-139 — Per-request scaffold opt-out: the façade as a plain proxy

Status: Accepted · 2026-09-10 · Extends ADR-135 D1 (the stable model door) and ADR-138 (the
grounding contract) · Realised in `loom_options.scaffold`

## Context

ADR-135 D1 makes the façade the one door every consumer holds; the model behind it swaps with
no consumer change. That door was built for subjects the ontology covers, and the serving
regime (F1 verbatim, `LOOM_VERBATIM_MODE=1` on profile A) rewards it: a high-confidence
lexical hit is served from the corpus in milliseconds.

On 2026-09-09 a consumer with a subject outside the ontology, a repository explainer
drafting a section about a test script, sent a packet whose prose contained the words
*node* and *verification*. The lexical gate scored the blockchain class **Node** at 42 against
a verbatim threshold of 8 and served it as the answer in 40 ms with zero completion tokens.
The consumer had no way to say "not this corpus": `loom_options.verbatim=false` only declines
the verbatim serve and still injects the scaffold into the prompt, and the only alternative
was to bypass the façade and target the model port directly, which forfeits the door.

## Decision

1. `loom_options.scaffold: false` on `POST /v1/chat/completions` makes the façade a plain
   proxy for that request: no retrieval, no injection, no verbatim serve, no thinking
   control, the Loom-private field stripped, the body otherwise forwarded unchanged.
2. The response stays honest. `loom.served_mode` is `passthrough` (a new `ServedMode`
   variant), `grounding.status` is `passthrough` (a new `GroundingStatus` variant, distinct
   from `no-match` because the corpus was never consulted), `corpus_backed` is `false`, and
   `injected_tokens` is 0. The request is not recorded in the confidence window and does not
   mark the generation as served.
3. Absence of the key, or any value other than the boolean `false`, leaves the scaffold on.
   No header, no environment switch: the choice belongs to the request, like `verbatim`.
4. The direct model port stays what it was: an implementation detail behind the door, not a
   documented consumer path.

## Consequences

- Consumers with mixed subjects (an email gateway, an agent mesh, a documentation skill) hold
  one base URL and decide per request whether the corpus applies.
- Benchmarks can separate "the corpus had nothing" from "the caller did not ask", and the
  confidence counters describe only requests that consulted the gate.
- A passthrough is never corpus-backed; a consumer that wants grounding must not send it.

## Verification

`crates/loom-facade/tests/exp012_serving.rs`: `passthrough_forwards_the_request_unchanged`
(messages identical, no `chat_template_kwargs`, no `loom_options`, telemetry as above under
verbatim and no-think both on), `passthrough_never_serves_verbatim` (503 with a retrieval-only
backend proves no verbatim serve), `scaffold_stays_on_unless_explicitly_false`. Unit tests in
`serving.rs` and `model_tests.rs`. Live check after redeploy: the 2026-09-09 packet answered by
the model with `served_mode: passthrough`.
