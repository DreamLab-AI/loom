# RUST-ARCHITECTURE — the Ontology Loom serving node, Rust realisation

**Status:** Design (implementation blueprint). **Companions:** [PRD-027](./PRD-027-rust-loom-reengineering.md) (why + scope), [ADR-137](./ADR-137-loom-rust-replatform.md) (the decision + deployment resolution), [ddd-ontology-loom-context.md](./ddd-ontology-loom-context.md) (bounded-context / aggregate mapping). **Supersedes nothing** — it is the buildable projection of the substrate change those three govern.
**Governing prior art (honour, do not relitigate):** [ADR-135](./ADR-135-ontology-loom-node.md) (node boundary, model-is-a-URL, generation discipline, Deployment A/B), [ADR-136](./ADR-136-loom-tooling-allocation.md) (RuVector-behind-the-markdown, keep oxigraph SPARQL, HNSW as gated third signal, gate → ProofGate, Whelk-rs build-time authority).
**Audience:** the implementation mesh. This document is the source you build from. Where it says *port*, write the trait exactly; where it says *shipped* it exists in Python today and is being carried over; where it says *planned/gated* it is new and lands behind a benchmark flag.

---

## 0. THE PRIZE (the invariant every line below is subordinate to)

> The **human-scrutible markdown-with-ontology block** is the canonical, served unit: one per-IRI block of curated research prose (`dfull`) headed by its typed ontology relations, that a human can read, review and audit. RuVector (semantic index), oxigraph (SPARQL), the lexical matcher, Xinference embeddings — all **accelerators behind the markdown**, never replacing it as the served unit.

Structural consequence for this architecture: **every port method returns, or resolves to, an `Iri` that addresses a `CanonicalUnit`.** No adapter is permitted to return a vector row, a triple, a community summary or an embedding *as* the answer. This is **Invariant I-P1** and it is encoded in the port signatures below (§4), not left to reviewer vigilance. If a signature would let an engine's own shape leak out as the served payload, the signature is wrong.

---

## 1. Workspace layout

A single Cargo workspace, `resolver = "2"`, tokio-async, deny-unsafe — matching the sibling Rust repos (`ruvector`, `solid-pod-rs`, `nostr-rust-forum`). Eight crates: one pure domain core, one pure domain policy crate, five adapters, one thin façade binary. This is the VisionClaw ADR-090 hexagonal ring: **domain in the centre, ports as traits in the domain, adapters on the outside, the binary as composition root**.

```
loom/                              # existing repo root; Rust lands alongside docs/, retires app/
├── Cargo.toml                     # [workspace]; shared lints + release profile
├── Cargo.lock                     # committed (binary crate)
├── rust-toolchain.toml            # pinned stable (MSRV 1.89 — see §2.1 note; ruvector-core SIMD needs it)
├── deny.toml                      # cargo-deny: licences + advisories gate
├── flake.nix                      # Nix build (agentbox pattern); musl static target
├── crates/
│   ├── loom-domain/               # PURE. types + ports. no I/O, no framework.
│   ├── loom-scaffold/             # PURE. lexical matcher + confidence-gated injection policy.
│   ├── loom-graph-oxigraph/       # adapter: GraphStore over native oxigraph.
│   ├── loom-vector-ruvector/      # adapter: VectorIndex (in-proc HNSW) + off-turn PG write channel.
│   ├── loom-embed-xinference/     # adapter: EmbeddingProvider → Xinference bge-small/384.
│   ├── loom-backend-openai/       # adapter: ModelBackend → DISTILL_BACKEND_URL (reqwest).
│   ├── loom-attest-proofgate/     # adapter: AttestationLedger → RuVector ProofGate (build/CI-time).
│   └── loom-facade/               # BIN. axum/tower composition root; the two deploy profiles.
├── deploy/
│   ├── Dockerfile                 # multi-stage: cargo-chef build → scratch/distroless static.
│   ├── compose.profile-a.yml      # host-colocated on HP (network_mode: host, :8084).
│   └── compose.profile-b.yml      # sidecar on visionclaw_network (:8080).
└── docs/design/                   # this file + PRD-027 + ADR-137 + DDD.
```

### 1.1 Dependency direction (the ring, enforced by `cargo build`)

```
loom-facade ──depends-on──▶ loom-domain (ports + types)
     │                          ▲
     ├── loom-scaffold ─────────┤     (implements LexicalIndex)
     ├── loom-graph-oxigraph ───┤     (implements GraphStore)
     ├── loom-vector-ruvector ──┤     (implements VectorIndex)
     ├── loom-embed-xinference ─┤     (implements EmbeddingProvider)
     ├── loom-backend-openai ───┤     (implements ModelBackend)
     └── loom-attest-proofgate ─┘     (implements AttestationLedger)

loom-domain depends on NOTHING in the workspace (leaf).
No adapter depends on another adapter. Adapters never depend on loom-facade.
```

The compiler is the enforcement mechanism for the accelerator boundary: an adapter physically cannot reach the router, and the domain physically cannot reach an adapter — so "the accelerator sits behind the markdown" is a *build fact*, not a convention. This is the durable win ADR-136 D1 asked for and Python could only ask nicely for.

---

## 2. Cargo manifests

### 2.1 Workspace root `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/loom-domain",
    "crates/loom-scaffold",
    "crates/loom-graph-oxigraph",
    "crates/loom-vector-ruvector",
    "crates/loom-embed-xinference",
    "crates/loom-backend-openai",
    "crates/loom-attest-proofgate",
    "crates/loom-facade",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.89"   # Erratum (2026-08-17): bumped 1.88→1.89. ruvector-core's `simd` feature (in our explicit feature set since aefa831, §2.1 Erratum C) uses AVX-512 intrinsics (`_mm512_*`) stabilised only in Rust 1.89; on 1.88 the deploy image build fails with E0658 (verified by the deploy agent). Toolchain facts: MSRV floor 1.89; host `machinelearn` = 1.89.0; the container's plain cargo = 1.97.0 (no rustup, ignores rust-toolchain.toml); deploy builder image = `rust:1.97-bookworm`. The sibling repos' 1.88 floor no longer suffices with SIMD on.
license = "AGPL-3.0-only"
repository = "https://github.com/DreamLab-AI/loom"
authors = ["DreamLab-AI contributors"]

[workspace.lints.rust]
unsafe_code = "deny"                       # matches solid-pod-rs; the façade has no cause for unsafe
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }

# One place for versions; every crate uses `x = { workspace = true }`.
[workspace.dependencies]
tokio       = { version = "1.40", features = ["rt-multi-thread", "macros", "net", "signal", "fs", "time"] }
axum        = { version = "0.7",  features = ["json", "http1"] }
tower       = { version = "0.5",  features = ["timeout", "limit"] }
tower-http  = { version = "0.6",  features = ["trace", "cors", "limit"] }
serde       = { version = "1",    features = ["derive"] }
serde_json  = "1"
thiserror   = "1"
anyhow      = "1"
tracing     = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
reqwest     = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
oxigraph    = "0.4"
regex       = "1.10"
sha2        = "0.10"
async-trait = "0.1"
tokio-postgres = "0.7"
pgvector    = { version = "0.4", features = ["postgres"] }
# In-process HNSW: path-dep on the sibling workspace's core crate.
# HNSW search in ruvector-core is behind the `hnsw` Cargo feature (which enables
# the optional `hnsw_rs` dep); persistence is behind `storage` (redb + memmap2).
#
# Erratum (2026-08-17): this block originally said "rely on the crate's default
# feature set". The implemented workspace does NOT — it takes an EXPLICIT feature
# list and turns defaults off, because ruvector-core's default set pulls in
# `api-embeddings`, which drags `reqwest 0.11` and the RUSTSEC advisories that
# trips `cargo deny`. The serving path needs only `hnsw`+`storage` (read) and
# `simd`+`parallel` (speed); it never calls ruvector-core's own embedding client
# (we own `loom-embed-xinference`). Trimming to the four features below is what
# keeps the supply-chain gate green (commit aefa831 — "trim ruvector-core
# features, justify residual advisories"). Do NOT spell the feature "hnsw_rs"
# (that is the dep alias, not the feature name).
ruvector-core = { path = "../ruvector/crates/ruvector-core", default-features = false, features = ["hnsw", "storage", "simd", "parallel"] }

