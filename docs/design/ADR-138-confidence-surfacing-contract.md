# ADR-138 — Surface the confidence gate on the wire: an always-present `grounding` block, a named score scale, and `/health` policy + counters

**Status:** Proposed
**Date:** 2026-09-02
**Decision-type:** Interface (wire contract — no new retrieval heuristic; the existing gate is made legible)
**Deciders:** Dr John O'Hare (operator)
**Extends:** ADR-136 (D3 — the confidence-gated selective-injection policy; the gate itself is **unchanged**), ADR-137 (the Rust facade that serves it).
**Relates (this repo, loom):** `crates/loom-scaffold/src/policy.rs`, `crates/loom-scaffold/src/tuning.rs`, `crates/loom-facade/src/routes.rs`, `crates/loom-facade/src/config.rs`, `crates/loom-domain/src/model.rs`, `deploy/compose.profile-a.yml`, `dream.config.json`, `bench/UPLIFT-BENCH-PROTOCOL.md`, `README.md`.
**Relates (agentbox):** **ADR-051** (Loom client + deferred distillation — the harness-side consumer; this ADR is additive to its wire contract, nothing it reads today changes).

> This ADR records an **interface** decision. It does not change what the gate decides, only what the node says about the decision. Every number below already existed inside the process; none of them could be read from outside it.

---

## Context — the 2026-09-02 falsification

The nightly dream cycle **rejected** the slot `confidence-injection` (`dream.config.json` `slots[2]`). Not because the feature was wrong, but because it was **unevaluable**. Three independent findings, all verified at `865e9ea` and re-verified against the live HP node (`loom-facade-a`, `http://192.168.2.132:8084`) on 2026-09-02:

1. **No confidence surface exists.** `GET /health` (`routes.rs:70-104`) reports `ok`, `backend`, `index_classes`, `graph`, `semantic`, `generation`, `deploy_profile` — and nothing at all about the injection policy or what it has been deciding. `POST /loom/scaffold` returns `top_score` and `effective_budget`, but with no scale, no threshold and no decision, a consumer cannot tell a strong match from a weak one, or a skip from a failure.

2. **No sanctioned evaluator reads the per-request surface.** The two `evaluatorEntrypoints` were `"bench"` (pytest over `tests/`, one file: `test_mirror_generation.py`, which tests `mirror.sh`) and `"graph"` (`curl /health | tests/graph_check.py`). Neither touches injection. Worse, `graph_check.py:15` printed `graph.engine` — a field `GraphStatus` (`loom-domain/src/model.rs:327-332`) has never emitted, so it printed `None` every night it ran.

3. **The documentation contradicted the deployment.** `README.md:185` stated `LOOM_CONFIDENCE_INJECTION` was `1` in the HP compose. `deploy/compose.profile-a.yml` set none of `LOOM_CONFIDENCE_INJECTION`, `LOOM_STRONG_MATCH_SCORE`, `LOOM_MIN_INJECT_SCORE`, `LOOM_MIN_INJECT_FRACTION`, and the code default (`policy.rs`, `InjectionPolicy::default`) is **off**. The reference deployment therefore ran the *ungated* path while the README claimed otherwise. Nothing in the repo could have caught this, because nothing read the policy back off a running node.

The three compound: an unstated flag, an unreported decision, and an evaluator that looks at neither. That is the whole failure — a feature can be deployed, documented, and silently inert, and the nightly cycle can only record that it cannot tell.

## Decision

### D1 — An always-present `grounding` block on both request surfaces

`POST /loom/scaffold` gains a top-level `grounding`; `POST /v1/chat/completions` gains the same object at `loom.grounding`. **Always present** — a skipped request reports a skip. Absence and "no match" are different facts and must not share a representation.

