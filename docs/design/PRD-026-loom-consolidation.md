---
id: PRD-026
title: "Loom Consolidation: single-source build, semantic fallback, and enforced admission control — RuVector strictly behind the human-scrutible markdown"
status: proposed
date: 2026-08-16
authors: VisionFlow operator (did:nostr:jjohare) + opus consolidation mesh
linked_adrs: [
  ADR-136 (loom tooling allocation — the decision record this PRD operationalises),
  ADR-135 (loom node boundary + façade + deferred distillation),
  RuVector ADR-004 (HNSW production index),
  RuVector ADR-047 (ProofGate<T> / MutationLedger — proof-gated mutation),
  ruflo ADR-344 (KG index for ReasoningBank — Proposed, feature-flag-off, deferred),
  VisionClaw ADR-099 (Whelk-rs EL++ reasoner authority)
]
linked_prd: PRD-025 (Ontology Loom & Connector Platform — the capstone this extends)
linked_ddd: docs/design/ddd-ontology-loom-context.md
relates: [
  PRD-020 (pervasive ontology augmentation),
  VisionClaw PRD-022 (semantic-trust-layer),
  ADR-112 (retrieval spine / one brain),
  ADR-116 (tiered token budgets),
  ADR-119 (verifiable liveness telemetry),
  ADR-121 (self-improving writeback loop),
  agentbox ADR-013 (canonical URI grammar),
  agentbox ADR-049 (bitemporal facts / runtime provenance),
  agentbox ADR-050 (decision-elevation inverse-corpus-path),
  agentbox ADR-051 (harness-side loom-client decisions — proposed),
  agentbox PRD-022 (semantic-integrity-provenance-decisions)
]
supersedes: "Extends PRD-025 (does not replace it). Supersedes PRD-025's implicit assumption that the three parallel index materialisations and the bespoke retrieval stack are the end-state; resolves the reasoner tooling allocation PRD-025 left to ADR-135 (D3-a), and introduces the retrieval-acceleration and gate-attestation allocations as new ADR-136 decisions, not resolutions of prior ADR-135 open decisions."
---

# PRD-026 — Loom Consolidation

