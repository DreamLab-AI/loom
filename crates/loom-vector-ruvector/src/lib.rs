//! `loom-vector-ruvector` — the ground-truth wiring: `loom_domain::VectorIndex`
//! over an in-process `ruvector_core::VectorDB` HNSW (§11.2).
//!
//! Two channels, hard-separated by feature flag (RUST-ARCHITECTURE §11.2):
//! - **Query hot path (default):** [`HnswIndex`] opens the `LOOM_HNSW_ARTIFACT`
//!   ruvector-core storage DB (redb), which auto-rebuilds its HNSW index from the
//!   persisted vectors on open, and answers [`loom_domain::ports::VectorIndex`]
//!   queries in-process, network-free. Absent/broken artifact ⇒ `is_ready()==false`
//!   and `nearest()` returns [`loom_domain::error::LoomError::SemanticUnready`]
//!   (fail-open → the fusion pipeline degrades to lexical-only; never a panic).
//! - **Off-turn write channel (`pg-write` feature):** the `export_corpus` bin
//!   bootstraps the artifact from the verified `ontology-corpus` namespace in
//!   ruvector-postgres. Compiled out of the serving binary by default.

mod hnsw;

pub use hnsw::{HnswIndex, DEFAULT_ARTIFACT_PATH, EMBEDDING_DIMENSIONS};

// Re-exported so downstream wiring can name the port types without a second
// `loom-domain` import.
pub use loom_domain;
