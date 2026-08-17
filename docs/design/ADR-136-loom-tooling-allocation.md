# ADR-136 — Loom tooling allocation: RuVector behind the markdown, pyoxigraph stays, gate re-platformed, mesh deferred

**Status:** Proposed (direct-to-target; extends and partially supersedes keystone ADR-135)
**Date:** 2026-08-16
**Decision-type:** Architecture (tooling allocation — resolves the ADR-135 open reasoner decision (D3-a), and takes new ADR-136 positions on retrieval acceleration and gate attestation)
**Deciders:** Dr John O'Hare (operator)
**Extends:** ADR-135 (Ontology Loom node — keystone). **Node boundary and corpus-lifecycle ownership are unchanged.**
**Supersedes:** nothing wholesale. **New ADR-136 position** (not a §5 supersession): the bespoke retrieval stack is a temporary lead, not a capability to *defend*. ADR-135 §5 is *Consequences* and posts no retrieval-as-moat posture, so this is an original stance ADR-136 takes, not a reversal of a stated ADR-135 decision. **Resolves** the one ADR-135 open decision this ADR touches — **D3-a** (canonical reasoner) → Whelk-rs at build time. Retrieval acceleration (→ RuVector HNSW strictly behind the markdown) and gate attestation (→ RuVector ProofGate/MutationLedger) are **new ADR-136 positions**, not pre-existing ADR-135 open decisions.
**Relates (this repo, loom):** `app/ontology_scaffold.py`, `app/loom_graph.py`, `app/pipeline/{gate,conflicts,reason,validate,build}.py`, `app/data/{ontology.ttl,ontology-inferred.ttl,scaffold-index.json,prose-index.json}`, `PRD-025`, `PRD-026`, `ddd-ontology-loom-context.md`, `LOOM-POSITIONING.md`.
**Relates (RuVector):** **RuVector ADR-001** (HNSW, production), **RuVector ADR-047** (ProofGate<T> / MutationLedger / HashChainGate). Also (descriptions only — ADR numbers not verified against source, cite by capability not number): RuVector's mincut / pi-shared-web-memory work; RuVector's gnn-rerank nightly rerank branch.
**Relates (ruflo):** **ruflo ADR-344** (kg index / hybridRetrieve — *Proposed, flag-off, unbenchmarked*). Also (description only — ADR number not verified against source): ruflo's graph-intelligence integration work.
**Relates (metaharness):** `metaharness/docs/research/ruvector-applications/GRAPH-ANALYTICS-PROOF.md` (@ruvector/graph-node v2.0.4 audit).
**Relates (VisionClaw):** **VisionClaw ADR-099** (Whelk-rs EL primary reasoner), **VisionClaw ADR-050 (pod-backed-kgnode)** — repo-qualified to disambiguate from **agentbox ADR-050 (decision-elevation)**.

> This ADR does not re-derive the design brief (PRD-025 owns product goal; PRD-026 owns requirements and the build order). It records **which tool owns which capability**, with the evidence for each call, and it holds every allocation subordinate to the invariant below.

---

## THE PRIZE (non-negotiable driver — quoted verbatim, governs every decision here)

> The one canonical, load-bearing artifact of this system is the per-IRI human-scrutible unit: one block of curated research prose (dfull, corpusNature: synthetic-ai-generated-human-directed) headed by its typed ontology relations (subClassOf, requires/enables/implements/uses/relatedTo/contrastsWith), that a human can read, review and audit end-to-end at single-entity granularity. Everything else — HNSW vectors, the lexical inverted index, pyoxigraph SPARQL, mincut, GNN, ProofGate ledgers — is an accelerator that indexes, finds, ranks and attests THAT unit. None of them ever becomes the thing served in its place, and none ever becomes the thing a human must trust instead of the markdown. We explicitly reject the GraphRAG/G-Retriever trajectory where knowledge degrades into opaque LLM community summaries or GNN-encoded subgraphs; our attribution granularity — one reviewable markdown per IRI, behind a propose→consistency-check→human-PR-merge gate — is the real, non-eroding moat.

Any design in this document that reduces the legibility of that unit is a **regression to be justified with a documented answer-quality trade**, not a default to be accepted.

---

## 1. Context

