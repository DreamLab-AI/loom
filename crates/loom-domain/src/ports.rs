//! The ports (§4) — the DDD/PRD/ADR ubiquitous language as traits. All async
//! (`async_trait`), all fallible with `LoomError`. Read the return types as the
//! enforcement of Invariant I-P1: every retrieval port yields
//! `Iri`/`ConceptMatch`/`CanonicalUnit`/`Scaffold`, never a raw engine artefact.

use crate::error::LoomError;
use crate::model::*;
use async_trait::async_trait;

/// LEXICAL PRIMARY. The inverted-index matcher (loom-scaffold). Sole authority
/// over the confidence gate: it decides WHICH units inject and how much budget.
#[async_trait]
pub trait LexicalIndex: Send + Sync {
    /// Score the query against the class titles; return seeds above the gate.
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

    /// Resolve an IRI to its full `CanonicalUnit` (markdown body source). The
    /// address→unit step that keeps every projection honest.
    fn resolve(&self, iri: &Iri) -> Option<CanonicalUnit>;

    fn generation(&self) -> Generation;
    fn class_count(&self) -> usize;
}

/// SEMANTIC FALLBACK (planned/gated). In-process HNSW over the ontology-corpus
/// namespace. Called ONLY on a lexical miss. Returns IRI-keyed candidates that
/// are handed BACK to `LexicalIndex::assemble` — never injected directly.
#[async_trait]
pub trait VectorIndex: Send + Sync {
    /// ANN over the embedded query vector. `k` bounded. Cosine. Each hit carries
    /// its IRI (primary key) and cosine score (∈ [0,1]).
    async fn nearest(&self, query_vec: &[f32], k: usize) -> Result<Vec<ConceptMatch>, LoomError>;
    fn is_ready(&self) -> bool; // false ⇒ fusion degrades to lexical-only
    fn generation(&self) -> Generation; // parity with the lexical generation is asserted
}

/// SPARQL over the Whelk-reasoned closure. Read-only, clamped. Native oxigraph.
#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn query(&self, sparql: &str) -> Result<SparqlResult, LoomError>;
    async fn search_labels(&self, needle: &str, limit: usize) -> Result<Vec<LabelHit>, LoomError>;
    fn status(&self) -> GraphStatus; // available|triples|loaded_files|error → /health
}

/// Query-time + build-time embeddings. Xinference bge-small-en-v1.5/384 (LOCKED).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, LoomError>; // 384-dim, or LoomError::Dimension
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LoomError>;
    fn model_id(&self) -> &str; // asserted == "bge-small-en-v1.5" (ops law)
    fn dimensions(&self) -> usize; // asserted == 384
}

/// The model-swap seam. OpenAI-compatible chat delegation to DISTILL_BACKEND_URL.
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Delegate a (scaffold-injected) chat request. Floors max_tokens ≥ floor.
    async fn chat(&self, body: serde_json::Value) -> Result<BackendResponse, LoomError>;
    async fn models(&self) -> Result<serde_json::Value, LoomError>; // /v1/models passthrough
    async fn reachable(&self) -> bool; // /health probe (5s)
    fn endpoint(&self) -> &str; // the URL — model identity NEVER encoded here (ADR-135 D1.2)
}

/// The generation identity source + the atomic mirror commit marker.
#[async_trait]
pub trait GenerationStore: Send + Sync {
    fn current(&self) -> Generation; // best-source-first (build-manifest → mirror → scaffold)
    async fn verify_atomicity(&self) -> Result<(), LoomError>; // all artifact shas verify, one generation
}

/// Build/CI-time attestation of gate verdicts onto ProofGate/MutationLedger.
/// NOT on the serving hot path.
#[async_trait]
pub trait AttestationLedger: Send + Sync {
    async fn attest(&self, verdict: &GateVerdict) -> Result<LedgerEntryId, LoomError>;
    async fn verify_chain(&self) -> Result<bool, LoomError>; // tamper check
}
