---
id: DOC-REENGINEERING-PLAN
title: "Documentation re-engineering & cross-link plan for the Rust Loom (PRD-027 / ADR-137)"
status: proposed
date: 2026-08-17
authors: VisionFlow operator (did:nostr:jjohare) + opus re-platform mesh
governs: ["docs/**", "README.md", "bench/**", "legacy/** (freeze)"]
decision_authority: ADR-137 (Rust re-platform); requirements PRD-027; model DDD (Rust rev)
relates: [ADR-135, ADR-136, PRD-025, PRD-026, ddd-ontology-loom-context.md, LOOM-POSITIONING.md]
supersedes: "nothing wholesale — this plan schedules the doc changes ADR-137/PRD-027 require; it authors no design decision of its own."
---

# DOC-REENGINEERING-PLAN — the Rust Loom documentation set

**Status:** **Executed (2026-08-17)** — the doc re-engineering this plan schedules is done: README rewritten to the Rust ground truth; `docs/README.md` index created; ADR-137 marked Accepted+Implemented with ADR-135/136 implementation-notes; PRD-027 §10.1 status added (recall floor unmet → gated-off documented); errata folded in (ProofGate location, redb ro-mount hazard, ruvector-core feature trim, gpt-5.4 audit outcomes); agentbox ADR-051 de-vendored to a stub; `ONTOLOGY-UPLIFT-PLAN.md` + `MODEL-BENCHMARKS.md` retired to `legacy/`; `tools/ingest/README.md` created; `ONTOLOGY-LOOM-PIPELINE.md` retargeted to the consumer-side generation contract; Python serving code retired (§12 map). Where the plan and reality diverged, reality won and is annotated. Retained as the record of the intent. (Originally: Proposed — planning artefact; changes no design decision.)
**Date:** 2026-08-17
**Owner:** VisionFlow operator (Dr J. O'Hare, did:nostr:jjohare)
**Scope:** every narrative document in `loom/` — `README.md`, `docs/`, `docs/design/*`, `docs/research/*`, `bench/*.md`, `legacy/*.md`, and the undocumented `tools/ingest/`. It assigns each a disposition (**keep / rewrite / supersede / retire / create**), fixes the cross-repo link map with per-target verification status, specifies the README rewrite, defines the docs index, and states the consistency checklist the implementation phase must pass before any doc is considered done.

> **EXECUTION NOTE (read first).** This is a *documentation* plan. It writes no Rust and moves no code. It presupposes the three governing docs of the Rust re-platform are authored in parallel — **ADR-137** (the decision), **PRD-027** (the requirements/build order), and the **DDD Rust revision** — and it is the map for bringing every other doc in the repo into agreement with them. Where this plan says "rewrite", it means *retarget the prose to the Rust substrate without changing any decision already recorded in ADR-135/136/PRD-025/026*; the substrate changes, the invariants do not.

---

## 0. Why the docs need re-engineering at all

The substrate change (stdlib Python serving mirror → single-binary Rust node) touches the docs on exactly three axes, and no others:

1. **Deployment prose is now wrong in two places.** `README.md` and `docs/REMOTE-CLIENT-SETUP.md` describe "a stdlib-Python + one-Node-dep container" and a single reference deployment. ADR-137 resolves deployment to **both compose profiles** (host-colocated A + sidecar B) over one static musl binary. That is a factual change consumers read.
2. **A third retrieval signal now exists.** The ontology-corpus RuVector namespace (8,146 IRI-keyed bge-small/384 records) landed. Every doc that describes retrieval as "lexical + SPARQL" is now missing the **confidence-gated HNSW semantic fallback** (ADR-136 D3, built by PRD-027). This must be documented as *default-off, benchmark-gated*, never as shipped.
3. **The build/serve boundary sharpens.** `app/pipeline/*` (the vendored logseq builder) is dropped (#21); `app/ontology_proxy.py` and `app/test_proxy.py` retire. The docs that still point at those files (`ONTOLOGY-UPLIFT-PLAN.md`, the vendored agentbox ADR copy) must retarget or retire.

Everything else — the research corpus, the positioning, the two keystone ADRs, the measured-result tables — is **substrate-neutral and stays**. The re-engineering is surgical, not a rewrite of the doc set.

---

## 1. Principles that govern every disposition

These bind the implementation phase. A doc change that violates one is a defect, not a style choice.

- **P1 — The Prize is stated once canonically, quoted verbatim elsewhere.** THE PRIZE currently appears *verbatim* in three docs (ADR-136, PRD-026, ddd). This plan promotes **`docs/design/ddd-ontology-loom-context.md` §THE PRIZE as the single source-of-record** for the canonical wording. ADR-137, PRD-027 and README quote it by reference (`> THE PRIZE — see ddd §…`) or reproduce it *byte-identically*; no doc may paraphrase it. A CI check (see §7) asserts byte-identity across every occurrence.
- **P2 — Shipped ≠ aspirational, everywhere.** The mandatory shipped-vs-aspirational honesty table (ADR-136 §1 / PRD-026 / ddd §1) gains one row set for the Rust substrate; every new doc reproduces it unchanged and may not contradict it. "The Rust node" is *aspirational/planned* until a binary ships; write it that way.
- **P3 — Substrate change, not feature change.** No doc may present the Rust rewrite as new capability. Endpoints, the injection policy, the served unit, the model-swap seam are *preserved*; the doc verbs are "port", "re-platform", "collapse the binding", not "add".
- **P4 — No fourth copy, no re-vendoring.** The doc set caused drift by *duplicating* upstream material (the vendored `app/pipeline/`, the vendored agentbox ADR-051). The rewrite de-vendors: cross-link the canonical doc in its owning repo, keep a one-line stub, never a full copy.
- **P5 — Generation discipline is a doc invariant too.** Every doc that shows a `/health`, `/loom/generation`, or `loom` response block must show the generation stamp and state the atomicity/parity rule (ADR-135 D1.1, ADR-136 D4). Consumers audit provenance from the docs.

---

## 2. Full document inventory & disposition

Legend: **KEEP** (accurate as-is or with a one-line note) · **EDIT** (keep, targeted factual edit) · **REWRITE** (retarget prose, decisions unchanged) · **SUPERSEDE** (replaced by a named new/other doc; leave a stub) · **RETIRE** (freeze as historical, move under `legacy/` or mark `Status: Historical`) · **CREATE** (new).

### 2.1 Root & top-level docs

| Doc | Disposition | Rationale / action |
|---|---|---|
| `README.md` | **REWRITE** | New one-paragraph positioning (§4), two-profile deploy, retrieval-fusion note, Rust substrate honesty. Measured-result tables and ecosystem diagram **kept verbatim** (substrate-neutral). |
| `docs/QWEN3.8-CONNECTION.md` | **KEEP + EDIT** | Model reference is correct and substrate-neutral (model stays a URL). Add one line: under the Rust façade the model is still `DISTILL_BACKEND_URL`, `loom-model` container unchanged; nothing else edits. |
| `docs/REMOTE-CLIENT-SETUP.md` | **EDIT** | Add the Profile-B endpoint (`http://loom:8080/v1` on `visionclaw_network`) beside the existing `10.10.10.1:8084/8085` rows; state that A and B serve byte-identical generations. No structural change. |
| `docs/ONTOLOGY-UPLIFT-PLAN.md` | **RETIRE** | HP-side usage guide for `legacy/scripts/` bench drivers and the vendored pipeline — both retiring. Mark `Status: Historical (2026-08-11 toolkit)`, move to `legacy/`. Its live content (the retrieval recipe, the bench ask) is subsumed by PRD-027 §bench and `bench/UPLIFT-BENCH-PROTOCOL.md`. |

### 2.2 `docs/design/` — the governance set

| Doc | Disposition | Rationale / action |
|---|---|---|
| `ADR-135-ontology-loom-node.md` | **KEEP** | Keystone node-boundary decision — immutable historical record. ADR-137 *extends* it (Rust substrate); it is not reopened. Add one back-reference line in its status block: "Extended by ADR-137 (Rust re-platform)." |
| `ADR-136-loom-tooling-allocation.md` | **KEEP** | Tooling-allocation decision — immutable. ADR-137 realises its D3 (HNSW fallback) and D-attestation (ProofGate) on the Rust substrate. Add back-reference line only. |
| `ADR-137-loom-rust-replatform.md` | **CREATE** | The decision record: axum/tokio, hexagonal crates, oxigraph-native, in-process HNSW, **both** compose profiles. Spec in §3.1. |
| `PRD-025-ontology-loom-and-connector-platform.md` | **KEEP** | Product capstone — substrate-neutral. Add "Extended by PRD-027" to `relates`. |
| `PRD-026-loom-consolidation.md` | **KEEP** | Consolidation requirements — substrate-neutral (single-source build, semantic fallback, admission control). PRD-027 *operationalises PRD-026 on Rust*; add cross-ref only. |
| `PRD-027-loom-rust-reengineering.md` | **CREATE** | Requirements + WS build order for the Rust re-platform. Spec in §3.2. |
| `ddd-ontology-loom-context.md` | **REWRITE** | Add the **hexagonal crate-realisation** layer: map the CanonicalUnit aggregate root and every port (LexicalIndex, VectorIndex, EmbeddingProvider, GraphStore, ModelBackend, AttestationLedger) to the `loom-domain`/adapter crates. Becomes the **canonical home of THE PRIZE wording** (P1). Spec in §3.3. |
| `LOOM-POSITIONING.md` | **KEEP** | Product framing — substrate-neutral. Add one sentence: the Rust rewrite changes the substrate, not the multivariate bar or the "curated corpus vs generic web search" frame. |
| `ONTOLOGY-LOOM-PIPELINE.md` | **REWRITE** | Currently documents the *logseq* build stage (WS-A) as if partly Loom-owned. The Rust Loom drops `app/pipeline/*` and is a *serving mirror*. Retarget to: "**Generation-identity contract the Rust `loom-facade` mirror consumes and verifies**" — describe `build-manifest.json`, `urn:ngm:generation:<sha>`, the atomic mirror, and cross-link the *canonical* builder in `jjohare/logseq`. Authority for the build moves upstream; this doc becomes the consumer-side contract. |
| `agentbox-ADR-051-loom-client-and-deferred-distillation.md` | **SUPERSEDE (de-vendor)** | A full vendored copy of an agentbox ADR — exactly the duplication P4 forbids. Replace with a ~10-line stub: title, one-paragraph summary, and a cross-link to the canonical `DreamLab-AI/agentbox` ADR-051. Removes a drift surface. |

### 2.3 `docs/research/` — evidence & reports

| Doc | Disposition | Rationale / action |
|---|---|---|
| `ontology-uplift-report.pdf` + `latex/` + `preprint/` | **KEEP (frozen)** | Flagship two-study report. Evidence is substrate-neutral and must not be re-touched. `preprint/*.aux/.bbl/.log/.fls` are LaTeX build artefacts — add to `.gitignore` (housekeeping, not a doc change). |
| `report.md` | **KEEP** | Local-model uplift report (37-q). Frozen. |
| `report-gemini-3.7-flash.md` | **KEEP** | First cloud model (510-q). Frozen. |
| `research-notes.md`, `refs.bib` | **KEEP** | Working notes/bibliography for the preprint. |
| `MODEL-BENCHMARKS.md` | **RETIRE** | Dated 2026-04-07, pre-Qwen (Aria/Nemotron/Gemma quality bench, RTX 6000 Ada hardware that is not this host). Not ontology-related; superseded by `QWEN3.8-CONNECTION.md` for model facts. Mark `Status: Historical`, move to `legacy/`. |
| `evidence/*` (logs, `*.json`, `*.png`) | **KEEP (frozen)** | Raw witnesses behind the reports. Never edited. Confirm covered by `.gitignore` policy for logs where appropriate; the `.json` grade files stay tracked as cited evidence. |

### 2.4 `bench/` — reproduction harness docs

| Doc | Disposition | Rationale / action |
|---|---|---|
| `UPLIFT-BENCH-PROTOCOL.md` | **EDIT + EXTEND** | The paired-bootstrap protocol is correct and stays. Add the **WS-O multivariate section**: the standing regression guard (over-retrieval Δ=−0.40, n=285) and the three axes HNSW fusion must beat the lexical baseline on (in-domain recall AND general-question non-jaggedness AND OOV recovery) before default-on. This is the gate PRD-027 references. |
| `bench-integration.md` | **KEEP + EDIT** | Add the two-profile assertion (generation parity A≡B) and the Rust `cargo test --all-features` entrypoint alongside the Python bench, once the harness is ported. |

### 2.5 `legacy/`, `tools/`, `tests/`

| Path | Disposition | Rationale / action |
|---|---|---|
| `legacy/README.md`, `legacy/GEMMA4-*.md`, `legacy/MUSE-GLIMMER-CONNECTION.md` | **KEEP (frozen)** | Already correctly quarantined as historical model-connection docs. No change; they are the archive target for retired docs above. |
| `tools/ingest/build_concept_records.py`, `embed_and_stage.py` | **CREATE doc** | These are the **new ground-truth ingestion scripts** (ontology-corpus → RuVector) and are undocumented. Author `tools/ingest/README.md`: what they build (IRI-keyed bge-small/384 records, `source_type=loom`), the **build/off-turn write channel** discipline (never the query hot path, DDD §6.1), and the HNSW index-law (non-concurrent rebuild, m=16, ef_construction=128; never `CREATE INDEX CONCURRENTLY`). Cross-link RuVector ADR-001 (HNSW production index) and the ops-law in `~/workspace/CLAUDE.md`. |
| `tests/test_confidence_injection.py` | **N/A (code)** | Not a doc, but the consistency checklist (§7) requires the Rust port to carry an equivalent test; noted here so the doc→code parity is tracked. |
| `docs/dream-cycle/LEDGER.md`, `dream.config.json` | **KEEP** | Append-only nightly ledger + dream config. Substrate-neutral. When the Rust node lands, `dream.config.json` `buildStep`/`evaluatorEntrypoints` get a `cargo` variant — an *operational* edit tracked by PRD-027, not a doc rewrite. |

### 2.6 Docs that do not exist yet but must

| New doc | Home | Why |
|---|---|---|
| `docs/README.md` | repo | **CREATE** the docs index / navigation map (§6). None exists; the ecosystem is not navigable without it. |
| `docs/design/ADR-137-*.md` | design | The decision (§3.1). |
| `docs/design/PRD-027-*.md` | design | The requirements (§3.2). |
| `tools/ingest/README.md` | tools | Ingestion channel (§2.5). |

---

## 3. Specification of the three new governance docs

These three are authored by the PRD-027/ADR-137 workstream, not by this plan; the specs below fix their scope, section skeleton, and cross-links so the set stays coherent.

### 3.1 ADR-137 — Re-platform the Loom to Rust

- **Status/type:** Proposed · Architecture (substrate re-platform) · Extends ADR-135 (node boundary unchanged) and ADR-136 (tooling allocation unchanged); realises ADR-136 D3 (HNSW fallback) and the ProofGate attestation move on the Rust substrate.
- **Skeleton:** §1 Context (why now: the ontology-corpus landing + the pyoxigraph-binding tax) → §2 Decision (D1 axum/tower+tokio; D2 oxigraph as a direct crate; D3 in-process `@ruvector/core` HNSW on the query path + MCP/postgres write channel off it; D4 Xinference bge-small/384 embeddings (LOCKED); D5 model-is-a-URL OpenAI-compatible backend (`DISTILL_BACKEND_URL`); D6 hexagonal crate ring — the eight crates; D7 single static musl binary; D8 **both compose profiles**; D9 ProofGate/MutationLedger attestation on the Rust substrate) → §3 Each decision with its rejected alternative and its **Prize impact** line → §4 the honest ADR-135 D1 trade (stdlib-portability given up, better-delivered portability bought back; the code-legibility traded is the *façade's own source*, never the served data) → §5 Consequences (generation parity A≡B as CI/health assertion; the ruvector-postgres path is build/off-turn only) → §6 the shipped-vs-aspirational table with the new substrate rows.
- **Must state, once:** the eight-crate skeleton (`loom-domain`, `loom-scaffold`, `loom-graph-oxigraph`, `loom-vector-ruvector`, `loom-embed-xinference`, `loom-backend-openai`, `loom-attest-proofgate`, `loom-facade`) with one responsibility line each. Do not re-derive the fusion flow — reference PRD-027.
- **Cross-links:** ADR-135, ADR-136 (this repo); VisionClaw ADR-090 (hexagonal ring), ADR-099 (Whelk-rs build-time reasoner); RuVector ADR-001 (HNSW), ADR-047 (ProofGate/MutationLedger); agentbox ADR-051 (loom client), ADR-112 (one-brain/no hot-path LLM); sibling Rust repos as style exemplars (`ruvector`, `solid-pod-rs`, `nostr-rust-forum`, `logseq-publisher-rust`).

### 3.2 PRD-027 — Rust re-engineering requirements

- **Status:** Proposed · builds on PRD-025 (capstone) and PRD-026 (consolidation); operationalises ADR-137.
- **Skeleton:** §0 execution note (design + WS plan; changes no code by itself) → §1 THE PRIZE (quoted by reference to the ddd, P1) → §2 shipped-vs-aspirational table (identical to ADR-137/ddd) → §3 the retrieval-fusion requirement (candidate-union → one confidence gate; HNSW is a *source*, never a bypass; default-off, WS-O-gated) → §4 the deprecation map (port/replace-with-crate/drop, per the shared frame) → §5 WS build order (domain crate + ports first, then adapters, then façade, then the two profiles, then fusion behind the bench) → §6 evidence bars per WS (`cargo test --all-features`, clippy, the WS-O multivariate bench, generation-parity health assertion) → §7 non-goals (no query-time Whelk; no graph-node Cypher; no markdown-replacing encoding; not a corpus builder; mesh deferred).
- **Cross-links:** as ADR-137, plus `bench/UPLIFT-BENCH-PROTOCOL.md` (WS-O), `tools/ingest/README.md` (write channel), logseq `pipeline/` + `publish.yml` (the upstream builder it mirrors from).

### 3.3 DDD — Ontology Loom Bounded Context (Rust revision)

- **Action:** revise in place; bump the revision date; retain the 2026-08-16 authority (CanonicalUnit as aggregate root, accelerator boundary, MeshCoordination quarantine) and **add** §"Hexagonal crate realisation (Rust rev)".
- **New section content:** the mapping table — aggregate/port → crate → adapter → external system:

  | Domain concept (port) | Crate | Adapter target |
  |---|---|---|
  | CanonicalUnit / CorpusGeneration (aggregate) | `loom-domain` | — (pure, no I/O) |
  | LexicalIndex | `loom-scaffold` | in-process inverted index |
  | GraphStore | `loom-graph-oxigraph` | native oxigraph (replaces pyoxigraph) |
  | VectorIndex | `loom-vector-ruvector` | in-process `@ruvector/core` HNSW (query) + ruvector-postgres (write, off-turn) |
  | EmbeddingProvider | `loom-embed-xinference` | Xinference bge-small/384 (LOCKED) |
  | ModelBackend | `loom-backend-openai` | `DISTILL_BACKEND_URL` |
  | AttestationLedger | `loom-attest-proofgate` | RuVector ProofGate/MutationLedger |
  | composition root | `loom-facade` | axum/tower |

- **Prize:** this doc holds the canonical wording (P1); its §THE PRIZE block is the source-of-record every other doc quotes.

---

## 4. README rewrite specification

The README stays the front door; the rewrite is targeted, not wholesale. **Keep verbatim:** the measured-result tables, the three-findings block, the research links, the ecosystem diagram, the boundary note, the status/honesty section. **Change:**

**4.1 New one-paragraph positioning (replaces the current lead two paragraphs):**

> **Loom is a single-binary Rust node that grounds LLM responses in a formal ontology behind a stable, model-swappable façade.** Point any OpenAI-compatible consumer at one endpoint; Loom retrieves the relevant slice of the reasoned ontology as **human-scrutible per-IRI markdown-with-ontology blocks**, injects them as budget-clamped, confidence-gated context, and delegates generation to whatever model is deployed behind it (`DISTILL_BACKEND_URL`). Retrieval fuses three signals over one canonical unit — a lexical inverted index, RuVector HNSW semantic fallback for out-of-vocabulary queries, and oxigraph SPARQL over the Whelk-reasoned closure — but the served, reviewable unit is always the markdown; the indexes only find and rank it. Swap the model — Gemma → Muse-Glimmer → Qwen3.8 → next — and no consumer changes.

**4.2 Deploy section:** replace "stdlib-Python + one-Node-dep container" with "one static Rust binary, no interpreter, in **two compose profiles**": Profile A (host-colocated with the model on HP, reference serving) and Profile B (sidecar on `visionclaw_network`, `http://loom:8080`, the consumer-facing door + the build/off-turn write channel). State the both-profiles rationale in one sentence and cross-link ADR-137.

**4.3 Retrieval note:** one new sub-paragraph under "What" — the HNSW fallback is **default-off, benchmark-gated** (the −0.40 over-retrieval guard); it is a candidate source into the existing confidence gate, never a bypass. Do not present it as on-by-default.

**4.4 Substrate honesty line** in the status section: "The Rust node is the *planned* substrate (ADR-137/PRD-027); the shipped serving code today is the stdlib-Python façade. This README describes the target end-state and flags what is not yet built."

**4.5 Endpoint table:** add `POST /loom/sparql`, `POST /loom/search` (the vector facet), keep the rest unchanged with their "needs a model?" column.

---

## 5. Cross-repo link map (with verification status)

Every external reference a loom doc makes, its owning repo, and whether the ADR/PRD number is verified. **Verified** = confirmed against source (ruvnet-brain fetch or an existing loom doc's grounded citation). **Verify** = plausible but not re-confirmed in this pass; the authoring workstream must confirm before merge. **By-capability** = cite the capability, not a number (number unverifiable/known-absent).

| From (loom doc) | → Target | Repo | Status | Note |
|---|---|---|---|---|
| ADR-137, PRD-027, ddd | ADR-047 ProofGate\<T\> / MutationLedger | `ruvnet/ruvector` | **Verified** | Fetched: `docs/adr/ADR-047-proof-gated-mutation-protocol.md`; `crates/ruvector-graph-transformer/src/proof_gated.rs` confirms `ProofGate<T>`, `MutationLedger`, FNV-1a chain hash. |
| ADR-137, PRD-027, ddd | ADR-001 HNSW production index | `ruvnet/ruvector` | **Verified (corrected)** | **ADR-004 was WRONG** — ADR-004 is *KV Cache Management*, not HNSW. The HNSW production index is RuVector **ADR-001** (confirmed against source). ADR-136/PRD-026 carry the mistaken ADR-004 citation; fix it set-wide when those docs are next touched. |
| ADR-137, PRD-027 | ADR-090 hexagonal crate ring | `DreamLab-AI/VisionClaw` | **Verified** | Grounded in ADR-135 relates-list. |
| ADR-137, PRD-027, ONTOLOGY-LOOM-PIPELINE | ADR-099 Whelk-rs EL reasoner | `DreamLab-AI/VisionClaw` | **Verified** | Grounded in ADR-135/136. Build-time authority only. |
| ADR-137 | PRD-016 (hexagonal ring / crate modularisation) | `DreamLab-AI/VisionClaw` | **Verify** | Named in the re-platform brief but not confirmed this pass. If PRD-016 does not carry the hexagonal mandate, cite ADR-090 alone. |
| ADR-135/136 (kept), PRD-027 | ADR-050 pod-backed-kgnode | `DreamLab-AI/VisionClaw` | **Verified** | Repo-qualified to disambiguate from agentbox ADR-050. |
| README, ADR-137, PRD-027 | ADR-051 loom client + deferred distillation | `DreamLab-AI/agentbox` | **Verified** | Vendored copy exists in-repo (being de-vendored to a stub); canonical lives in agentbox. |
| ADR-137, PRD-027, LEDGER | ADR-112 one-brain / no hot-path LLM | `DreamLab-AI/agentbox` | **Verify (repo-qualifier)** | Number verified; the *repo* attribution drifts (ADR-135 lists it under VisionClaw, agentbox ADR-051 depends on it). Pick one qualifier and use it consistently. |
| PRD-027 | ADR-013 URI grammar; ADR-049 bitemporal facts | `DreamLab-AI/agentbox` | **Verified** | Grounded in ADR-135/PRD-025 relates-lists. |
| PRD-026 (kept), PRD-027 | ADR-344 kg index / hybridRetrieve | `ruflo` | **Verified (as Proposed, flag-off)** | `CLAUDE_FLOW_KG_ENABLED` default off, gated on an unrun bench — cite with that status, never as shipped. |
| ADR-136 (kept), ADR-137 | mincut / pi-shared-web-memory; gnn-rerank nightly rerank | `ruvnet/ruvector` | **By-capability** | ADR numbers **unverified** — cite the capability, not a number (per ADR-136's own note). Both **deferred**: adopt only if shipped AND beats the bench. |
| ADR-136 (kept) | GRAPH-ANALYTICS-PROOF.md (@ruvector/graph-node v2.0.4 audit) | `metaharness` | **Verified (path)** | The "Cypher is label-scan-only" evidence; keep the file-path citation. |
| ONTOLOGY-LOOM-PIPELINE (rewrite), PRD-027, README | `pipeline/`, `publish.yml`, `enrich-gate.yml` (canonical builder + CI gate) | `jjohare/logseq` | **Verified** | The corpus builder + CI-enforced admission gate. The Loom mirrors its output; authority stays upstream. |
| README, ecosystem diagram | knowledgeGraph (corpus + pipeline + explorer); VisionClaw (reasoner); dream-engine (nightly) | `DreamLab-AI/*` | **Verified** | Existing README links; keep. |
| ADR-137 (style) | `ruvector`, `solid-pod-rs`, `nostr-rust-forum`, `logseq-publisher-rust` | sibling Rust repos | **Verified (named by brief)** | Style exemplars (tokio workspace, resolver=2, deny-unsafe, thin-LTO release). Not decision authorities. |

**Rule for the authoring phase:** every **Verify** row must be confirmed against the target repo before the doc citing it is merged. Any that cannot be confirmed drops to **By-capability** (cite the capability, not the number) rather than shipping an invented number.

---

## 6. Docs index / navigation map (`docs/README.md`, to CREATE)

The index states the reading order and the authority chain. Skeleton:

```
Loom documentation map
======================

START HERE
  README.md ................. what Loom is, the measured result, how to deploy

GOVERNANCE (authority chain — read top-down)
  design/PRD-025 ............ product capstone (the loop)
  design/PRD-026 ............ consolidation requirements (single-source, fallback, admission)
  design/PRD-027 ............ Rust re-engineering requirements + WS build order        [new]
  design/ADR-135 ........... keystone: node boundary, façade, generation discipline
  design/ADR-136 ........... tooling allocation: RuVector behind the markdown
  design/ADR-137 ........... Rust re-platform + both compose profiles                  [new]
  design/ddd-ontology-loom-context.md .. bounded-context model + THE PRIZE (source-of-record)
  design/LOOM-POSITIONING.md ........... product framing, the multivariate bar
  design/ONTOLOGY-LOOM-PIPELINE.md ..... generation-identity contract (mirror consumes)

OPERATIONS
  QWEN3.8-CONNECTION.md .... the model behind the façade today
  REMOTE-CLIENT-SETUP.md ... connect a LAN machine (Profile A/B endpoints)
  tools/ingest/README.md ... ontology-corpus ingestion (build/off-turn write channel)  [new]
  dream-cycle/LEDGER.md .... nightly dream-cycle ledger

EVIDENCE
  research/ontology-uplift-report.pdf ... flagship two-study report
  research/report.md, report-gemini-3.7-flash.md ... per-run reports
  bench/UPLIFT-BENCH-PROTOCOL.md ......... reproduce it (+ WS-O multivariate gate)

HISTORICAL (frozen — do not extend)
  legacy/** ............. retired model-connection docs, old benches
  research/MODEL-BENCHMARKS.md (moved) ... pre-Qwen model quality bench
  ONTOLOGY-UPLIFT-PLAN.md (moved) ........ 2026-08 HP toolkit guide

EXTERNAL (canonical elsewhere — we link, never copy)
  agentbox ADR-051 ...... harness-side loom client + deferred distillation
  VisionClaw ADR-099/090  Whelk-rs reasoner / hexagonal ring
  ruvector ADR-001/0027, ADR-047 .. HNSW production index / ProofGate
  jjohare/logseq ........ the corpus builder + CI gate
```

The index also carries the **one-number-one-decision** note (mirroring the VisionClaw convention): each ADR/PRD number names exactly one decision; cross-repo numbers are always repo-qualified.

---

## 7. Consistency checklist (implementation phase must pass)

A doc change is not done until all apply. This is CI-checkable where marked `[CI]`.

**Terminology (one canonical term each):**

| Use | Not | Note |
|---|---|---|
| CanonicalUnit / per-IRI markdown-with-ontology block | "record", "chunk", "document" | the served unit; ddd is the source-of-record term |
| confidence-gated selective injection | "blanket grounding", "RAG" | the shipped policy |
| HNSW semantic fallback | "vector search", "embedding retrieval" | default-off, benchmark-gated |
| model-is-a-URL / `DISTILL_BACKEND_URL` | "the LLM", "the backend model" (as if baked) | the swap seam |
| Profile A (host-colocated) / Profile B (sidecar) | "Deployment A/B" is acceptable as legacy alias; prefer "Profile" | two-profile deploy |
| generation (sha-addressable) | "version", "snapshot" loosely | ADR-135 discipline |
| serving mirror | "builder" | the Loom does not build the corpus |

**The Prize (P1):** `[CI]` the canonical wording appears **byte-identically** in every doc that quotes it, and every quote is traceable to the ddd source-of-record. No doc paraphrases it. `[CI]` grep the exact opening clause across `docs/**` and assert a single normalised form.

**Shipped-vs-aspirational (P2):** `[CI]` the honesty table is byte-identical across ADR-136, PRD-026, PRD-027, ADR-137, ddd. No prose in any doc contradicts a table cell (e.g. no doc calls the HNSW fallback or the Rust binary "shipped").

**Generation discipline (P5):** every doc showing `/health`, `/loom/generation`, or a `loom` response block shows the generation stamp and states atomicity + A≡B parity. `[CI]` health-example snippets include a `generation` field.

**Cross-link integrity:** `[CI]` no vendored full copies of external ADRs remain under `docs/` (grep for a second `# ADR-0` H1 whose number belongs to another repo); every external reference is repo-qualified; every §5 **Verify** row is confirmed or downgraded to by-capability; the HNSW production index is cited as RuVector **ADR-001**, never ADR-004 (which is KV Cache Management).

**Rust-substrate honesty (P3):** no doc presents the rewrite as new capability; the endpoint set, injection policy, served unit and swap seam are described as *preserved*. The one honest trade (ADR-135 D1 stdlib-portability) is stated wherever the Rust choice is justified, never hidden.

**Doc→code parity:** `tests/test_confidence_injection.py` has a named Rust equivalent in the PRD-027 WS list; `app/mirror.sh` behaviour (atomic generation-verified mirror) is documented as ported, not dropped; the dropped files (`ontology_proxy.py`, `test_proxy.py`, `app/pipeline/*`) are referenced by no surviving doc.

---

## 8. Sequencing

The doc work has a strict order because authority flows downhill:

1. **ddd Rust revision** first — it is the source-of-record for THE PRIZE and the crate mapping every other doc references.
2. **ADR-137** — the decision the rest cite.
3. **PRD-027** — requirements, references ADR-137 + ddd.
4. **README rewrite** + **REMOTE-CLIENT-SETUP / QWEN3.8 edits** — consumer-facing, cite the above.
5. **ONTOLOGY-LOOM-PIPELINE rewrite** + **tools/ingest/README create** + **bench protocol extend** — the operational contracts.
6. **De-vendor agentbox ADR-051 stub**; **retire** ONTOLOGY-UPLIFT-PLAN + MODEL-BENCHMARKS into `legacy/`.
7. **docs/README.md index** last — it can only be accurate once the moves above are done.
8. **Back-reference edits** to ADR-135/136, PRD-025/026 (one line each) — trivial, do alongside step 2–3.

Steps 1–3 gate everything; 4–6 parallelise; 7 closes.

---

## 9. Open questions / flags for the authoring workstream

- **VisionClaw PRD-016** (§5) — confirm it carries the hexagonal mandate, else cite ADR-090 alone.
- **RuVector HNSW ADR number** — *resolved*: ADR-004 is KV Cache Management; the HNSW production index is **ADR-001**. Correct the ADR-004→ADR-001 citation everywhere it appears (this set, plus the ADR-136/PRD-026 carry-over) when those docs are next touched. Do not re-open as a hedge.
- **ADR-112 repo attribution** — pick VisionClaw *or* agentbox as the canonical home and qualify it consistently across the set; it currently drifts.
- **mincut / gnn-rerank ADR numbers** — remain **by-capability** unless the workstream fetches confirmed numbers; do not invent.
- **`logseq-publisher-rust`** — confirm whether the Rust publisher is in-scope as the mirror source or purely a style exemplar; the ONTOLOGY-LOOM-PIPELINE rewrite depends on which repo owns the published generation the Rust mirror pulls.

---

*This plan authors no design decision. It schedules the documentation changes ADR-137 and PRD-027 require, fixes the cross-repo link map, and states the checklist that keeps the set coherent. THE PRIZE — the per-IRI human-scrutible markdown-with-ontology block as the one served, reviewable unit — is untouched by every change above; the docs change their substrate description, not the thing they describe.*