ADR-135 stood the Ontology Loom up as a first-class node: stable model-swappable façade, single corpus-lifecycle authority, deferred distillation. It left three decisions open (its D1-a, D3-a, and the implicit "is our retrieval stack a moat?") and, in §5, it read the Loom's bespoke lexical+traversal retrieval as a capability to be consolidated *into* rather than *behind* the ecosystem.

A six-facet Opus/Sonnet research mesh has since ground the ecosystem claims against the ecosystem's own source and audits. Its findings (treated here as ground truth) sharpen the picture in four ways that force a tooling-allocation decision ADR-135 deferred:

1. **The Loom is not losing on retrieval today — but it should not be *defended* on retrieval.** The lexical title-matcher (inverted index over 8,146 class titles, `<50ms` self-tested) plus pyoxigraph native-Rust SPARQL over the reasoned closure is small, fast, and currently **ahead** of the shipped ecosystem equivalent. `@ruvector/graph-node` v2.0.4 "Cypher" is a label-scoped node scan — every relationship pattern, `WHERE`, variable-length path, and aggregation returns empty (`GRAPH-ANALYTICS-PROOF.md`). `ruvector-bounded-rag` mincut is a research baseline measured at **1.27s vs TopK's 302µs at n=3000** (~4000× slower). RuVector's `gnn-rerank` (nightly rerank branch) is branch-only and solves ANN-quantisation recall loss — a problem the Loom does not have because it runs zero embeddings. ruflo ADR-344's `hybridRetrieve()` is **Proposed, feature-flag-off (`CLAUDE_FLOW_KG_ENABLED` default off), gated on an unrun benchmark.** So "the ecosystem subsumes our retrieval" is false *today* — but our lead is a temporary artifact of unfinished ecosystem capability, not a durable moat. We do not build a defensive strategy on it.

2. **There is exactly one real retrieval gap:** semantic fallback for out-of-vocabulary / paraphrase queries the lexical matcher structurally misses (today: silent no-injection below `MIN_INJECT_SCORE`). `@ruvector/core` HNSW (RuVector ADR-001, genuinely production, sub-ms at scale) fills it — *if* added as a benchmark-gated third signal, not a wholesale replacement.

3. **The sharpest, least-defensible waste is a build-pipeline problem, not a retrieval-algorithm problem:** three redundant materialisations of the same graph — `ontology.ttl` (13M) + `scaffold-index.json` (7.9M) + `prose-index.json` (4.3M) ≈ 25M. This is the mechanism behind the 8152-vs-5975 class-count drift. No RuVector migration collapses it; an HNSW index would just become a *fourth* copy unless the derivation is restructured to one source.

4. **The genuine non-eroding advantage is write-time admission control + domain-semantic authority**, categorically different from read-time retrieval — no retrieval sophistication ever blocks a bad write. But the honest status is modest: the gate (`pipeline/gate.py`, `conflicts.py`) is a tested, importable library **not wired into any CI** (no `.github/workflows/` exists in the repo); Whelk EL++ **does not run inside the Loom** (`loom_graph.py` serves a pre-reasoned snapshot); the corpus has **zero `owl:disjointWith` axioms**, so a DL reasoner has structurally nothing to catch beyond what `conflicts.py` already does; and RuVector ADR-047 ProofGate<T>/MutationLedger is a *more general, cryptographically attested* version of the same gate pattern.

The reframe this ADR commits to: **the Loom is a thin governed façade whose durable job is (1) serving the canonical per-IRI markdown-with-ontology unit, (2) write-time admission control with domain-semantic authority, and (3) a confidence-gated injection policy — over a corpus that RuVector indexes and accelerates strictly behind the markdown.** Not a retrieval engine. Not (yet) a coordination hub.

### Shipped-vs-aspirational honesty table (mandatory; identical across ADR-136 / PRD-026 / DDD)

No author of the three docs may contradict this table on what is live today.

