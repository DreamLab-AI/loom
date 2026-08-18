---
id: PRD-027
title: "Rust Re-engineering of the Ontology Loom Serving Node: substrate re-platform (axum/tokio, hexagonal crates, oxigraph-native, in-process HNSW), semantic-fallback wiring, and a two-profile deployment"
status: proposed
date: 2026-08-17
authors: VisionFlow operator (did:nostr:jjohare) + opus re-platform mesh
linked_adrs: [
  ADR-137 (Rust re-platform + two-profile deployment — the decision record this PRD operationalises),
  ADR-136 (loom tooling allocation — RuVector behind the markdown, pyoxigraph stays, gate re-platformed),
  ADR-135 (loom node boundary + model-swappable façade + deferred distillation),
  RuVector ADR-001 (ruvector-core architecture — vector database core with HNSW indexing),
  RuVector ADR-001 (hnsw-parameterized-query-fix),
  RuVector ADR-047 (ProofGate<T> / MutationLedger — proof-gated mutation),
  VisionClaw ADR-099 (Whelk-rs EL++ reasoner authority),
  VisionClaw ADR-090 (hexagonal crate ring),
  ruflo ADR-344 (KG index for ReasoningBank — Proposed, feature-flag-off, deferred)
]
linked_prd: [PRD-026 (Loom Consolidation — the layer beneath the serving unit), PRD-025 (Ontology Loom & Connector Platform — the capstone)]
linked_ddd: docs/design/ddd-ontology-loom-context.md
relates: [
  PRD-020 (pervasive ontology augmentation),
  VisionClaw PRD-016 (hexagonal crate modularisation),
  VisionClaw PRD-022 (semantic-trust-layer),
  ADR-112 (retrieval spine / one brain / no hot-path LLM),
  ADR-116 (tiered token budgets),
  ADR-119 (verifiable liveness telemetry),
  ADR-125 (did:nostr multikey),
  agentbox ADR-051 (harness-side loom-client — proposed),
  agentbox PRD-022 (semantic-integrity-provenance-decisions)
]
supersedes: "Extends PRD-025 and PRD-026; supersedes nothing wholesale. Supersedes the implicit assumption in ADR-135 D1 that the façade's stdlib-Python substrate is the end-state artifact — the substrate re-platforms to a single Rust binary while every ADR-135/136 governing decision (model-is-a-URL, LLM-free/network-free hot path, single-source build, build-time-only reasoning, published-ontology-only serving, RuVector strictly behind the markdown) is honoured, not relitigated. Resolves ADR-135 D1-a (reference deployment) to BOTH compose profiles via ADR-137."

---

# PRD-027 — Rust Re-engineering of the Ontology Loom Serving Node

