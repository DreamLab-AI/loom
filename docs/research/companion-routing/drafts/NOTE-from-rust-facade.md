# rust-facade → rust-core / team-lead (SendMessage is disabled this session)

## 1. Workspace root
`crates/Cargo.toml` does NOT exist and `crates/` is not a workspace — the repo convention is
one workspace root per family (`crates/colloquy/Cargo.toml` is its own root). I have created
**`crates/system-one/Cargo.toml`** as the SSO workspace root. `rust-core`: please ADD
`system-one-core` / `system-one-client` to its `members` rather than creating `crates/Cargo.toml`
(a parent workspace at `crates/` collides with colloquy's root and with this one).

## 2. Dependency posture
Core/client were not on disk when I started, so the façade is self-contained today
(`protocol.rs`, `tokens.rs`, `fit.rs`). When core lands I collapse those to
`pub use system_one_core::…` if signatures line up. Shapes I coded against:

- `State = serde_json::Value`; `render_state`: sorted keys, `key: value` per line, non-strings as compact JSON.
- `Question` `#[serde(tag="type", rename_all="lowercase")]`: Choice{instructions, criteria: IndexMap<String,String>}, Score{instructions, criteria: Vec<String>}, Noul{instructions}.
- `Answer` (untagged out): Choice{choice, confidence, probabilities: IndexMap<String,f64>}, Score{score, distribution, confidence}, Noul{noul}.
- `estimate_tokens` is the jev-compaction heuristic ported verbatim from
  `config/claude-plugins/jev-compaction/lib/state.ts` (word: 1+floor((len-1)/6); digits: len/2;
  other symbol: 0.9; ceil). Consumers budget with that function, so core must not invent another.
- `fit_options(opts, scores, budget_tokens, k, always) -> Result<IndexMap,_>`,
  `window_state(text, window_tokens, overlap) -> Vec<String>` (line-aware).
- IndexMap everywhere order matters; serde_json with `preserve_order`.

## 3. To the Python engine author (laya-engine)
The façade calls `POST /predict` with **`state` as a plain STRING** (the façade owns rendering,
windowing and shortlisting) and `questions` in the §2 shape. It reads back
`{"answers": {...}, "usage": {"input_tokens": n, "output_tokens": n}, "ms": n}`.
`GET /v1/models` is parsed permissively: `max_len`/`head_max_len` are accepted at the top level,
inside `data[0]`, or inside `models[0]`. Please emit at least one of those.

## 4. agentbox.sh (not mine to edit)
`./agentbox.sh systemone eval` should exec:
  `system-one-eval run --backend <endpoint> --cases tests/system-one/routing-cases.json --skills-dir /opt/agentbox/skills`
and `parity --backend-a <sso> --backend-b <typesafe>` for the backend-vs-backend diff.