[profile.release]
lto = "thin"          # sibling convention (solid-pod-rs); the static-binary win
codegen-units = 1
strip = "symbols"
panic = "abort"       # façade has no unwinding contract to preserve; smaller binary
```

**`ruvector-core` as a path dependency, deliberately.** It is the **in-process Rust crate `ruvector-core`** — *not* the npm-scoped `@ruvector/core` package (an unrelated JS artefact); the serving path links the Rust crate only, and the Cargo manifest above is the single source of truth for that name. It is not published to crates.io and the ecosystem convention is intra-monorepo path/git deps. The build assumes the `ruvector` sibling repo is checked out beside `loom` (as it is on `machinelearn` and in the Nix build inputs). If the mesh prefers hermetic builds, pin it as a git dep with a rev — either way it is an **in-process** dependency, never a network client (ADR-136 §3: the HNSW read must be network-free on the hot path). Its in-process HNSW (`VectorDB::search`) is gated behind the crate's `hnsw` feature and its file persistence behind `storage` — both in the default feature set, so the plain path-dep above suffices.

### 2.2 Per-crate dependency sketch (rationale attached)

| Crate | Key deps | Why exactly these |
|---|---|---|
| `loom-domain` | `serde`, `thiserror`, `async-trait` | Pure. `async-trait` because ports are async (adapters do I/O); `thiserror` for the typed error enum; `serde` because `CanonicalUnit`/`Generation` cross the wire. **No tokio, no framework** — keeps the core testable in milliseconds and un-coupled to the runtime. |
| `loom-scaffold` | `serde_json`, `regex` | Direct port of `ontology_scaffold.py`. `regex` for the word/slug tokenisers (`_WORD_RE`, `_SLUG_RE`); `serde_json` to load `scaffold-index.json` + `prose-index.json`. **No network deps** — this crate is the LLM-free/network-free hot path. |
| `loom-graph-oxigraph` | `oxigraph`, `regex` | `oxigraph` native store replaces `pyoxigraph` (the clean win). `regex` for the read-only SPARQL clamp (`_FORBIDDEN`/`_READ_FORM`/LIMIT injection, carried verbatim from `loom_graph.py`). |
| `loom-vector-ruvector` | `ruvector-core` (explicit `hnsw`+`storage`+`simd`+`parallel`, defaults **off** — Erratum C above), `tokio-postgres`, `pgvector` | `ruvector-core::VectorDB` for the in-process HNSW read (hot path) — HNSW behind the crate's `hnsw` feature, file persistence behind `storage`; the workspace turns the crate's *defaults* off to shed `api-embeddings`→`reqwest 0.11`. `tokio-postgres` + `pgvector` **only** for the build/off-turn write channel to `ruvector-postgres` — feature-gated (`pg-write`) so the serving binary need not link it. |
| `loom-embed-xinference` | `reqwest`, `serde` | Thin OpenAI-embeddings client to `XINFERENCE_URL`. `rustls-tls` (no OpenSSL system dep — keeps the musl static build clean). |
| `loom-backend-openai` | `reqwest`, `serde_json` | The `DISTILL_BACKEND_URL` delegate. Streams disabled (parity with Python, which pops `stream`); `max_tokens` floor logic lives here. |
| `loom-attest-proofgate` | `sha2` (own `ChainedLedger`) | **Erratum A (2026-08-17):** `ProofGate<T>`/`MutationLedger` are **not** in `ruvector-core` (they live in `ruvector-graph-transformer::proof_gated`, atop `ruvector-verified`), and `ruvector-core::agenticdb::WitnessLog` is unsound as an attestation anchor (see §11.5). The implemented adapter therefore ships **Loom's own `sha2` head-checkpointed `ChainedLedger`** as the attestation substrate; binding the real `ProofGate` is a one-line future rewiring. Build/CI-time only; behind `attest` feature so serving builds omit it. |
| `loom-facade` | `axum`, `tower`, `tower-http`, `tokio`, `tracing*`, `serde_json`, `anyhow` | Composition root + router. `tower-http::limit` for body caps, `tower::timeout` for the long distill timeout, `tracing` for structured logs. |

---

## 3. Core domain types (`loom-domain`)

All in `loom-domain/src/model.rs` unless noted. These are the nouns the whole system speaks. They are `serde`-serialisable because they cross the HTTP boundary and the mirror boundary, but they carry **no I/O**.

```rust
// --- identity ---------------------------------------------------------------

/// A concept-class IRI, e.g. `urn:ngm:class:knowledge-graph`. The addressing key
/// for EVERY CanonicalUnit. Newtype so an adapter can never hand back a bare
/// String and pretend it is an answer (I-P1 at the type level).
#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Iri(String);

impl Iri {
    /// The kebab slug after the last `:` — the join key across ttl / scaffold /
    /// prose / HNSW projections. Carries the `_ref_to_slug` rule from Python.
    pub fn slug(&self) -> &str { self.0.rsplit(':').next().unwrap_or(&self.0) }
    pub fn from_slug(slug: &str) -> Self { Iri(format!("urn:ngm:class:{slug}")) }
    pub fn as_str(&self) -> &str { &self.0 }
}

// --- THE PRIZE, as a type ---------------------------------------------------

/// The canonical served unit — a per-IRI markdown-with-ontology block.
/// This is the aggregate root of the bounded context (see DDD). Everything
/// else is a projection that resolves back to `iri`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CanonicalUnit {
    pub iri: Iri,
    pub title: String,                 // scaffold "t"
    pub definition: String,            // scaffold "d" (<=400 chars, truncated)
    pub dfull: Option<String>,         // prose "dfull" — untruncated curated prose (THE PRIZE body)
    pub landscape: Option<String>,     // prose "cl" — Current Landscape research prose
    pub domain: Option<String>,        // "dom"
    pub maturity: Option<String>,      // "m"
    pub quality: Option<f32>,          // "q"
    pub is_a: Vec<Iri>,                // "sup" (direct parents)
    pub ancestors: Vec<Iri>,           // "isup" (inferred ancestors from the reasoned closure)
    pub relations: Vec<Relation>,      // typed ontology-relation header ("rel")
    pub backlinks: Vec<Iri>,           // "bl"
    pub corpus_nature: CorpusNature,   // provenance stamp (see below)
    pub generation: GenerationId,      // which build this unit belongs to
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Relation { pub predicate: RelationKind, pub targets: Vec<Iri> }

/// The 12 ordered relation types from REL_ORDER + an open Other(String) tail.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RelationKind {
    HasPart, Requires, Enables, DependsOn, Implements, Uses,
    PartOf, RelatedTo, BridgesTo, Supports, StandardizedBy, ContrastsWith,
    Other(String),
}

/// corpusNature: synthetic-ai-generated-human-directed (PRD frame). The
/// provenance the reviewer needs to trust the prose. Never dropped on serialise.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CorpusNature { SyntheticAiGeneratedHumanDirected }

// --- the retrieval nouns ----------------------------------------------------

/// One scored candidate from ANY retriever. `iri` is mandatory; `score` is the
/// retriever's own confidence in the retriever's own scale. The `provenance`
/// records WHICH engine surfaced it — audit + the fusion telemetry.
#[derive(Clone)]
pub struct ConceptMatch {
    pub iri: Iri,
    pub score: f32,
    pub provenance: MatchProvenance,
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum MatchProvenance { Lexical, SemanticHnsw }

/// The assembled, budget-clamped `[ONTOLOGY CONTEXT] … [END …]` block — the
/// exact string injected into the system message. Carries the grounding
/// telemetry `meta_out` produced in Python, now typed.
#[derive(Clone, serde::Serialize)]
pub struct Scaffold {
    pub block: String,                 // the markdown; "" ⇒ nothing injected
    pub engaged: bool,
    pub approx_tokens: usize,
    pub seeds: Vec<ConceptMatch>,      // which units injected, with scores + provenance
    pub top_score: f32,
    pub effective_budget: usize,
    pub fusion_path: FusionPath,       // lexical-only | semantic-fallback | none
    pub generation: GenerationId,
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum FusionPath { LexicalHit, SemanticFallback, NoMatch }

// --- the generation boundary (ADR-135 D2.1) ---------------------------------

/// Content-addressed corpus snapshot identity. Two units with different
/// GenerationId must NEVER be served together (never-mixed-build).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GenerationId(pub String);   // commitSha||buildId, or the mirror generation ISO stamp

/// The full generation descriptor the mirror promotes and /health reports.
/// Byte-identical across Profile A and Profile B for the same commitSha is a
/// CI/health assertion (ADR-137 §sidecar consequence 3).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Generation {
    pub id: GenerationId,
    pub source: GenerationSource,          // build-manifest | mirror-manifest | scaffold-index
    pub generated_at: Option<String>,
    pub commit_sha: Option<String>,
    pub promoted_at: Option<String>,
    pub cluster_span_seconds: Option<f64>,
    pub artifacts: Vec<ArtifactSha>,       // per-artifact sha256 (never-mixed proof)
    pub verified_single_generation: bool,
    pub class_count: Option<usize>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ArtifactSha { pub name: String, pub sha256: String, pub bytes: u64 }

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum GenerationSource { BuildManifest, MirrorManifest, ScaffoldIndex, Unavailable }

// --- the gate verdict (ADR-136 D5) ------------------------------------------

/// Outcome of a domain-predicate check (SSOT/conflict predicates stay Loom-owned).
/// On the build/CI path this becomes a chain-hashed AttestationLedger entry.
#[derive(Clone, serde::Serialize)]
pub struct GateVerdict {
    pub predicate: String,             // e.g. "class_count_parity", "no_mixed_generation"
    pub passed: bool,
    pub detail: Option<String>,
    pub subject: Option<Iri>,          // the unit/generation the predicate ran over
}
```

**Serialisation contract:** `CanonicalUnit` serialises to the *markdown block* on the serve path (via `loom-scaffold`'s serialiser, §5), and to JSON only for machine consumers of `/loom/search`. The markdown is the source of truth; JSON is a courtesy view. `dfull`, `landscape` and `corpus_nature` are **never** dropped by a "compact" serialiser — that would degrade legibility, which the frame forbids.

---

## 4. Ports (traits in `loom-domain/src/ports.rs`)

The ports carry the DDD/PRD/ADR ubiquitous language exactly — `LexicalIndex` / `VectorIndex` / `GraphStore` / `EmbeddingProvider` / `ModelBackend`, plus `GenerationStore` and the build/CI `AttestationLedger`. All async (`async_trait`), all fallible with the domain error (§7). **Read the return types as the enforcement of I-P1**: every retrieval port yields `Iri`/`ConceptMatch`/`CanonicalUnit`, never a raw engine artefact.

```rust
use async_trait::async_trait;
use crate::model::*;
use crate::error::LoomError;

/// LEXICAL PRIMARY. The inverted-index matcher (loom-scaffold). Sole authority
/// over the confidence gate: it decides WHICH units inject and how much budget.
/// Implemented by loom-scaffold::LexicalRetriever.
#[async_trait]
pub trait LexicalIndex: Send + Sync {
    /// Score the query against the 8,146 class titles; return seeds above the
    /// gate. `budget_tokens`/`max_seeds`/`hops` carry the Python knobs.
    async fn seeds(&self, query: &str, max_seeds: usize) -> Result<Vec<ConceptMatch>, LoomError>;

