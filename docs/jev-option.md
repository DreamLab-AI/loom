# Jev option (opt-in typed decisions, off by default)

An optional System One step (Choice / Score / Noul over `POST /v1/systemone`) beside the
chat path. **With it off, or unavailable for any reason, the forwarded request and the
response are byte-identical to the pre-change ones** (asserted against a Jev-off baseline in
`crates/loom-facade/tests/jev_option.rs`). Status: *proposed research candidate; no measured
uplift* — wiring and fallback tests do not show uplift.

| What leaves | Destination | Flag |
|---|---|---|
| The user question, and for each candidate record its slug, title and a ≤400-char definition excerpt; after generation, the served block and the answer text (each capped at 6,000 chars) | The endpoint resolved below, **local only by default** | `LOOM_JEV=1` (default off); nothing is built or sent otherwise |

Stages (each also switchable: `LOOM_JEV_GATE`, `LOOM_JEV_RANK`, `LOOM_JEV_SUPPORT`, on once `LOOM_JEV=1`):

- **Gate** (LM-01-S1-01): Choice over `grounded_lookup | mixed_request | ordinary_passthrough | insufficient_evidence` plus a Noul "answerable". It can only *withhold* an already-engaged scaffold, and only when `LOOM_JEV_GATE_MIN_CONFIDENCE` is set (unset ⇒ the verdict is recorded in `jev.gate`, never acted on). It runs after retrieval, the generation guard and `loom_options.scaffold=false`, all of which keep precedence.
- **Rank** (LM-02-S1-01): one Score per candidate record; the candidates are reordered and re-assembled through the same gate (`LexicalIndex::assemble`). Kept only if the granted budget, injection decision and generation are unchanged; retrieval and the budget stay deterministic. `LOOM_JEV_RANK_MIN_CONFIDENCE` (default 0, untuned).
- **Support** (LM-02-S1-01): one Noul after generation, annotation only (`jev.support`: `supported_probability`, `flagged_unsupported` below `LOOM_JEV_SUPPORT_THRESHOLD`=0.5, `source_ids`). The answer is never altered. Verbatim-served answers are not checked.

Endpoint resolution (first match wins): `LOOM_JEV_ENDPOINT` (+ `LOOM_JEV_MODEL`, `LOOM_JEV_KEY_ENV` = *name* of the env var holding a bearer token) → the agentbox loopback proxy (`AGENTBOX_SYSTEM_ONE_PROXY=1`, `AGENTBOX_SYSTEM_ONE_ENDPOINT`/`_MODEL`, no `Authorization` header) → `https://api.typesafe.ai/v1/systemone` with `TYPESAFE_API_KEY`. Loom serves private corpus content, so a **non-local endpoint is refused** (loopback, private, `.local` and single-label hosts count as local; `LOOM_JEV_LOCAL=1` declares an explicit endpoint local; the proxy needs `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL=1`) unless `LOOM_JEV_ALLOW_CLOUD=1`. The public endpoint always needs `LOOM_JEV_ALLOW_CLOUD=1`.

Fallback (one log line with the reason only, never request content): `flag-off`, `no-endpoint`, `non-local-refused`, `timeout` (hard deadline `LOOM_JEV_TIMEOUT_MS`, default 4000, no retries), `transport`, `http-<code>` (429/529 are not retried), `malformed` (bad body, missing answer IDs, Choice key outside the offered set), `low-confidence`, `invariant-broken` (rank only). Question sets are versioned (`loom.lm-01-s1-01.v1`, `loom.lm-02-s1-01.rank.v1`, `loom.lm-02-s1-01.support.v1`). See `crates/loom-facade/src/jev.rs`.