**Status:** Proposed (design + workstream plan; grounded in the 2026-08 re-platform mesh and the newly-landed ontology-corpus RuVector namespace)
**Date:** 2026-08-17
**Owner:** VisionFlow operator (Dr J. O'Hare, did:nostr:jjohare)
**Decision authority:** [ADR-137](./ADR-137-loom-rust-replatform.md): this PRD cites ADR-137 for every substrate and deployment decision and does not re-derive them.
**Builds on:** [PRD-026](./PRD-026-loom-consolidation.md) (single-source build, semantic fallback, enforced admission control) and [PRD-025](./PRD-025-ontology-loom-and-connector-platform.md) (the serving + distillation loop). PRD-027 re-platforms the *substrate under* those two; it does not restate their product goal or their consolidation requirements.
**Audience:** VisionFlow engineering.

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
> propose→consistency-check→human-PR-merge gate — is the real, non-eroding moat.

**The one thing this PRD must not do:** the re-platform is a *substrate* change, not a feature change. The served unit is byte-for-byte the same human-scrutible markdown-with-ontology block before and after. If any decision here reduces the legibility of *that unit*, it is a regression to be reversed — not a trade to be accepted. The legibility we are allowed to trade (§2) is the façade's *own source code*, never the data a human reviews.

---

> **EXECUTION NOTE (read first).** This is a **design + workstream plan**, not a code change. It writes no Rust and deletes no Python by itself; implementation is the WS-R…WS-Y build order in §12, each gated by the evidence bars in §10. This is a **dev/test estate**, so we build the target end-state directly (a single Rust binary, no dual-language transitional shim) rather than staging a live migration. Every requirement below is testable, and every capability is tagged **shipped** (in the Python Loom today) or **target** (this re-platform) — the §3 honesty table is the single source of truth for that split and no other section may contradict it.

> **Citation discipline (mandatory — inherited from PRD-025/026).** Two `PRD-022`s and two `ADR-050`s exist across repos. Every cross-repo citation is repo-qualified: **VisionClaw PRD-022** = *semantic-trust-layer*; **agentbox PRD-022** = *semantic-integrity-provenance-decisions*. **VisionClaw ADR-050** = *pod-backed-kgnode-schema*; **agentbox ADR-050** = *decision-elevation-inverse-corpus-path*. RuVector and ruflo ADRs are always repo-prefixed (**RuVector ADR-001/ADR-047**, **ruflo ADR-344**). ADR-090/099/112/135/136 without a repo prefix are VisionClaw's. An unqualified `PRD-022`, `ADR-050`, or a bare `ADR-001/0027/047/344` is a defect.

---

## 1. Problem & vision

### 1.1 Lead with the Prize

For any entity in the private corpus a human pulls up *one* markdown block — the curated `dfull` prose headed by its typed ontology relations, stamped `corpusNature: synthetic-ai-generated-human-directed` — and reads, reviews and audits exactly what the system knows and will serve about that entity. That block is the deliverable. It is what makes a grounded answer *attributable* rather than merely plausible. Everything the Loom does is in service of finding, ranking, attesting, and serving that block and resolving back to its IRI.

PRD-025 built the serving and distillation loop around that unit. PRD-026 closed the three gaps in the layer beneath it (the three-copy build, the OOV/paraphrase retrieval gap, the unenforced write gate). **PRD-027 re-platforms the substrate those two run on** — from the deliberately-stdlib Python serving mirror (`app/loom_facade.py`, `app/loom_graph.py`, `app/ontology_scaffold.py`) to a single Rust binary — and in doing so makes the accelerator-behind-the-markdown boundary *structural* rather than *conventional*: the hexagonal crate ring (§5) enforces by construction what today is enforced only by discipline.

### 1.2 Why now — the substrate has three pressures converging

The re-platform is not a rewrite-for-its-own-sake. Three independent forces make Rust the honest next substrate, and they arrive together:

**(a) The new ground truth: the ontology-corpus RuVector namespace has landed.** 8,146 concept classes are embedded (Xinference `bge-small-en-v1.5`/384-dim, LOCKED per the ops embedding-law), generation-stamped, IRI-keyed (`urn:ngm:class:<slug>`), `source_type=loom`, in `ruvector-postgres` under the cosine HNSW index (`idx_memory_embedding_hnsw`, `m=16`, `ef_construction=128`). Spot-checked recall via `hnsw-xinference` (post-ingest, live): rgb-protocol 0.87, single-use-seals 0.82, sovereign-keyset 0.75 vs decoys ~0.45 — a spot-check across several concepts, **not** a formal recall-gate (band self≥175/200); that gate run is pending. This is the semantic fallback ADR-136 D3 *named but did not build* — the fix for the lexical matcher's structural OOV/paraphrase miss. Wiring it in (PRD-026 WS-M / this PRD FR-3) as an *in-process* read rather than a network/MCP round-trip is exactly the kind of thing that is clean in Rust (`ruvector-core` embedded in-process) and awkward in stdlib Python (an out-of-process MCP call on the hot path, which ADR-136 §3 and the DDD §6.1 both forbid).

**(b) pyoxigraph is a foreign-language binding wrapping a native-Rust store.** `app/loom_graph.py` is a Python wrapper (188 lines) over pyoxigraph, which is itself the Python binding to oxigraph — a native Rust SPARQL store. In a Rust Loom, `loom_graph.py`'s wrapper *collapses into a direct, typed `oxigraph` crate dependency*. This is the cleanest win in the whole re-platform: we stop paying for a language boundary that only exists because the façade was Python.

**(c) The ecosystem substrate is Rust, and the Loom is the odd node out.** The sibling repos the Loom must live alongside — `ruvector`, `solid-pod-rs`, `nostr-rust-forum`, `logseq-publisher-rust` — are all tokio workspaces (resolver = 2, deny-unsafe, thin-LTO release). VisionClaw ADR-090 / PRD-016 define the hexagonal crate ring the ecosystem builds to. The Python Loom cannot share a crate, a contract type, a ProofGate, or a `did:nostr` provenance primitive with any of them; it re-implements or shells out. Re-platforming folds the Loom into the ring.

### 1.3 What does *not* change (the invariants carried forward, not relitigated)

The re-platform is subordinate to every governing decision already made. It **honours, does not reopen**:

- **Model-is-a-URL (ADR-135 D1.2).** `DISTILL_BACKEND_URL` stays a single config line; the model (Qwen3.8-27B today, `loom-model` on `:8085`) swaps behind the façade with zero consumer change; model identity rides in results, never in the endpoint.
- **LLM-free and network-free augmentation hot path (ADR-112, ADR-136 §3).** Lexical match + in-process HNSW + in-context oxigraph SPARQL touch no model and no network. Only `/v1/chat/completions` delegation and the deferred `/loom/distill` (PRD-025) touch a model.
- **Single source of truth / no fourth copy (ADR-136 D4).** ttl + scaffold + prose + HNSW are all generation-stamped projections of one build source; re-embed-on-promote is delta-diffed, honouring the HNSW index-law.
- **Build-time-only reasoning (ADR-136 D6).** Whelk-rs is the closure authority at build/CI time; the Loom serves the pre-reasoned snapshot and never runs a reasoner at query time.
- **Published-ontology-only serving (DDD BC24 I11).** The Loom serves `ontology.ttl` + `ontology-inferred.ttl`, never the working graph.
- **The confidence gate is the sole injection authority (ADR-136 D3).** HNSW is a candidate source feeding the existing `STRONG_MATCH_SCORE`/`MIN_INJECT_SCORE`/`MIN_INJECT_FRACTION` policy, never a bypass; fusion is default-off until it beats the lexical baseline on the multivariate bench.

The Rust rewrite changes the *substrate under* these invariants. It changes none of them.

---

## 2. Why Rust — with the honest portability trade against ADR-135 D1

### 2.1 The decision

**Framework: axum + tower on the tokio runtime**, with **oxigraph as a direct crate dependency** (replacing the pyoxigraph binding) and **`ruvector-core` HNSW embedded in-process** (via its default-on `hnsw` + `storage` features). This is the ecosystem norm (VisionClaw ADR-090 hexagonal ring; the four sibling Rust repos are all tokio workspaces, resolver = 2, deny-unsafe, thin-LTO release). The artifact is a **single static musl binary** (a few MB, no interpreter, no wheel), built with Nix (the agentbox pattern), shipped in two compose profiles (§8).

### 2.2 The honest technical wins (all on the substrate axis)

1. **Two foreign-language bindings stop being foreign-language bindings.** oxigraph is native Rust — `loom_graph.py`'s pyoxigraph wrapper collapses into a direct, typed store. The HNSW fallback becomes an in-process `ruvector-core` read, not a network/MCP round-trip (ADR-136 §3), which is what keeps the hot path network-free *by construction* rather than by careful avoidance.
2. **A strictly more portable artifact.** A single static musl binary is *more* portable than `python:3.12-slim` + a pyoxigraph wheel + an rdflib stack: no interpreter to provision, no wheel to resolve, faster cold start, no runtime dependency surface. This is a *better-delivered version of the same goal ADR-135 D1 optimised for*.
3. **The accelerator boundary becomes structural.** The hexagonal crate ring (§5) makes it a *compile error* for an adapter to leak its index shape into a served unit — `loom-domain` has no dependency on any adapter crate, and every port method resolves to an IRI addressing a `CanonicalUnit`. Today that boundary is a convention a careful author maintains; in Rust it is enforced by the dependency graph.
4. **Ecosystem convergence.** Shared contract types, a shared `did:nostr` + NIP-98 provenance primitive, the RuVector `ProofGate<T>`/`MutationLedger` types (RuVector ADR-047) as a direct dependency rather than a re-implementation, and a Nix build that matches agentbox.

### 2.3 The honest trade — what ADR-135 D1 deliberately chose, and what we give up

ADR-135 D1 chose stdlib `http.server` **on purpose**. The rationale was zero-toolchain portability and radical legibility: *"the model is a URL, the façade runs anywhere Python 3.10+ runs,"* and any operator could read all 260 lines of `loom_facade.py` end to end with no build step. That is a real property and the re-platform gives part of it up. Stated plainly, with no varnish:

| ADR-135 D1 optimised for | Rust re-platform delivers |
|---|---|
| Zero-toolchain portability of the façade | **Improved** — a static binary is *more* portable than interpreter + wheel, but it now requires a compile / cross-compile step and a Nix build to *produce* (slower edit-run iteration; not editable in place on the host) |
| Radical legibility of the façade **source** (260 lines any operator can skim) | **Given up** — the façade source is no longer a single skimmable file a non-Rust operator can read; it is a typed multi-crate workspace |
| The model-swap seam being a single config line | **Unchanged** — `DISTILL_BACKEND_URL` stays one line |
| Legibility of the **served data** (the Prize) | **Unchanged** — the served unit is the same markdown block; §10 AC-1 makes this a hard acceptance test |

**Net verdict: worth it.** The durable value (oxigraph-native SPARQL, in-process HNSW, one deployable binary, hexagonal enforcement of the accelerator boundary, ecosystem convergence) all lands on the substrate axis. The one thing ADR-135 D1 optimised for — portability of the model-swappable façade — is *improved*, not lost. The legibility we trade away is the façade's *own source*, which is not the thing a human reviews; the thing a human reviews is the markdown, and it is untouched. The one honest cost we accept and name: an operator can no longer skim the façade source without a Rust toolchain, and edit-run iteration on the façade is slower. We judge that cost acceptable because the façade contract is now stable (ADR-135 has stood; the endpoints are frozen) — the phase where reading and editing the façade source constantly mattered is behind us.

### 2.4 Alternative substrates considered and rejected

- **Stay on stdlib Python (status quo).** Rejected: it forces the in-process HNSW read (FR-3) to be an out-of-process MCP call on the hot path — which ADR-136 §3 and DDD §6.1 forbid — or leaves the semantic fallback unbuilt. It also keeps pyoxigraph as a foreign binding and keeps the Loom outside the ecosystem crate ring. The one thing it preserves (façade-source legibility) is worth less now that the contract is frozen.
- **Python + a native extension (PyO3 for oxigraph/HNSW).** Rejected: this is the worst of both — it *adds* a Rust toolchain and a build step (losing D1's zero-toolchain property) *without* delivering the single-binary artifact, the hexagonal enforcement, or ecosystem crate-sharing. If we are paying the Rust-toolchain cost, we take the whole win.
- **Go.** Rejected: no native oxigraph, no `ruvector-core`, no shared contract types with the four sibling Rust repos or VisionClaw's ADR-090 ring. The entire technical case is *convergence with an existing Rust ecosystem*; Go re-opens both foreign-binding problems.

---

## 3. Shipped-vs-target honesty table (mandatory; the single source of truth for the split)

No section of this PRD may contradict this table. "Shipped" = live in the Python Loom today. "Target" = this re-platform delivers it. This table extends (does not restate) the ADR-136 §1 / PRD-026 §3 / DDD §1 honesty table for the *substrate* axis.

| Capability | Status today (Python Loom) | This PRD's target (Rust Loom) |
|---|---|---|
| Per-IRI markdown-with-ontology unit as served canonical | **Shipped** | keep byte-identical (the Prize) |
| Model-swappable `/v1` façade (Qwen3.8-27B behind it) | **Shipped** (`loom_facade.py`) | port to axum, contract unchanged |
| `/health`, `/loom/generation`, `/loom/scaffold` (retrieval, no LLM), `/v1/chat/completions`, `/v1/models` | **Shipped** | port, contract-preserved (§6.1) |
| `/loom/sparql`, `/loom/search` | **Shipped** (undocumented in ADR-135 D1.1; live in `loom_facade.py`) | port; formalise the clamp (FR-5) |
| Lexical inverted-index matcher (<50 ms, 8,146 titles) | **Shipped** (`ontology_scaffold.py`) | port exactly to `loom-scaffold` |
| Confidence-gated selective-injection policy | **Shipped** (`STRONG_MATCH_SCORE`, `MIN_INJECT_SCORE`, `MIN_INJECT_FRACTION=0.4`, budget taper) | port exactly; same constants, same behaviour |
| pyoxigraph SPARQL over the reasoned closure | **Shipped** (`loom_graph.py` binding) | **replace with native `oxigraph` crate** (`loom-graph-oxigraph`) |
| HNSW semantic fallback for OOV/paraphrase | **Not built** (silent no-injection below `MIN_INJECT_SCORE`) | build as in-process `ruvector-core` read, benchmark-gated, default-OFF (FR-3) |
| ontology-corpus RuVector namespace (8,146 IRI-keyed embeddings) | **Landed** (the new ground truth; not yet wired to the Loom) | wire in as the fallback candidate source (FR-3) |
| Atomic generation-verified mirror (`mirror.sh`, ADR-136 D4) | **Shipped** (shell) | port to `loom-facade` (Rust); same atomic discipline |
| Single static binary artifact | **Not built** (interpreter + wheel image) | ship (musl static, Nix build) |
| Hexagonal crate boundary enforcing accelerator-behind-markdown | **Not built** (convention only) | ship (`loom-domain` has no adapter deps; FR-6) |
| ProofGate/MutationLedger attestation (RuVector ADR-047) | **Not built** (unattested Python `CheckResult`) | adapter stub in `loom-attest-proofgate`, build/CI-time (PRD-026 owns enforcement) |
| Two compose profiles both live | **Not built** (Deployment A running; B specified, not running) | ship both (§8, ADR-137) |
| Vendored `app/pipeline/*` corpus builder | **Present but out-of-scope** (vendored copy of logseq/pipeline) | **dropped** (#21; canonical builder stays jjohare/logseq) |
| `app/ontology_proxy.py` (524-line legacy proxy) | **Present, legacy** | **dropped** |
| Whelk EL++ at Loom **query** time | **Not run** (pre-reasoned snapshot) | remains not-run (build-time only, non-goal) |
| Multi-agent coordination substrate / mesh | **Aspirational** (only live consumer = email gateway single-LLM RAG) | remains deferred (WS-Q class; non-goal) |

---

## 4. Consumers — who binds to the façade and what they need

The façade contract is the *only* thing consumers bind to. The re-platform must be invisible to every one of them: same endpoints, same wire shapes, same generation descriptor. The consumers, and the specific obligation each places on the Rust Loom:

### 4.1 agentbox loom client (agentbox ADR-051 — proposed)

agentbox ADR-051 owns the **harness side** of the capstone: agentbox as a *client* of the Loom façade, the consumer-side deferred-distillation MCP tools (`ontology_distill_submit`/`fetch`/`await`, ADR-135 D4.7), and the beads adapter that makes a distillation job a durable, fenced, content-addressed work item. ADR-051's review-trigger is explicit: *"the Loom façade contract (VisionClaw ADR-135) changes its generation/index shape."* **Obligation on this PRD:** the re-platform must NOT change the generation descriptor shape or the `/loom/*` and `/v1/*` wire shapes ADR-051 consumes. Where ADR-051 names a wire shape, PRD-027 preserves it. If the descriptor shape must evolve (e.g. to carry the HNSW artifact sha), it is an additive field, versioned, and ADR-051's review-trigger fires — see AC-2.

### 4.2 email gateway (the one live production consumer)

The email gateway binds `REASONER_BASE_URL=http://loom:8080/v1` — a **docker-network consumer** doing single-LLM RAG for private-email question-answering, and it is the email-privacy system's grounding path (content stays on-LAN; the Loom delegates only to a LAN/local model). This consumer is the single strongest reason Profile B (sidecar on `visionclaw_network`) is *required, not optional* (§8): the gateway must reach a Loom **on the docker network**, not one behind the HP DNAT. **Obligation:** Profile B must expose the identical `/v1/chat/completions` contract on `visionclaw_network`, and the `.48-is-dead` stale-route trap that black-holes gateway synthesis (documented in the email-search skill) must not be reintroduced by the re-platform — the model backend stays a single config line pointed at a live route.

### 4.3 the agent mesh (deferred; not a live consumer)

The "shared substrate for a mesh" is aspirational per ADR-136 D7 and DDD §9. The only confirmed live consumer today is the email gateway. **Obligation:** the re-platform ships no mesh-coordination surface (explicit non-goal, §9); *when* the mesh is built (WS-Q class), it must still resolve every claim to the same per-IRI markdown identity the façade serves now — the Rust `CanonicalUnit` aggregate root (§5) makes that resolution the type-level default.

---

## 5. Crate architecture — the hexagonal realisation

The Loom becomes a tokio workspace of eight crates on the VisionClaw ADR-090 acyclic ring (domain core → ports → adapters → thin facade binary). `loom-domain` is the pure core with no I/O and no framework dependency; every other crate depends inward toward it, never outward. This is the structural enforcement of the accelerator-behind-the-markdown boundary: an adapter physically *cannot* return anything but an IRI addressing a `CanonicalUnit`, because the port trait it implements says so and it has no dependency that would let it do otherwise.

| Crate | Responsibility |
|---|---|
| **`loom-domain`** | Pure hexagonal core. The `CanonicalUnit` aggregate root (IRI identity, `dfull`, typed ontology-relation header, `corpusNature`), the `CorpusGeneration` version boundary, and the port traits every adapter implements: `LexicalIndex`, `VectorIndex`, `EmbeddingProvider`, `GraphStore`, `ModelBackend`, `AttestationLedger`. No I/O, no framework deps. Encodes **Invariant I-P1**: every port method returns or resolves to an IRI that addresses a `CanonicalUnit`. |
| **`loom-scaffold`** | Exact port of `ontology_scaffold.py`: the lexical inverted-index matcher (<50 ms over 8,146 class titles) plus the confidence-gated selective-injection policy (`STRONG_MATCH_SCORE`, `MIN_INJECT_SCORE`, `MIN_INJECT_FRACTION`, budget taper, link→seed→expand→serialise). Pure domain logic; the single authority over WHICH units inject. The retrieval fusion feeds candidates into this gate; nothing bypasses it. |
| **`loom-graph-oxigraph`** | `GraphStore` adapter over the native-Rust **`oxigraph` crate** (replaces `loom_graph.py` + pyoxigraph). Loads `ontology.ttl` + `ontology-inferred.ttl` only (published-ontology-only, DDD BC24 I11), read-only + clamped SPARQL (SELECT/ASK/CONSTRUCT/DESCRIBE, `SERVICE` forbidden, `LIMIT` clamp). Fail-open: an absent store degrades to lexical and is reported in `/health`. |
| **`loom-vector-ruvector`** | `VectorIndex` adapter: in-process `ruvector-core` HNSW projection over the ontology-corpus namespace (8,146 IRI-keyed `bge-small`/384 records, cosine, `m=16`/`ef_construction=128`) for the query hot path (network-free); plus a build/off-turn *write* channel to `ruvector-postgres` via the MCP embedding pipeline (never the query path). Anti-corruption: rows carry the IRI as primary key so the index never leaks its shape back into a `CanonicalUnit`. |
| **`loom-embed-xinference`** | `EmbeddingProvider` adapter to Xinference `bge-small-en-v1.5`/384 (LOCKED per ops law) at `http://xinference:9997/v1/embeddings`. Two call sites: build-time embed-on-promote (delta-diffed touched IRIs only) and query-time OOV/paraphrase embed for the semantic-fallback gate. Not on the augmentation read path unless a lexical miss triggers it. |
| **`loom-backend-openai`** | `ModelBackend` adapter: the OpenAI-compatible `DISTILL_BACKEND_URL` client. Scaffold-injects the last user message then delegates `/v1/chat/completions` to the model (Qwen3.8-27B today), floors `max_tokens` for reasoning backends (≥1536; the `LOOM_MIN_MAX_TOKENS` behaviour ported verbatim), and stamps model identity + generation into results. The model-swap seam; model identity rides in results, never the endpoint (ADR-135 D1.2). |
| **`loom-attest-proofgate`** | `AttestationLedger` adapter re-platforming the gate verdict onto RuVector `ProofGate<T>`/`MutationLedger` (RuVector ADR-047): domain predicates stay Loom-owned (in `loom-domain`); their attestation becomes chain-hashed tamper-evident ledger entries. Build/CI-time; not on the serving hot path. **Enforcement is PRD-026's gate**, not this PRD's — PRD-027 delivers the adapter surface. |
| **`loom-facade`** | Thin axum/tower binary — the composition root. Wires ports to adapters; serves `/health`, `/loom/generation`, `/loom/scaffold`, `/loom/sparql`, `/loom/search`, `/v1/chat/completions`, `/v1/models`; owns the atomic generation-verified mirror (ADR-136 D4, porting `mirror.sh`) and the two deployment profiles. Contains no domain logic; every decision lives in a domain port. |

Workspace conventions match the sibling Rust repos: `resolver = 2`, `#![forbid(unsafe_code)]` in every crate, `lto = "thin"` release profile, `cargo test --all-features` + `clippy` clean as a CI gate, Nix build (agentbox pattern). Typed contract structs (generation descriptor, scaffold response, result envelope) live either in `loom-domain` or a leaf `loom-contracts` crate so Profile A and Profile B share them byte-for-byte.

---

## 6. Functional requirements

### FR-1 — Preserve every façade endpoint, contract-identical

The Rust `loom-facade` serves the exact wire contract of `loom_facade.py`. No consumer changes.

| Endpoint | Contract obligation |
|---|---|
| `GET /health` | Returns `{ok, facet, mode, backend, backend_reachable, index_classes, graph:{available,triples,...}, generation}` — plus a new `vector` block reporting HNSW-projection availability (additive). |
| `GET /loom/generation` (alias `/generation`) | The generation descriptor, best-source-first: upstream `build-manifest.json` → mirror `.generation.json` (`verifiedSingleGeneration`) → scaffold-index stamp. Shape preserved for ADR-051. |
| `POST /loom/scaffold` (alias `/scaffold`) | Budget-clamped scaffold retrieval, **no LLM**. Body `{prompt, budget_tokens, max_seeds, hops, prose}` → `{scaffold, engaged, approx_tokens, prose, generation}`. Proves the retrieval facet with no backend. |
| `POST /v1/chat/completions` | Scaffold-inject the last user message → floor `max_tokens` (≥`LOOM_MIN_MAX_TOKENS`, default 1536; only ever raises) → delegate to `DISTILL_BACKEND_URL` → annotate result with `loom:{mode, injected_tokens, grounding, generation}`. |
| `GET /v1/models` (and other `/v1/*`) | Passthrough to the backend (identity probe). |
| `POST /loom/sparql`, `POST /loom/search` | Preserved from today's façade; SPARQL clamped per FR-5. |

`/loom/distill` (the deferred-distillation submit/collect surface, ADR-135 D4 / PRD-025) is **out of scope for this PRD's serving binary** and stays as specified in ADR-135/ADR-051 — PRD-027 does not re-home it, but the axum facade reserves the route and MUST NOT break its contract when it lands.

> **Addendum (2026-08-18).** The `/v1/chat/completions` row above describes unconditional delegation. Since the copy-ceiling measurement ([`docs/research/paper-v2/main.pdf`](../research/paper-v2/main.pdf)), the flow gained an opt-in `LOOM_VERBATIM_MODE`: on a gate-engaged high-confidence lookup it may return the canonical markdown block **with no backend call** (per-request opt-out `"loom_options":{"verbatim":false}`; multi-turn and streaming bypass). Two more default-off knobs landed: `LOOM_EXPOSURE_APPEND` (per-answer `exposure` telemetry in the `loom` block) and `LOOM_BACKEND_NO_THINK` / `LOOM_THINK_TOKEN_FLOOR` (thinking and budget control). Canonical table: `RUST-ARCHITECTURE.md` §10 addendum. Backend today is Qwen3.8-27B Heretic Q8_0 (see `../QWEN3.8-CONNECTION.md`).

### FR-2 — Lexical primary retrieval, ported exactly

`loom-scaffold` reproduces `ontology_scaffold.py` behaviour to the constant: `MIN_SEED_SCORE=2.0`, `EXACT_TITLE_WEIGHT=8.0`, `OVERLAP_WEIGHT=2.0`, `SUBSTRING_WEIGHT=0.75`, `STRONG_MATCH_SCORE` (=`EXACT_TITLE_WEIGHT` default), `MIN_INJECT_SCORE` (=`MIN_SEED_SCORE` default), `MIN_INJECT_FRACTION=0.4`, the budget taper `frac = min(1.0, max(MIN_INJECT_FRACTION, top_score/STRONG_MATCH_SCORE))`, and the skip-below-`MIN_INJECT_SCORE` rule. All constants stay env-overridable with identical names (`LOOM_STRONG_MATCH_SCORE`, `LOOM_MIN_INJECT_SCORE`, `LOOM_MIN_INJECT_FRACTION`). A golden-fixture test asserts the Rust matcher produces the same seed set and same injected block as the Python matcher over a corpus of representative prompts (§10 AC-3).

### FR-3 — Retrieval fusion: the ontology-corpus semantic fallback, gated

Fusion is a **candidate-union feeding one confidence gate**, not a blind RRF blend. The flow, exactly:

1. **Lexical primary.** `loom-scaffold`'s inverted-index matcher scores the query against 8,146 class titles, as today.
2. **Clear the gate → inject as now.** If the top score clears `MIN_INJECT_SCORE`, inject — no embedding call. The hot path stays LLM-free and network-free.
3. **Lexical miss → semantic fallback (ONLY here).** If, and only if, the top lexical score is below `MIN_INJECT_SCORE` (the OOV/paraphrase gap the matcher structurally misses), embed the query via `loom-embed-xinference` (`bge-small-en-v1.5`/384) and run ANN over the in-process `ruvector-core` HNSW projection of the ontology-corpus namespace (IRI-keyed, cosine; spot-checked recall (live post-ingest) rgb-protocol 0.87 (single-use-seals 0.82, sovereign-keyset 0.75 vs decoys ~0.45; formal recall-gate pending) vs decoys ~0.45).
4. **HNSW hits → back into the gate as candidate seeds.** The ANN hits are handed to `loom-scaffold`'s existing confidence-gated selective-injection policy as candidate seeds — the same `STRONG_MATCH_SCORE`/`MIN_INJECT_FRACTION` budget logic decides whether and how much to inject. **HNSW is a candidate source, never a bypass of the gate.**
5. **What injects is the markdown.** Whatever injects is the retrieved `CanonicalUnit`'s human-readable markdown block resolved by IRI. oxigraph SPARQL and the vector row are only the address-and-rank path to it; the served unit is always the markdown.

**The wiring is default-OFF and benchmark-gated.** Our own naive over-retrieval result — **Δ = −0.40 [−0.58, −0.22], n=285, across 5 models, worst on the weakest model (haiku −1.30)** — is the standing regression guard: naive fusion of a weak signal against a strong one underperforms the strong signal alone. HNSW fusion ships behind the WS-O multivariate bench (in-domain recall **AND** general-question non-jaggedness **AND** OOV recovery) and becomes default-on only once it beats the lexical baseline on **all** axes. This is the same gate ADR-136 D3 sets; PRD-027 builds the machinery, PRD-026 WS-O owns the bench.

### FR-4 — oxigraph-native graph store

`loom-graph-oxigraph` replaces `loom_graph.py` + pyoxigraph with a direct `oxigraph` crate dependency. It loads `ontology.ttl` + `ontology-inferred.ttl` only, exposes `sparql(query)` and `search(q, limit)`, and is read-only. Behaviour parity with the Python store is a fixture test: the same relationship-pattern + aggregation SPARQL query returns the same result set. Fail-open: an absent or unparseable store degrades the Loom to lexical-only and is reported `graph.available=false` in `/health` (never a hard crash).

### FR-5 — SPARQL clamp, formalised

The clamp (which `loom_facade.py` today applies loosely) is formalised in `loom-graph-oxigraph`: accept only `SELECT`/`ASK`/`CONSTRUCT`/`DESCRIBE`; reject `SERVICE` (federation forbidden, ADR-117/ADR-011 posture inherited from ADR-135 D7); apply a server-side `LIMIT` clamp; reject `INSERT`/`DELETE`/`LOAD`/`CLEAR`/`DROP` (the façade never exposes a write door, ADR-135 D1). A malformed or forbidden query returns a labelled 400, never executes.

### FR-6 — Generation identity, SSOT mirror, and the accelerator boundary

- The atomic generation-verified mirror (`mirror.sh`, ADR-136 D4) ports into `loom-facade`: a generation is fully present (all artifact shas verify) or absent — never mixed-build. The `.generation.json` descriptor with `verifiedSingleGeneration` is preserved.
- The HNSW artifact is a **generation-stamped projection of the one build source** (ADR-136 D4 SSOT), not a fourth copy: it is loaded into the in-process `ruvector-core` projection alongside the reasoned generation, stamped with the same `buildId`. Re-embed on promote is delta-diffed (touched IRIs only), honouring the HNSW index-law (non-concurrent rebuild, `m=16`, `ef_construction=128`; never `CREATE INDEX CONCURRENTLY` on the ruvector HNSW AM).
- The accelerator boundary is a type-level guarantee (FR-6a): `loom-domain` declares no dependency on any adapter crate; every port trait resolves to an IRI addressing a `CanonicalUnit`; a `cargo tree` assertion in CI proves the ring is acyclic and inward-only (§10 AC-6).

### FR-7 — Model-swap backend seam

`loom-backend-openai` keeps `DISTILL_BACKEND_URL` a single config line. Swapping the model (Qwen3.8-27B → next, on `:8085` behind the Loom) requires no code change and no consumer change; the model id + file metadata are probed and stamped into every result (`loom:{...generation...}` and, for `/loom/distill` when it lands, the ADR-135 D4.6 result envelope's `model_id_probed`/`model_file_meta`). The `max_tokens` floor for reasoning backends (≥1536, the verified truncation guard) is preserved and env-tunable (`LOOM_MIN_MAX_TOKENS`).

### FR-8 — ProofGate attestation adapter (surface only; enforcement is PRD-026)

`loom-attest-proofgate` provides the `AttestationLedger` port implementation over RuVector `ProofGate<T>`/`MutationLedger` (RuVector ADR-047): the domain predicates (subclass-acyclicity, duplicate-label, type-match, relation-contradiction) stay Loom-owned in `loom-domain`; their attestation becomes chain-hashed tamper-evident ledger entries. This is build/CI-time and off the serving hot path. **PRD-027 delivers the adapter; PRD-026 owns wiring the gate into CI as an enforced control** — this PRD does not claim gate enforcement as shipped.

---

## 7. Non-functional requirements

### NFR-1 — Latency budget (the hot path stays fast)

| Path | Budget | Basis |
|---|---|---|
| Lexical match over 8,146 titles | **< 50 ms** (p99) | matches today's self-tested `ontology_scaffold.py` |
| Scaffold serialise (`/loom/scaffold`, no LLM) | **< 80 ms** (p99) end-to-end | lexical + link→seed→expand→serialise |
| in-context oxigraph SPARQL (typical class query) | **< 30 ms** (p99) | native store, in-memory reasoned closure |
| Semantic-fallback path (Xinference embed + ANN), **only on lexical miss** | **< 250 ms** (p99) added latency | dominated by the Xinference round-trip; ANN itself is sub-ms at 8,146 records (RuVector ADR-001); triggered only below `MIN_INJECT_SCORE`, never on the common path |
| `/v1/chat/completions` total | model-bound | unchanged; the model dominates; augmentation is < 80 ms of it |

The re-platform must not regress the lexical path. A Rust matcher over 8,146 titles is expected to beat the Python one comfortably; the < 50 ms budget is a floor, not a target.

### NFR-2 — Portability

A single static musl binary (a few MB), no interpreter, no wheel, no runtime dependency surface beyond the mounted generation artifacts and (Profile B) network reach to `ruvector-postgres`/Xinference for the off-turn write channel. Cold start < 500 ms to first-served request (index warm on boot, matching today's `main()` warm-the-index behaviour). Nix-buildable (agentbox pattern); cross-compilable for the host and the sidecar targets from one workspace.

### NFR-3 — Benchmark-first gate on fusion (non-negotiable)

HNSW fusion is **default-OFF** and does not become default-on until it beats the lexical baseline on the WS-O multivariate bench across **all** axes (in-domain recall, general-question non-jaggedness, OOV recovery). The Δ = −0.40 over-retrieval fixture is a permanent regression guard in CI: a fusion change that reproduces a net-negative on the weak-model axis fails the build. This is a *gate*, not a formality — it is the single most important NFR because it is the one place the re-platform could quietly degrade answer quality.

### NFR-4 — Generation parity across profiles

The generation descriptor is **byte-identical across Profile A and Profile B for the same `commitSha`** (ADR-135 D1.1 verification). The in-process HNSW artifact and the reasoned generation are mirrored into both deployments under the ADR-136 D4 atomic-generation discipline; parity is a CI/health assertion (§10 AC-7).

### NFR-5 — Observability and liveness (ADR-119)

`/health` reports generation stamp, backend reachability, graph-store status, and HNSW-projection status — enough to catch the green-but-zero / `.48-is-dead` class (a `/health` 200 while the model route is black-holed). Fail-open on channel (a missing HNSW projection degrades to lexical, never a crash), fail-labelled on payload (a `scaffold_engaged=false` result is never delivered as grounded — ADR-135 D4.4).

### NFR-6 — Identity/provenance convergence

The Rust Loom uses the ecosystem `did:nostr` + NIP-98 primitives (ADR-125) where it signs or verifies (result envelopes for `/loom/distill`; the ProofGate ledger entries). It does not re-implement a bespoke signing path. This is convergence, not new capability — the serving hot path signs nothing.

---

## 8. Deployment — two compose profiles (resolves ADR-135 D1-a)

**Decision (ADR-137): ship one Rust binary in two compose profiles — Profile A (host-colocated on HP, the reference serving deployment) and Profile B (sidecar on `visionclaw_network`) — with A as reference and B as the consumer-facing door and the build/off-turn write channel.** This resolves ADR-135's open decision D1-a (which ADR-135 recommended as "ship A, keep B green in CI") to a genuine *both*.

**Why the Rust rewrite tips D1-a from "A only, B green in CI" to "both are live":** a single static musl binary with no interpreter/wheel makes running two profiles nearly free. Under the Python image, Profile B was a second maintenance surface (a second image to build, patch, and keep in sync). With one binary in two compose profiles, B stops being a maintenance cost and becomes a first-class deployment.

### 8.1 Profile A — host-colocated on HP (reference)

- The model (Qwen3.8-27B `loom-model`) is GPU-colocated on HP at `:8085`; the augmentation hot path is **in-process** — lexical + in-process `ruvector-core` HNSW + in-context oxigraph SPARQL, network-free per ADR-136 §3.
- A serves fully **even with no docker-network access**: everything on the hot path is in-process or on-disk. DNAT keeps `:8084` on the LAN (the `hp-nat.service` DNAT with the MSS clamp for the 9000→1500 step-down).
- This is the reference deployment because it is the one that serves with the model colocated and no network hop.

### 8.2 Profile B — sidecar on `visionclaw_network` (required, not optional)

Profile B is **required, not optional-CI-only**, for two live reasons the new ground truth surfaces:

1. **The email gateway already binds `REASONER_BASE_URL=http://loom:8080/v1`** — a docker-network consumer that must reach a Loom *on* `visionclaw_network`, not one behind the HP DNAT. B is that Loom.
2. **The build-time/off-turn write channel** to `ruvector-postgres` + Xinference embeddings (both docker-network services) needs an in-network home. B is that home — it runs the delta-diffed re-embed-on-promote against `ruvector-postgres` via the MCP embedding pipeline.

B is **GPU-free** and delegates the model via `DISTILL_BACKEND_URL` to HP `:8084` or a model container, preserving model-is-a-URL. B's serving hot path is still in-process (its own HNSW projection + oxigraph store, mirrored per NFR-4); the network reach is for the *write* channel and the *model delegation*, never the augmentation *read*.

### 8.3 Consequences and obligations

- The in-process HNSW artifact and the reasoned generation are mirrored into **both** deployments under the ADR-136 D4 atomic generation-verified discipline; generation parity across A and B is a CI/health assertion (NFR-4, AC-7).
- The `ruvector-postgres`/MCP path is **build/off-turn ONLY** and never the query hot path (DDD §6.1) — so a Profile-A instance cut off from the docker network still serves.
- **Rejected — host-colocated-only (A only):** strands the email-gateway consumer behind a DNAT and leaves the ruvector/Xinference write channel no clean in-network home.
- **Rejected — sidecar-only (B only):** surrenders GPU-colocation with the model and the network-free in-process hot path on the reference deployment, and inserts a network hop to the model for the demo path.

---

## 9. Non-goals (explicit)

- **No corpus building.** The vendored `app/pipeline/*` is dropped (#21). The canonical builder stays **jjohare/logseq** with its CI-enforced gate (`pipeline/`, `publish.yml`). The Rust Loom is a **serving mirror**, not a builder. It consumes generations; it does not mint them.
- **No query-time reasoning.** No Whelk EL++ or any DL reasoning at Loom query time — reasoning stays build-time-only (Whelk-rs authority, ADR-136 D6). The Loom serves the pre-reasoned snapshot; this is what keeps the façade GPU-free and portable.
- **No mesh coordination substrate.** The multi-agent blackboard (WS-Q class) is explicitly deferred, not shipped (ADR-136 D7, DDD §9). When built it must resolve every claim to the same per-IRI markdown identity.
- **No rejected/deferred retrieval stacks.** No `@ruvector/graph-node` Cypher (label-scan-only regression, ADR-136 D2); no ruvector-hybrid/mincut/gnn-rerank fusion (deferred until it ships into `ruvector-server` AND beats the bench, ADR-136 D8); no ruflo `hybridRetrieve()` (Proposed, flag-off, unbenchmarked).
- **No markdown replacement.** No GraphRAG community summaries, no GNN-encoded soft-prompt subgraphs, no RuVector row-as-source-of-record. The served unit is always the markdown (the Prize).
- **No gate-enforcement claim.** PRD-027 delivers the `loom-attest-proofgate` adapter surface; wiring the gate into CI as an enforced control is PRD-026's acceptance gate, not this PRD's.
- **No re-homing of `/loom/distill`.** The deferred-distillation submit/collect surface stays as specified in ADR-135 D4 / agentbox ADR-051; the axum facade reserves the route but does not re-specify it.

---

## 10. Acceptance criteria (testable)

Each criterion is a pass/fail gate. AC-1 and AC-8 are the two that most directly protect the Prize.

| # | Acceptance criterion | Test |
|---|---|---|
| **AC-1** | **Human-scrutability check (the Prize).** For a sampled set of IRIs, the markdown-with-ontology block the Rust Loom serves (via `/loom/scaffold` and as injected into `/v1/chat/completions`) is **byte-identical** to the block the Python Loom serves for the same IRI at the same generation. No block is ever replaced by a vector, a summary, or an encoding. | Golden-corpus diff across ≥ 500 IRIs; zero non-identical blocks; a manual reviewer confirms a served block is readable, has its typed relations header, and carries `corpusNature`. |
| **AC-2** | **Consumer contract preserved.** `GET /loom/generation`, `/loom/scaffold`, `/v1/chat/completions`, `/v1/models` return wire shapes identical to today's; agentbox ADR-051's client and the email gateway bind unchanged. Any additive field (e.g. HNSW sha in the descriptor) is versioned and fires ADR-051's review-trigger. | Contract-test suite replaying recorded consumer requests against both Loom implementations; byte-shape parity on the frozen fields. |
| **AC-3** | **Lexical parity.** The Rust `loom-scaffold` produces the same seed set and same injected block as `ontology_scaffold.py` over a representative prompt corpus, at identical constants. | Golden-fixture test; 100% seed-set match; identical serialised block. |
| **AC-4** | **oxigraph parity.** A relationship-pattern + aggregation SPARQL query returns the same result set from the Rust `oxigraph` store as from pyoxigraph; a `SERVICE` query and an `INSERT` are both rejected with a labelled 400. | Fixture SPARQL suite; result-set equality; clamp rejection tests. |
| **AC-5** | **Recall-gate on the semantic fallback.** The ontology-corpus HNSW projection reproduces the validated recall band (rgb-protocol ≥ 0.85, decoys ≤ ~0.50) in-process, and passes the estate recall-gate (`agentbox.sh ruvector recall`: self ≥ 175/200, true ≥ 102/120). | `hnsw-xinference` protocol run against the in-process projection; recall-gate band assertion. |
| **AC-6** | **Accelerator boundary is structural.** `cargo tree` proves `loom-domain` depends on no adapter crate and the ring is acyclic/inward-only; every port trait resolves to an IRI addressing a `CanonicalUnit`. | CI `cargo tree` + a compile-time test that no adapter type is returned in place of a `CanonicalUnit`. |
| **AC-7** | **Generation parity across profiles.** `GET /loom/generation` returns a byte-identical descriptor from Profile A and Profile B for the same `commitSha`; both serve the same HNSW artifact sha. | Cross-profile health assertion in CI; descriptor + artifact-sha diff = empty. |
| **AC-8** | **Fusion is default-OFF and benchmark-gated.** With fusion disabled (default), behaviour equals today's lexical baseline exactly. Fusion cannot be turned default-on in config until the WS-O bench shows it beats the baseline on all three axes; the Δ = −0.40 fixture is a CI regression guard that fails a net-negative fusion change. | Config default assertion; the WS-O bench harness as a merge gate; the −0.40 fixture in CI. |
| **AC-9** | **Single-binary portability.** The release artifact is a single static musl binary that runs with no interpreter and no wheel; cold start < 500 ms; Nix build reproducible. | Build + run on a bare container with only the binary + mounted generation; timing assertion. |
| **AC-10** | **Model-swap seam.** Changing `DISTILL_BACKEND_URL` swaps the model with zero code and zero consumer change; a fresh completion stamps the new model identity in `loom:{...}`. | Swap backend URL; assert new model id in result; no consumer diff. |

### 10.1 Implementation status (2026-08-17 — honest per-AC verdict)

The eight-crate workspace is built, gate-green, and adversarially audited (gpt-5.4; `.claude/evidence/AUDIT-gpt54.md`). Status against each acceptance criterion — **met**, **gated-off (honest RED)**, or **operational tail** (code done, live-deploy assertion pending):

| AC | Status | Evidence / honest note |
|---|---|---|
| AC-1 (Prize byte-identical) | ✅ **Met** | Golden byte-equality tests green (`loom-scaffold`; EXP-002). No block replaced by a vector/summary/encoding — enforced by the ring (AC-6). |
| AC-2 (consumer contract) | ✅ **Met** | Router oneshot tests green (EXP-005); frozen `/v1/*`, `/loom/*` shapes preserved. |
| AC-3 (lexical parity) | ✅ **Met** | Golden-fixture parity, constants ported verbatim (EXP-002/003). |
| AC-4 (oxigraph parity + clamp) | ✅ **Met (clamp strengthened)** | Native `oxigraph`; `SERVICE`/`INSERT` rejected 400; LIMIT clamp made **PREFIX/BASE-prologue-aware** — a deliberate security divergence beyond Python (EXP-004, audit finding 3). |
| **AC-5 (recall-gate on the fallback)** | ❌ **NOT MET — gated-off (honest RED)** | Measured `rgb-protocol 0.816` in the document-embedding regime, **below** the AC's `≥ 0.85` band and the `0.87` design floor. The recall gate is RED, so **`LOOM_SEMANTIC_FALLBACK` stays default-off** and the estate recall-gate is not cleared. This is the one criterion the implementation does **not** satisfy; the fix needs a query-shaped embedding (or a bench-justified floor), not a threshold fudge (EXP-008, audit finding 5). |
| AC-6 (accelerator boundary structural) | ✅ **Met** | `cargo tree` acyclic inward-only ring; adapters cannot mint a `CanonicalUnit` (compile-time). The single-gate invariant survived audit finding 1 (semantic debug surface now default-off; EXP-007). |
| AC-7 (generation parity across profiles) | ◑ **Operational tail** | Both compose profiles are authored and the descriptor/mirror logic is implemented; the **live cross-profile A≡B byte-identity health assertion** runs at deployment cutover (deploy layer), not yet asserted against running instances. |
| AC-8 (fusion default-OFF, benchmark-gated) | ✅ **Met** | Default-off enforced; behaviour equals the lexical baseline; the −0.40 fixture is the standing guard. Cannot flip default-on until AC-5 clears and WS-W wins all axes. |
| AC-9 (single-binary portability) | ◑ **Met (build); timing at deploy** | Static musl binary, no interpreter/wheel; the `< 500 ms` cold-start + Nix-reproducibility assertions are measured on the deploy target (WS-U/WS-X). |
| AC-10 (model-swap seam) | ✅ **Met** | `DISTILL_BACKEND_URL` one config line; model id stamped into `loom:{...}` (EXP-006). |

**Bottom line:** the substrate re-platform (AC-1/2/3/4/6/8/10) is **met and audited**; the semantic-fallback recall gate (**AC-5**) is **honestly unmet and correctly gated-off**; two deployment assertions (AC-7, AC-9 timing) are the **operational tail** the deploy layer closes at cutover. No criterion is reported met that is not.

---

## 11. Risks

| # | Risk | Likelihood × Impact | Mitigation |
|---|---|---|---|
| R-1 | **Behavioural drift in the port** — the Rust scaffold matcher subtly diverges from the Python one, changing which blocks inject. | Med × High | AC-3 golden-fixture parity as a merge gate; port the constants verbatim; keep the Python matcher as a conformance oracle during WS-R/WS-S (the same discipline ADR-136 D6 uses for the reasoner). |
| R-2 | **Fusion degrades weak models** — the Δ = −0.40 over-retrieval trap reappears once HNSW is wired. | Med × High | Fusion default-OFF (NFR-3, AC-8); the −0.40 fixture is a permanent CI guard; HNSW is a candidate source under the existing gate, never a bypass. |
| R-3 | **Generation drift between profiles** — A and B serve different generations or different HNSW shas. | Med × Med | Atomic generation-verified mirror into both (ADR-136 D4); NFR-4 byte-identical descriptor parity as a health/CI assertion (AC-7). |
| R-4 | **Loss of façade-source legibility** blocks a non-Rust operator from a quick fix. | Med × Low | Accepted and named (§2.3); mitigated by the frozen façade contract (the phase of constant façade edits is behind us), the thin `loom-facade` binary (composition only, no logic), and comprehensive `/health` observability. |
| R-5 | **In-process `ruvector-core` maturity** — the embedded HNSW read path is less battle-tested than the MCP server path. | Med × Med | Fail-open to lexical if the projection is absent/unhealthy (NFR-5); AC-5 recall-gate before default-on; the MCP/`ruvector-postgres` path remains the build-time write channel (a known-good fallback for indexing). |
| R-6 | **Xinference embedding-model drift** breaks the fallback (the 384-dim `bge-small` is LOCKED, but an ops change could bump it). | Low × High | The embedding-law is codified (LOCKED, `bge-small-en-v1.5`/384); `loom-embed-xinference` asserts the model id + dimension on boot and refuses to serve fusion against a mismatched embedder (fail-labelled). |
| R-7 | **Scope creep into a builder** — pressure to keep the vendored `pipeline/` "just in case." | Med × Med | Explicit non-goal (§9); WS-R drops `app/pipeline/*`, `app/ontology_proxy.py`, `app/test_proxy.py`; canonical builder stays jjohare/logseq. |
| R-8 | **Nix/musl cross-compile friction** slows the two-profile ship. | Med × Low | Reuse the agentbox Nix pattern and the sibling Rust repos' release profile; one workspace, two targets; treat the build as WS-U with its own acceptance (AC-9). |

---

## 12. Phasing / workstream build order

Continues the workstream lettering from PRD-025 (WS-A…WS-J) and PRD-026 (WS-K…WS-Q). PRD-027 owns **WS-R…WS-Y**. Each is gated by the §10 acceptance criteria named. Direct-to-target (dev/test estate): no dual-language transitional Loom is kept alive.

| WS | Scope | Delivers | Gate |
|---|---|---|---|
| **WS-R** | Workspace skeleton + drops | The tokio workspace, the eight crates as stubs with port traits in `loom-domain`; drop `app/pipeline/*`, `app/ontology_proxy.py`, `app/test_proxy.py`. | `cargo tree` acyclic; AC-6 (boundary). |
| **WS-S** | `loom-scaffold` port | Lexical matcher + confidence-gated injection, constant-for-constant. | AC-3 (lexical parity). |
| **WS-T** | `loom-graph-oxigraph` | Native oxigraph store; SPARQL clamp (FR-5); `/loom/sparql`, `/loom/search`. | AC-4 (oxigraph parity + clamp). |
| **WS-U** | `loom-facade` + build | axum/tower binary serving all preserved endpoints (FR-1); `max_tokens` floor; atomic mirror (FR-6); Nix/musl single-binary build. | AC-1, AC-2, AC-9, AC-10. |
| **WS-V** | `loom-vector-ruvector` + `loom-embed-xinference` | In-process `ruvector-core` HNSW projection over the ontology-corpus namespace; Xinference embed adapter; build-time delta-diffed write channel. **Fusion wired but default-OFF.** | AC-5 (recall-gate); fusion disabled. |
| **WS-W** | Fusion bench (with PRD-026 WS-O) | Run the WS-O multivariate bench (in-domain recall + non-jaggedness + OOV recovery) against fusion vs lexical baseline; the −0.40 fixture as a guard. Default-on **only** if it wins all axes. | AC-8 (benchmark-gated). |
| **WS-X** | Two-profile deployment | Profile A (HP host, reference) + Profile B (sidecar on `visionclaw_network`) compose files; generation parity mirror; email-gateway `REASONER_BASE_URL` repoint to Profile B. | AC-7 (generation parity); §8. |
| **WS-Y** | `loom-attest-proofgate` adapter | `AttestationLedger` port over RuVector `ProofGate<T>`/`MutationLedger` (surface only; enforcement is PRD-026's CI gate). | Adapter present; ledger entry chain-verifies. |

**Sequencing note:** WS-R→WS-U delivers a contract-identical Rust serving Loom with lexical retrieval and native oxigraph — this is the substrate re-platform, shippable on its own and already the whole win on the portability/oxigraph axes. WS-V→WS-W adds the semantic fallback behind the benchmark gate. WS-X makes both profiles live. WS-Y closes the attestation surface. No workstream turns fusion default-on without WS-W passing.

---

## 13. Cross-reference discipline

PRD-027 is the requirements record for the Rust re-platform; **ADR-137** is its decision-of-record (substrate choice, crate ring, two-profile deployment). This PRD cites ADR-137 for every substrate and deployment call and does not re-derive them. It extends **PRD-025** (the capstone loop) and **PRD-026** (the consolidation beneath the serving unit); it restates neither. The **DDD** (`ddd-ontology-loom-context.md`) is revised in the Rust rev to map the `CanonicalUnit` aggregate root onto `loom-domain` ports and the ruvector/xinference/oxigraph/backend adapters — the aggregate-root model is unchanged; only its crate realisation is added.

All four docs (ADR-137, PRD-027, the DDD Rust rev, and the extended honesty table) carry THE PRIZE verbatim at the head as the non-negotiable driver, carry the identical shipped-vs-target split (§3), and repo-qualify every cross-repo citation per the PRD-025/026 citation discipline (two `PRD-022`s, two `ADR-050`s; RuVector/ruflo ADRs always repo-prefixed). Governing decisions from **ADR-135** (node boundary, model-is-a-URL, generation discipline, deferred distillation) and **ADR-136** (RuVector behind the markdown, pyoxigraph→oxigraph as a native win, HNSW benchmark-gated, Whelk-rs build-time-only, SSOT/no-fourth-copy, gate re-platformed, mesh deferred) are **honoured and extended, never relitigated**. The single decision this PRD *resolves* is ADR-135 D1-a (reference deployment) → **both compose profiles**, via ADR-137.