```jsonc
"grounding": {
  "signal": "lexical" | "semantic" | "none",
  "top_score": 10.75 | null,
  "score_scale": "lexical-additive" | "cosine",
  "confidence": 1.0,
  "decision": "full" | "scaled" | "skipped" | "verbatim",
  "threshold": 2.0,
  "effective_budget": 1500 | null,
  "engaged": true,
  "seeds": [ { "iri", "score", "confidence", "quality", "provenance", "injected" } ]
}
```

Enum values are lowercase on the wire.

### D2 — `confidence` is the existing gate ratio, not a new heuristic

`confidence = clamp(top_score / strong_match_score, 0, 1)` on the lexical path — the *same* ratio `InjectionPolicy::effective_budget` already computes to scale the budget, now named and reported rather than discarded. **No new scoring, ranking or thresholding is introduced by this ADR.** That constraint is deliberate: a surfacing change that also changed behaviour would leave the A/B in `bench/UPLIFT-BENCH-PROTOCOL.md` uninterpretable, because the thing being measured would have moved at the same time as the instrument.

### D3 — The score scale is named on the wire

`score_scale` carries `lexical-additive` (the additive per-matched-n-gram scale where `EXACT_TITLE_WEIGHT = 8.0` is one exact title word) or `cosine`. A bare number is meaningless across paths: 0.82 is a strong cosine and a sub-threshold lexical score. Naming the scale is what makes `confidence` comparable and what makes a future semantic path expressible in the same block.

### D4 — `/health` reports the policy and a rolling decision window

Three new blocks: `injection_policy` (`confidence_injection`, `strong_match_score`, `min_inject_score`, `min_inject_fraction`, `score_scale`), `serving` (`verbatim_mode`, `verbatim_threshold`, `semantic_fallback`, `semantic_min_inject`), and `confidence` — a rolling window of `window` (1000), `requests`, `engaged`, `skipped`, `scaled`, `full`, `verbatim`, `mean_confidence`.

`injection_policy` is the direct remedy for finding 3: the node states what it is running, so a compose file and a README become checkable claims rather than assertions. The counters answer the operational question the per-request block cannot — *is the gate engaging at all, over real traffic?*

### D5 — The reference deployment states the gate explicitly

`deploy/compose.profile-a.yml` now sets all four variables, the three thresholds pinned at their code defaults (8.0 / 2.0 / 0.4). Pinning rather than omitting is the point: the bench A/B must vary only the master switch, and an omitted variable is an invisible dependency on a default that can change.

### D6 — The evaluators move to Rust and read the surface

`tests/graph_check.py`, `tests/generation_drift_check.py` and `tests/test_mirror_generation.py` are retired; their subjects are now `crates/loom-facade/src/bin/{graph_check,generation_check,confidence_check}.rs` and `crates/loom-facade/tests/{contract_live,mirror_generation}.rs`. The stdin contract (`curl /health | <bin>`) is preserved verbatim — it exists because inline `python3 -c "…"` lost its inner quotes crossing the annexe ssh `bash -lc` boundary, and the same boundary still applies to a Rust bin.

## Consequences

**Gained.** The gate becomes falsifiable from outside the process: a nightly evaluator can now fail on a policy that is off when it should be on, on counters that never move, and on a `grounding` block that is missing. The doc/deploy class of drift becomes machine-detectable rather than reader-detectable. The bench A/B gains a per-node aggregate (`/health.confidence`) that does not require re-parsing every answer.

**Cost.** The response bodies grow — bounded, but non-zero on high-volume paths. `/health` becomes stateful: the counters are a rolling in-memory window, so they reset on restart and are per-process (correct for profile A, which is a single container; a future multi-replica deployment would report per-replica windows, which is a known and accepted limitation of this ADR, not a defect).

**Not done here.** No semantic-path confidence is claimed beyond naming `cosine` as a scale — the HNSW fallback is still default-off pending the recall gate (ADR-137 §D5), and this ADR does not move that gate. No new heuristic (D2). No persistence of the counters.