    /// Assemble the served markdown from a set of seed candidates (from ANY
    /// source — lexical or handed-back HNSW), applying the confidence-gated
    /// selective-injection policy and the budget clamp. THE gate. Nothing
    /// bypasses this to inject.
    async fn assemble(
        &self,
        query: &str,
        candidates: &[ConceptMatch],
        opts: ScaffoldOpts,
    ) -> Result<Scaffold, LoomError>;

    /// Resolve an IRI to its full CanonicalUnit (markdown body source). The
    /// address→unit step that keeps every projection honest.
    fn resolve(&self, iri: &Iri) -> Option<CanonicalUnit>;

    fn generation(&self) -> Generation;
    fn class_count(&self) -> usize;
}

/// SEMANTIC FALLBACK (planned/gated). In-process HNSW over the ontology-corpus
/// namespace. Called ONLY on a lexical miss. Returns IRI-keyed candidates that
/// are handed BACK to LexicalIndex::assemble — never injected directly.
/// Implemented by loom-vector-ruvector::HnswIndex.
#[async_trait]
pub trait VectorIndex: Send + Sync {
    /// ANN over the embedded query vector. `k` bounded. Cosine. Each hit carries
    /// its IRI (primary key) and cosine score (∈ [0,1]).
    async fn nearest(&self, query_vec: &[f32], k: usize) -> Result<Vec<ConceptMatch>, LoomError>;
    fn is_ready(&self) -> bool;             // false ⇒ fusion degrades to lexical-only
    fn generation(&self) -> Generation;     // parity with the lexical generation is asserted
}

/// SPARQL over the Whelk-reasoned closure. Read-only, clamped. Native oxigraph.
/// Implemented by loom-graph-oxigraph::OxigraphStore.
#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn query(&self, sparql: &str) -> Result<SparqlResult, LoomError>;
    async fn search_labels(&self, needle: &str, limit: usize) -> Result<Vec<LabelHit>, LoomError>;
    fn status(&self) -> GraphStatus;        // available|triples|loaded_files|error → /health
}

/// Query-time + build-time embeddings. Xinference bge-small-en-v1.5/384 (LOCKED).
/// Implemented by loom-embed-xinference::XinferenceEmbedder.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, LoomError>;   // 384-dim, or LoomError::Dimension
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LoomError>;
    fn model_id(&self) -> &str;             // asserted == "bge-small-en-v1.5" (ops law)
    fn dimensions(&self) -> usize;          // asserted == 384
}

/// The model-swap seam. OpenAI-compatible chat delegation to DISTILL_BACKEND_URL.
/// Implemented by loom-backend-openai::OpenAiBackend.
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Delegate a (scaffold-injected) chat request. Floors max_tokens ≥ floor.
    async fn chat(&self, body: serde_json::Value) -> Result<BackendResponse, LoomError>;
    async fn models(&self) -> Result<serde_json::Value, LoomError>;   // /v1/models passthrough
    async fn reachable(&self) -> bool;      // /health probe (5s)
    fn endpoint(&self) -> &str;             // the URL — model identity NEVER encoded here (ADR-135 D1.2)
}

/// The generation identity source + the atomic mirror commit marker.
/// Implemented by loom-facade::MirrorStore (reads data/.generation.json etc).
#[async_trait]
pub trait GenerationStore: Send + Sync {
    fn current(&self) -> Generation;                        // best-source-first (build-manifest → mirror → scaffold)
    async fn verify_atomicity(&self) -> Result<(), LoomError>;   // all artifact shas verify, one generation
}

/// Build/CI-time attestation of gate verdicts onto ProofGate/MutationLedger.
/// NOT on the serving hot path. Implemented by loom-attest-proofgate.
#[async_trait]
pub trait AttestationLedger: Send + Sync {
    async fn attest(&self, verdict: &GateVerdict) -> Result<LedgerEntryId, LoomError>;
    async fn verify_chain(&self) -> Result<bool, LoomError>;     // tamper check
}
```

Supporting value types (`ScaffoldOpts { budget_tokens, hops, prose, confidence_injection }`, `SparqlResult`, `LabelHit`, `GraphStatus`, `BackendResponse`, `LedgerEntryId`) live in `loom-domain/src/model.rs` alongside the aggregate.

---

## 5. `loom-scaffold` — the lexical matcher + injection gate (direct port)

This is the load-bearing port of `ontology_scaffold.py`, 1:1 with the Python semantics. It is **pure** and it is **the gate**.

### 5.1 Modules

```
loom-scaffold/src/
├── lib.rs          # LexicalRetriever (impls LexicalIndex); get_index singleton
├── index.rs        # ScaffoldIndex: inverted title-word index; loads scaffold-index.json v1
├── match_.rs       # match(): exact n-gram + inverted overlap + slug-substring scoring
├── policy.rs       # confidence-gated selective injection (STRONG/MIN_INJECT/FRACTION)
├── serialise.rs    # _section_for + _clamp → the [ONTOLOGY CONTEXT] markdown block
├── prose.rs        # prose-index.json loader (dfull + cl); fail-open on missing
└── tuning.rs       # all knobs as consts, env-overridable (parity table §5.4)
```

### 5.2 `ScaffoldIndex` (index.rs) — the same inverted index, typed

```rust
pub struct ScaffoldIndex {
    generated: String,
    classes: HashMap<String /*slug*/, ClassEntry>,
    by_title: HashMap<String /*slugified title*/, String /*slug*/>,
    inverted: HashMap<String /*title word*/, HashSet<String /*slug*/>>,
    title_len: HashMap<String, usize>,
    slugs: Vec<String>,
}
```

`ClassEntry` is the on-disk `scaffold-index.json` shape (serde-derived: `t,d,dom,q,m,sup,isup,rel,bl`). Load is O(index size); `match_` touches only inverted-index-surfaced classes plus one slug-substring pass — the **<50ms over 8,146 classes** budget is a benchmark assertion carried from the Python self-test (§8.3). Rust removes the interpreter overhead, so this is expected to be materially faster; the gate is *≤50ms* regardless, so we do not overclaim.

### 5.3 `match_` scoring (identical weights, ported constant-for-constant)

```rust
// tuning.rs — env-overridable, defaults byte-identical to Python
pub const MIN_SEED_SCORE: f32     = 2.0;
pub const EXACT_TITLE_WEIGHT: f32 = 8.0;
pub const OVERLAP_WEIGHT: f32     = 2.0;
pub const SUBSTRING_WEIGHT: f32   = 0.75;
pub const SUBSTRING_MIN_LEN: usize = 5;
pub const MAX_NGRAM: usize        = 4;
// … ISUP_CAP=5, REL_CAP=3, NEIGHBOUR_DEFS=2, NEIGHBOUR_DEF_CHARS=220 …
```

`match(query, max_seeds)` returns `Vec<ConceptMatch>` (provenance `Lexical`), sorted by `(-score, -quality, slug)` — the exact tri-key from Python. The three scoring stages (exact n-gram title/slug, inverted title-word overlap normalised by title length, slug-substring for terms ≥5 chars) are ported verbatim; the stopword set and `_WORD_RE`/`_SLUG_RE` regexes move to `regex` crate literals.

### 5.4 The confidence gate (policy.rs) — the sole injection authority

```rust
pub struct InjectionPolicy {
    confidence_injection: bool,   // LOOM_CONFIDENCE_INJECTION
    strong_match_score: f32,      // LOOM_STRONG_MATCH_SCORE (default EXACT_TITLE_WEIGHT)
    min_inject_score: f32,        // LOOM_MIN_INJECT_SCORE  (default MIN_SEED_SCORE)
    min_inject_fraction: f32,     // LOOM_MIN_INJECT_FRACTION (default 0.4)
}

