# ADR-137 — Re-platform the Loom to Rust (axum/tokio, hexagonal crates, oxigraph-native, in-process HNSW) and resolve deployment to both compose profiles

**Status:** Proposed (design-only; direct-to-target end state. Implementation is a later phase — this ADR records the architecture decision, it does not ship code)
**Date:** 2026-08-17
**Decision-type:** Architecture (substrate re-platform — language/runtime change + semantic-fallback wiring + deployment resolution)
**Deciders:** Dr John O'Hare (operator)
**Extends:** ADR-135 (Ontology Loom node — keystone; node boundary, generation discipline, model-swap seam, Deployment A/B **unchanged**), ADR-136 (tooling allocation — RuVector behind the markdown, pyoxigraph→oxigraph SPARQL, Whelk-rs build-time, gate on ProofGate, mesh deferred **unchanged**).
**Supersedes:** **ADR-135 D1's stdlib-Python implementation choice only** (the *artifact*: stdlib `http.server` → single Rust binary). It does **not** supersede ADR-135 D1's *goal* (a portable, model-swappable façade) — §D1 below argues Rust delivers that goal better. Every other ADR-135/136 decision stands.
**Resolves:** ADR-135 **D1-a** (open operator decision: reference deployment A vs B) → **both compose profiles** (§D8), with A as reference.
**Relates (this repo, loom):** `app/loom_facade.py`, `app/loom_graph.py`, `app/ontology_scaffold.py`, `app/ontology_proxy.py`, `app/mirror.sh`, `app/pipeline/*`, `app/test_proxy.py`, `docker-compose.yml`, `Dockerfile`, `PRD-025`, `PRD-026`, `PRD-027` (companion — owns requirements + WS build order for this re-platform), `ddd-ontology-loom-context.md` (Rust-rev — owns the CanonicalUnit→ports/adapters realisation), `LOOM-POSITIONING.md`. Issues **#16** (HNSW semantic-fallback wiring), **#21** (drop vendored pipeline).
**Relates (VisionClaw):** **ADR-090** (hexagonal acyclic crate ring — the modularisation law this ADR obeys; PRD-016), **ADR-099** (Whelk-rs EL primary reasoner — build-time authority, unchanged), **ADR-112** (one-brain / no hot-path LLM), **ADR-117** (server-side SPARQL clamp).
**Relates (RuVector):** **ADR-001** (RuVector Core Architecture / HNSW, production `ruvector-core`), **ADR-047** (`ProofGate<T>` / `MutationLedger` / HashChainGate).
**Relates (agentbox):** **ADR-051** (Loom client + deferred distillation — the harness-side consumer of this façade; its wire contract is unchanged by this re-platform).
**Relates (logseq):** the canonical builder `jjohare/logseq` (`publish.yml` — `pytest pipeline/tests` + `pipeline.validate` before deploy; `enrich-gate.yml` on enrichment PRs) — the Loom is a serving mirror of its output, never a second builder.
**Sibling Rust repos matched for style:** `ruvector`, `solid-pod-rs`, `nostr-rust-forum`, `logseq-publisher-rust` (all `resolver = "2"` tokio workspaces; `unsafe_code = "deny"`, `rust_2018_idioms` warn at workspace root; thin-LTO release).

> This ADR records the **decision-of-record for the Rust re-platform**. It does not re-derive the product goal (PRD-025), the tooling allocation (ADR-136), or the bounded-context model (DDD Rust-rev). PRD-027 owns requirements and the workstream build order; this ADR owns the *why* for each substrate call, each with its rejected alternative and its Prize impact.

---

## THE PRIZE (non-negotiable driver — quoted verbatim, governs every decision here)

> The one canonical, load-bearing artifact of this system is the per-IRI human-scrutible unit: one block of curated research prose (`dfull`, `corpusNature: synthetic-ai-generated-human-directed`) headed by its typed ontology relations (`subClassOf`, `requires`/`enables`/`implements`/`uses`/`relatedTo`/`contrastsWith`), that a human can read, review and audit end-to-end at single-entity granularity. Everything else — HNSW vectors, the lexical inverted index, oxigraph SPARQL, mincut, GNN, ProofGate ledgers — is an accelerator that indexes, finds, ranks and attests THAT unit. None of them ever becomes the thing served in its place, and none ever becomes the thing a human must trust instead of the markdown. We explicitly reject the GraphRAG/G-Retriever trajectory where knowledge degrades into opaque LLM community summaries or GNN-encoded subgraphs; our attribution granularity — one reviewable markdown per IRI, behind a propose→consistency-check→human-PR-merge gate — is the real, non-eroding moat.

Any design in this document that reduces the legibility of that unit is a **regression to be justified with a documented answer-quality trade**, not a default. The re-platform changes the *substrate under* the unit; it does not touch the unit. That constraint is load-bearing and is checked at every decision below (§Invariants, §Verification).

---

## 1. Context

The Loom shipped as a deliberately stdlib-only Python serving node (ADR-135 D1): `app/loom_facade.py` (260 lines, `http.server`) serving `/health`, `/loom/generation`, `/loom/scaffold` (retrieval, no LLM), `/v1/chat/completions` (scaffold-inject then delegate to `DISTILL_BACKEND_URL`), `/v1/models`; `app/loom_graph.py` (pyoxigraph SPARQL over the reasoned closure); `app/ontology_scaffold.py` (lexical inverted-index matcher `<50ms` over 8,146 class titles + the confidence-gated selective-injection policy — `STRONG_MATCH_SCORE`, `MIN_INJECT_SCORE`, `MIN_INJECT_FRACTION`); `app/mirror.sh` (atomic generation-verified mirror, ADR-136 D4). It runs today as the `loom` container (`network_mode: host`, `:8084`) delegating to `loom-model` (Qwen3.8-27B on `:8085`). Deployment B (sidecar `loom:8080` on `visionclaw_network`) is specified in ADR-135 D1.3 but is not the running deployment; ADR-135 D1-a left the reference-deployment choice open.