**Operational.** **The HP container `loom-facade-a` must be redeployed** for D1, D4 and D5 to take effect. Until it is, `confidence-check` against the live node fails with exactly the three "block is MISSING" findings — which is the correct report, and is the evidence that the evaluator works.

## Verification

The evaluators ARE the verification; each maps to a finding above.

| Evaluator | Command | Holds |
|---|---|---|
| `confidence-check` | `curl -s :8084/health \| cargo run -q -p loom-facade --bin confidence-check` | D4, D5 — the three blocks exist and cohere: `min_inject_score <= strong_match_score`, fraction in (0, 1], `mean_confidence` in [0, 1], counters non-negative and integral, `engaged + skipped <= requests`, decision counters partition `engaged` |
| `contract_live` | `LOOM_URL=… cargo test -q -p loom-facade --test contract_live` | D1, D2, D3 — against a live node: an on-ontology prompt engages with `decision` in {`full`, `verbatim`} and `confidence >= 0.9`, seeds carry all six fields; an off-ontology prompt reports `skipped` / `confidence` 0 / empty seeds *with the block present*; two probes advance `/health.confidence.requests` |
| `graph-check` | `curl -s :8084/health \| cargo run -q -p loom-facade --bin graph-check` | D6 — asserts the fields `GraphStatus` actually emits (the `graph.engine` drift is fixed) |
| `generation-check` | `curl -s :8084/health \| cargo run -q -p loom-facade --bin generation-check` | D6 — the never-mixed-build read side (ADR-135 D2.1), warn-only unless `--strict` |
| `mirror_generation` | `cargo test -q -p loom-facade --test mirror_generation` | D6 — the promote-side verifier in `app/mirror.sh`, extracted from the shipped script so it cannot drift |

Prompts used by `contract_live`, both verified against the live HP node on 2026-09-02 before being fixed in the test:

- on-ontology: **`"rollup in blockchain scaling"`** → `engaged: true`, `top_score` 10.75, `fusion_path` `LexicalHit`, seeds `urn:ngm:class:rollup` and `urn:ngm:class:blockchain`. Against `strong_match_score` 8.0 that clamps to `confidence` 1.0.
- off-ontology: **`"banana pancakes recipe"`** → `engaged: false`, `top_score` 0.0, `fusion_path` `NoMatch`, zero seeds.

They are the same pair the bench protocol's live-probe status note used on 2026-08-11, so the contract test and the bench evidence describe the same two points on the scale.

`dream.config.json` `evaluatorEntrypoints` now runs all five. The `confidence-injection` slot has an evaluator for the first time.

## Closeout extension — 2026-09-04

Scope: Grounding visibility and consumer interpretation. Work packages: CP-01/02/03/08. Existing decision status, dates and deciders are retained; this review does not ratify a proposed decision or establish deployment activation. Accountable roles: Loom maintainer, corpus publisher and consuming-agent maintainer for their respective boundaries.

The inspected routes now build grounding objects for scaffold and successful delegated/verbatim chat. Non-200 backend/error responses follow separate handling. Presence of diagnostics does not ensure every consumer interprets them.

**Acceptance condition:** Test no-match, opt-out, semantic fallback, verbatim, delegated success and backend failure. Define the required grounding contract for each status and prove consuming agents preserve engagement, scale, generation and degradation.

Dependencies: authoritative corpus and release identity, publisher visibility rules and explicit consumer policy. Reopen on corpus format, model/serving mode, generation reporting or consumer changes. Source revision: `8cdef36bb571f0aed2d599d97a3efab02760b6d5`; current test receipts are kept separately so component tests cannot be mistaken for production evidence.

See the [estate grounding review](../../../VisionFlow/docs/estate-review/grounding-delivery.md) and [roadmap](../../../VisionFlow/docs/estate-review/closeout/README.md).

### Semantic artefact qualification — 2026-09-04