| Capability | Status today | This ADR's target |
|---|---|---|
| Per-IRI markdown-with-ontology unit as served canonical | **Shipped** | keep (the Prize) |
| `corpusNature` honesty metadata in every response | **Shipped** | keep |
| Confidence-gated selective-injection policy (`STRONG_MATCH_SCORE`, `MIN_INJECT_FRACTION`, skip `< MIN_INJECT_SCORE`) | **Shipped** | keep |
| Model-swappable `/v1` façade (Qwen3.8-27B behind it today) | **Shipped** | keep |
| Lexical title-matcher (`<50ms`, 8,146 titles) | **Shipped** | keep as first-tier signal |
| pyoxigraph SPARQL over reasoned closure | **Shipped** | keep |
| HNSW semantic fallback for OOV/paraphrase | **Not built** (silent no-injection) | add, benchmark-gated (RuVector ADR-001) |
| Single-source build (one source → ttl+scaffold+prose+HNSW) | **Not built** (3 parallel copies) | add |
| Admission-control domain predicates (`conflicts.py`) | **Real + tested, NOT wired to CI** | keep + enforce (PRD-026) |
| Gate attestation / tamper-evident ledger | **Not built** (unattested Python `CheckResult`) | move to RuVector ProofGate/MutationLedger (ADR-047) |
| EL++ closure authority | **Split / drifting** (VisionClaw Whelk vs `reason.py` BFS) | resolve to Whelk-rs at build time |
| Whelk EL++ at Loom **query** time | **Not run** (pre-reasoned snapshot) | remains build-time only |
| `owl:disjointWith` contradiction catching | **Nothing to catch** (0 disjointness axioms) | honest caveat, not a claim |
| `@ruvector/graph-node` Cypher as graph engine | Candidate | **rejected** (label-scan-only) |
| ruvector-hybrid / mincut / gnn-rerank fusion | Nightly PoC, slower than baseline | **deferred** (adopt only if it ships AND beats bench) |
| Multi-agent coordination substrate | **Aspirational** (only live consumer = email gateway single-LLM RAG) | **deferred phase** (WS-Q class) |

---

## 2. Decision

The Loom is the **human-scrutible governed façade + domain-semantic authority**. RuVector is the **retrieval and attestation accelerator, strictly behind the markdown**. pyoxigraph stays as the in-context SPARQL engine. Whelk-rs becomes the single build-time reasoning authority. The multi-agent mesh is a named, deferred phase. Decisions **D1–D8** below; each carries its rejected alternative and its Prize impact.

### D1 — Canonical unit is the per-IRI markdown-with-ontology block; every engine sits behind it

The served, canonical, load-bearing unit is the per-IRI block of curated `dfull` prose headed by its typed ontology relations, carrying `corpusNature: synthetic-ai-generated-human-directed`. This is the aggregate root. The lexical index, HNSW, pyoxigraph SPARQL, mincut, GNN, and ProofGate ledgers are **projections and adapters** that index, find, rank, or attest that unit and resolve back to its IRI. None is ever returned in its place; none becomes the thing a human trusts instead of the markdown.

- **Rejected alternative — treat a derived representation as canonical** (GraphRAG community summaries; G-Retriever's GNN-encoded soft-prompt subgraph; a RuVector namespace row as the source of record). Rejected because it collapses attribution granularity from one-reviewable-markdown-per-IRI to lossy, inspection-resistant aggregates (mesh Facet 4; `arXiv:2404.16130`, `arXiv:2402.07630`). That is the moat we are protecting, not a cost we accept.
- **Prize impact:** this *is* the invariant. Every other decision is justified only insofar as it serves this row.

### D2 — Do NOT install `@ruvector/graph-node`; keep pyoxigraph for SPARQL over the reasoned closure

pyoxigraph is a native-Rust indexed SPARQL store executing full pattern matching over the Whelk-reasoned EL++ closure. `@ruvector/graph-node` v2.0.4 "Cypher" executes label-scoped node scans only — every relationship pattern, `WHERE`, path, and aggregation returns empty (`GRAPH-ANALYTICS-PROOF.md`). `loom_graph.py` does not hand-roll BFS; it validates/clamps then delegates to pyoxigraph.

- **Rejected alternative — swap pyoxigraph for graph-node Cypher.** Rejected as **a regression dressed as an upgrade**: strictly weaker query capability, and it would break every relationship/aggregation query the Loom relies on. graph-node's own embeddings are 8-dim character-hash stubs (per ruflo's graph-intelligence integration notes); its k-hop neighbours are opaque string IDs with no relevance scores.
- **Prize impact:** *help.* SPARQL returns IRIs that address the exact same canonical markdown identity; keeping the working engine protects that resolution path.

### D3 — ADD `@ruvector/core` HNSW as a benchmark-gated semantic fallback ONLY; never default-on unbenchmarked