impl InjectionPolicy {
    /// Given the top candidate score + requested budget, decide the EFFECTIVE
    /// budget (or reject). Byte-identical to Python's scaffold() gate branch.
    fn effective_budget(&self, top_score: f32, requested: usize) -> Option<usize> {
        if !self.confidence_injection { return Some(requested); }        // opt-in; default off
        if top_score < self.min_inject_score { return None; }            // skip: below MIN_INJECT_SCORE
        let frac = if self.strong_match_score > 0.0 {
            (top_score / self.strong_match_score).clamp(self.min_inject_fraction, 1.0)
        } else { 1.0 };
        Some(((requested as f32) * frac).max(1.0) as usize)
    }
}
```

**This method is the gate named in Invariant I-P1 / ADR-136 D3.** Both lexical seeds and (planned) HNSW-fallback candidates flow through `LexicalIndex::assemble`, which calls `effective_budget` before serialising. **There is no other code path that injects.** The fusion pipeline (§6) is a candidate-*union* feeding this one gate — never a bypass.

### 5.5 Serialiser (serialise.rs) — produces THE PRIZE, on the wire

`_section_for` and `_clamp` port directly: `## Title (domain, maturity)`, then `dfull` (prose) or `d` (structural), then `is-a`/`ancestors`, then `relations`, then 1-hop neighbour defs, then `landscape` prose last (so the end-trimming clamp keeps structural facts under pressure). Header `[ONTOLOGY CONTEXT]` / footer `[END ONTOLOGY CONTEXT]`. **The output string is the CanonicalUnit's served markdown** — this is where THE PRIZE reaches the wire, and the serialiser must never emit an encoding the Python one would not.

---

## 6. The retrieval-fusion pipeline (code-shaped)

Lives in `loom-facade/src/fusion.rs` as the orchestration wiring — it holds *no policy*, it only sequences ports. The gate stays in `loom-scaffold`. Default **OFF** (`LOOM_SEMANTIC_FALLBACK=0`) until the WS-O multivariate bench clears it (§8.4); the −0.40 over-retrieval result (n=285, 5 models, haiku −1.30) is the standing regression guard.

```rust
/// lexical primary → gate → (miss?) semantic fallback → gate → assemble markdown.
/// Returns a Scaffold whose `.block` is the served per-IRI markdown, or empty.
async fn build_scaffold(
    ctx: &AppState,
    query: &str,
    opts: ScaffoldOpts,
) -> Result<Scaffold, LoomError> {
    // (1) LEXICAL PRIMARY — inverted-index match over 8,146 titles. Hot, LLM-free.
    let lexical = ctx.retriever.seeds(query, opts.max_seeds).await?;
    let top = lexical.first().map(|m| m.score).unwrap_or(0.0);

    // (2) GATE — if the lexical top clears MIN_INJECT_SCORE, assemble as today.
    //     NO embedding call. Hot path stays network-free.
    if top >= ctx.policy.min_inject_score {
        return ctx.retriever
            .assemble(query, &lexical, opts.with_path(FusionPath::LexicalHit))
            .await;
    }

    // (3) SEMANTIC FALLBACK — only on a lexical miss / below-gate score, and only
    //     when enabled + ready. This is the OOV/paraphrase gap the matcher
    //     structurally misses (ADR-136 D3). Off by default.
    if ctx.semantic_fallback_enabled && ctx.semantic.is_ready() {
        // generation parity guard: never fuse across builds (never-mixed).
        if ctx.semantic.generation() != ctx.retriever.generation() {
            tracing::warn!("semantic index generation != lexical; skipping fallback");
        } else {
            let qvec = ctx.embedder.embed(query).await?;                 // Xinference bge/384
            let hits = ctx.semantic.nearest(&qvec, opts.k_semantic).await?;  // in-proc HNSW, IRI-keyed
            // (4) HANDED BACK INTO THE GATE — HNSW hits are candidate SEEDS, not
            //     an injection. The SAME policy decides whether/how much injects.
            //     Union with any weak lexical candidates; dedupe by IRI.
            let candidates = union_dedupe_by_iri(&lexical, &hits);
            if let Some(best) = candidates.first() {
                if best.score_normalised() >= ctx.policy.semantic_min_inject {
                    return ctx.retriever
                        .assemble(query, &candidates, opts.with_path(FusionPath::SemanticFallback))
                        .await;
                }
            }
        }
    }

    // (5) NO MATCH — return empty scaffold; caller falls back to the raw prompt.
    Ok(Scaffold::empty(FusionPath::NoMatch, ctx.retriever.generation()))
}
```

**Invariant checkpoints in this function:**
- Step (4) is the ADR-136 D3 rule made structural: *HNSW is a candidate source, never a gate bypass.* The only way a semantic hit reaches the wire is through `retriever.assemble`, which resolves each `Iri` to a `CanonicalUnit` and serialises **its markdown**. No vector, no cosine score, no summary is ever served.
- Step (3)'s generation-parity guard enforces never-mixed-build across the lexical and semantic projections (both are stamped with the same `buildId`; a mismatch means one lags a promote and fusion is skipped, not silently blended).
- `score_normalised()` maps the two retrievers onto a comparable scale before the gate — cosine ∈ [0,1] from HNSW vs the lexical additive score. The normalisation constant is itself a bench-tuned knob (`LOOM_SEMANTIC_SCORE_SCALE`), not a guess baked in; until the bench sets it, fallback stays off.

---

## 7. Error model (`loom-domain/src/error.rs`)

One typed error enum, `thiserror`-derived, mapped to HTTP at the router edge. **Fail-open on the channel, fail-labelled on the payload** (ADR-135 liveness rule): a missing accelerator degrades the answer and is reported, it does not 500 the request.

```rust
#[derive(thiserror::Error, Debug)]
pub enum LoomError {
    #[error("scaffold index unavailable: {0}")]        IndexUnavailable(String),
    #[error("graph store unavailable: {0}")]           GraphUnavailable(String),  // fail-open → lexical
    #[error("bad sparql: {0}")]                        BadQuery(String),          // 400
    #[error("semantic index not ready: {0}")]          SemanticUnready(String),   // fail-open → lexical
    #[error("embedder error: {0}")]                    Embed(String),             // 502 on /embed; skip on fallback
    #[error("embedding dimension mismatch: got {got}, want {want}")]
                                                       Dimension { got: usize, want: usize },
    #[error("no DISTILL_BACKEND_URL configured")]      NoBackend,                 // 503
    #[error("backend unreachable: {0}")]               BackendUnreachable(String),// 502
    #[error("backend http {status}: {body}")]          BackendHttp { status: u16, body: String },
    #[error("generation not atomic: {0}")]             GenerationDrift(String),   // mirror reject
    #[error("attestation failed: {0}")]                Attest(String),            // build/CI only
    #[error(transparent)]                              Io(#[from] std::io::Error),
    #[error(transparent)]                              Json(#[from] serde_json::Error),
}
```

Router mapping (in `loom-facade/src/error.rs`, `impl IntoResponse for LoomError`):

| Variant | HTTP | Behaviour |
|---|---|---|
| `BadQuery` | 400 | reject the SPARQL/search |
| `NoBackend` | 503 | `/v1/*` with no `DISTILL_BACKEND_URL` |
| `BackendUnreachable`, `BackendHttp` | 502 | propagate the model's failure, labelled |
| `GraphUnavailable`, `SemanticUnready` | **not raised to the client** | degrade to lexical; surfaced in `/health` and in `Scaffold.fusion_path` |
| `IndexUnavailable` | 500 | the lexical index is the floor; if it is gone the node cannot serve its purpose |
| everything else | 500 | logged with `tracing`, generation stamp attached |

The distinction is deliberate: the **lexical index is the hard floor** (losing it is a real 500); the graph store and the semantic index are **accelerators**, so their absence is a degrade-and-report, never a client error. That is the accelerator boundary expressed in the error model.

---

## 8. Test strategy

Three layers, all under `cargo test --all-features` + `cargo clippy --all-targets -- -D warnings` in CI, plus the Nix build. Sibling convention: unit tests inline (`#[cfg(test)] mod tests`), integration tests in `crates/*/tests/`, benches in `benches/`.

### 8.1 Unit — pure domain, fast

