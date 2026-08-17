# EXP-embed — loom-embed-xinference (Stage 2)

**Verdict:** **PASS** — build, mock suite, live probe, and clippy (`-D warnings`) all green.
Feeds EXP-008's chain.

- **Date (UTC):** 2026-08-17T14:39:07Z
- **Git SHA (parent):** a3f72847568d9544c156db9c3fe873e0a431e254 (`rust: workspace foundation + loom-domain [stage 1]`)
- **Crate:** `crates/loom-embed-xinference/**` (owned paths only)
- **Toolchain:** container `cargo` 1.97.0 (rust-toolchain.toml pins stable; ignored by non-rustup cargo)
- **Model lock:** `bge-small-en-v1.5` / 384 dims (const — ops-law lock, RUST-ARCHITECTURE §11.3)

## Deliverable summary

`XinferenceEmbedder` implements `loom_domain::EmbeddingProvider`:
- `from_env()` reads `XINFERENCE_URL` (default `http://xinference:9997/v1`) and
  `XINFERENCE_TIMEOUT_SECS` (default 60); `new(base, timeout)` for tests/DI.
- POSTs `{"model":"bge-small-en-v1.5","input":[…]}` to `{base}/embeddings`.
- Response `data` sorted by `.index` (parity with `tools/ingest/embed_and_stage.py`);
  count-per-input checked; each vector length-checked `== 384` else `LoomError::Dimension{got,want}`.
- `embed()` delegates to `embed_batch` of one; empty batch short-circuits (no request).
- Transport failures (connect / timeout / decode), non-200, and malformed JSON → `LoomError::Embed(detail)`.
- `verify(&self)` startup probe: embeds `"probe"`, asserts 384 (for the facade to call).
- `model_id()` → `"bge-small-en-v1.5"` (const); `dimensions()` → `384` (const).

## Commands + raw tails

### 1. `cargo build -p loom-embed-xinference`
```
   Compiling loom-embed-xinference v0.1.0 (/home/devuser/workspace/loom/crates/loom-embed-xinference)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.89s
```

### 2. `cargo test -p loom-embed-xinference` (mock suite — wiremock dev-dep)
```
running 7 tests
test tests::model_and_dimensions_are_locked ... ok
test tests::empty_batch_short_circuits ... ok
test tests::connection_refused_is_embed_error ... ok
test tests::single_embed_delegates_and_checks_dimension ... ok
test tests::happy_path_returns_vectors_in_input_order ... ok
test tests::server_500_is_embed_error ... ok
test tests::wrong_width_is_dimension_error ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/live.rs
test live_embed_is_384_unit_norm ... ignored, requires a live XINFERENCE_URL
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```
Mock coverage: happy path (2 inputs, shuffled response indices → returned in input
order), single-embed delegation, dimension mismatch (383 → `Dimension{got:383,want:384}`),
HTTP 500 → `Embed`, connection refused → `Embed`, empty-batch short-circuit, locked-const check.

### 3. `cargo test -p loom-embed-xinference -- --ignored` (live smoke against real Xinference)
```
     Running tests/live.rs (target/debug/deps/live-e707fc3086a8a1c4)
running 1 test
test live_embed_is_384_unit_norm ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```
Embeds `"rgb protocol"` against `http://xinference:9997/v1`, asserts 384-dim + unit-norm (±1e-3).

**Independent verification of the live vector (direct POST, python):**
```
model=bge-small-en-v1.5 dims=384 l2_norm=1.000000
```

### 4. `cargo clippy -p loom-embed-xinference --all-targets -- -D warnings`
```
    Checking loom-embed-xinference v0.1.0 (/home/devuser/workspace/loom/crates/loom-embed-xinference)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.69s
```
Zero warnings under workspace pedantic lints with `-D warnings`.

## Live-probe result
- **Endpoint:** `http://xinference:9997/v1/embeddings`
- **Model:** `bge-small-en-v1.5`
- **Dimensions:** 384
- **L2 norm of embed("rgb protocol"):** 1.000000 (unit-norm, within ±1e-3)

## Deviations
- `from_env()` returns `Self` (infallible), not `Result` — matches the design's call-site
  usage (§8.4 recall-gate wiring `let embed = XinferenceEmbedder::from_env();`); a rustls
  client-build failure is treated as an unrecoverable startup fault (`.expect`).
- Added a defensive response-count check (`data.len() == input.len()`) beyond the brief —
  a malformed reply is `LoomError::Embed`, never a silently-partial answer.
- wiremock 0.6 chosen over httpmock (async-native, tokio-friendly) as the dev-dep.