Add HNSW (RuVector ADR-001) as a **third** retrieval signal alongside lexical + precomputed-graph, engaged for the one real gap: OOV/paraphrase queries below `MIN_INJECT_SCORE` where the lexical matcher today injects nothing. It **must** beat the lexical baseline on the multivariate bench (in-domain recall AND general-question non-jaggedness, per `LOOM-POSITIONING.md` axes 1–2) before it becomes a default.

The standing counter-example that makes this a hard gate, not a formality: **our own naive over-retrieval (1-hop preload) result was Δ = −0.40 [−0.58, −0.22], n=285, across 5 models, worst on the weakest model (haiku −1.30)** — a documented lost-in-the-middle / irrelevant-skew degradation (replicated; commit `9fe57c5`). Naive fusion of a weak signal against a strong one *underperforms the strong signal alone* (mirrored in the ecosystem's own RRF history). Budget and gating matter more than raw recall.

- **Rejected alternative — adopt ruvector-hybrid / mincut-bounded-rag / gnn-rerank now.** Rejected: unshipped nightly PoCs, not wired into `ruvector-server`, measurably slower than naive top-k at Loom scale (mincut ~4000× at n=3000), and gnn-rerank solves an ANN-quantisation problem the Loom's zero-embedding architecture does not have. Revisit only if/when they ship AND beat the lexical+graph+HNSW fusion on our bench (deferred; see D8).
- **Rejected alternative — turn HNSW fusion on by default because "more recall is better."** Rejected by the −0.40 result: more context is not more answer quality when it is off-topic. The existing confidence-gated injection policy (which caps budget and skips below threshold) is the safety rail; HNSW plugs into it as a candidate source, not as a bypass.
- **Prize impact:** *neutral-if-gated.* Vectors rank and find markdown; they never replace it. The threat exists only if fusion is turned on unbenchmarked — foreclosed by the benchmark-gate and the injection policy.

### D4 — Collapse the three graph materialisations to ONE single-source-of-truth build; any index derives from it

`ontology.ttl` + `scaffold-index.json` + `prose-index.json` (~25M) are three copies of one graph and the mechanism behind the 8152-vs-5975 drift. Restructure the build so **one source** derives ttl + scaffold-index + prose-index + the HNSW artifact. Every index becomes a *projection* of the canonical corpus, stamped with the generation `buildId` (ADR-135 D2.1). **Re-embed on promote** — delta-diff touched IRIs against the prior generation and re-embed only those — not full-corpus re-embed per build (respects this project's HNSW index-law: non-concurrent rebuild, m=16, ef_construction=128; never `CREATE INDEX CONCURRENTLY` on the ruvector HNSW AM).

- **Rejected alternative — add HNSW as its own artifact alongside the existing three.** Rejected: that makes a *fourth* independent copy and *widens* the drift surface the −0.40-adjacent 8152-vs-5975 bug already demonstrates. **No index is a new copy** is an invariant, not a preference.
- **Rejected alternative — leave the three copies and "just be careful."** Rejected: the drift already happened once in production; carefulness is not a control.
- **Prize impact:** *help.* One source means the human always reviews one authoritative copy; every accelerator is provably a projection of it, not a separate artifact demanding separate trust.

### D5 — Re-platform gate MECHANICS onto RuVector ProofGate/MutationLedger; keep domain PREDICATES local

`conflicts.py` predicates (subclass-acyclicity, duplicate-label, type-match, relation-contradiction) are domain-semantic — *what a contradiction means for this ontology* — and stay Loom-owned. Their **attestation mechanics** re-platform onto RuVector ADR-047: the predicates become `ProofRequirement::InvariantPreserved` obligations routed through `ProofGate<T>`, and the current unattested Python `CheckResult` is replaced by a tamper-evident `MutationLedger` entry (FNV-1a/BLAKE3 hash-chain, HashChainGate/MerkleGate write receipts). ADR-047's types are domain-agnostic, so this is a straight upgrade with **zero domain-knowledge cost**. Enforcement (wiring the gate into CI — it is a library today, not a control) is a PRD-026 acceptance gate.

- **Rejected alternative — keep the bespoke Python gate as-is.** Rejected: it has no attestation, no tamper-evidence, no proof composition — a thinner reimplementation of a pattern the ecosystem ships better. And it is **not enforced by any CI**, so an agent can write straight past it.
- **Rejected alternative — move the domain predicates into RuVector too.** Rejected: RuVector's proof-gate has (and should have) zero opinion about ontology vocabulary. What a "duplicate concept" or "contradictory relation pair" means for *this* corpus is the Loom's authority and must not leak into a domain-agnostic substrate.
- **Prize impact:** *help.* A tamper-evident gate blocks a bad write before it reaches the canonical corpus, protecting exactly what the human reviews. Attestation records *that the gate ran*; the markdown remains the human-facing artifact.

### D6 — Resolve the canonical-reasoner decision to Whelk-rs at build time; retire the duplicate BFS closure

Resolve ADR-135's open reasoner decision (its **D3-a**): **VisionClaw Whelk-rs EL++ is the single closure authority, run once per generation at build/CI time** (VisionClaw ADR-099 posture). This is **Option 1 (ADR-135) = option (a) (PRD-025)** — Whelk-rs as authority — and it **explicitly overrides ADR-135 D3-a's own recommendation of Option 2** (make `reason.py` the authority, VisionClaw imports it), while **aligning with PRD-025 OD-1's default option (a)**. The override is deliberate: Option 2 loses Whelk's EL++ completeness and would keep two divergent closure implementations alive as peers. `pipeline/reason.py`'s cycle-safe BFS transitive closure is retired to a **conformance oracle** under the ADR-135 D3.1 gate (both engines over a fixed fixture; set-equality over `(s,p,o)` after IRI canonicalisation; divergence fails the build; class-count parity is a first-class assertion — this closes the 8152-vs-5975 drift class). Whelk-rs remains **build-time only**; the Loom continues to serve the pre-reasoned snapshot.

Honest caveat, recorded so no downstream doc over-claims: the corpus has **zero `owl:disjointWith` (and zero `owl:equivalentClass`/`intersectionOf`/`unionOf`) axioms**. Whelk is authority for **closure and subsumption**, not a contradiction-catcher we can currently exercise — a DL reasoner has structurally nothing to catch here beyond what non-DL `conflicts.py` already does. If disjointness axioms are ever authored, that changes; until then, "the DL reasoner catches contradictions" is not written as a shipped property.

- **Rejected alternative — make `reason.py` the authority, VisionClaw imports it (ADR-135 D3-a's own recommended Option 2).** Rejected as the *canonical* choice, **overriding ADR-135's D3-a recommendation**: it loses Whelk's EL++ completeness and keeps two divergent closure implementations alive as peers. We keep `reason.py` only as the conformance oracle, not as an authority.
- **Rejected alternative — run Whelk EL++ inside the Loom at query time.** Rejected: it pulls a Rust reasoner into the stdlib-portable lifecycle façade (breaking Deployment B's GPU-free portability) for no query-time gain, since the Loom serves a static reasoned snapshot. Query-time DL reasoning is **out of scope** (non-goal).
- **Prize impact:** *help.* One authoritative closure means one reasoned ontology block per IRI for the human to read — no reasoner drift between what was reviewed and what is served.

### D7 — Defer the multi-agent coordination substrate to a named later phase

The "shared substrate for a mesh" framing is aspirational. The DDD doc §3 states it is not implemented as of 2026-08-11; the only confirmed live consumer is the **email gateway doing single-LLM RAG** (`REASONER_BASE_URL=http://loom:8080/v1`), i.e. single-consumer RAG-for-QA, not a blackboard multiple agents read and write. Scope it as an explicit deferred phase (WS-Q class in PRD-026), **not** as a shipped property. When built, it must still resolve every claim to the same per-IRI markdown identity.

- **Rejected alternative — describe the mesh as a current capability to justify the node.** Rejected: it is over-claiming, and it is the weakest plank in the whole positioning (mesh Facet 3). The node is justified today by corpus-lifecycle ownership, admission control, and the model-swap façade — not by a coordination hub that does not exist.
- **Prize impact:** *neutral.* Deferring avoids over-claiming; the canonical unit is unaffected either way.

### D8 — Adopt ruvector-hybrid / mincut / gnn-rerank ONLY if it ships AND beats the benchmark

Hold the door open, gated. These are revisited only after the D3 HNSW fallback lands, only if they ship into `ruvector-server` (not nightly branches), and only if they beat the lexical+graph+HNSW fusion on our multivariate bench. The −0.40 result and the mincut 4000×-slower measurement are the standing reasons this is a *later, evidence-bound* decision, not a now one.

- **Rejected alternative — adopt on the ecosystem's marketing ("unified neuro-symbolic retrieval substrate").** Rejected: marketing is not a benchmark. ruflo ADR-344 itself gates `hybridRetrieve()` on an unrun benchmark; we hold the same bar.
- **Prize impact:** *neutral-if-gated.* Would rank markdown, not replace it; kept safe by the benchmark-first invariant.

---

## 3. How this reconciles with ADR-135 and ADR-112

ADR-135's node boundary, corpus-lifecycle ownership, generation/manifest discipline, deferred distillation, and security posture are **unchanged**. This ADR only fills the tools inside that boundary: it resolves D3-a to Whelk-rs, names RuVector HNSW as the retrieval accelerator, and names RuVector ProofGate as the gate's attestation substrate. ADR-112's "one brain / no hot-path LLM" holds: the hot path reads the published generation and its derived indices (lexical + pyoxigraph today; **HNSW as the D3 target/planned third signal, not yet built**), all LLM-free; only `/loom/v1/distill` touches a model, deferred and off-turn. HNSW is an index read, not a model call — it stays on the LLM-free hot path. To keep the hot path **network-free** consistently with the DDD §6.1 RuVector-access model, the planned HNSW read is an **in-process `@ruvector/core` projection** — the index embedded in-process alongside the published generation, not a network round-trip to `ruvector-postgres` over MCP. (The `ruvector-postgres`/MCP path is a build-time / off-turn write channel, never the query hot path.)

---

## 4. Consequences

### Positive
- **The Loom's durable position is now defensible.** It is committed to as a governed façade + admission control + domain-semantic authority — not defended on a retrieval lead the ecosystem is actively closing. **Prize impact: help** — the position is *about* keeping the canonical unit primary.
- **One real gap closes without a fourth copy.** HNSW plugs the OOV/paraphrase hole, derived from the single source (D4). **Prize impact: neutral-if-gated.**
- **Drift class closed at the root.** SSOT build + Whelk-rs conformance gate kill the 8152-vs-5975 divergence. **Prize impact: help** — the human reviews one authoritative copy.
- **The gate becomes tamper-evident and (via PRD-026) enforced.** ProofGate/MutationLedger replaces unattested Python; predicates stay local. **Prize impact: help** — bad writes are blocked before the corpus.
- **pyoxigraph's genuine capability is preserved**, not traded down for label-scan Cypher. **Prize impact: help.**

### What breaks (deliberately)
- **`pipeline/reason.py` is demoted** from a peer authority to a conformance oracle; any code treating its `ontology-inferred.ttl` as independently canonical must re-point to the Whelk-derived generation artifact.
- **The three build artifacts stop being independently authored.** Any tooling that reads `scaffold-index.json` / `prose-index.json` as standalone sources must read them as generation-stamped projections of the one source.
- **The bespoke Python `CheckResult` typing is retired** in favour of `ProofRequirement`/`MutationLedger`; callers of `gate.py`'s verdict type change.

### Negative / honest caveats
- **HNSW adds an embedding artifact and a re-embed-on-promote step** to the build. Mitigated by delta-diffing and the SSOT derivation; still net-new machinery.
- **The DL-reasoner story has nothing to catch yet** (zero disjointness axioms). Whelk is authority for closure/subsumption only; we do not claim contradiction-catching until disjointness is authored. **Prize impact: neutral** — legibility of the served unit is unaffected.
- **The gate is not enforced until PRD-026 wires it into CI.** Today it remains an importable library; this ADR decides the *substrate*, PRD-026 owns the *enforcement*.

### Neutral
- The coordination substrate remains deferred (D7); the node is justified without it.
- The confidence-gated injection policy is untouched by any of this — it operates on *which* markdown blocks inject, never on the blocks themselves.

---

## 5. Alternatives considered (whole-ADR level)

### A1 — Defend the Loom as a retrieval engine (ADR-135 §5 posture, unchanged)
Keep positioning the bespoke lexical+traversal stack as the moat. **Rejected:** our lead is real *today* but is a temporary artifact of unfinished ecosystem capability (graph-node Cypher immature, ADR-344 unshipped). The moment either matures, a retrieval-quality defence collapses. The durable position is governance + admission control + domain authority, which no RuVector primitive addresses.

### A2 — Retire the Loom's retrieval wholesale onto ruvector-hybrid now
The naive "just use the shipped hybrid substrate" instinct. **Rejected:** ruvector-hybrid is not shipped (nightly PoCs, not in `ruvector-server`), is slower than baseline at our scale (mincut 4000× at n=3000), and graph-node Cypher is strictly weaker than the pyoxigraph SPARQL already running. Retiring working, faster code for slower unshipped code is a regression.

### A3 — Adopt HNSW fusion default-on for maximum recall
**Rejected by our own evidence:** the −0.40 over-retrieval result (n=285, 5 models, haiku −1.30) shows naive fusion *underperforms* the strong signal alone — lost-in-the-middle / irrelevant-skew. HNSW is added as a benchmark-gated candidate source under the existing injection policy, not as a default.

### A4 — Move the domain predicates onto RuVector along with the mechanics
**Rejected:** RuVector's proof-gate is, and should be, domain-agnostic. Ontology-specific meaning (contradictory relation pairs, OWL duplicate semantics) is the Loom's authority; leaking it into a generic substrate erases the one non-eroding advantage.

### A5 — Ship the multi-agent mesh framing as current
**Rejected:** aspirational per the DDD's own §3; the only live consumer is single-LLM RAG. Named as a deferred phase instead (D7).

### A6 — Run Whelk EL++ at Loom query time for "live" reasoning
**Rejected:** breaks Deployment B's stdlib-portable, GPU-free façade for no gain over the served pre-reasoned snapshot. Reasoning stays build-time (D6).

---

## 6. Verification

| Decision | Verification |
|---|---|
| D1 canonical unit | A served answer always resolves to a retrievable per-IRI markdown block with its ontology relations + `corpusNature`; no engine output is served in its place |
| D2 keep pyoxigraph | A relationship-pattern + aggregation SPARQL query returns non-empty against the Loom; the same query shape against `@ruvector/graph-node` returns empty (audit reproduced) |
| D3 HNSW gated | HNSW fusion is default-off until a multivariate bench run shows it beats the lexical baseline on axis-1 recall AND axis-2 non-jaggedness; the −0.40 fixture is a regression guard |
| D4 SSOT | One build source emits ttl + scaffold-index + prose-index + HNSW, all stamped with the same `buildId`; class-count parity asserted; no standalone fourth copy exists |
| D5 gate mechanics | `conflicts.py` predicates run as `ProofRequirement::InvariantPreserved`; a gate run writes a chained `MutationLedger` entry; a tampered entry fails verification |
| D6 reasoner | ADR-135 D3.1 conformance gate: Whelk-rs vs `reason.py` set-equality over the fixture; seeded divergence fails the build with a triple diff; Whelk does not run on the Loom query path |
| D7 mesh deferred | No doc describes the coordination substrate as live; the email gateway remains the single confirmed consumer |
| D8 hybrid gated | ruvector-hybrid/mincut/gnn-rerank remain unadopted until shipped into `ruvector-server` AND beating the fusion bench |

---

## 7. Cross-reference discipline

ADR-136 is the decision-of-record for tooling allocation; PRD-026 cites it for every allocation call and owns requirements, the WS build order, and the multivariate acceptance gates (carrying the **benchmark-first** and **SSOT-single-copy** invariants as gates). The DDD (`ddd-ontology-loom-context.md`, revised in place) names the per-IRI markdown unit as the aggregate root, draws the accelerator boundary (RuVector = downstream index/attestation adapter; pyoxigraph = in-context SPARQL; Whelk-rs = upstream build-time reasoner), and marks the mesh-coordination context as a deferred region. All three carry the identical shipped-vs-aspirational honesty table (§1) and repo-qualify every cross-repo citation (two ADR-050s — **VisionClaw pod-backed-kgnode** vs **agentbox decision-elevation**; two PRD-022s — **VisionClaw semantic-trust-layer** vs **agentbox semantic-integrity**) per PRD-025's citation discipline. THE PRIZE statement is quoted verbatim at the head of each doc as the non-negotiable driver.