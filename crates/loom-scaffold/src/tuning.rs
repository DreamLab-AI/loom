//! Tuning knobs — ported constant-for-constant from `ontology_scaffold.py`.
//!
//! The scoring weights (`MIN_SEED_SCORE` … `NEIGHBOUR_DEF_CHARS`) are plain
//! module constants in Python and are NOT env-overridable there; they stay
//! `const` here so byte-parity is a compile-time fact. Only the confidence-gate
//! knobs and the index paths read the environment (Python does the same, at
//! import time) — see `policy::InjectionPolicy::from_env` and `index`/`prose`.

// --- scoring weights (Python module constants) ------------------------------
pub const MIN_SEED_SCORE: f64 = 2.0; // below this, class is not a seed
pub const EXACT_TITLE_WEIGHT: f64 = 8.0; // per word of an exactly-matched title n-gram
pub const OVERLAP_WEIGHT: f64 = 2.0; // full title-word coverage earns this in total
pub const SUBSTRING_WEIGHT: f64 = 0.75; // prompt term is a substring of the slug
pub const SUBSTRING_MIN_LEN: usize = 5; // shorter terms are too noisy for substring match
pub const MAX_NGRAM: usize = 4; // longest phrase tried for exact title match
pub const ISUP_CAP: usize = 5; // inferred ancestors listed per seed
pub const REL_CAP: usize = 3; // relation targets listed per relation type
pub const NEIGHBOUR_DEFS: usize = 2; // 1-hop neighbour definitions per seed (hops>=1)
pub const NEIGHBOUR_DEF_CHARS: usize = 220; // neighbour one-liner definition truncation

// --- confidence-gate defaults (env-overridable, Python parity) --------------
pub const STRONG_MATCH_SCORE_DEFAULT: f64 = EXACT_TITLE_WEIGHT; // full budget at/above
pub const MIN_INJECT_SCORE_DEFAULT: f64 = MIN_SEED_SCORE; // below this top score → skip
pub const MIN_INJECT_FRACTION_DEFAULT: f64 = 0.4; // weakest match still gets this fraction

// --- prose index budget discipline ------------------------------------------
pub const PROSE_SEEDS: usize = 2; // only the top seeds get prose
pub const PROSE_CL_CHARS: usize = 1200; // landscape prose used per seed
pub const PROSE_DEF_CHARS: usize = 900; // full-definition chars used per seed

// --- serialisation literals -------------------------------------------------
pub const HEADER: &str = "[ONTOLOGY CONTEXT]";
pub const FOOTER: &str = "[END ONTOLOGY CONTEXT]";

pub const SYSTEM_PREAMBLE: &str = "The following ontology context was retrieved from a curated knowledge \
graph. Where it is relevant to the user's request, treat it as ground \
truth for definitions and relationships between the concepts it covers. \
Where it is not relevant, ignore it and answer normally.";

/// Deterministic relation ordering for serialisation and neighbour picking.
/// These are the camelCase scaffold-index predicate keys, emitted VERBATIM into
/// the served block (byte-parity: the block shows `hasPart`, not `has-part`).
pub const REL_ORDER: [&str; 12] = [
    "hasPart",
    "requires",
    "enables",
    "dependsOn",
    "implements",
    "uses",
    "partOf",
    "relatedTo",
    "bridgesTo",
    "supports",
    "standardizedBy",
    "contrastsWith",
];

/// Stopwords — the exact frozenset from Python, order-independent (59 words).
pub const STOPWORDS: &[&str] = &[
    "a", "an", "the", "of", "and", "or", "to", "in", "on", "for", "with", "is", "are", "was",
    "were", "be", "been", "what", "how", "why", "when", "where", "which", "who", "whom", "me",
    "my", "mine", "i", "you", "your", "we", "our", "it", "its", "this", "that", "these", "those",
    "about", "tell", "explain", "describe", "does", "do", "did", "can", "could", "would", "should",
    "will", "vs", "versus", "between", "using", "use", "please", "give", "show",
];

// Default index/prose paths (Python defaults; expanded/overridden at load).
pub const DEFAULT_INDEX_PATH: &str = "~/githubs/loom/app/data/scaffold-index.json";
pub const ENV_INDEX_VAR: &str = "ONTOLOGY_INDEX";
pub const DEFAULT_PROSE_PATH: &str = "~/githubs/loom/app/data/prose-index.json";
pub const ENV_PROSE_VAR: &str = "ONTOLOGY_PROSE_INDEX";
