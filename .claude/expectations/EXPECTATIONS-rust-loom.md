# EXP — Rust Loom re-platform (PRD-027 / ADR-137 / RUST-ARCHITECTURE.md)

EDD contract for the implementation mesh. Every EXP needs **executed evidence**
(command + raw output + timestamp + git SHA) from an evidence-producer agent,
then an adversarial audit by GPT-5.4 via `codex exec -m gpt-5.4` (anti-fox,
different model family). Narrative evidence is auto-rejected.

Source of truth for behaviour: `docs/design/RUST-ARCHITECTURE.md`. Python
reference semantics: `app/ontology_scaffold.py`, `app/loom_facade.py`,
`app/loom_graph.py`, `app/mirror.sh`.

---

## EXP-001 — workspace compiles in both feature planes
category: executable · regression_critical: true
The Cargo workspace builds clean with `cargo build --workspace --all-features`
AND `cargo build --workspace --no-default-features` (serving binary must not
link pg-write / attest / semantic-fallback code when those features are off).
stabilized_by: CI gate (deploy/ci or justfile target)

## EXP-002 — scaffold output byte-identical to Python on the fixture
category: executable · regression_critical: true
The ported `_selftest()` fixture (7-class inline fixture from
`app/ontology_scaffold.py`) passes in Rust with the SAME assertions, and a
golden-file test pins the exact `[ONTOLOGY CONTEXT] … [END ONTOLOGY CONTEXT]`
block string produced by Python for the fixture query. Byte-identical.
stabilized_by: loom-scaffold golden test

## EXP-003 — confidence-gate math parity
category: executable · regression_critical: true
`InjectionPolicy::effective_budget` matches the Python gate branch exactly
across the table: confidence_injection on/off × top_score below/at/above
MIN_INJECT_SCORE and STRONG_MATCH_SCORE, incl. the MIN_INJECT_FRACTION clamp.
stabilized_by: policy table tests

## EXP-004 — SPARQL clamp holds
category: executable · regression_critical: true
The graph adapter rejects INSERT/DELETE/LOAD/CLEAR/DROP/SERVICE, requires
SELECT/ASK/CONSTRUCT/DESCRIBE, injects LIMIT on unclamped SELECT, and caps
rows — same regex semantics as `loom_graph.py`. Loads ONLY ontology.ttl +
ontology-inferred.ttl (hard allowlist, no glob).
stabilized_by: loom-graph-oxigraph clamp tests

## EXP-005 — endpoint parity with the Python façade
category: executable · regression_critical: true
Router serves /health, /loom/generation(+alias), /loom/scaffold(+alias),
/loom/sparql(+alias), /loom/search(+alias), /loom/search/semantic,
/v1/chat/completions, /v1/models with the shapes in RUST-ARCHITECTURE §9;
NoBackend → 503; backend unreachable → 502; graph/semantic absence degrades
(never a client error). Verified by axum oneshot tests.
stabilized_by: loom-facade router tests

## EXP-006 — chat delegation semantics
category: executable · regression_critical: true
`/v1/chat/completions`: scaffold built from the LAST user message, merged into
the system message (insert at 0 if absent), max_tokens/max_completion_tokens
floored to ≥ LOOM_MIN_MAX_TOKENS=1536 (never lowering a higher ask), `stream`
stripped, response annotated with the `loom:{mode, injected_tokens, grounding,
fusion_path, generation}` block. Verified against a wiremock backend.
stabilized_by: loom-backend-openai + facade integration tests

## EXP-007 — I-P1: no engine shape escapes as an answer
category: executable · regression_critical: true
Every retrieval port returns Iri/ConceptMatch/CanonicalUnit/Scaffold. HNSW
candidates reach the wire ONLY via LexicalIndex::assemble (the single gate);
`LOOM_SEMANTIC_FALLBACK` defaults to 0 (off). Evidence: the port signatures +
a fusion test proving semantic candidates flow through assemble and that
disabling the flag yields lexical-only behaviour.
Carve-out: the one labelled, default-off debug endpoint
(`/loom/search/semantic`, gated by `LOOM_SEMANTIC_DEBUG_SURFACE`) may expose
bare IRI+score BECAUSE it is labelled as the index and can never feed
`/v1/chat/completions`; every ANSWER path goes through `assemble`.
stabilized_by: fusion tests + compile-time port signatures

## EXP-008 — semantic artifact bootstrap + recall floor
category: partially_verifiable (needs live PG/Xinference) · regression_critical: true
The exporter builds `ontology-corpus.rvdb` from the 8,146 verified embeddings
in ruvector-postgres ontology-corpus (no re-embedding), and
`VectorIndex::nearest(embed("rgb protocol"), 5)` surfaces IRI slug
`rgb-protocol` while an off-ontology decoy stays < 0.55.
The cosine ≥ 0.87 floor is the **PRECONDITION FOR DEFAULT-ON**, not a current
pass: measured recall is **0.816** in the document-embedding regime, so the gate
is **RED — the correct honest state** and `LOOM_SEMANTIC_FALLBACK` stays
default-off (audit finding 5). The `recall_gate` test hard-asserts the floor
when `LOOM_SEMANTIC_FALLBACK=1` (flip-on precondition) and asserts the gate is
reported RED when off (staleness tripwire); override the floor via
`LOOM_SEMANTIC_RECALL_FLOOR` once a query-shaped embedding lands.
stabilized_by: recall_gate integration test (feature semantic-fallback)

## EXP-009 — generation parity guard (never-mixed-build)
category: executable · regression_critical: true
Fusion skips the semantic fallback (with a warning, lexical-only degrade) when
the semantic index generation != lexical generation. GenerationStore prefers
build-manifest → mirror-manifest (.generation.json) → scaffold-index.
stabilized_by: fusion generation-mismatch test

## EXP-010 — lexical match performance gate
category: executable · regression_critical: false
`match()` p99 < 50ms over a synthetic 8k-class index (criterion bench ported
from the Python self-test generator).
stabilized_by: criterion bench in CI

## EXP-011 — lint/safety bar
category: executable · regression_critical: true
`cargo clippy --all-targets --all-features -- -D warnings` is clean (pedantic
warn), `unsafe_code = "deny"` workspace-wide, `cargo test --workspace
--all-features` fully green.
stabilized_by: CI gate

---

Evidence lands in `.claude/evidence/EXP-NNN.evidence.md`. Audit verdicts in
`.claude/evidence/AUDIT-gpt54.md`.