**Status:** Proposed (grounded in the 2026-08 consolidation mesh; operationalises ADR-136)
**Date:** 2026-08-16
**Owner:** VisionFlow operator (Dr J. O'Hare, did:nostr:jjohare)
**Decision authority:** [ADR-136](./ADR-136-loom-tooling-allocation.md) — this PRD cites ADR-136 for every allocation decision and does not re-derive them.
**Builds on:** [PRD-025](./PRD-025-ontology-loom-and-connector-platform.md) (the capstone loop) — PRD-026 operationalises the *consolidation*, it does not restate PRD-025's product goal.

---

> ## THE PRIZE (the non-negotiable driver — quoted verbatim, do not edit)
>
> The one canonical, load-bearing artifact of this system is the per-IRI human-scrutible unit:
> one block of curated research prose (`dfull`, `corpusNature: synthetic-ai-generated-human-directed`)
> headed by its typed ontology relations (`subClassOf`, `requires`/`enables`/`implements`/`uses`/
> `relatedTo`/`contrastsWith`), that a human can read, review and audit end-to-end at single-entity
> granularity. Everything else — HNSW vectors, the lexical inverted index, pyoxigraph SPARQL,
> mincut, GNN, ProofGate ledgers — is an accelerator that indexes, finds, ranks and attests THAT
> unit. None of them ever becomes the thing served in its place, and none ever becomes the thing a
> human must trust instead of the markdown. We explicitly reject the GraphRAG/G-Retriever trajectory
> where knowledge degrades into opaque LLM community summaries or GNN-encoded subgraphs; our
> attribution granularity — one reviewable markdown per IRI, behind a
> propose→consistency-check→human-PR-merge gate — is the real, non-eroding moat. Every decision in
> this document is subordinate to keeping this unit primary and legible; any design that reduces its
> legibility is a REGRESSION to be justified with a documented answer-quality trade, not a default.

---

> **EXECUTION NOTE (read first).** This is a **design + workstream plan**, not a code change. It
> changes no Loom / RuVector / VisionClaw / agentbox code by itself; implementation is the WS-K…WS-Q
> build order in §9, each gated by the evidence bars in §7. This is a **dev/test estate**, so we
> build the target end-state directly rather than staging a live migration. Every requirement below
> is testable, and every capability is tagged **shipped** or **aspirational** — the §3 honesty table
> is the single source of truth for that split and no other section may contradict it.

> **Citation discipline (mandatory — inherited from PRD-025).** Two `PRD-022`s and two `ADR-050`s
> exist across repos. Every cross-repo citation is repo-qualified: **VisionClaw PRD-022** =
> *semantic-trust-layer*; **agentbox PRD-022** = *semantic-integrity-provenance-decisions*.
> **VisionClaw ADR-050** = *pod-backed-kgnode-schema*; **agentbox ADR-050** =
> *decision-elevation-inverse-corpus-path*. RuVector and ruflo ADRs are always prefixed with their
> repo (**RuVector ADR-004/ADR-047**, **ruflo ADR-344**). An unqualified `PRD-022`, `ADR-050`, or a
> bare `ADR-004/047/344` in this document is a defect.

---

## 1. Problem & vision

### 1.1 The vision, stated as the promise the product must keep

VisionFlow is **thin agents over a shared, reasoned ontology** — the architecture the industry
calls neurosymbolic — with the private corpus kept on-LAN for privacy. The Loom is the
load-bearing external-LLM subunit: a model-swappable OpenAI-compatible `/v1` façade with a
confidence-gated `/loom/scaffold` retrieval door that grounds every call in the ontology.

The product promise is **THE PRIZE above**: for any entity in the customer's private corpus, a
human can pull up *one* markdown block — the curated `dfull` prose headed by its typed ontology
relations — and read, review and audit exactly what the system knows and will serve about that
entity. That artifact is the deliverable. It is what makes a grounded answer *attributable* rather
than merely plausible, and it is the property the GraphRAG/G-Retriever line of work structurally
gives up when it collapses knowledge into LLM community summaries or GNN-encoded soft prompts.

PRD-025 built the *serving and distillation* loop around that unit. This PRD closes the three gaps
PRD-025 left open in the layer *beneath* it — the build pipeline, the retrieval signals, and the
write gate — and it does so under one rule: **RuVector and every other index sit strictly behind
the markdown; none is ever the thing served or the thing a human has to trust in its place.**

### 1.2 The three problems this PRD closes

The 2026-08 consolidation mesh (six-facet adversarial audit; full synthesis in the research
record) isolated exactly what is genuinely broken versus what is merely unfashionable. Three
problems survive scrutiny.

**(a) The corpus is materialised three times, and the copies drift.** The same graph is emitted as
`app/data/ontology.ttl` (13.0 MB), `app/data/scaffold-index.json` (7.9 MB) and
`app/data/prose-index.json` (4.3 MB) — roughly 25 MB of parallel derivations, plus
`ontology-inferred.ttl` (1.3 MB). Nothing binds them to one generation identity, so they can and
do diverge (the observed 8,152-vs-5,975 class-count divergence). This is a **build-pipeline /
single-source-of-truth problem, not a retrieval-algorithm problem** — it persists after any
retrieval change unless the derivation is restructured. It is the sharpest, least-defensible waste
in the system. **Any new index (HNSW) that does not derive from the one source becomes a fourth
copy** and makes the problem worse.

**(b) There is one real retrieval gap: out-of-vocabulary / paraphrase queries.** The lexical
title-matcher (`app/ontology_scaffold.py`, inverted index over 8,146 class titles, <50 ms
self-tested) is small, fast, and currently *ahead* of the shipped ecosystem equivalent — but it
has no answer for a query that misses the vocabulary. Today that produces **silent no-injection**
below `MIN_INJECT_SCORE`: the customer asks a paraphrased question, the ontology has the answer,
and the Loom serves nothing. This is the one place a semantic (embedding) signal genuinely adds
recall the lexical matcher cannot reach. It must be added as a **third signal**, and it must
**beat the lexical baseline on the multivariate bench before it is default-on** — our own
over-retrieval result (Δ = −0.40, 95% CI [−0.58, −0.22], n = 285, five models, worst on the
weakest model at −1.30) proves naive fusion can underperform.

**(c) The write gate is enforced — in the canonical builder, not in this serving mirror.**
`gate.py` + `conflicts.py` are real, tested code that detects subclass cycles, duplicate
concepts, type conflicts and relation contradictions — the genuine non-eroding advantage, because
a write-time gate is categorically different from any read-time retrieval trick. **Correction (2026-08-16,
against the canonical `jjohare/logseq` repo):** these live canonically in `logseq/pipeline/`, and
that repo's `publish.yml` runs `pytest pipeline/tests` + `python -m pipeline.validate` **before
deploy**, with `enrich-gate.yml` gating enrichment PRs — so admission control **is** CI-enforced at
build/publish time, where it belongs. The loom is a **serving mirror**: it correctly does not
re-gate, it serves pre-gated artifacts. `app/pipeline/` here is a **stale vendored copy** (missing
`prose_index.py` + `iri_integrity.py`), not the enforcement point — the earlier "no `.github/workflows/`
in *this* repo, therefore unenforced" reasoning wrongly generalised from the mirror to the ecosystem.
The one genuinely-remaining delta is **attestation**: the gate's verdict is an unattested Python
`CheckResult` — no tamper-evidence, no ledger — where RuVector ADR-047 ships a more general,
cryptographically-attested version. Re-platforming onto that is the real (optional) improvement, not
"turning on" a control that is already on.

### 1.3 The commitment (from ADR-136)

The Loom is **not defended as a retrieval engine** — that lead over the ecosystem is a temporary
artifact of unfinished ecosystem capability, not a durable moat. It is committed to as a **thin
governed façade** whose durable job is: (1) serving the canonical per-IRI markdown unit; (2)
write-time admission control with domain-semantic authority; (3) a confidence-gated injection
policy over a corpus that RuVector indexes and accelerates strictly behind the markdown. Retrieval
acceleration re-platforms onto RuVector HNSW; attestation mechanics re-platform onto RuVector
ProofGate; reasoned-closure SPARQL stays on pyoxigraph; EL++ authority moves to Whelk-rs at build
time; the multi-agent mesh is an explicitly deferred phase. ADR-136 is the decision record for all
of that; this PRD turns it into testable requirements. Of these, **only the reasoner authority
resolves a prior ADR-135 open decision (D3-a)**; the HNSW retrieval acceleration and the ProofGate
attestation are **new allocations introduced by ADR-136, not resolutions of any prior ADR-135 open
decision**.

---

## 2. Users & consumers

| Consumer | What they hold | What they need from this PRD | Prize relationship |
|---|---|---|---|
| **Private-corpus customer** (the buyer) | The curated corpus and the answers grounded in it | Accurate, attributable answers on their private domain; the ability for a human on their side to audit any served fact down to the per-IRI markdown | **Direct.** The markdown unit is the thing they audit and trust. |
| **Email gateway** (only live consumer today) | `REASONER_BASE_URL = http://loom:8080/v1` | The semantic fallback so paraphrased mail queries stop returning ungrounded answers; unchanged façade contract | Indirect — grounds a single-LLM RAG answer in the markdown. |
| **Thin agents (the mesh)** — *aspirational* | The `/v1` façade + `ontology-bridge` MCP tools | Eventually: a shared substrate to read grounded context from. **Not built today** (single-consumer RAG only); scoped as a deferred phase (§6, §9 WS-Q). | Must resolve to the same per-IRI markdown when built. |
| **Human reviewer / operator** | The corpus repo + the PR gate | An *enforced* gate that blocks a bad write before it lands, and a legible diff to review | **Direct.** This PRD's whole point is to protect what this person reviews. |
| **Downstream indices** (RuVector HNSW, pyoxigraph, lexical) | Projections of the one source | To be *derivations*, never independent artifacts the human must separately trust | Behind the prize, by construction. |

The load-bearing consumer for the *product* is the private-corpus customer and their human
auditor. Everything RuVector accelerates exists so that customer gets a faster, higher-recall path
to the same markdown their auditor can read. When a design choice trades legibility for recall, it
is that auditor's trust we are spending, and §7's human-scrutability metric is how we measure the
spend.

---

## 3. Shipped-vs-aspirational honesty table (authoritative — no section may contradict it)

This table is mandatory and shared verbatim across ADR-136, PRD-026 and the DDD. If any prose in
this document implies a different status, the table wins and the prose is a defect.

| Capability | Status today (2026-08-16) | Honest caveat |
|---|---|---|
| Per-IRI markdown-with-ontology unit (curated `dfull` + typed relations), served | **SHIPPED** | The canonical unit. This is the prize. |
| `corpusNature` honesty metadata + `grounding.top_score` + generation in the response block | **SHIPPED** | — |
| Confidence-gated selective-injection policy (`STRONG_MATCH_SCORE`, `MIN_INJECT_FRACTION`, `MIN_INJECT_SCORE` skip) | **SHIPPED** | Benchmarked product decision; no RuVector analogue. |
| Model-swappable `/v1` OpenAI-compatible façade | **SHIPPED** | Qwen3.8-27B behind it today; consumers hold the façade. |
| Lexical title-matcher (inverted index, 8,146 titles, <50 ms) | **SHIPPED** | First-tier signal; currently ahead of the shipped ecosystem equivalent. |
| pyoxigraph native-Rust SPARQL over the reasoned closure | **SHIPPED** | More capable than `@ruvector/graph-node` Cypher (label-scan-only). |
| Admission-control domain predicates (`conflicts.py`/`gate.py`) | **SHIPPED + CI-enforced (canonically)** | Enforced in the `jjohare/logseq` builder: `publish.yml` runs `pytest pipeline/tests` + `pipeline.validate` before deploy; `enrich-gate.yml` gates enrichment PRs. This serving mirror serves pre-gated artifacts and does not re-gate; `app/pipeline/` here is a stale vendored copy. Aspirational delta = **attestation** (ProofGate ledger), not enforcement. |
| Semantic/embedding HNSW fallback for OOV/paraphrase queries | **ASPIRATIONAL** | The one real retrieval gap. Must beat the lexical baseline on the multivariate bench before default-on. |
| Single-source-of-truth build (one source → ttl + scaffold + prose + HNSW) | **ASPIRATIONAL** | Today three parallel materialisations drift. |
| ProofGate/MutationLedger attestation of the gate (RuVector ADR-047) | **ASPIRATIONAL** | Re-platform the mechanics; predicates stay in the Loom. |
| Whelk-rs EL++ as canonical build-time reasoner (retire `reason.py` BFS closure) | **ASPIRATIONAL** | Resolves ADR-135 D3-a (reasoner open decision). Caveat: corpus has **zero `owl:disjointWith` axioms** — Whelk is authority for closure/subsumption, **not** a contradiction-catcher we can currently exercise. |
| EL++ reasoning at Loom **query** time | **NOT DONE — and out of scope** | `loom_graph.py` serves a pre-reasoned snapshot; Whelk does not run inside the Loom. |
| Multi-agent coordination substrate (shared blackboard) | **ASPIRATIONAL / DEFERRED** | DDD §9: "not implemented as of 2026-08-11." Only live consumer is the email gateway doing single-LLM RAG. |
| `@ruvector/graph-node` Cypher as a graph engine | **REJECTED — not installed** | Label-scan-only; strictly weaker than pyoxigraph SPARQL. |
| `ruvector-hybrid` / `mincut-bounded-rag` / `gnn-rerank` fusion | **DEFERRED** | Nightly PoCs, not in `ruvector-server`; mincut 4000× slower than top-k at n=3000; ruflo ADR-344 is Proposed, flag-off, unbenchmarked. |

---

## 4. Functional requirements

Requirements are testable. Each has an ID, a statement, and an acceptance test. **MUST** =
binding; **SHOULD** = strong default with a documented-exception escape hatch.

### 4.1 The canonical unit and the accelerator boundary (the prize invariants)

- **FR-1 (canonical unit is served, never a surrogate).** Every grounded response MUST resolve to
  one or more per-IRI markdown blocks addressable by IRI, and the served block MUST be
  byte-identical to the block a human reviews in the corpus. No accelerator output (vector
  neighbour, mincut cluster, GNN encoding, community summary) may be returned in place of the
  markdown.
  *Test:* for a sample of 200 grounded responses, assert every injected block's IRI resolves to a
  corpus markdown block whose sha256 matches the served bytes; assert zero responses inject a
  synthesised summary not traceable to a corpus IRI.

- **FR-2 (no index is a new copy).** Every retrieval index — lexical, HNSW, scaffold, prose — MUST
  be a derivation of the single-source build (FR-6) stamped with the same `generation_id`. Adding
  an index that is not derived from the one source is prohibited.
  *Test:* the build emits a manifest listing every index with its source `generation_id`; a CI
  check fails the build if any index's `generation_id` differs from the source generation, or if
  any index file has no recorded source.

- **FR-3 (legibility is the default; regressions are documented).** Any change that reduces the
  human-readability of the canonical unit (e.g. replacing prose with a vector, pooling multiple
  IRIs into one served block) MUST be accompanied by a documented answer-quality justification in
  the change's ADR/PR, and MUST clear the §7 human-scrutability metric.
  *Test:* PR template requires a "prize-impact" field; CI blocks merge on a corpus-schema change
  that lowers the scrutability metric below its threshold without a linked justification.

### 4.2 Single-source build pipeline (SSOT)

- **FR-4 (one build generation).** The pipeline MUST emit a `build-manifest.json` written last,
  carrying `{commitSha, buildId, generatedAt, artifacts: {<path>: {sha256, bytes, count}}}`, with
  every artifact (ttl, scaffold, prose, HNSW) listed. (This extends PRD-025 WS-A's manifest to
  cover the HNSW index.)
  *Test:* build fails if any emitted artifact is absent from the manifest, or if the manifest is
  not the last file written.

- **FR-5 (one derivation, not three parallel emitters).** `ontology.ttl`, `scaffold-index.json`,
  `prose-index.json` and the HNSW index MUST all derive from one in-memory canonical model per
  build — not from independent parses. The class count MUST be identical across all artifacts.
  *Test:* a conformance check asserts `count(classes in ttl) == count(scaffold entries) ==
  count(prose entries)` and that the HNSW vector count equals the number of IRIs eligible for
  embedding; the 8,152-vs-5,975 divergence class is a failing case.

- **FR-6 (re-embed on promote, delta-diffed — not full re-embed per build).** On promotion of a
  new generation, the HNSW index MUST be rebuilt only for IRIs whose `dfull` prose changed since
  the prior generation, stamped per-row with `generation_id`; unchanged IRIs' vectors carry
  forward.
  *Test:* given a generation that changes k IRIs, assert exactly k vectors are recomputed and the
  rest are carried by reference; assert every vector row's `generation_id` matches the promoted
  generation.

- **FR-7 (index-law compliance).** After bulk HNSW ingest/delete, the index MUST be rebuilt
  non-concurrently (m=16, ef_construction=128) per the project index-law; `CREATE INDEX
  CONCURRENTLY` on the RuVector HNSW AM is prohibited (double-insertion, verified).
  *Test:* the build script uses the non-concurrent rebuild path; a lint check fails on any
  `CONCURRENTLY` against the HNSW AM.

### 4.3 Semantic fallback (HNSW as a third signal, behind the markdown)

- **FR-8 (HNSW is additive, gated, and benchmarked-first).** An HNSW semantic signal (RuVector
  ADR-004 / `@ruvector/core`) MUST be added as a **third** signal alongside lexical + precomputed
  graph, engaged **only** when the lexical top score falls below `MIN_INJECT_SCORE` (the current
  silent-no-injection case). It MUST NOT replace or reorder the lexical first-tier match when the
  lexical match is strong.
  *Test:* with a strong lexical match, assert the HNSW signal does not change the injected set;
  with a below-threshold lexical score, assert the HNSW signal can supply candidates.

- **FR-9 (benchmark gate before default-on — BINDING).** HNSW fusion MUST NOT become a default
  until it beats the lexical baseline on the **multivariate** bench: in-domain recall MUST not
  regress AND general-question jaggedness MUST not worsen (the two axes together — see §7). The
  standing counter-example (over-retrieval Δ = −0.40 [−0.58, −0.22], n = 285) is the bar it must
  clear.
  *Test:* CI holds the HNSW-default feature flag off until a recorded bench run shows in-domain
  recall ≥ lexical baseline (CI-non-inferior) AND general-set recall not worse than raw by more
  than a pre-registered margin. The flag flip is blocked without that recorded run.

- **FR-10 (fallback still resolves to markdown).** HNSW candidates MUST be IRIs that address the
  same canonical markdown blocks; the vector is a pointer, never the served content.
  *Test:* every HNSW-sourced injection is a corpus IRI whose markdown is served (re-uses the FR-1
  harness).

- **FR-11 (fallback carries honesty metadata).** A response grounded via the semantic fallback
  MUST label the grounding path (`grounding.signal: lexical | graph | semantic`) and carry
  `grounding.top_score`, so the customer's auditor can see *why* a block was injected.
  *Test:* assert the response block records the signal and score for every injected IRI.

### 4.4 Reasoned-closure query path (unchanged, explicitly protected)

- **FR-12 (pyoxigraph stays).** SPARQL over the reasoned closure MUST remain on pyoxigraph.
  `@ruvector/graph-node` MUST NOT be installed as a graph engine (label-scan-only; every
  relationship pattern / WHERE / aggregation returns empty — strictly weaker).
  *Test:* a representative relationship-pattern SPARQL query returns non-empty results on the Loom
  path; the same query on graph-node returns empty (regression evidence retained in the ADR).

### 4.5 Enforced admission control (domain predicates in the Loom, mechanics on RuVector)

- **FR-13 (the gate becomes an enforced CI control).** `gate.py` + `conflicts.py` MUST run in CI on
  every corpus write / PR and MUST block merge on a failing verdict. The gate stops being an
  opt-in library.
  *Test:* a PR that introduces a subclass cycle, a duplicate concept, a type conflict, or a
  relation contradiction fails CI and cannot merge; a clean PR passes. (This is the aspirational
  delta made real — the honest status until this lands is "library, not control".)

- **FR-14 (attestation re-platforms onto ProofGate/MutationLedger).** The gate's verdict MUST be
  recorded as a tamper-evident ledger entry via RuVector ProofGate<T> / MutationLedger (ADR-047):
  the domain predicates map to `ProofRequirement::InvariantPreserved` obligations; the unattested
  Python `CheckResult` is replaced by a chain-hashed ledger row.
  *Test:* every gate run produces a ledger entry whose hash chains to the prior entry; tampering
  with a recorded verdict is detectable by chain re-validation.

- **FR-15 (domain semantics stay in the Loom).** The *meaning* of a violation — which relation
  pairs are contradictory, what a duplicate OWL class is, the JSON-LD shape rules — MUST remain
  Loom-owned. RuVector's proof-gate carries the mechanics and has zero domain vocabulary.
  *Test:* the predicate definitions live in the Loom repo; the ProofGate integration imports them
  and does not re-encode domain rules.

- **FR-16 (write path never widened).** The governed write path
  (`propose → consistency-check → human PR merge`) is unchanged. Enforcement (FR-13) hardens the
  *existing* door; it opens no new one, and `ontology_axiom_add` stays disabled-by-default.
  *Test:* the only merge path into the corpus is the PR gate; no code path writes asserted corpus
  state bypassing it.

### 4.6 Build-time reasoner authority

- **FR-17 (Whelk-rs is canonical at build time; retire the BFS duplicate).** The canonical reasoned
  closure MUST be produced by VisionClaw Whelk-rs at build time; `app/pipeline/reason.py`'s
  Python BFS transitive closure is retired as the authority (resolving ADR-135 D3-a
  (= PRD-025 §12 OD-1) to Whelk-rs [ADR-135 Option 1 / PRD-025 option (a)], overriding ADR-135's
  Option-2 recommendation; rationale: build-time Whelk leaves Deployment B's runtime façade
  GPU-free and stdlib-portable). One closure, one reasoned ontology block per IRI.
  *Test:* the published `ontology-inferred.ttl` derives from the Whelk-rs closure; a conformance
  test asserts set-equality between the served closure and the Whelk-rs output; the BFS path is
  removed or demoted to a cross-check only.

- **FR-18 (honest reasoner scope).** Documentation MUST NOT claim the reasoner catches
  contradictions the corpus cannot express: with **zero `owl:disjointWith` axioms**, Whelk is the
  authority for closure/subsumption, not a live contradiction-catcher. Query-time DL reasoning
  stays out of scope (the Loom serves a pre-reasoned snapshot).
  *Test:* a doc-lint / review check flags any claim of "DL reasoner catches contradictions" not
  qualified by the zero-disjointness caveat.

---

## 5. Tooling allocation as product capabilities

This is ADR-136's allocation, expressed as what each capability *delivers to a consumer* and where
it lives. It is cited, not re-decided, here.

| Capability (consumer-facing) | Home | Action | Status | Why here (prize impact) |
|---|---|---|---|---|
| Serve the per-IRI markdown unit | **Loom** | keep | shipped | The prize. Every other row is justified only by serving it. |
| `corpusNature` + grounding honesty metadata | **Loom** | keep | shipped | Lets the human judge trustworthiness of the source. |
| Confidence-gated injection policy | **Loom** | keep | shipped | Prevents jaggedness; no RuVector analogue (product policy, not an index). |
| Model-swappable `/v1` façade | **Loom** | keep | shipped | Stable door; model swaps behind it with zero consumer change. |
| Lexical title-matcher (first-tier signal) | **Loom** | keep | shipped | Fast, ahead of the ecosystem; points at markdown. |
| pyoxigraph SPARQL over the reasoned closure | **Loom** | keep | shipped | More capable than graph-node Cypher; resolves canonical IRIs. |
| Semantic HNSW fallback (OOV/paraphrase) | **RuVector** (ADR-004) | add | aspirational | The one real gap; ranks/finds markdown, never replaces it. Gated by FR-9. |
| Collapse three materialisations → one source | **Loom SSOT build** | retire the redundancy | aspirational | Removes drift; the human always reviews one authoritative copy. |
| SSOT build (one source → ttl+scaffold+prose+HNSW) | **Loom build** | add | aspirational | Guarantees every accelerator is a projection, not a separate artifact to trust. |
| Admission-control **domain predicates** | **Loom** | keep + enforce (FR-13) | aspirational (enforcement) | Blocks a bad write before it reaches the reviewed corpus. |
| Admission-control **attestation mechanics** | **RuVector ProofGate/MutationLedger** (ADR-047) | move | aspirational | Attests the gate ran; markdown stays human-facing. |
| EL++ closure authority (build-time) | **VisionClaw Whelk-rs** | keep + make canonical (FR-17) | aspirational | One authoritative closure; no reasoner drift. |
| EL++ at query time | none — build-time only | defer | not done | Static snapshot keeps served == reviewed. |
| `@ruvector/graph-node` Cypher as graph engine | **rejected — not installed** | retire | — | Label-scan-only; strictly weaker regression. |
| `ruvector-hybrid` / mincut / gnn-rerank | **RuVector, if/when it ships AND beats the bench** | defer | deferred | Unshipped, slower at our scale; adopt only on benchmark, never marketing. |
| Multi-agent coordination substrate | **deferred phase (WS-Q)** | defer | aspirational | Avoids over-claiming; when built, still resolves to per-IRI markdown. |

---

## 6. Non-goals (explicit)

1. **Not building the multi-agent coordination substrate / shared blackboard now.** It is an
   explicitly deferred phase (WS-Q). Today's only live consumer is the email gateway doing
   single-LLM RAG. It MUST NOT be described as shipped anywhere.
2. **Not replacing pyoxigraph SPARQL with `@ruvector/graph-node` Cypher.** That is a regression
   (label-scan-only) dressed as an upgrade.
3. **Not running Whelk EL++ inside the Loom at query time.** The Loom serves a pre-reasoned
   build-time snapshot; live query-time DL reasoning is out of scope.
4. **Not adopting the GraphRAG / G-Retriever trajectory.** No opaque LLM community summaries, no
   GNN-encoded subgraphs served in place of the markdown. The per-IRI reviewable unit is the
   non-eroding moat and must not degrade into an inspection-resistant representation.
5. **Not adopting `ruvector-hybrid` / mincut / gnn-rerank on marketing.** They are unshipped
   nightly PoCs, slower than baseline at our scale (mincut 4000× at n=3000); revisited only if and
   when they ship AND beat the multivariate bench.
6. **Not defending the Loom as a retrieval engine.** Its retrieval lead is a temporary artifact of
   unfinished ecosystem capability; the durable position is governed-façade + admission-control +
   domain-semantic authority.
7. **Not full-re-embedding the corpus on every build.** Re-embed on promote, delta-diffed against
   the prior generation (FR-6).

---

## 7. Success metrics

Metrics are observed, not asserted. Each carries a threshold and a measurement method.

### 7.1 Human-scrutability metric (the prize's own KPI — load-bearing)

The prize is only real if the canonical unit stays legible. We measure it directly.

- **SM-1 (per-IRI attribution granularity).** ≥ 99% of served grounded facts trace to exactly one
  per-IRI markdown block (not a pooled multi-entity summary).
  *Method:* sample 200 grounded answers; count facts whose attribution resolves to a single IRI's
  block. GraphRAG-style pooling (many entities → one summary) is a failing case.
- **SM-2 (round-trip legibility).** For a random sample of 50 IRIs, a human reviewer can, from the
  served block alone, restate the entity's definition and its typed relations without consulting
  any index internals. Target: ≥ 95% pass.
  *Method:* structured review; a fail is any block where the served content is not self-explanatory
  without decoding a vector/index artifact.
- **SM-3 (served == reviewed).** 100% of served blocks are byte-identical (sha256) to the corpus
  block a human would review (FR-1). Any divergence is a P1 defect.
- **SM-4 (no legibility regression over time).** The scrutability score (composite of SM-1..3) MUST
  NOT fall across generations without a documented, justified answer-quality trade (FR-3).

### 7.2 Benchmark-first gate on any retrieval change (BINDING)

- **SM-5 (multivariate non-inferiority before default-on).** No retrieval fusion change (HNSW as a
  default signal, any future `ruvector-hybrid` adoption) becomes a default until a recorded bench
  run shows: **(a)** in-domain recall ≥ current lexical baseline (CI-non-inferior), AND **(b)**
  general-question recall not worse than raw by more than the pre-registered margin (no
  jaggedness). Our over-retrieval Δ = −0.40 [−0.58, −0.22], n = 285 is the standing failure the
  gate exists to catch.
  *Method:* `bench_ontology_uplift.py` multivariate suite (in-domain + general/off-ontology arms,
  bootstrap 95% CIs); the feature flag stays off until both arms pass; the passing run is linked in
  the flip PR.
- **SM-6 (semantic fallback recovers the OOV gap).** On a held-out paraphrase/OOV question set (the
  queries that currently produce silent no-injection), the HNSW fallback recovers a
  pre-registered fraction of correct injections that the lexical matcher misses, without regressing
  SM-5.
  *Method:* labelled OOV set; measure injection-recall with fallback on vs off.

### 7.3 Consolidation and correctness metrics

- **SM-7 (single source, zero drift).** All corpus-derived artifacts report the same
  `generation_id` and identical class counts (FR-5). Target: 0 divergences; the 8,152-vs-5,975
  class is a failing state.
- **SM-8 (storage collapse).** The three redundant materialisations no longer exist as independent
  parses; every index is a manifest-recorded derivation of one source (FR-2). Measured as: 0 index
  artifacts without a recorded source generation.
- **SM-9 (gate enforced).** 100% of corpus-writing PRs run the gate in CI; a seeded
  cycle/dupe/type/contradiction PR is blocked (FR-13). Baseline today: 0% (no CI).
- **SM-10 (gate attested).** 100% of gate runs produce a chain-valid MutationLedger entry (FR-14);
  a tampered verdict is detectable.
- **SM-11 (one reasoner authority).** Published closure == Whelk-rs closure by set-equality
  conformance test (FR-17); 0 reasoner-drift divergences.
- **SM-12 (re-embed efficiency).** On a generation changing k IRIs, exactly k vectors recomputed
  (FR-6); full re-embed is a failing state.

---

## 8. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **HNSW fusion turned on unbenchmarked → jaggedness (the −0.40 failure recurs)** | Medium | High | SM-5 benchmark gate is binding; the default flag cannot flip without a recorded passing multivariate run. |
| **HNSW becomes a 4th copy** (index not derived from the one source) | Medium | Medium | FR-2 + FR-6 + manifest CI check; an index without a recorded source generation fails the build. |
| **Prize erosion by convenience** — a served vector/summary quietly replaces the markdown | Low | Critical | FR-1 + SM-1..3; served-==-reviewed sha check is a P1 gate; PR template forces a prize-impact field. |
| **Ecosystem corpus is stale** — graph-node ships real Cypher or ADR-344 merges + passes its bench, weakening "keep pyoxigraph" | Low | Medium | Decision is benchmark-conditional (§6.5, FR-9); revisit only on a measured beat, documented in a superseding ADR. Do not pre-commit. |
| **Whelk-rs closure diverges from the served snapshot** | Medium | Medium | FR-17 set-equality conformance test blocks the build on divergence; the retired BFS path stays as a cross-check for one generation. |
| **Gate enforcement (FR-13) slows the write path / false-positives block good PRs** | Medium | Low | Predicates are already tested (`test_conflicts.py`); tune thresholds with a documented exception path; the gate proves *well-formedness only*, not correctness (its own docstring), so scope stays narrow. |
| **ProofGate re-platform adds coupling to RuVector on the write path** | Low | Medium | Only the domain-agnostic mechanics move; predicates stay Loom-owned (FR-15); ledger write is off the read hot path. |
| **Over-claiming the mesh** — docs describe the deferred substrate as shipped | Medium | Medium | §3 honesty table is authoritative; doc-lint flags any "shared substrate is live" claim; WS-Q is explicitly deferred. |
| **Human-review discipline is aspirational** — the PR gate is spot-checked, not exhaustive, at 8,146 classes | Medium | Medium | Enforcement (FR-13) makes the gate mechanical; SM-2 measures review legibility; honestly labelled as a discipline claim, not a proven control, until sampled. |

---

## 9. Phasing — build order

Build the target end-state directly (dev/test estate, per PRD-025 §6). Workstreams continue
PRD-025's WS-A…WS-J lettering.

### Phase 1 — SSOT + enforced gate (the drift and the unenforced-gate fixes)

| WS | Owner (repo) | Content | Requirements |
|---|---|---|---|
| **WS-K** | Loom build pipeline | Collapse the three parallel emitters to one canonical in-memory model per build; extend `build-manifest.json` (PRD-025 WS-A) to list every index with its `generation_id`; conformance check on class-count equality. | FR-4, FR-5, FR-7, SM-7, SM-8 |
| **WS-L** | Loom + CI | Wire `gate.py`+`conflicts.py` into a CI workflow that blocks merge on a failing verdict (first `.github/workflows/` in the repo). | FR-13, FR-16, SM-9 |
| **WS-M** | Loom + VisionClaw | Make Whelk-rs the canonical build-time reasoner; retire `reason.py` BFS as authority; set-equality conformance test; correct the zero-disjointness caveat in all docs. | FR-17, FR-18, SM-11 |

### Phase 2 — semantic fallback + attestation (behind the markdown, benchmark-gated)

| WS | Owner (repo) | Content | Requirements |
|---|---|---|---|
| **WS-N** | Loom + RuVector | Add `@ruvector/core` HNSW (ADR-004) as the third signal; derive it from the one source; re-embed-on-promote delta-diffed; engage only below `MIN_INJECT_SCORE`. | FR-6, FR-8, FR-10, FR-11, SM-12 |
| **WS-O** | Loom bench | Multivariate bench arms (in-domain + general jaggedness + OOV recovery); the SM-5 flag gate; keep HNSW default **off** until it passes. | FR-9, SM-5, SM-6 |
| **WS-P** | Loom + RuVector | Re-platform the gate verdict onto ProofGate<T>/MutationLedger (ADR-047): predicates → `InvariantPreserved` obligations; chain-hashed ledger entry per run. | FR-14, FR-15, SM-10 |

### Phase 3 — deferred (named, not built)

| WS | Owner | Content | Status |
|---|---|---|---|
| **WS-Q** | Loom / mesh | Multi-agent coordination substrate (shared blackboard multiple agents read+write). Named later phase; NOT shipped; must resolve to the same per-IRI markdown when built. | Deferred (§6.1) |
| **(revisit)** | RuVector | `ruvector-hybrid` / mincut / gnn-rerank — adopt only if it ships AND beats the WS-O multivariate bench. | Deferred (§6.5) |

**Smallest honest consolidation win:** WS-K + WS-L + WS-M — the corpus stops drifting (one source,
one reasoner) and the gate becomes a real control — with zero change to the served unit and no
retrieval risk. WS-N/O/P add recall and attestation strictly behind the markdown, each behind its
own gate.

---

## 10. Relationship to PRD-025 and ADR-136

- **PRD-025** built the serving + deferred-distillation loop and the corpus-generation identity
  (WS-A). **PRD-026 extends it**: it takes PRD-025's build-manifest and generation discipline and
  drives it down into the index layer (one source → every index), hardens PRD-025's gate reference
  from a library into a CI control, and settles the retrieval/reasoner/attestation allocation
  PRD-025 deferred. Of that allocation, only the reasoner authority resolves a prior ADR-135 open
  decision (D3-a); the HNSW retrieval acceleration and the ProofGate attestation are new ADR-136
  allocations, not resolutions of any prior ADR-135 open decision. It does **not** replace
  PRD-025's product goal or its distillation channel.
- **ADR-136** is the decision authority. Every allocation in §5 is ADR-136's; this PRD cites it and
  does not re-litigate. ADR-136 extends keystone ADR-135 (node boundary unchanged), supersedes
  ADR-135 §5 (bespoke retrieval-stack posture), and **resolves ADR-135 D3-a (= PRD-025 §12 OD-1),
  the reasoner open decision** — canonical reasoner = Whelk-rs at build time (FR-17), selecting
  Whelk-rs [ADR-135 Option 1 / PRD-025 option (a)] and overriding ADR-135's Option-2 recommendation
  (rationale: build-time Whelk leaves Deployment B's runtime façade GPU-free and stdlib-portable).
  The other two allocations are **new ADR-136 decisions, not resolutions of prior ADR-135 open
  decisions**: retrieval acceleration = RuVector HNSW behind the markdown (FR-8); gate attestation
  = RuVector ProofGate/MutationLedger (FR-14). It cites RuVector ADR-004 (HNSW production),
  RuVector ADR-047 (ProofGate/MutationLedger), VisionClaw ADR-099 (Whelk-rs EL++ reasoner
  authority), and notes ruflo ADR-344 as Proposed/deferred.
- **The DDD** (`ddd-ontology-loom-context.md`, revised in place) names the per-IRI markdown unit as
  the aggregate root, draws the accelerator boundary (RuVector = downstream index/attestation
  adapter, pyoxigraph = in-context SPARQL, Whelk-rs = upstream build-time reasoner), and marks the
  mesh-coordination context as a deferred/aspirational region.

One source, one reasoner authority, one enforced gate, and one canonical per-IRI markdown unit that
every accelerator points at and no accelerator replaces. That is the consolidation, and the prize
stays primary through all of it.
