//! Optional prose index (`prose-index.json`) loader. Carries the prose layer the
//! structural index truncates: per-slug `dfull` (full definition) + `cl`
//! (Current Landscape research prose), under a top-level `pages` map. OPTIONAL
//! by design — a missing file, bad JSON, or missing slug degrades to
//! structural-only, silently (fail-open, exactly as Python `get_prose`).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProseEntry {
    #[serde(default)]
    pub dfull: Option<String>,
    #[serde(default)]
    pub cl: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProseFile {
    #[serde(default)]
    pages: HashMap<String, ProseEntry>,
}

/// The prose index: slug → {dfull, cl}. Empty when absent.
pub type ProseIndex = HashMap<String, ProseEntry>;

/// Load the prose index from `path`; return an empty map on any error
/// (missing file / bad JSON) — never fails the scaffold.
#[must_use]
pub fn load_prose(path: &Path) -> ProseIndex {
    let Ok(text) = std::fs::read_to_string(path) else {
        return ProseIndex::new();
    };
    match serde_json::from_str::<ProseFile>(&text) {
        Ok(f) => f.pages,
        Err(_) => ProseIndex::new(),
    }
}
