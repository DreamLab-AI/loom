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

## EXP-012 — F1 verbatim serving mode
category: executable · regression_critical: true
env: LOOM_VERBATIM_MODE (default 0 = current behaviour), LOOM_VERBATIM_THRESHOLD
(default 8.0). When verbatim mode is ON **and** the injection gate engages **and**
the scaffold `top_score` (lexical additive scale; `EXACT_TITLE_WEIGHT`=8.0/word,
`MIN_SEED_SCORE`=2.0 floor, `STRONG_MATCH_SCORE`=8.0) clears the threshold **and**
the request is a delivery-lookup shape (last message user, no assistant turns)
**and** it is not streaming **and** not opted out, `/v1/chat/completions` returns
a valid OpenAI chat completion WITHOUT calling the backend: `model:"loom-verbatim"`,
`object:"chat.completion"`, `choices[0].finish_reason:"stop"`, zero `usage`,
message content = the scaffold's canonical markdown (wrapper stripped, one-line
provenance header naming the generation), and `loom.served_mode:"verbatim"`.
Design decision: a NEW `ServedMode {Delegated|Verbatim}` domain enum carries the
marker rather than a `FusionPath::Verbatim` variant — `FusionPath` is the
RETRIEVAL axis (which index produced candidates) and a verbatim serve is still a
`LexicalHit`; overloading it would conflate retrieval with delivery and perturb
every existing `FusionPath` serialisation/test. Threshold default 8.0 = an exact
single-word title match (conservative: paraphrase/overlap-only hits score below 8
and delegate). Opt-out `{"loom_options":{"verbatim":false}}` is a properly-parsed
request field, stripped before delegation so a strict backend never sees it.
Streaming and multi-turn bypass verbatim (delegate as today). Default OFF preserves
current behaviour EXACTLY (proven by the unchanged EXP-006 suite).
evidenced by: exp012_serving.rs (`f1_*`) — serves-without-backend (200 vs 503 the
retrieval-only backend would give on delegate), opt-out/multi-turn/streaming
bypass, threshold boundary (below→delegate, above→verbatim), default-off delegates;
serving.rs unit tests (shape, wrapper strip, eligibility).
stabilized_by: loom-facade exp012 + serving unit tests

## EXP-013 — F2 exposure telemetry
category: executable · regression_critical: true
env: LOOM_EXPOSURE_APPEND (default 0). Whenever a scaffold was injected, the 200
response's `loom` block carries `exposure:{targets:N, delivered:M, dropped:[...]}`
(dropped capped at 12) — the count of served titles (class titles + serialised
relation-target titles that survived the budget clamp), how many the answer
restated, and the served-but-omitted ones. The matcher is a pure port of the
paper's deterministic `normalise`/`gold_hit` (tools/paper/decompose_exposure.py):
normalise = lowercase + collapse non-`[a-z0-9\s]` to spaces + trim; hit = substring
OR ≥80% of the title's length-≥4 words present. Semantic parity (not byte parity)
is the bar. Served titles are the seeds' resolved titles filtered to those present
in the served block (same matcher) — "what the model saw", without markdown
parsing. O(targets × answer). With LOOM_EXPOSURE_APPEND=1 a single
`Not covered above: X, Y, Z.` line is appended to the answer content on drops;
default off = telemetry only, zero content change. Not engaged ⇒ `exposure:null`.
evidenced by: loom-scaffold exposure.rs unit tests (normalise/title_hit/report
dedupe+cap fixtures); exp012_serving.rs (`f2_*`) — drops reported against a
wiremock answer, append line present under the flag, null when not engaged.
stabilized_by: loom-scaffold exposure tests + loom-facade exp012

## EXP-014 — F3 thinking + budget control
category: executable · regression_critical: true
env: LOOM_BACKEND_NO_THINK (default 0), LOOM_THINK_TOKEN_FLOOR (default 0 = OFF;
Profile A sets 1536). Audit remediation (finding 1): the code default is 0, NOT
1536 — with F3 unconfigured the backend's LOOM_MIN_MAX_TOKENS remains the sole
token floor, so a deployment that set LOOM_MIN_MAX_TOKENS=0 is never silently
re-floored (defaults preserve current behaviour EXACTLY). For an ENGAGED
(scaffold-injected) delegation ONLY: with NO_THINK on and
the client NOT having set `chat_template_kwargs`, add
`chat_template_kwargs:{"enable_thinking":false}` to the delegated body; NEVER add it
to a non-engaged passthrough request. When thinking stays active (NO_THINK off, or
the client overrode `chat_template_kwargs`) and a think-floor > 0 is set, raise a
sub-floor INTEGER `max_tokens` the client sent up to the floor — reusing the backend
adapter's audited integer-only floor primitive (`raise_integer_token_floor`,
single-sourced from the LOOM_MIN_MAX_TOKENS remediation): only serde integers are
raised (negatives too), higher asks / strings / floats / overflow / null pass
through, no key is inserted. Defaults OFF preserve current behaviour.
evidenced by: loom-backend-openai backend.rs (`raise_integer_token_floor_semantics`);
serving.rs unit tests (`thinking_controls_*`); exp012_serving.rs (`f3_*`) — kwargs
injected when engaged, NEVER on passthrough, client-override keeps thinking + floors,
floor not applied to passthrough.
stabilized_by: loom-backend-openai + loom-facade serving/exp012 tests

---

Evidence lands in `.claude/evidence/EXP-NNN.evidence.md`. Audit verdicts in
`.claude/evidence/AUDIT-gpt54.md`.
