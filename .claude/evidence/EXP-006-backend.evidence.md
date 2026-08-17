# EXP-006 (backend half) — the model-swap seam delegates faithfully — EVIDENCE

**Verdict: PASS**

- Date (UTC): 2026-08-17T14:48:52Z
- Git SHA (evidence produced against): abb544b6ae76ebb1349696667d4c54fe90723387
- Toolchain: `cargo 1.97.0 (c980f4866 2026-06-30)` (container plain cargo, no rustup)
- Crate: `crates/loom-backend-openai` (`OpenAiBackend` impl `loom_domain::ModelBackend`)
- Reference ported: `app/loom_facade.py` — `_backend(path, body, method)` (delegate)
  and `_probe_backend()` (reachability), plus the `do_POST` `/v1/chat/completions`
  pre-delegation rewrite (`j.pop("stream")`, `max_tokens`/`max_completion_tokens`
  floor). RUST-ARCHITECTURE §11.4 + §10.
- Engine: pooled `reqwest::Client` (rustls-tls, `default-features = false`);
  mock upstream via `wiremock 0.6` (dev-dep, this crate only).

## Assertions ↔ evidence

| # | Assertion (Python semantics) | Test | Result |
|---|---|---|---|
| 1 | sub-floor `max_tokens` raised 256 → 1536 | `floor_raises_sub_floor_max_tokens` | ok |
| 2 | higher ask preserved (4096 stays 4096, never lowered) | `higher_ask_is_preserved` | ok |
| 3 | floor disabled at `0` (256 stays 256, no default insert) | `floor_disabled_at_zero_leaves_body_untouched` | ok |
| 4 | `stream` stripped before delegation | `stream_is_stripped` | ok |
| 5 | `max_completion_tokens` floored too (100 → 1536) | `max_completion_tokens_is_floored_too` | ok |
| 6 | neither field present + active floor → `max_tokens = floor` | `absent_token_fields_get_default_floor_inserted` | ok |
| 7 | non-2xx → `LoomError::BackendHttp{status,body}` (502) | `non_2xx_maps_to_backend_http` | ok |
| 8 | `/models` passthrough returns the upstream JSON verbatim | `models_passes_through` | ok |
| 9 | `reachable()` true on 2xx `/models` probe | `reachable_true_on_2xx` | ok |
| 10 | `reachable()` false on non-2xx probe (mirrors urlopen raising) | `reachable_false_on_5xx` | ok |
| 11 | empty `DISTILL_BACKEND_URL` → `NoBackend`, `endpoint()` == "" | `empty_endpoint_is_retrieval_only` | ok |

Assertions 1/2/5/6 verify the floor via `wiremock`'s exact `body_json` match: the
upstream only answers 200 when it receives the **floored** body, so an Ok result
proves the rewrite; a mismatch falls through to wiremock's default 404 and fails
the test. Assertions 3/4 use `body_json` on the exact expected (unfloored /
stream-less) body, so they prove the negative (no over-flooring, `stream` gone).

## Divergences from the Python façade (Python semantics win)

1. **`content_type` dropped.** The mission brief names `BackendResponse{status,
   body, content_type}`, but the FROZEN domain type is `BackendResponse{status,
   body: serde_json::Value}` (`loom-domain/src/model.rs:303`) — no `content_type`.
   The adapter conforms to the frozen type: 2xx bodies are parsed to `Value`
   (raw-string fallback if a 2xx body is not JSON). Python's third tuple element
   (the upstream `Content-Type`) has no home in the frozen contract and is not
   re-introduced. This is a domain-contract constraint, not a behaviour change.
2. **`reachable()` probes `/models`, not `/health`.** The `ModelBackend` port
   doc-comment says "`/health` probe"; Python's `_probe_backend` probes
   `{BACKEND}/models`. Mission pins Python semantics → `/models` it is. Success is
   defined as a 2xx response (Python's `urlopen` raises `HTTPError` on non-2xx and
   any transport error → `False`); mirrored via `is_ok_and(|r| r.status().is_success())`.
3. **`NoBackend` is a typed error at the adapter, 503 at the façade.** Python's
   `_backend` returns `503 {"error":"no DISTILL_BACKEND_URL configured"}` inline.
   The adapter returns `Err(LoomError::NoBackend)`; the façade's `IntoResponse`
   maps it to 503 (RUST-ARCHITECTURE §7 table). Same wire outcome, typed seam.
4. **URL join.** Python computes `f"{BACKEND}{path[len('/v1'):]}"`; since
   `DISTILL_BACKEND_URL` already carries the `/v1` suffix, this reduces to
   appending the bare sub-path (`/chat/completions`, `/models`). Endpoint has any
   trailing `/` stripped so joins stay canonical. Model identity is NEVER encoded
   in `endpoint()` (ADR-135 D1.2) — it rides in the response body.

## Raw command tails

```
### $ cargo build -p loom-backend-openai
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s
[exit=0]

### $ cargo test -p loom-backend-openai
running 11 tests
test empty_endpoint_is_retrieval_only ... ok
test reachable_true_on_2xx ... ok
test max_completion_tokens_is_floored_too ... ok
test reachable_false_on_5xx ... ok
test absent_token_fields_get_default_floor_inserted ... ok
test non_2xx_maps_to_backend_http ... ok
test higher_ask_is_preserved ... ok
test floor_disabled_at_zero_leaves_body_untouched ... ok
test models_passes_through ... ok
test floor_raises_sub_floor_max_tokens ... ok
test stream_is_stripped ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
[exit=0]

### $ cargo clippy -p loom-backend-openai --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s
[exit=0]
```

**Test count: 11 passed / 0 failed.** Build, test, clippy (`-D warnings`,
`--all-targets --all-features`) all exit 0. The recurring `warning: failed to
auto-clean cache data … Permission denied` line is a shared read-only
`.cargo/registry` housekeeping warning unrelated to this workspace — not a
compiler/clippy diagnostic (no `src/` path referenced).