CP-01/03/07/08 acceptance also requires validating the stored database configuration before readiness and score labelling. RuVector restores stored settings over caller defaults. The actual adapter probe accepts a Euclidean artefact and converts its distance as cosine, and reports a wrong-width artefact ready before query failure. These are synthetic local results, not a deployed recall failure.

Require effective metric/dimension/model validation, database-to-sidecar binding, and wrong-configuration/restart fixtures before certifying semantic readiness or the cosine score scale. Preserve this record's existing decision status. See the [consumed-vector review](../../../VisionFlow/docs/estate-review/consumed-vector-storage.md) and [probe receipt](../../../VisionFlow/docs/estate-review/evidence/loom-vector-config-probe.json).

## Acceptance progress — 2026-09-05

Decision status unchanged. Recorded against the closeout extension's acceptance condition ("Test no-match, opt-out, semantic fallback, verbatim, delegated success and backend failure. Define the required grounding contract for each status…").

**Implemented.** The required contract is now named rather than implied: `loom_domain::REQUIRED_GROUNDING_FIELDS` lists the fourteen keys every grounding object must carry, and `GroundingStatus` enumerates the six answer paths. `loom-facade::routes::grounding::envelope` builds one shape for all of them, adding four keys the domain object cannot know to the nine it already had: `status`; `corpus_backed`, the single predicate a consuming agent should branch on, true only when the scaffold engaged AND the path can carry evidence; `generation` and `content_digest`, the LOADED serving identity (ADR-135), so an answer and the bytes behind it are named together instead of requiring a second `/health` call that could race a promotion; and `degraded`, always a list, naming the accelerators that were unavailable for that request. The gap the review named is closed: `error.rs` was split so the §7 mapping is available as data (`api_error_parts`), and the chat path's failure branch now attaches the contract to a 502/503 body. The status mapping is unchanged — what changed is that a consumer receiving a failure can now tell an unreachable model from an empty corpus. `ServedMode::Failed` gives the delivery axis its third value. `opt-out` is reported distinctly from `delegated`, because the node would have served verbatim and the caller declined: a benchmark that cannot see that choice will mis-attribute the latency to the serving regime.

**Tests and results.** `crates/loom-facade/tests/exp014_grounding_contract.rs`, 11 tests, all passing, one per status plus cross-status invariants. Every test runs the same `assert_contract` over `REQUIRED_GROUNDING_FIELDS` and then the status-specific meaning, so the field list cannot drift without failures. `backend_failure_carries_the_contract_and_is_never_corpus_backed` pins the central case: retrieval genuinely succeeded (`engaged: true`) and the answer is still not corpus-backed, with `backend-failure` named in `degraded`. `degradations_are_named_on_the_response_that_suffered_them` asserts that an unqualified semantic artefact surfaces its failing axis, not just its outcome. Router `debug_assert`s enforce the contract on the scaffold, chat and failure paths in every debug build. `cargo test --workspace` 278 passed / 0 failed; clippy clean under `-D warnings`.

**Receipts.** [Local façade receipt](../estate-closeout/2026-09-05/local-facade-receipt.json) — a live `POST /v1/chat/completions` against a retrieval-only node returned HTTP 503 with `loom.served_mode: "failed"`, `grounding.status: "backend-failure"`, `corpus_backed: false` and `degraded: ["graph-unavailable","backend-failure"]`. [Browser receipt](../estate-closeout/2026-09-05/browser-receipt.json) — a verbatim serve driven through a real browser reported `status: "verbatim"`, `corpus_backed: true`, and a generation and content digest matching `/health` exactly.

**Remaining.** The acceptance condition's second half — "prove consuming agents preserve engagement, scale, generation and degradation" — is not met here: this work makes the contract emittable and tested at the façade boundary, but the agent-side consumers (agentbox ADR-051's Loom client, the email gateway) were not modified or tested, so nothing yet proves the diagnostics reach a human rather than disappearing behind a fluent answer. HP deployment is outstanding.
