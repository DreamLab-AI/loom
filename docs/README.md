# Loom documentation map

The reading order and the authority chain. Each ADR/PRD number names exactly **one** decision;
every cross-repo number is **repo-qualified** (the one-number-one-decision convention). Where a
design doc and the implementation diverged, the docs carry an **`Erratum (2026-08-17)`** note
with the ground truth — reality wins, but history is annotated, not rewritten.

## START HERE

- **[`../README.md`](../README.md)** — what Loom is, the measured result, how to build & deploy,
  and the honest shipped / gated-off / operational-tail split.

## GOVERNANCE (authority chain — read top-down)

```
design/PRD-025 ............ product capstone (the ontology-loom + connector platform)
design/PRD-026 ............ consolidation requirements (single-source build, fallback, admission)
design/PRD-027 ............ Rust re-engineering requirements + acceptance criteria (§10.1 status)
design/PRD-028 ............ does Loom earn its complexity on genuinely private knowledge? (preregistration spec; pilot first; proposed — arms extended by ADR-140 D8)
design/ADR-135 ........... keystone: node boundary, model-is-a-URL, generation discipline, A/B
design/ADR-136 ........... tooling allocation: RuVector behind the markdown, keep oxigraph SPARQL
design/ADR-137 ........... Rust re-platform + both compose profiles   [Accepted + Implemented]
design/ADR-138 ........... confidence-surfacing / grounding contract         [Accepted]
design/ADR-139 ........... per-request scaffold opt-out (the plain-proxy door) [Accepted]
design/ADR-140 ........... Loom v2: agentic MCP plane beside the injection plane; Mapping/Evidence; governed evolution  [Proposed; D1+D4 amended by ADR-141]
design/ADR-141 ........... Loom consumes the vault build: generation = visionGraph@sha; OKF vocabulary; /loom/attest ledger; stdio dropped  [Proposed]
design/RUST-ARCHITECTURE.md  the buildable blueprint (crates, ports, fusion, deploy) + errata
design/ddd-ontology-loom-context.md  bounded context + THE PRIZE (source-of-record)
design/LOOM-POSITIONING.md  product framing, the multivariate bar
design/ONTOLOGY-LOOM-PIPELINE.md  generation-identity contract the mirror consumes (amended 2026-09-22: the two commit-marker shapes, local wasDerivedFrom)
design/agentbox-ADR-051-*  harness-side client (de-vendored stub → canonical in agentbox)
design/DOC-REENGINEERING-PLAN.md  the plan this doc set was re-engineered against  [executed]
```

**Authority flow:** PRD-025 (goal) → PRD-026 (consolidation) → ADR-135 (keystone) → ADR-136
(tools) → ADR-137 / PRD-027 (Rust substrate) → RUST-ARCHITECTURE (build) → DDD (model). ADR-137
**extends** ADR-135/136 and supersedes only ADR-135 D1's stdlib-Python *implementation* choice.

## OPERATIONS

```
QWEN3.8-CONNECTION.md .... the model behind the façade today (Qwen3.8-27B on :8085)
REMOTE-CLIENT-SETUP.md ... connect a LAN machine (Profile A/B endpoints)
published-generation-reload.md  verify-then-reload a promoted bundle; the timer's precondition + enable commands
../tools/ingest/README.md  ontology-corpus ingestion (build/off-turn write channel) + HNSW law
../justfile .............. build/test/ci recipes + docker-run-a/b (deploy per ../deploy/)
dream-cycle/LEDGER.md .... nightly dream-cycle ledger
```

## EVIDENCE

```
research/README.md ..................... the evidence index (paper, data, harness, precursors)
research/gain-over-copy-paper/ ......... PRIMARY evidence: the copy-ceiling paper (current build + archive/ of every earlier version)
uplift-results/paper-v2/ ............... the paper's per-row data + exposure/recovery decomposition
../tools/paper/ ........................ the paper's harness (live/control/judge/decompose)
research/companion-routing/ ............ the routing-seam companion note (gain over a judge-free ranker)
research/ontology-uplift-report.pdf .... earlier two-study report (precursor; LaTeX beside the PDF)
research/report.md, report-gemini-3.7-flash.md ... per-run uplift reports (frozen)
../bench/UPLIFT-BENCH-PROTOCOL.md ...... reproduce it (+ the WS-O multivariate fusion gate)
../.claude/evidence/ ................... per-expectation executed evidence
../.claude/evidence/AUDIT-gpt54.md ..... the gpt-5.4 adversarial audit + the five remediations
../tests/golden-python/ ................ frozen byte-parity goldens (the Rust port's oracle)
```

> **`research/gain-over-copy-paper/archive/` is frozen**: every earlier manuscript version (v2 preprint
> through v8), its reviews and ledgers, kept for provenance. Link to them, never edit them.

## HISTORICAL (frozen — do not extend)

```
../legacy/** ........................... retired model-connection docs + old benches
../legacy/ONTOLOGY-UPLIFT-PLAN.md ...... 2026-08 HP toolkit guide (moved 2026-08-17)
../legacy/MODEL-BENCHMARKS.md .......... pre-Qwen model-quality bench (moved 2026-08-17)
```

## EXTERNAL (canonical elsewhere — we link, never copy)

```
agentbox ADR-051 ....... harness-side loom client + deferred distillation
VisionClaw ADR-099 ..... Whelk-rs EL reasoner (build-time authority)
VisionClaw ADR-090 ..... hexagonal acyclic crate ring (the modularisation law)
VisionClaw ADR-135/136/137  the Loom design capstone (mirrored into design/ here)
RuVector ADR-001 ....... HNSW production index (the in-process read behind the markdown)
RuVector ADR-047 ....... ProofGate<T> / MutationLedger  (attestation design target — see the
                          Erratum in RUST-ARCHITECTURE §11.5 for where they actually live)
vault build ............ the canonical corpus builder over visionGraph (ADR-141; replaced jjohare/logseq's publish.yml)
```

## Conventions this set enforces

- **THE PRIZE is stated once canonically** (`ddd-ontology-loom-context.md`) and quoted verbatim
  elsewhere — never paraphrased.
- **Shipped ≠ aspirational, everywhere.** The honesty tables are the single source of truth for
  the split; no prose contradicts a table cell. Post-implementation the split reads
  *built + audited* vs *gated-off (semantic fusion, recall RED)* vs *operational-tail (cutover)*.
- **No fourth copy.** External ADRs are cross-linked in their owning repo, never re-vendored
  (the agentbox ADR-051 full copy was replaced by a stub, 2026-08-17).
- **Generation discipline.** Every `/health` / `/loom/generation` example shows the sha-addressable
  generation stamp; a generation is fully present or absent, byte-identical across Profile A and B.