- `loom-scaffold`: **port the entire `_selftest()` fixture** (the 7-class inline fixture + all its assertions — wrapper present, seed section, is-a line, relations line, 1-hop neighbour defs, seed-not-repeated, hops=0 suppression, irrelevant→empty, budget clamp trims sections, impossible budget→empty, `scaffold_messages` insert/merge/parts/no-match) as Rust `#[test]`s. This is the correctness anchor: **Rust output must be byte-identical to Python** on the fixture. A golden-file test pins the exact `[ONTOLOGY CONTEXT]` block string.
- `policy.rs`: table-test `effective_budget` across (`confidence_injection` on/off × top_score below/at/above thresholds) — proves the gate math matches Python's branch exactly.
- `loom-graph-oxigraph`: the SPARQL clamp — `_FORBIDDEN` rejects INSERT/DELETE/LOAD/CLEAR/DROP/SERVICE; `_READ_FORM` requires SELECT/ASK/CONSTRUCT/DESCRIBE; LIMIT injection on unclamped SELECT; row cap truncation flag. **Erratum D (2026-08-17, audit finding 3, EXP-004):** the implemented clamp is *stronger than the Python original by design* — a naive `^\s*SELECT` test let a `PREFIX ex:<…> SELECT …` (or `BASE`/comment-prologue-led) query slip past LIMIT injection and evaluate unbounded until the post-hoc row cap. The clamp is a **security control, not a parity feature**, so the Rust version consumes any leading `BASE`/`PREFIX`/comment prologue and injects LIMIT for those SELECTs too — a deliberate, documented divergence from Python (`crates/loom-graph-oxigraph/src/lib.rs`; 4 new clamp tests).
- `Iri::slug` / `Iri::from_slug` round-trips; `_ref_to_slug` equivalence for `urn:ngm:class:<slug>` and bare slug.

### 8.2 Integration — adapters against real deps (feature-gated, CI services)

- `loom-graph-oxigraph/tests/`: load a fixture `ontology.ttl` + `ontology-inferred.ttl`, run a relationship-pattern + aggregation SPARQL, assert non-empty (the ADR-136 D2 verification: this shape returns empty on `@ruvector/graph-node`, non-empty here — the reason oxigraph stays).
- `loom-embed-xinference/tests/`: against a live `XINFERENCE_URL`, assert `dimensions()==384` and `model_id()=="bge-small-en-v1.5"` (ops-law lock); dimension-mismatch path returns `LoomError::Dimension`.
- `loom-backend-openai/tests/`: against a mock OpenAI server (`wiremock`), assert `max_tokens` floor (256 → ≥1536), `stream` stripped, `loom` annotation block attached on 200, 502 on unreachable.
- `loom-facade/tests/`: spin the axum app with an in-memory `AppState` (fixture index, no backend) and hit every route (§9) with `tower::ServiceExt::oneshot` — `/health` shape, `/loom/scaffold` engaged/empty, `/loom/generation`, `/loom/sparql` clamp, `/v1/chat/completions` with `NoBackend`→503.

### 8.3 Performance gate