Two things have changed that make the substrate the right thing to revisit now — **not** the served unit, and **not** any governing invariant:

1. **The semantic fallback landed as real ground truth.** The `ontology-corpus` RuVector namespace now holds all **8,146 concept classes** embedded (Xinference `bge-small-en-v1.5`/384, LOCKED per ops law), generation-stamped, IRI-keyed (`urn:ngm:class:<slug>`, `source_type=loom`), in `ruvector-postgres` under the cosine HNSW index `idx_memory_embedding_hnsw` (`m=16`, `ef_construction=128`). Validated recall via `hnsw-xinference`: `rgb-protocol` 0.87, decoys ~0.45. This is exactly the "one real retrieval gap" ADR-136 D3 named but did not build — the OOV/paraphrase miss where the lexical matcher today injects nothing below `MIN_INJECT_SCORE`. It has to be *wired in* (issue #16); wiring it in Python means a network/MCP round-trip to `ruvector-postgres` on the query path, which contradicts the ADR-136 §3 network-free hot-path model. In Rust it becomes an in-process `ruvector-core` read.

2. **The two heaviest dependencies are foreign-language bindings whose native homes are Rust.** `loom_graph.py`'s SPARQL store *is* oxigraph, reached through the `pyoxigraph` Python binding. The HNSW fallback *is* `ruvector-core`, which is a Rust crate. A Rust re-platform turns both from foreign-language bindings into direct, typed, in-process crate dependencies — `loom_graph.py`'s wrapper collapses into a native `oxigraph::Store`, and the HNSW read stops being a network hop. This is the honest technical win, and it lands squarely on the substrate axis, not the feature axis.

The corpus, the served markdown, the injection policy, the model-swap seam, and the generation discipline are **all unchanged**. This is a substrate re-platform: same Prize, same contract, better-delivered version of the same goal ADR-135 D1 optimised for (one portable artifact, no interpreter to provision), plus the hexagonal boundary that keeps every accelerator behind the markdown *by construction* rather than by discipline.

The honest starting position (shared honesty table, ADR-136 §1 / DDD §1): everything in this ADR is **Aspirational** — the Rust node is design-only. The Python Loom is what is **Shipped** today. This ADR must not write the Rust node as shipped anywhere.

---

## 2. Decision

Re-engineer the Loom as a **single static Rust binary** (`axum`/`tower` on `tokio`), organised as a **hexagonal crate workspace** (VisionClaw ADR-090 ring), with `oxigraph` as a **direct native-Rust dependency**, `ruvector-core` HNSW **embedded in-process** as the confidence-gated semantic-fallback third signal (ADR-136 D3 / issue #16), and shipped in **both compose profiles** (resolving ADR-135 D1-a). The served, canonical unit stays the per-IRI markdown-with-ontology block; every crate is a projection that resolves back to its IRI. Decisions **D1–D9**, each with its rejected alternative and its Prize impact.

### D1 — Re-platform from stdlib-Python to Rust (accept the ADR-135 D1 portability tradeoff, honestly)

The façade, graph store, scaffold matcher and mirror move from Python to a Rust workspace. The durable value is on the substrate axis: (a) oxigraph becomes a native store (D3); (b) the HNSW fallback becomes an in-process read, not a network round-trip (D5); (c) one static musl binary (~a few MB, no interpreter, no wheel) replaces `python:3.12-slim` + a `pyoxigraph` wheel, with faster cold start and near-zero runtime dependency surface; (d) the hexagonal boundary (D4) enforces the accelerator-behind-the-markdown rule at compile time.

- **Rejected alternative — keep the stdlib-Python Loom, add the HNSW fallback in Python.** This is the null option and it is not free. ADR-136 §3 requires the augmentation hot path to be network-free; a Python HNSW fallback either (i) round-trips to `ruvector-postgres` over MCP on the query path (violating §3, adding ~100ms + a network-partition failure mode to a path that must serve when the docker network is unreachable), or (ii) re-implements an in-process HNSW reader in Python against `ruvector-core`'s on-disk format — i.e. a foreign re-implementation of a Rust crate, the exact "fourth copy / re-derive" anti-pattern ADR-136 D4 forbids. Rejected because the null option cannot satisfy the very wiring that motivates this ADR without importing the network dependency ADR-136 §3 rules out.
- **The honest tradeoff we accept (stated in full, per ADR-135 D1).** ADR-135 D1 chose stdlib `http.server` *precisely* so the façade was zero-toolchain-portable and any operator could read all 260 lines — "the model is a URL, the façade runs anywhere Python 3.10+ runs." Rust gives that specific property up: it adds a compile/cross-compile step, a Nix build (agentbox pattern), slower edit-run iteration, and the façade source is no longer a single skimmable file a non-Rust operator reads in one sitting. What it buys back is a **better-delivered version of the same goal** D1 wanted — a *more* portable single artifact (no interpreter, no wheel, no `pip`/PEP-668 friction — the class of failure the first nightly cycle already hit) with faster cold start — plus type-safety and the enforced hexagonal boundary. **Critically, the tradeoff does not touch THE PRIZE or the model-swap seam:** `DISTILL_BACKEND_URL` stays a single config line, the served unit stays markdown, and the code-legibility we trade away is *the façade's own source*, never the data a human reviews. Net verdict: worth it, because the durable value all lands on the substrate axis and the one thing D1 optimised for (portability of the model-swappable façade) is *improved*, not lost.
- **Prize impact:** *neutral-to-help.* The served unit is untouched. The hexagonal boundary makes "accelerators behind the markdown" a compile-time invariant (D4) rather than a discipline, which is a mild *help* to Prize protection.

### D2 — Framework: axum + tower on tokio; not actix-web, not staying on stdlib

`axum`/`tower` on the `tokio` runtime is the ecosystem norm across every sibling Rust repo (`ruvector`, `solid-pod-rs`, `nostr-rust-forum` are all `resolver = "2"` tokio workspaces). `tower` middleware gives the SPARQL clamp (ADR-117), the budget clamp (ADR-116 lineage), and the CORS/liveness surface as composable layers rather than hand-rolled handler code.

- **Rejected alternative — actix-web.** A capable framework, but off-ecosystem here: no sibling repo uses it, it brings its own actor runtime concepts that duplicate what we already get from `tokio`, and `tower`'s middleware model maps more directly onto the layered clamps the Loom needs. Rejected on ecosystem-consistency and reviewer-familiarity grounds, not on raw capability.
- **Rejected alternative — hand-rolled `hyper`/stdlib server (mechanical port of `http.server`).** Rejected: it would preserve the Python structure's ad-hoc request routing and re-implement middleware the ecosystem already standardises on `tower`. The point of the re-platform is to land on the ecosystem substrate, not to transliterate.
- **Prize impact:** *neutral.* Framework choice is invisible to the served unit; it only shapes the composition root (D4, `loom-facade`).

### D3 — oxigraph as a direct native-Rust crate dependency; not pyoxigraph FFI, not graph-node Cypher

`loom_graph.py` + `pyoxigraph` collapse into a direct `oxigraph::Store` dependency in the `loom-graph-oxigraph` adapter crate. Same engine, one fewer language boundary: pyoxigraph *is* the Python binding to this exact Rust store, so this is a clean removal of an FFI layer, not a migration. The store loads `ontology.ttl` + `ontology-inferred.ttl` **only** (published-ontology-only, DDD BC24 I11 — never the working graph), read-only, with the ADR-117 clamp (SELECT/ASK/CONSTRUCT/DESCRIBE only, `SERVICE` forbidden, server-side `LIMIT` clamp — a straight port of `loom_graph.py`'s `_FORBIDDEN`/`_READ_FORM`/`_clamp` guards, now type-enforced). Absent store degrades to lexical, reported in `/health` (fail-open, unchanged).

- **Rejected alternative — keep pyoxigraph over a Python↔Rust FFI shim, or call it out-of-process.** Rejected: it keeps a foreign-language binding on the path for no benefit once the host process is already Rust — the binding exists *only* to reach Rust from Python. Removing it is the definition of a clean win (ADR-136 §3).
- **Rejected alternative — `@ruvector/graph-node` Cypher as the graph engine.** Already rejected in ADR-136 D2 and re-affirmed here: v2.0.4 "Cypher" is a label-scoped node scan — every relationship pattern, `WHERE`, variable-length path and aggregation returns empty (`GRAPH-ANALYTICS-PROOF.md`). Swapping native oxigraph SPARQL for it is a regression dressed as an upgrade. The Rust re-platform is the moment someone might be tempted to "unify on the RuVector graph engine"; this decision forecloses it.
- **Prize impact:** *help.* SPARQL returns IRIs that address the exact same canonical markdown; keeping (and natively strengthening) the working engine protects that resolution path.

### D4 — Hexagonal crate workspace: `loom-domain` core + adapters + thin `loom-facade` binary (ADR-090 ring)

The workspace obeys VisionClaw ADR-090's acyclic hexagonal ring: a pure domain core with port traits, adapters that implement them, and a thin composition-root binary. The **CanonicalUnit is the aggregate root** (DDD Rust-rev §4/§7): IRI identity, `dfull`, typed ontology-relation header, `corpusNature`; `CorpusGeneration` is its version boundary, not the root. Workspace conventions match the siblings: `resolver = "2"`, `unsafe_code = "deny"`, `rust_2018_idioms` warn at root, thin-LTO release, `cargo test --all-features` + clippy, Nix build (agentbox pattern).

| Crate | Ring position | Responsibility |
|---|---|---|
| `loom-domain` | core (no I/O, no framework deps) | The `CanonicalUnit` aggregate root, `CorpusGeneration` version boundary, and the port traits every adapter implements: `LexicalIndex`, `VectorIndex`, `EmbeddingProvider`, `GraphStore`, `ModelBackend`, `AttestationLedger`. Encodes **Invariant I-P1**: every port method returns or resolves to an IRI that addresses a `CanonicalUnit`. |
| `loom-scaffold` | domain logic | Exact port of `ontology_scaffold.py`: the lexical inverted-index matcher (`<50ms`, 8,146 titles) + the confidence-gated selective-injection policy (`STRONG_MATCH_SCORE`, `MIN_INJECT_SCORE`, `MIN_INJECT_FRACTION`, budget clamp, link→seed→expand→serialise). **The sole authority over which units inject.** |
| `loom-graph-oxigraph` | adapter | `GraphStore` over native `oxigraph` (D3). |
| `loom-vector-ruvector` | adapter | `VectorIndex`: in-process `ruvector-core` HNSW projection over `ontology-corpus` for the query hot path (network-free); plus a build/off-turn write channel to `ruvector-postgres` via the MCP embedding pipeline (never the query path). Anti-corruption: rows carry the IRI as primary key so the index shape never leaks back into a `CanonicalUnit`. |
| `loom-embed-xinference` | adapter | `EmbeddingProvider` to Xinference `bge-small-en-v1.5`/384 (LOCKED). Two call sites: build-time embed-on-promote (delta-diffed) and query-time OOV embed for the fallback gate. |
| `loom-backend-openai` | adapter | `ModelBackend`: the OpenAI-compatible `DISTILL_BACKEND_URL` client. Scaffold-injects the last user message, delegates `/v1/chat/completions`, floors `max_tokens ≥ 1536` for reasoning backends (ported verbatim from the façade's `MIN_MAX_TOKENS` guard — 400 truncates reasoning models to empty), stamps model identity + generation into results (never in the endpoint — ADR-135 D1.2). |
| `loom-attest-proofgate` | adapter (build/CI-time) | `AttestationLedger` re-platforming the gate verdict onto RuVector `ProofGate<T>`/`MutationLedger` (ADR-047 / ADR-136 D5). Domain predicates stay Loom-owned in `loom-domain`; only the attestation mechanics move. Not on the serving hot path. |
| `loom-facade` | composition root (thin binary) | `axum`/`tower` binary. Wires ports→adapters; serves `/health`, `/loom/generation`, `/loom/scaffold`, `/loom/sparql`, `/loom/search`, `/v1/chat/completions`, `/v1/models`; owns the atomic generation-verified mirror (ADR-136 D4, ported from `mirror.sh`) and the two deployment profiles. **Contains no domain logic** — every decision lives in a domain port. |

- **Rejected alternative — a single-crate binary (mechanical 1:1 port of the four Python files).** Rejected: it would let an adapter's index shape leak into the served unit with nothing to stop it, which is exactly the Prize regression the hexagonal boundary exists to prevent. The whole point of the ring is that "the HNSW row / the SPARQL result / the vector is not the served unit" becomes a *type-level* fact (`VectorIndex` returns IRIs, `CanonicalUnit` is minted only in `loom-domain`), not a convention a future edit can quietly break.
- **Prize impact:** *help.* I-P1 is enforced by the crate graph: adapters cannot construct a `CanonicalUnit`, only resolve an IRI to one owned by the domain.

### D5 — Retrieval fusion: lexical primary → semantic fallback → one confidence gate; default-OFF, benchmark-gated

Fusion is a candidate-union feeding **one** confidence gate, not a blind RRF blend (issue #16 wiring, ADR-136 D3). Flow:

1. **Lexical primary** — `loom-scaffold`'s inverted-index matcher scores the query against the 8,146 class titles, exactly as today.
2. If the top score clears the gate, **inject as now** — no embedding call, hot path stays LLM-free and network-free.
3. **Only** on a lexical miss / score below `MIN_INJECT_SCORE` (the OOV/paraphrase gap the matcher structurally misses), embed the query via Xinference `bge-small-en-v1.5`/384 and run ANN over the **in-process `ruvector-core` HNSW projection** of `ontology-corpus` (IRI-keyed, cosine, validated recall `rgb-protocol` 0.87 vs decoys ~0.45).
4. HNSW hits are handed **back into `loom-scaffold`'s existing confidence-gated policy as candidate seeds** — the same `STRONG_MATCH_SCORE`/`MIN_INJECT_FRACTION`/budget logic decides whether and how much to inject. **HNSW is a candidate source, never a bypass of the gate.**
5. Whatever injects is the retrieved `CanonicalUnit`'s human-readable markdown block resolved by IRI. The served unit is always the markdown; oxigraph SPARQL and the vector row are only the address-and-rank path to it.

The wiring is **default-OFF and benchmark-gated**. The standing regression guard is **our own naive over-retrieval result: Δ = −0.40 [−0.58, −0.22], n=285, across 5 models, worst on the weakest (haiku −1.30)** (replicated; commit `9fe57c5`) — naive fusion of a weak signal against a strong one *underperforms the strong signal alone* (lost-in-the-middle / irrelevant-skew). HNSW fusion ships behind the WS-O multivariate bench (in-domain recall **AND** general-question non-jaggedness **AND** OOV recovery) and becomes default-on only once it beats the lexical baseline on **all** axes. The in-process HNSW projection is the production `ruvector-core` index (RuVector ADR-001).

- **Rejected alternative — blind RRF / weighted blend of lexical + HNSW on every query.** Rejected by the −0.40 result: more context is not more answer quality when it is off-topic, and blending calls the embedder on the hot path for every query even when lexical already cleared the gate. The gate, not the blend, is the safety rail.
- **Rejected alternative — HNSW as a query-path call to `ruvector-postgres` over MCP.** Rejected: it puts a network round-trip on the augmentation hot path, breaking ADR-136 §3's network-free guarantee and Profile A's ability to serve when cut off from the docker network. The query path reads the **in-process** projection; `ruvector-postgres`/MCP is build/off-turn write only.
- **Rejected alternative — default-on because "recall is landed and validated."** Rejected: validated *retrieval* recall (0.87) is necessary but not sufficient; the −0.40 fixture proves a good retriever can still degrade a weak generator. Retrieval recall is not answer quality; the WS-O bench, not the recall number, is the gate.
- **Prize impact:** *neutral-if-gated.* Vectors rank and find markdown; they never replace it. The gate operates on *which* blocks inject, never on the blocks themselves.

### D6 — RuVector-postgres client: `sqlx`; Xinference embed client: `reqwest`; both off the query hot path

The **build/off-turn write channel** to `ruvector-postgres` (embed-on-promote, delta-diffed touched IRIs) uses **`sqlx`** (compile-time-checked queries, `tokio`-native, the ecosystem default across the siblings) — *not* on the query path. The query-path HNSW read is the in-process `ruvector-core` projection (D5), which touches no Postgres client at all. The Xinference embed client (`bge-small-en-v1.5`/384, LOCKED, at `http://xinference:9997/v1/embeddings`) is a thin **`reqwest`** JSON client in `loom-embed-xinference`, called at two sites only: build-time embed-on-promote and query-time OOV embed after a lexical miss. Writes honour the HNSW index-law (non-concurrent rebuild, `m=16`, `ef_construction=128`; **never** `CREATE INDEX CONCURRENTLY` on the ruvector HNSW AM — double-insertion, verified).

- **Rejected alternative — `tokio-postgres` for the write channel.** A fine driver, but `sqlx`'s compile-time query checking catches schema drift against `ruvector-postgres` at build time (valuable given the generation-stamped, IRI-keyed row contract must not drift), and `sqlx` is already the ecosystem's Postgres default. `tokio-postgres` is the fallback if a `sqlx`-incompatible extension surfaces; not expected. Rejected on the margin, for compile-time safety.
- **Rejected alternative — write to `ruvector-postgres` via raw SQL / the CLI.** Categorically rejected (ops law): raw SQL `INSERT` and the CLI bypass the embedding pipeline, so rows become invisible to HNSW search. The write channel goes through the MCP embedding pipeline; `sqlx` is used only where direct SQL is legitimately needed (generation-descriptor bookkeeping, not embedding writes).
- **Rejected alternative — a heavier embedding client / swapping the embedder.** Rejected: the embedder is LOCKED per ops law (`bge-small-en-v1.5`/384) — the whole `ontology-corpus` namespace is embedded with it and cosine-comparability requires the same model. A thin `reqwest` client is all that is needed; anything more is surface for a lock violation.
- **Prize impact:** *neutral.* Both clients are off the served-unit path; the write channel only maintains the index that *finds* markdown.

### D7 — Drop the vendored `app/pipeline/*` (and `ontology_proxy.py`, `test_proxy.py`); the Loom is a serving mirror, not a builder (issue #21)

The vendored `app/pipeline/*` (a stale copy of `jjohare/logseq`'s `pipeline/`), `app/ontology_proxy.py` (524-line legacy proxy), and `app/test_proxy.py` are **dropped** in the re-platform, not ported. The canonical builder stays `jjohare/logseq` with its CI-enforced gate (`publish.yml`: `pytest pipeline/tests` + `pipeline.validate` before deploy; `enrich-gate.yml` on enrichment PRs). The Rust Loom serves **pre-gated artifacts**; it does not build or gate the corpus. The admission-control *attestation mechanics* re-platform onto ProofGate (D4 `loom-attest-proofgate`, ADR-136 D5), but that is build/CI-time and consumes the canonical builder's verdicts — it does not resurrect a second builder inside the Loom.

- **Rejected alternative — port `app/pipeline/*` to Rust so the Loom can build too.** Rejected: it recreates the two-builder drift ADR-135/136 exist to kill (the 8152-vs-5975 divergence traces to redundant materialisations). One builder, one gate, in `jjohare/logseq`. A serving mirror that also builds is two sources of truth.
- **Rejected alternative — keep the vendored copy "for reference / offline build."** Rejected: a stale copy is a drift hazard, not a safety net; the DDD already flags `app/pipeline/` as a stale copy. Delete it (#21).
- **Prize impact:** *help.* One builder means the human reviews and merges the canonical markdown in one place (`jjohare/logseq` PR gate); the Loom never mints a second, divergent copy.

### D8 — Deployment: ship one binary in BOTH compose profiles (resolves ADR-135 D1-a); A reference, B required

Ship the one static Rust binary in **two compose profiles**, resolving ADR-135's open D1-a to **both**:

- **Profile A — host-colocated on HP (reference serving deployment).** The model (`loom-model` Qwen3.8-27B on `:8085`) is GPU-colocated on HP; the augmentation hot path is fully in-process — lexical + in-process `ruvector-core` HNSW + in-context oxigraph SPARQL, network-free per ADR-136 §3 — so A serves fully **even with no docker-network access**. `hp-nat.service` DNATs `:8084` onto the LAN. This is the reference because it is the demo/capstone path and the only one GPU-colocated with the model.
- **Profile B — sidecar on `visionclaw_network` (required, not CI-only).** GPU-free; delegates the model via `DISTILL_BACKEND_URL` (to HP `:8084` or a model container), preserving model-is-a-URL. B is **required** for two live reasons the new ground truth surfaces: (i) the **email gateway already binds `REASONER_BASE_URL=http://loom:8080/v1`** — a docker-network consumer that must reach a Loom on `visionclaw_network`, not behind a DNAT (agentbox ADR-051 is the client-side contract); and (ii) the **build/off-turn write channel** to `ruvector-postgres` + Xinference (both docker-network services, D6) needs an in-network home — B is that home.

The Rust rewrite is exactly what tips this from ADR-135 D1-a's "ship A, keep B green in CI" to a genuine **both**: a single static musl binary with no interpreter/wheel makes running two profiles nearly free, where the Python image made B a second maintenance surface.

- **Rejected alternative — host-colocated only (Profile A, ADR-135 D1-a's recommendation).** Rejected: it strands the email-gateway consumer behind a DNAT and leaves the `ruvector`/Xinference write channel no clean in-network home. The landed ground truth (an in-network write channel + a live docker-network consumer) is precisely what changes this from "A is enough" to "B is required."
- **Rejected alternative — sidecar only (Profile B).** Rejected: it surrenders GPU-colocation with the model and the network-free in-process hot path on the reference deployment, and inserts a network hop to the model for the demo path.
- **Obligations/consequences this creates:** the in-process HNSW artifact and the reasoned generation must be mirrored into **both** deployments under the ADR-136 D4 atomic generation-verified discipline; **generation parity across A and B becomes a CI/health assertion** (byte-identical generation descriptor per commit `commitSha`, ADR-135 D1.1); the `ruvector-postgres`/MCP path is build/off-turn **only** (DDD §6.1), so a Profile-A instance cut off from the docker network still serves.
- **Prize impact:** *neutral.* Same reviewable-markdown identity is served whether A or B; the deployment topology is invisible to the served unit (identical façade contract, ADR-135 D1.3).

### D9 — ProofGate attestation is a build/CI-time adapter, not a serving-path dependency

`loom-attest-proofgate` re-platforms the gate verdict onto RuVector `ProofGate<T>`/`MutationLedger` (ADR-047, per ADR-136 D5), but strictly at build/CI time. Domain predicates (subclass-acyclicity, dupe-label, type-match, relation-contradiction) stay Loom-owned in `loom-domain`; only their *attestation mechanics* (verdict → chain-hashed tamper-evident ledger entry) move to RuVector. The serving hot path never depends on it.

- **Rejected alternative — attest on the serving path / per request.** Rejected: attestation records *that the gate ran* at write time; putting it on the read path adds a dependency to a path that must stay LLM-free and network-free for no serving benefit.
- **Prize impact:** *help.* A tamper-evident gate blocks a bad write before the canonical corpus; the markdown a human reviews is what it protects.

---

## 3. How this reconciles with ADR-135, ADR-136 and ADR-112

- **ADR-135** node boundary, generation/manifest discipline, model-swap seam (`DISTILL_BACKEND_URL`), and Deployment A/B topology are **unchanged**. This ADR supersedes only D1's *implementation choice* (stdlib Python → Rust binary) and resolves D1-a (→ both profiles). The generation descriptor shape, the `/v1` contract, and "model identity rides in results, never in the endpoint" (D1.2) are preserved verbatim in `loom-facade` + `loom-backend-openai`.
- **ADR-136** tooling allocation is **unchanged and now built**: D2 (keep the oxigraph SPARQL engine) becomes native (D3); D3 (HNSW as benchmark-gated third signal) is wired (D5); D4 (SSOT build, no fourth copy) is honoured by the in-process projection + delta-diffed embed-on-promote (D6); D5 (gate on ProofGate) is the `loom-attest-proofgate` adapter (D9); D6 (Whelk-rs build-time) is untouched — the Loom still serves a pre-reasoned snapshot and runs no reasoner at query time; D7 (mesh deferred) is untouched.
- **ADR-112** one-brain / no-hot-path-LLM holds: the augmentation hot path (lexical + in-process HNSW + in-context oxigraph SPARQL) touches no model and no network; only `/v1/chat/completions` delegation touches a model, off the augmentation path. The Rust re-platform *strengthens* this — the HNSW read moving in-process removes the one network hop a Python fallback would have added.

---

## 4. Consequences

### Positive
- **The ADR-136 D3 wiring finally lands, correctly.** The semantic fallback plugs the OOV/paraphrase gap as an in-process, network-free, gate-governed candidate source — the only design that satisfies both the wiring goal (#16) and the network-free hot-path law (§3) at once. **Prize impact: neutral-if-gated.**
- **Two foreign-language bindings become native dependencies.** `pyoxigraph`→`oxigraph` and a network HNSW hop→in-process `ruvector-core` (D3, D5; the production HNSW index, RuVector ADR-001). One deployable static binary, faster cold start, no interpreter/wheel/PEP-668 surface. **Prize impact: help** (hexagonal boundary enforces I-P1 by construction).
- **Both profiles ship nearly for free.** The static binary makes Profile B a real deployment, not a CI stub — unblocking the email-gateway consumer and giving the write channel an in-network home (D8). **Prize impact: neutral.**
- **One builder, enforced.** Dropping `app/pipeline/*` (D7) removes the last vendored second-builder; the canonical `jjohare/logseq` gate (`publish.yml`/`enrich-gate.yml`) is the only place corpus markdown is minted and merged. **Prize impact: help.**

### What breaks (deliberately)
- **The stdlib-Python Loom is retired**, not kept as a dual path (direct-to-target, dev/test estate). Any runbook that skims `loom_facade.py`'s 260 lines to understand behaviour must instead read `loom-facade` + the domain ports; the compensating artifact is the type-enforced ring, which makes behaviour *checkable* rather than merely *readable*.
- **`app/pipeline/*`, `app/ontology_proxy.py`, `app/test_proxy.py` are deleted** (#21), not ported.
- **`docker-compose.yml` gains a second profile** and the mirror discipline now asserts generation parity across A and B as a CI/health gate (D8).

### Negative / honest caveats
- **We give up ADR-135 D1's exact portability property** (any operator reads all 260 lines; runs anywhere Python 3.10+ runs) in exchange for a *different, better* portability (one static musl binary, no interpreter) plus a compile/cross-compile/Nix toolchain and slower edit-run iteration. This is a real cost on the developer-ergonomics axis; it is justified because the durable value is on the substrate axis and D1's actual *goal* (portable model-swappable façade) is improved, not lost (D1). Stated so no downstream doc reads the tradeoff as free.
- **HNSW fusion remains OFF until the WS-O bench passes** — the −0.40 over-retrieval result is the standing regression guard (D5). Landing the retrieval recall (0.87) is *not* the same as landing answer-quality; the re-platform makes the wiring *possible*, the bench makes it *default*. No doc may write HNSW-fusion as on-by-default until the bench clears all axes.
- **Everything in this ADR is Aspirational** — the Rust node is design-only; the Python Loom is what is Shipped today. This is honoured in the shared honesty table (ADR-136 §1 / DDD §1); implementation is PRD-027's phased build.
- **The DL-reasoner story still has nothing to catch** (zero `owl:disjointWith` axioms, ADR-136 D6); Whelk is authority for closure/subsumption only. Unchanged by this ADR; noted so it is not over-claimed.

### Neutral
- The model-swap seam, generation discipline, and served-unit identity are byte-for-byte the same contract across the re-platform and across both profiles.

---

## 5. Alternatives considered (whole-ADR level)

### A1 — Keep the stdlib-Python Loom unchanged; do not re-platform
**Rejected:** it cannot wire in the landed HNSW fallback (#16) without either a network round-trip on the hot path (breaks ADR-136 §3) or a Python re-implementation of a Rust crate's on-disk index (a fourth copy, breaks ADR-136 D4). The motivating ground truth is unreachable from the null option. (D1.)

### A2 — Rewrite in Rust but as a single mechanical-port crate (no hexagonal ring)
**Rejected:** it drops the one structural guarantee the re-platform is *for* — that accelerators cannot become the served unit. Without the ring, "the vector/SPARQL result is not the markdown" is a convention a future edit silently breaks; with it, it is a type-level fact (D4).

### A3 — Rust, but unify graph + vector on the RuVector stack (graph-node Cypher + ruvector-hybrid)
**Rejected:** graph-node Cypher is label-scan-only (strictly weaker than native oxigraph SPARQL, ADR-136 D2), and ruvector-hybrid/mincut/gnn-rerank are unshipped nightly PoCs, measurably slower at Loom scale (mincut ~4000× at n=3000), deferred by ADR-136 D8 until they ship *and* beat the bench. The re-platform keeps native oxigraph SPARQL and adds only the production `ruvector-core` HNSW (ADR-001) behind the gate.

### A4 — actix-web instead of axum
**Rejected:** off-ecosystem here; no sibling repo uses it, and `tower` middleware maps onto the Loom's layered clamps more directly than actix's actor model. Ecosystem consistency + reviewer familiarity, not capability. (D2.)

### A5 — Ship Profile A only (ADR-135 D1-a's recommendation), keep B green in CI
**Rejected:** the landed ground truth (a live docker-network consumer in the email gateway; an in-network `ruvector`/Xinference write channel) makes B *required*, and the static binary makes running both nearly free — which is exactly the condition ADR-135 D1-a's recommendation was contingent on not holding. (D8.)

### A6 — Default-on HNSW fusion since retrieval recall is validated
**Rejected by our own evidence:** the −0.40 result (n=285, 5 models, haiku −1.30) shows a good retriever can still degrade a weak generator via lost-in-the-middle. HNSW is a gate-governed candidate source, default-off until WS-O passes. (D5.)

---

## 6. Invariants (carried into Rust, checked at §7)

1. **THE PRIZE (I-P1):** the served, canonical, load-bearing unit is the per-IRI markdown-with-ontology block (`dfull` + typed relations + `corpusNature`); every crate/adapter — lexical index, HNSW, oxigraph SPARQL, ProofGate — is a projection that resolves back to its IRI and none is ever returned or trusted in its place. Enforced by the crate graph: only `loom-domain` mints a `CanonicalUnit`.
2. **Model-is-a-URL:** `DISTILL_BACKEND_URL` stays a single config line; the model swaps behind the axum façade with zero consumer change; model identity rides in results, never in the endpoint (ADR-135 D1.2).
3. **LLM-free and network-free augmentation hot path:** lexical + in-process HNSW + in-context oxigraph SPARQL touch no model and no network; only `/v1/chat/completions` delegation touches a model, off the augmentation path (ADR-112).
4. **One source of truth / no fourth copy:** ttl + scaffold + prose + HNSW are all generation-stamped projections of one build source; re-embed-on-promote is delta-diffed, honouring the HNSW index-law (non-concurrent rebuild, `m=16`, `ef_construction=128`; never `CREATE INDEX CONCURRENTLY`) (ADR-136 D4).
5. **The confidence gate is the sole injection authority:** HNSW is a candidate source feeding `loom-scaffold`'s `STRONG_MATCH_SCORE`/`MIN_INJECT_SCORE`/`MIN_INJECT_FRACTION` policy, never a bypass; fusion is default-off until the WS-O bench beats the lexical baseline (the −0.40 guard).
6. **Published-ontology-only, read-only, clamped:** the Loom serves `ontology.ttl` + `ontology-inferred.ttl` only, never the working graph (DDD BC24 I11); SPARQL is SELECT/ASK/CONSTRUCT/DESCRIBE with `SERVICE` forbidden and a server-side `LIMIT` clamp (ADR-117).
7. **Generation atomicity + parity:** a generation is fully present (all artifact shas verify) or absent, and the generation descriptor is byte-identical across Profile A and Profile B for the same `commitSha` (ADR-135 D1.1/D2.1); fail-labelled on payload, fail-open on channel.

## 7. Non-goals (explicit)

- Running Whelk EL++ or any DL reasoning at Loom query time — reasoning stays build-time only (Whelk-rs authority, ADR-136 D6); the Loom serves the pre-reasoned snapshot.
- Becoming a retrieval engine or adopting the rejected/deferred ecosystem stacks: no `@ruvector/graph-node` Cypher (label-scan-only), no ruvector-hybrid/mincut/gnn-rerank fusion (deferred until it ships **and** beats the bench, ADR-136 D8).
- Replacing the markdown as the served unit with any encoding — no GraphRAG community summaries, no GNN-encoded soft-prompt subgraphs, no RuVector row-as-source-of-record.
- The Loom being a corpus **builder**: the vendored `app/pipeline/*` is dropped (#21); the canonical builder stays `jjohare/logseq` with its CI-enforced gate.
- The multi-agent coordination substrate / shared blackboard (WS-Q): explicitly deferred (ADR-136 D7); when built it must still resolve every claim to the same per-IRI markdown identity.

## 8. Verification (liveness proofs — asserted by PRD-027's acceptance gates)

| Decision | Verification |
|---|---|
| D1 Rust re-platform | A single static musl binary boots and serves `/health`, `/loom/scaffold`, `/v1/chat/completions` with no interpreter/wheel present; cold-start time < the Python image's; `DISTILL_BACKEND_URL` is a single config line, model swap changes no consumer code (ADR-135 D1.2 preserved). |
| D2 axum/tower | The façade is `axum` on `tokio`; the SPARQL clamp + budget clamp are `tower` layers; `cargo tree` shows no actix. |
| D3 oxigraph-native | A relationship-pattern + aggregation SPARQL query returns non-empty against the native `oxigraph::Store`; no `pyoxigraph`/Python in the dependency graph; the same query shape against graph-node returns empty (audit reproduced). |
| D4 hexagonal ring | `cargo tree` shows the ADR-090 acyclic ring (domain has no adapter/framework deps); an adapter crate cannot construct a `CanonicalUnit` (compile-time — only `loom-domain` exports the constructor); every port method's return type resolves to an IRI. |
| D5 fusion gated | HNSW fusion is default-off; a lexical hit above `MIN_INJECT_SCORE` triggers no embedding call (hot path LLM-free/network-free); a lexical miss embeds via Xinference and ANN's the in-process projection, hits feed the gate; the −0.40 fixture is a CI regression guard; default-on only after WS-O beats lexical on all axes. |
| D6 clients | The `ruvector-postgres` write channel is `sqlx`, build/off-turn only (never invoked on a `/v1/chat/completions` or `/loom/scaffold` request); the Xinference client is `reqwest` at the two named sites; no raw-SQL/CLI embedding write exists; embed-on-promote is delta-diffed. |
| D7 no builder | `app/pipeline/*`, `ontology_proxy.py`, `test_proxy.py` are absent from the Rust workspace; the Loom serves pre-gated artifacts; the only corpus gate runs in `jjohare/logseq` (`publish.yml`/`enrich-gate.yml`). |
| D8 both profiles | The one binary runs in Profile A (host, `:8084` DNAT'd, in-process hot path serves with the docker network down) and Profile B (sidecar `:8080` on `visionclaw_network`, email gateway `REASONER_BASE_URL` reaches it); `GET /loom/generation` returns a byte-identical descriptor on both for the same `commitSha`. |
| D9 attestation build-time | A gate run writes a chained `MutationLedger` entry (ProofGate, ADR-047); a tampered entry fails verification; no serving request touches the ledger. |
| I-P1 (Prize) | A served answer always resolves to a retrievable per-IRI markdown block with its ontology relations + `corpusNature`; no engine output (vector row, SPARQL binding, ledger entry) is ever served in its place. |

## 9. Cross-reference discipline

ADR-137 is the decision-of-record for the Rust re-platform. **PRD-027** cites it for every substrate call and owns requirements + the WS build order (carrying the **benchmark-first** (−0.40) and **SSOT-single-copy** invariants as gates, and the generation-parity gate for D8). The **DDD (Rust-rev, `ddd-ontology-loom-context.md`)** maps the CanonicalUnit aggregate root onto the `loom-domain` ports and the `ruvector`/`xinference`/`oxigraph`/`backend` adapters, and marks the mesh-coordination context deferred. All three carry the identical shipped-vs-aspirational honesty table and repo-qualify every cross-repo citation (agentbox ADR-051 vs VisionClaw ADR-099; RuVector ADR-001/0027 for HNSW and ADR-047 for ProofGate; the `jjohare/logseq` builder), per PRD-025's citation discipline. THE PRIZE statement is quoted verbatim at the head of each doc as the non-negotiable driver. This ADR **extends** ADR-135/136 and supersedes only ADR-135 D1's stdlib-implementation choice; every other governing decision stands.
