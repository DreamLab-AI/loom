//! `Config` — the whole §10 environment surface in one struct, hand-parsed with
//! the Python-name defaults so existing compose files carry over unchanged.
//!
//! PARITY OVERRIDE (mission-critical): the Python façade's `/loom/scaffold` and
//! `scaffold_messages` default `max_seeds=4` and `prose=false`. The domain
//! `ScaffoldOpts::default()` differs (6/true); this façade defaults to the
//! Python values (env-overridable) or byte-parity with the Python façade breaks.

/// Every knob keeps its Python env-var name (§10). Absent ⇒ the default below.
#[derive(Debug, Clone)]
pub struct Config {
    // --- facade / transport ---
    pub facade_port: u16,       // LOOM_FACADE_PORT (8080)
    pub timeout_secs: u64,      // LOOM_TIMEOUT (600)
    pub max_body_bytes: usize,  // LOOM_MAX_BODY_BYTES (16 MiB; Python was unbounded)
    pub deploy_profile: String, // LOOM_DEPLOY_PROFILE ("a")

    // --- backend seam ---
    pub backend_url: String, // DISTILL_BACKEND_URL ("")
    pub min_max_tokens: u64, // LOOM_MIN_MAX_TOKENS (1536)

    // --- scaffold / lexical ---
    pub index_path: String,      // ONTOLOGY_INDEX (/app/data/scaffold-index.json)
    pub prose_path: String,      // ONTOLOGY_PROSE_INDEX (/app/data/prose-index.json)
    pub budget: usize,           // ONTOLOGY_BUDGET (1500)
    pub default_max_seeds: usize, // LOOM_MAX_SEEDS (4 — Python façade default, NOT the domain's 6)
    pub default_hops: usize,     // LOOM_SCAFFOLD_HOPS (1)
    pub default_prose: bool,     // LOOM_SCAFFOLD_PROSE (false — Python façade default, NOT the domain's true)

    // --- semantic fallback (gated OFF until the recall bench clears) ---
    pub semantic_fallback: bool,          // LOOM_SEMANTIC_FALLBACK (0)
    pub hnsw_artifact: String,            // LOOM_HNSW_ARTIFACT (/app/data/ontology-corpus.rvdb)
    pub semantic_k: usize,                // LOOM_SEMANTIC_K (5)
    pub semantic_min_inject: Option<f64>, // LOOM_SEMANTIC_MIN_INJECT (unset — bench-set, no default)
    pub semantic_score_scale: Option<f64>, // LOOM_SEMANTIC_SCORE_SCALE (unset — bench-tuned)
}

impl Config {
    /// Read the full §10 surface from the process environment.
    #[must_use]
    pub fn from_env() -> Self {
        let d = Self::default();
        Self {
            facade_port: env_parse("LOOM_FACADE_PORT", d.facade_port),
            timeout_secs: env_parse("LOOM_TIMEOUT", d.timeout_secs),
            max_body_bytes: env_parse("LOOM_MAX_BODY_BYTES", d.max_body_bytes),
            deploy_profile: env_string("LOOM_DEPLOY_PROFILE", d.deploy_profile),
            backend_url: env_string("DISTILL_BACKEND_URL", d.backend_url),
            min_max_tokens: env_parse("LOOM_MIN_MAX_TOKENS", d.min_max_tokens),
            index_path: env_string("ONTOLOGY_INDEX", d.index_path),
            prose_path: env_string("ONTOLOGY_PROSE_INDEX", d.prose_path),
            budget: env_parse("ONTOLOGY_BUDGET", d.budget),
            default_max_seeds: env_parse("LOOM_MAX_SEEDS", d.default_max_seeds),
            default_hops: env_parse("LOOM_SCAFFOLD_HOPS", d.default_hops),
            default_prose: env_bool("LOOM_SCAFFOLD_PROSE", d.default_prose),
            semantic_fallback: env_bool("LOOM_SEMANTIC_FALLBACK", d.semantic_fallback),
            hnsw_artifact: env_string("LOOM_HNSW_ARTIFACT", d.hnsw_artifact),
            semantic_k: env_parse("LOOM_SEMANTIC_K", d.semantic_k),
            semantic_min_inject: env_opt_f64("LOOM_SEMANTIC_MIN_INJECT"),
            semantic_score_scale: env_opt_f64("LOOM_SEMANTIC_SCORE_SCALE"),
        }
    }

    /// The mirror/generation data directory — the parent of the scaffold index,
    /// exactly as the Python façade derives it (`os.path.dirname(INDEX)`).
    #[must_use]
    pub fn data_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.index_path)
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), std::path::Path::to_path_buf)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            facade_port: 8080,
            timeout_secs: 600,
            max_body_bytes: 16 * 1024 * 1024,
            deploy_profile: "a".to_owned(),
            backend_url: String::new(),
            min_max_tokens: 1536,
            index_path: "/app/data/scaffold-index.json".to_owned(),
            prose_path: "/app/data/prose-index.json".to_owned(),
            budget: 1500,
            default_max_seeds: 4, // Python façade default (do NOT use the domain's 6)
            default_hops: 1,
            default_prose: false, // Python façade default (do NOT use the domain's true)
            semantic_fallback: false,
            hnsw_artifact: "/app/data/ontology-corpus.rvdb".to_owned(),
            semantic_k: 5,
            semantic_min_inject: None,
            semantic_score_scale: None,
        }
    }
}

fn env_string(key: &str, default: String) -> String {
    std::env::var(key).unwrap_or(default)
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

fn env_opt_f64(key: &str) -> Option<f64> {
    std::env::var(key).ok().and_then(|v| v.parse::<f64>().ok())
}

/// Truthiness mirrors `InjectionPolicy::from_env`: `0/false/no/""` ⇒ false.
fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key).ok().as_deref() {
        None => default,
        Some("0" | "false" | "no" | "") => false,
        Some(_) => true,
    }
}
