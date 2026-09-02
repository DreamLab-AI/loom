//! `loom-domain` — the pure centre of the Ontology Loom hexagon: the canonical
//! types (`model`), the ports the adapters implement (`ports`), and the one
//! error the whole system speaks (`error`). No I/O, no framework, no tokio —
//! the accelerator boundary is a build fact, and this crate is the leaf that
//! makes it one (RUST-ARCHITECTURE §1.1).

// These pedantic lints are pure style noise for a domain crate whose public
// surface is intentionally getter-heavy, whose docs name product terms (IRI,
// HNSW, SPARQL, RuVector) that would each need backticks, and whose ports all
// return `Result`. The safety-bearing lints (unsafe, correctness) stay on.
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::wildcard_imports)]

pub mod error;
pub mod grounding;
pub mod model;
pub mod ports;

pub use error::LoomError;
pub use grounding::*;
pub use model::*;
pub use ports::*;

// Test modules live beside their subject but in their own files, so no source
// file in this crate crosses the 500-line ceiling.
#[cfg(test)]
#[path = "grounding_tests.rs"]
mod grounding_tests;
#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