- Criterion bench in `loom-scaffold/benches/match.rs`: build an 8k-class synthetic index (the Python self-test's generator, ported), assert `match()` p99 **< 50ms**. Regression-fails the build if breached.

### 8.4 The recall-gate integration test (the ground-truth wiring gate)

This is the test that governs whether the semantic fallback may go default-on. Lives in `loom-vector-ruvector/tests/recall_gate.rs`, feature `semantic-fallback`, run against a live `ontology-corpus` namespace (or a checked-in HNSW artifact fixture).

> **Erratum D (2026-08-17, audit finding 5, EXP-008) — the recall gate is honestly RED; the `0.87` below is the flip-on precondition, not a measured pass.** The fixture asserts `rgb-protocol ≥ 0.87` as the *design floor*, but the measured recall in the current document-embedding regime is **`0.816`**. That is below floor, so the gate is **RED** and `LOOM_SEMANTIC_FALLBACK` stays **default-off** — the correct, honest state. The evidence verdict was corrected from a misleading "PASS" to **"WIRING PASS — DESIGN FLOOR NOT MET (recall gate RED)"**, and the test was hardened accordingly: with `LOOM_SEMANTIC_FALLBACK=1` it fails RED unless `rgb_score ≥ LOOM_SEMANTIC_RECALL_FLOOR` (default `0.87`); with the flag off it asserts the wiring invariants **and** that the gate is *reported red* — a staleness tripwire that will fail the day recall improves, forcing the evidence to be refreshed. Closing the gap needs a query-shaped embedding (or a bench-justified floor), not a threshold fudge (see `.claude/evidence/EXP-008.evidence.md`, `AUDIT-gpt54.md` finding 5).

```rust
/// ADR-136 D3 / D8 verification, mechanised. The semantic fallback ships
/// default-OFF and only flips on when THIS test's multivariate bench clears.
#[tokio::test]
async fn recall_gate() {
    let idx = load_ontology_corpus_hnsw();   // 8,146 IRI-keyed bge/384 records, cosine
    let embed = XinferenceEmbedder::from_env();

    // Axis 1 — in-domain recall. The validated numbers are the acceptance floor:
    //   rgb-protocol query ⇒ correct IRI in top-k with cosine ≥ 0.87;
    //   decoy query ⇒ nearest cosine ≈ 0.45 (must NOT clear the gate).
    let q = embed.embed("rgb protocol").await.unwrap();
    let hits = idx.nearest(&q, 5).await.unwrap();
    assert!(hits.iter().any(|h| h.iri.slug() == "rgb-protocol" && h.score >= 0.87));
    let decoy = embed.embed("<off-ontology decoy>").await.unwrap();
    assert!(idx.nearest(&decoy, 5).await.unwrap()[0].score < 0.55);  // stays below the inject gate

    // Axis 2 — OOV/paraphrase recovery: a paraphrase the LEXICAL matcher misses
    // (asserted score < MIN_INJECT_SCORE) is recovered by the fallback to the
    // correct IRI. This is the gap the wiring exists to close.
    assert!(lexical_misses("colour-channel protocol"));
    let para = embed.embed("colour-channel protocol").await.unwrap();
    assert!(idx.nearest(&para, 5).await.unwrap().iter().any(|h| h.iri.slug() == "rgb-protocol"));

    // Axis 3 — non-jaggedness guard (the −0.40 regression): the fused path must
    // NOT degrade the general-question baseline. Replays the WS-O fixture and
    // asserts fused_score >= lexical_baseline on EVERY axis before flip-on.
    let bench = replay_ws_o_fixture(&idx, &embed).await;
    assert!(bench.fused_beats_lexical_on_all_axes(),
            "semantic fallback stays OFF until it beats lexical on recall AND non-jaggedness AND OOV");
}
```

The test **passing** is the precondition for setting `LOOM_SEMANTIC_FALLBACK=1` as a default; until then it runs in CI as a gate on the *artifact/config*, not on the code. This mechanises ADR-136 D3's "benchmark-gated, default-off" clause.

---

## 9. The axum router (`loom-facade`) — endpoint parity

Mirrors the current stdlib façade exactly (`loom_facade.py` do_GET/do_POST), same paths, same aliases. `axum::Router` with `AppState` (an `Arc` bundle of the port trait objects) as extension state. `tower_http::limit::RequestBodyLimitLayer` caps bodies; `tower::timeout::TimeoutLayer` carries `LOOM_TIMEOUT` (default 600s — distillation is slow by design); `tower_http::cors` mirrors the `Access-Control-Allow-Origin: *`.

```rust
Router::new()
    .route("/health",               get(health))            // liveness + generation + backend + graph + index
    .route("/loom/generation",      get(generation))
    .route("/generation",           get(generation))         // alias (Python parity)
    .route("/loom/scaffold",        post(scaffold))          // retrieval, NO LLM — the fusion pipeline (§6)
    .route("/scaffold",             post(scaffold))          // alias
    .route("/loom/sparql",          post(sparql))            // read-only clamped SPARQL over the closure
    .route("/sparql",               post(sparql))            // alias
    .route("/loom/search",          post(search))            // label/substring search over the store
    .route("/search",               post(search))            // alias
    .route("/loom/search/semantic", post(semantic_search))   // NEW: exposes the HNSW as an IRI list (gated)
    .route("/v1/chat/completions",  post(chat_completions))  // scaffold-inject → delegate (model-swap seam)
    .route("/v1/models",            get(models))             // passthrough (identity probe)
    .with_state(app_state)
    .layer(TimeoutLayer::new(Duration::from_secs(cfg.timeout_secs)))
    .layer(RequestBodyLimitLayer::new(cfg.max_body_bytes))
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http());
```

Handler notes (parity-critical):
- `chat_completions`: read `messages`, run `build_scaffold` (§6) on the **last user message**, merge the block into the system message (or insert one at position 0), floor `max_tokens`/`max_completion_tokens` ≥ `LOOM_MIN_MAX_TOKENS` (never lower a higher ask), strip `stream`, delegate via `ModelBackend::chat`, then annotate the 200 JSON with `loom: { mode, injected_tokens, grounding, fusion_path, generation }` — the fail-labelled honesty block Python emits.
- `scaffold`: returns `{ scaffold, engaged, approx_tokens, seeds:[{iri,score,provenance}], fusion_path, generation }`. New over Python: `seeds` + `fusion_path` expose which IRIs grounded the answer and by which engine (audit surface for THE PRIZE).
- `health`: `{ ok, facet, mode, backend, backend_reachable, index_classes, graph:{available,triples,loaded_files,error}, semantic:{ready,generation}, generation }` — superset of Python's, adding the semantic-index readiness + generation-parity fields.
- `semantic_search` (new): returns a **list of IRIs + cosine scores only** — deliberately *not* markdown, because it is a debugging/eval surface, and it is the one endpoint where the raw index shape is allowed to show precisely because it is labelled as the index, not as an answer. It never feeds `/v1/chat/completions`; that path always goes through the gate. **Erratum D (2026-08-17, audit finding 1, EXP-007):** as first implemented this endpoint returned raw `nearest()` hits *always*, which technically put a second surface next to the single gate. It is now **default-OFF** (`LOOM_SEMANTIC_DEBUG_SURFACE=0`) — the route answers `404 {"error":"semantic debug surface disabled"}` unless explicitly enabled, so the single-gate invariant (I-P1) holds by default and the labelled index-debug view is opt-in.

---

## 10. Configuration / environment surface

One `Config` struct (`loom-facade/src/config.rs`), `figment`- or hand-parsed from env with the defaults below. Every knob keeps its Python name so operators and existing compose files carry over unchanged.

| Env var | Default | Owner crate | Meaning |
|---|---|---|---|
| `LOOM_FACADE_PORT` | `8080` | facade | listen port (Profile A DNATs `:8084`→ this) |
| `DISTILL_BACKEND_URL` | *(empty)* | backend | **the model-swap seam.** OpenAI base, e.g. `http://127.0.0.1:8085/v1`. Empty ⇒ retrieval-only node. |
| `LOOM_TIMEOUT` | `600` | facade | backend delegate timeout (s) — distillation is slow by design |
| `LOOM_MIN_MAX_TOKENS` | `1536` | backend | floor for reasoning backends (400→empty trap); `0` disables |
| `ONTOLOGY_INDEX` | `/app/data/scaffold-index.json` | scaffold | lexical index path |
| `ONTOLOGY_PROSE_INDEX` | `/app/data/prose-index.json` | scaffold | prose (`dfull`+`cl`) path; fail-open |
| `ONTOLOGY_BUDGET` | `1500` | scaffold | default scaffold token budget |
| `LOOM_CONFIDENCE_INJECTION` | `0` | scaffold | opt-in confidence-gated injection (off ⇒ Python-baseline behaviour) |
| `LOOM_STRONG_MATCH_SCORE` | `8.0` | scaffold | full-budget-at/above threshold |
| `LOOM_MIN_INJECT_SCORE` | `2.0` | scaffold | below-this top score ⇒ skip injection |
| `LOOM_MIN_INJECT_FRACTION` | `0.4` | scaffold | weakest match still gets this budget fraction |
| `LOOM_SPARQL_LIMIT` | `10000` | graph | injected LIMIT on unclamped SELECT |
| `LOOM_SPARQL_MAX_ROWS` | `10000` | graph | server-side row cap |
| `LOOM_SEMANTIC_FALLBACK` | `0` | facade | **master switch for the HNSW fallback.** Stays `0` until the recall-gate test (§8.4) clears. |
| `LOOM_HNSW_ARTIFACT` | `/app/data/ontology-corpus.rvdb` | vector | **ruvector-core storage DB path** (redb), mirrored per generation. `VectorDB::new(DbOptions{storage_path,..})` auto-rebuilds the HNSW index from the persisted vectors on open (verified in `vector_db.rs`) — it is NOT a serialised HNSW graph file. |
| `LOOM_SEMANTIC_K` | `5` | vector | ANN k for the fallback |
| `LOOM_SEMANTIC_MIN_INJECT` | *(unset)* | facade | normalised cosine gate for fallback candidates; **must be set by the bench**, no default |
| `LOOM_SEMANTIC_SCORE_SCALE` | *(unset)* | facade | lexical↔cosine normalisation; bench-tuned |
| `XINFERENCE_URL` | `http://xinference:9997/v1` | embed | bge-small-en-v1.5/384 endpoint (LOCKED model) |
| `RUVECTOR_PG_CONNINFO` | *(from env)* | vector (`pg-write`) | **build/off-turn write channel only** — never the query hot path |
| `LOOM_DEPLOY_PROFILE` | `a` | facade | `a` (host-colocated) or `b` (sidecar); selects health assertions + bind defaults |

**Boundary rule encoded in config:** `RUVECTOR_PG_CONNINFO` is read **only** by the `pg-write` feature of `loom-vector-ruvector`, which is compiled out of the serving binary by default. A Profile-A node cut off from the docker network still serves fully because the query path never touches Postgres — it reads `LOOM_HNSW_ARTIFACT` in-process (ADR-136 §3, DDD §6.1).

**Addendum (2026-08-18), findings-driven serving controls.** The measurement in [`docs/research/paper-v2/main.pdf`](../research/paper-v2/main.pdf) (*The Copy Ceiling*) adds three config knobs, all default-off (so the §9 flow above is unchanged unless a deployment opts in). Implemented in `loom-facade/src/config.rs` (`serving.rs`, `loom-scaffold/src/exposure.rs`); names below are as landed.

| Env var | Default | Owner crate | Meaning |
|---|---|---|---|
| `LOOM_VERBATIM_MODE` | `0` | facade/scaffold | on a gate-engaged lookup whose `top_score` clears `LOOM_VERBATIM_THRESHOLD`, `/v1/chat/completions` serves the canonical markdown block **with no backend call**; multi-turn and streaming bypass it; per-request opt-out `"loom_options":{"verbatim":false}` (serving-regime finding) |
| `LOOM_VERBATIM_THRESHOLD` | `8.0` | facade/scaffold | lexical `top_score` floor for the short-circuit; 8.0 is the exact-title weight, so the default admits only full exact-title matches and paraphrase/overlap hits still delegate |
| `LOOM_EXPOSURE_APPEND` | `0` | facade | the `exposure` object (targets/delivered/dropped, deterministic matcher) is emitted in the `loom` block on any injected answer **regardless**; this knob *additionally* appends a `Not covered above: …` line to the answer content when titles were dropped (copy-fidelity finding) |
| `LOOM_BACKEND_NO_THINK` | `0` | facade/backend | on an engaged delegation where the client did not set `chat_template_kwargs`, add `{"enable_thinking":false}`; never applied to a passthrough request (budget-interaction finding) |
| `LOOM_THINK_TOKEN_FLOOR` | `1536` | facade/backend | on an engaged delegation with thinking active, raise a sub-floor integer `max_tokens` up to this floor so reasoning cannot starve the answer; `0` disables (complements `LOOM_MIN_MAX_TOKENS`) |

With `LOOM_VERBATIM_MODE` engaged the §9 `chat_completions` path may resolve to the canonical markdown block and return without calling `ModelBackend::chat`; the `loom` annotation still attaches (`mode` reflects the verbatim path).

---

## 11. Adapters — realisation notes

### 11.1 `loom-graph-oxigraph` (replace-with-crate)

`OxigraphStore` wraps `oxigraph::store::Store` (in-memory). `load()` bulk-loads **only** `ontology.ttl` + `ontology-inferred.ttl` from the mirror dir — the DDD BC24 I11 published-ontology-only invariant is a hard-coded allowlist, never a glob, so the working graph can never be loaded by accident. The `validate`/`_clamp`/`sparql` logic ports verbatim from `loom_graph.py`; native oxigraph gives typed `QueryResults` (Solutions / Boolean / Graph), so the ad-hoc `_term_str` string-stripping in Python collapses into typed term matching. `search_labels` runs the same `rdfs:label`/`skos:prefLabel`/`ngm:title` CONTAINS query. **Fail-open:** absent/failed store ⇒ `status().available=false`, façade degrades to lexical, reported in `/health`.

### 11.2 `loom-vector-ruvector` (new — the ground-truth wiring)

Two channels, hard-separated by feature flag:
- **Query hot path (default):** `HnswIndex` holds a `ruvector_core::VectorDB` opened on `LOOM_HNSW_ARTIFACT` — a **ruvector-core storage DB** (redb; the crate's `storage` feature), generation-stamped and mirrored like the JSON indices. On open, `VectorDB::new` reads the stored config, then **auto-rebuilds the HNSW index from the persisted vectors** (verified in `vector_db.rs`) — sub-second at 8,146×384. `nearest()` calls `VectorDB::search(SearchQuery{ vector, k, metric: DistanceMetric::Cosine })` and maps each `SearchResult` → `ConceptMatch { iri: <record id>, score: cosine, provenance: SemanticHnsw }`. **In-process, network-free.** The IRI is the record's ID, so the index can never leak its own row shape back as an answer (anti-corruption).
- **Artifact bootstrap (the bulk-embedding plan, done once per generation):** the initial DB is built by an off-turn exporter (`loom-vector-ruvector` bin, `pg-write` feature) that **reads the 8,146 already-verified embeddings from the `ontology-corpus` namespace in ruvector-postgres** (ingested 2026-08-16 via Xinference; 0 NULL, all unit-norm) and inserts them into the `VectorDB` keyed by IRI. No re-embedding on bootstrap — embeddings are only recomputed for delta-touched IRIs at promote time (below). If PG is unreachable the exporter can fall back to re-embedding `concept-records.jsonl` through Xinference (`bge-small-en-v1.5`/384, the locked model) — never through any other embedder.
- **Build/off-turn write channel (`pg-write` feature):** `PgWriter` connects via `tokio-postgres`+`pgvector` to `ruvector-postgres`, used at promote-time to **delta-diff touched IRIs** and re-embed only those (ADR-136 D4 re-embed-on-promote), honouring the HNSW index-law: **non-concurrent** rebuild `m=16, ef_construction=128`; **never `CREATE INDEX CONCURRENTLY`** on the ruvector HNSW AM (double-insertion, verified). This channel is never linked into the serving binary.

### 11.3 `loom-embed-xinference` (new)

`XinferenceEmbedder` POSTs `{ model, input }` to `XINFERENCE_URL/embeddings`, returns the 384-float vector. Startup assertion: `model_id()=="bge-small-en-v1.5"` and `dimensions()==384` — mismatch is a hard config error (the ops-law lock; a different embedding model silently invalidates the HNSW artifact). Two call sites only: build-time embed-on-promote and query-time OOV embed — **never** on the augmentation read path unless a lexical miss triggers it.

### 11.4 `loom-backend-openai` (port)

`OpenAiBackend` is a `reqwest::Client` (rustls, connection-pooled) to `DISTILL_BACKEND_URL`. `chat()` posts the scaffold-injected body, applies the `max_tokens` floor, strips `stream`, returns `BackendResponse{ status, body, content_type }`. `models()` and `reachable()` port the `/v1/models` passthrough + 5s probe. **Model identity never appears in `endpoint()`** — it rides in the response body (ADR-135 D1.2), so swapping Qwen3.8-27B for the next model is one env-var change, zero consumer change.

**Erratum D (2026-08-17, audit finding 2, EXP-006) — the floor is integer-only, matching Python parity.** A first cut floored *any* present `max_tokens`/`max_completion_tokens` to `MIN`, so a higher string-typed ask (`"999999"`), a `-1`, and a `2^64` all collapsed to `1536` — which both diverges from `loom_facade.py` and violates "never lower a higher ask". The implemented `normalise_body` floors **only JSON integers** via an `i128` `max(v, MIN)` (so `-1` floors up, a large `u64` is preserved), leaves strings/floats/overflow/`null` untouched, inserts `max_tokens = MIN` only when **both** keys are absent, and no-ops entirely when `MIN == 0` — byte-for-byte the Python guard (`crates/loom-backend-openai/src/lib.rs`; wiremock counter-examples cover each case).

### 11.5 `loom-attest-proofgate` (re-platform, build/CI-time)

`ChainedLedger` implements `AttestationLedger`. Domain predicates (class-count parity, no-mixed-generation, SSOT/conflict checks) stay **Loom-owned** in `loom-domain`; only their *attestation* becomes chain-hashed ledger entries. Behind the `attest` feature; not linked into the serving binary. `verify_chain()` is the tamper check the CI gate runs.

> **Erratum A (2026-08-17) — the attestation substrate is Loom's own `sha2` ledger, not `ruvector-core`.** This section (and ADR-136 D5, ADR-137 D9) originally read `ProofGate<T>`/`MutationLedger` "in RuVector, ADR-047" as if they were reachable from the `ruvector-core` crate the serving path already links. Verified against source this is wrong on two counts, and the honest ground truth is:
>
> 1. **`ProofGate<T>` / `MutationLedger` live in `ruvector-graph-transformer` (`src/proof_gated.rs`), built atop `ruvector-verified` — not in `ruvector-core`.** Binding them means pulling in the graph-transformer crate (and its dependency cone), which the build-time `attest` path could do, but the serving path deliberately does not.
> 2. **`ruvector-core`'s own in-crate ledger, `agenticdb::WitnessLog`, is unsound as an attestation anchor**: despite a doc-comment claiming "SHA256", it chains with the non-cryptographic `std::collections::hash_map::DefaultHasher`. A `DefaultHasher` chain is trivially forgeable and gives no tamper evidence — using it *as if* it were SHA-256 would be attestation theatre. (Verified by reconstruction through vector-search of the crate source.)
>
> The Loom therefore **ships its own `sha2` `ChainedLedger`** as the attestation substrate: real SHA-256 entry hashing, a HEAD checkpoint sidecar written tmp+rename under the append lock (added remediating audit finding 4 — truncation-proofing, see `.claude/evidence/AUDIT-gpt54.md`), and a `verify_chain()` that fails on any tampered *or truncated* ledger. Binding the *real* `ProofGate` (from `ruvector-graph-transformer`) behind the `attest` feature is a **one-line future rewiring** — the `AttestationLedger` port already isolates the choice — and does not touch the serving binary either way. This is an honest, verifiable substrate today, not a placeholder; the RuVector `ProofGate` is a later upgrade, not a missing dependency.

### 11.6 Mirror (port of `mirror.sh` → `loom-facade::mirror`)

`mirror.sh`'s atomic, generation-verified promote (ADR-136 D4) ports to a Rust module (or stays a shell script invoked by the container entrypoint — the mesh decides; the *logic* is what matters). It fetches the four artifacts to staging, verifies their embedded generation stamps cluster within `GEN_TOL_SECONDS`, and `os::rename`-promotes atomically, writing `.generation.json` **last** as the commit marker. New for Rust: it also promotes `LOOM_HNSW_ARTIFACT` as a **fifth** artifact in the same generation cluster, so the HNSW projection is never spliced across builds (the generation-parity guard in §6 relies on this). `GenerationStore::verify_atomicity()` re-checks all five shas at load.

---

## 12. Deprecation map (what happens to each Python file)

Carried from the frame's `deprecation_map`, with the Rust destination made concrete.

| Python source | Fate | Rust destination |
|---|---|---|
| `app/loom_facade.py` (stdlib façade, all endpoints) | **port** | `loom-facade` (router §9, handlers, `AppState`) |
| `app/loom_graph.py` (pyoxigraph SPARQL wrapper) | **replace-with-crate** | `loom-graph-oxigraph` (native oxigraph) |
| `app/ontology_scaffold.py` (lexical matcher + injection policy) | **port** | `loom-scaffold` (§5), fixture ported to tests |
| `pyoxigraph` dependency (Python binding) | **replace-with-crate** | `oxigraph` direct crate dep |
| `app/ontology_proxy.py` (524-line legacy proxy) | **drop** | — (superseded by the façade; not carried) |
| `app/pipeline/*` (vendored logseq/pipeline copy) | **drop** | — (#21: Loom is a serving mirror, builder stays `jjohare/logseq`) |
| `app/test_proxy.py` (proxy unit tests) | **drop** | — (tests the dropped proxy) |
| `app/mirror.sh` (atomic generation-verified mirror) | **port** | `loom-facade::mirror` (or retained script; §11.6) |
| `app/entrypoint.sh`, `Dockerfile`, `docker-compose.yml` | **replace** | `deploy/Dockerfile` (multi-stage static) + `compose.profile-{a,b}.yml` |

---

## 13. Deployment (both profiles — the ADR-137 decision, realised)

One static musl binary, two compose profiles (see ADR-137 for the *why*; this is the *how*). The single-binary-no-interpreter artifact is exactly what makes two profiles nearly free — the durable payoff of the rewrite on the deployment axis.

**`deploy/Dockerfile`** — multi-stage, `cargo-chef` for dependency caching, static `x86_64-unknown-linux-musl` target, final stage `FROM scratch` (or `gcr.io/distroless/static`) with just the binary + the mirrored `data/` volume. No Python, no wheel, no interpreter. Cold start is process-exec + index load, not interpreter boot + import.

**Profile A — host-colocated on HP (reference serving deployment):**
```yaml
# compose.profile-a.yml
services:
  loom:
    image: loom:rust
    network_mode: host                 # :8084 DNAT'd to LAN by hp-nat.service
    environment:
      LOOM_FACADE_PORT: 8084
      DISTILL_BACKEND_URL: http://127.0.0.1:8085/v1   # loom-model Qwen3.8-27B, GPU-colocated
      LOOM_DEPLOY_PROFILE: a
      LOOM_SEMANTIC_FALLBACK: 0        # in-proc HNSW artifact present, gated off until bench
    volumes: [ ./data:/app/data:ro ]   # mirrored generation incl. ontology-corpus.rvdb
```
Serves fully with **no docker-network access** — lexical + in-process HNSW + in-context oxigraph, all network-free. This is the demo/reference path and the GPU-colocated model path.

**Profile B — sidecar on `visionclaw_network` (consumer-facing door + write channel):**
```yaml
# compose.profile-b.yml
services:
  loom:
    image: loom:rust
    networks: [ visionclaw_network ]
    ports: [ "8080:8080" ]
    environment:
      LOOM_FACADE_PORT: 8080
      DISTILL_BACKEND_URL: http://loom-model:8085/v1   # model-is-a-URL; delegated, GPU-free sidecar
      XINFERENCE_URL: http://xinference:9997/v1
      RUVECTOR_PG_CONNINFO: ${RUVECTOR_PG_CONNINFO}    # pg-write feature; build/off-turn ONLY
      LOOM_DEPLOY_PROFILE: b
    volumes: [ ./data:/app/data:ro ]
networks:
  visionclaw_network: { external: true }
```
Required (not CI-only) because the **email gateway binds `REASONER_BASE_URL=http://loom:8080/v1`** — an in-network consumer that must reach a Loom on `visionclaw_network`, not behind a DNAT — and because the **ruvector-postgres + Xinference write channel** needs an in-network home. B is GPU-free and delegates the model by URL.

> **Erratum B (2026-08-17) — the redb read-only-mount hazard.** Both profile blocks above mount the corpus `volumes: [ ./data:/app/data:ro ]` — correct for the JSON/TTL projections (the Loom only reads them) but a trap for `LOOM_HNSW_ARTIFACT` (`ontology-corpus.rvdb`). The `.rvdb` is a **redb** database, and redb **mutates the file on open** (lock page, WAL/commit bookkeeping) even for a read-only workload — so opening the artifact directly off a read-only bind mount fails at startup (redb cannot acquire its write lock), and a read-write bind mount would let the running node scribble into the shared generation and break byte-parity across A and B. The deploy layer resolves this at the entrypoint: it **copies `ontology-corpus.rvdb` from the read-only `./data` mount to a writable node-private path (`/run/loom/…`, tmpfs) with a sha256 verify against `.generation.json`**, and points `LOOM_HNSW_ARTIFACT` at the copy. The read-only mount stays the immutable source of truth; the mutable redb working file is a verified, per-node copy. (See `deploy/` for the entrypoint realisation — owned by the deploy layer, not this crate.)

**Obligation both profiles carry (ADR-137 consequence 3):** the mirrored generation — including the HNSW artifact — is promoted into **both** deployments under the ADR-136 D4 atomic discipline, and **generation parity across A and B is a CI/health assertion**: `/health.generation` must be byte-identical for the same `commitSha`. The `pg-write`/MCP path is build/off-turn only and never the query hot path, so an A instance cut off from the docker network still serves (the whole point of keeping A network-free).

---

## 14. Build & CI gates (what "green" means)

Matching the sibling repos' bar:

1. `cargo build --workspace --all-features` and `--no-default-features` (proves the serving binary compiles without the `pg-write`/`attest`/`semantic-fallback` features).
2. `cargo test --workspace --all-features` — incl. the ported `ontology_scaffold` fixture (byte-identical golden), the SPARQL clamp tests, the router oneshot tests.
3. `cargo clippy --all-targets --all-features -- -D warnings` (pedantic on).
4. `cargo deny check` — licence + advisory gate (`deny.toml`).
5. Criterion perf gate: `match()` p99 < 50ms on the 8k synthetic index.
6. **The recall-gate test (§8.4)** — runs against the `ontology-corpus` namespace; its verdict is the gate on flipping `LOOM_SEMANTIC_FALLBACK` default-on. Red ⇒ fallback stays off; the code still ships, the *default* does not change.
7. Nix build (`flake.nix`) produces the static binary + the Profile A/B images.
8. Generation-parity assertion across the two profile images (health-check in CI compose).

---

## 15. Cross-references

- **[PRD-027](./PRD-027-rust-loom-reengineering.md)** — product scope + success criteria for this rewrite.
- **[ADR-137](./ADR-137-loom-rust-replatform.md)** — the decision to re-platform + the both-profiles resolution (this doc is its build blueprint).
- **[ADR-135](./ADR-135-ontology-loom-node.md)** — node boundary (D1 model-is-a-URL, D1.3 Deployment A/B, D2.1 generation, D3 reasoner authority). All honoured, none relitigated.
- **[ADR-136](./ADR-136-loom-tooling-allocation.md)** — D1 (markdown canonical), D2 (keep oxigraph SPARQL), D3 (HNSW gated fallback), D4 (SSOT + re-embed-on-promote + index-law), D5 (gate→ProofGate), D6 (Whelk-rs build-time), §3 (in-process network-free HNSW).
- **[ddd-ontology-loom-context.md](./ddd-ontology-loom-context.md)** — the bounded context; `CanonicalUnit` maps to the aggregate root, BC24 I11 (published-ontology-only), §6.1 (RuVector access model). The port names above (`LexicalIndex`/`VectorIndex`/`GraphStore`/`EmbeddingProvider`/`ModelBackend`) are this document's ubiquitous language, held verbatim.
- **agentbox — ADR-051** ([agentbox-ADR-051-loom-client-and-deferred-distillation.md](./agentbox-ADR-051-loom-client-and-deferred-distillation.md)) — the loom client + deferred distillation consumer; the façade contract this binary must keep stable.
- **VisionClaw — ADR-099** (Whelk-rs EL++ reasoner) and **ADR-090/PRD-016** (hexagonal ring placement) — the reasoner is build-time authority; this workspace is the ADR-090 ring realised for the Loom surface.
- **ruvector — ADR-001** (HNSW production index) — the in-process `ruvector-core` HNSW read (behind the `hnsw` feature, explicitly enabled with defaults off; Erratum C §2.1). **ADR-047** (`ProofGate<T>`/`MutationLedger`) is the *design target* for attestation but is realised in `ruvector-graph-transformer::proof_gated`, **not** `ruvector-core`; the Loom ships its own `sha2` `ChainedLedger` today and rewires to the real `ProofGate` behind the `attest` feature later (Erratum A, §11.5).
- **logseq (`jjohare/logseq`)** — the canonical corpus builder + CI-enforced gate; the Loom serves its output, never rebuilds it (the dropped `pipeline/`).

---

## 16. What this rewrite explicitly does NOT do (non-goals, restated as build constraints)

- **No DL reasoning at query time.** No Whelk EL++, no live inference in any handler. The graph store serves the *pre-reasoned* closure only. (Keeps the façade GPU-free and portable — ADR-136 D6.)
- **No `@ruvector/graph-node` Cypher, no ruvector-hybrid/mincut/gnn-rerank.** oxigraph SPARQL stays (D2); the fusion is the lexical→gate→HNSW-candidate union of §6, nothing more, until D8's bench clears something better.
- **No encoding replaces the markdown.** No GraphRAG community summaries, no GNN soft-prompt subgraphs, no RuVector-row-as-record. The served unit is always the per-IRI markdown block resolved by `Iri`. This is not a preference; it is the type system (§3–§4).
- **The Loom is not a builder.** `app/pipeline/*` is dropped; `jjohare/logseq` stays the source of truth. The Rust Loom is a serving mirror with an in-process semantic accelerator — nothing writes the corpus here.
- **No multi-agent coordination substrate (WS-Q).** Deferred, not shipped. When built, it must still resolve every claim to the same per-IRI markdown identity — the `Iri`→`CanonicalUnit` port contract is the seam it will attach to.

_Written to `/home/devuser/workspace/loom/docs/design/RUST-ARCHITECTURE.md`._

## Estate closeout qualification — 2026-09-04

The [grounding review](../../../VisionFlow/docs/estate-review/grounding-delivery.md) qualifies the generation guarantee: `MirrorStore.current()` reads disk metadata while retriever/graph content is loaded at startup; `verify_atomicity()` is exercised by tests but has no located serving-path invocation. Manifest presence can report verified generation without hash verification. Closeout requires immutable loaded-bundle activation, actual rejection/reload receipts, profile parity and consumer preservation of grounding/degradation. Build identity must include the locally consumed sibling RuVector checkout and enabled features. ADR-135–138 retain their historical/proposed decision status and now carry explicit acceptance conditions.

## Semantic artefact acceptance qualification — 2026-09-04

The current adapter treats any nonempty opened database as ready, although stored RuVector settings override its cosine/384 defaults. Readiness therefore needs effective configuration and model validation; score labels must match the actual metric. The [consumer review](../../../VisionFlow/docs/estate-review/consumed-vector-storage.md) records wrong-metric and wrong-width local reproductions. No deployed semantic recall or profile parity is certified here.
