//! `Config` — the whole §10 environment surface in one struct, hand-parsed with
//! the Python-name defaults so existing compose files carry over unchanged.
//!
//! PARITY OVERRIDE (mission-critical): the Python façade's `/loom/scaffold` and
//! `scaffold_messages` default `max_seeds=4` and `prose=false`. The domain
//! `ScaffoldOpts::default()` differs (6/true); this façade defaults to the
//! Python values (env-overridable) or byte-parity with the Python façade breaks.

/// Every knob keeps its Python env-var name (§10). Absent ⇒ the default below.
// A config bag legitimately carries many independent on/off knobs; the
// excessive-bools lint (which guards against boolean-blindness in domain types)
// is noise for a flat env surface.
#[allow(clippy::struct_excessive_bools)]
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
    pub index_path: String, // ONTOLOGY_INDEX (/app/data/scaffold-index.json)
    pub prose_path: String, // ONTOLOGY_PROSE_INDEX (/app/data/prose-index.json)
    pub budget: usize,      // ONTOLOGY_BUDGET (1500)
    pub default_max_seeds: usize, // LOOM_MAX_SEEDS (4 — Python façade default, NOT the domain's 6)
    pub default_hops: usize, // LOOM_SCAFFOLD_HOPS (1)
    pub default_prose: bool, // LOOM_SCAFFOLD_PROSE (false — Python façade default, NOT the domain's true)

    // --- semantic fallback (gated OFF until the recall bench clears) ---
    pub semantic_fallback: bool,           // LOOM_SEMANTIC_FALLBACK (0)
    pub hnsw_artifact: String,             // LOOM_HNSW_ARTIFACT (/app/data/ontology-corpus.rvdb)
    pub semantic_k: usize,                 // LOOM_SEMANTIC_K (5)
    pub semantic_min_inject: Option<f64>, // LOOM_SEMANTIC_MIN_INJECT (unset — bench-set, no default)
    pub semantic_score_scale: Option<f64>, // LOOM_SEMANTIC_SCORE_SCALE (unset — bench-tuned)

    // --- F1: verbatim serving mode (findings-driven; default OFF = current behaviour) ---
    /// `LOOM_VERBATIM_MODE` (0). When on AND the injection gate engages AND the
    /// top lexical confidence clears `verbatim_threshold`, `/v1/chat/completions`
    /// serves the scaffold's canonical markdown WITHOUT calling the backend.
    pub verbatim_mode: bool,
    /// `LOOM_VERBATIM_THRESHOLD` (8.0). Compared against the scaffold's `top_score`
    /// on the LEXICAL additive scale (`match_`): `EXACT_TITLE_WEIGHT = 8.0` per
    /// exactly-matched title word, `MIN_SEED_SCORE = 2.0` the inject floor,
    /// `STRONG_MATCH_SCORE = 8.0` a strong exact single-word title hit. The 8.0
    /// default therefore means "serve verbatim only on a full exact-title match"
    /// — deliberately conservative (a multi-word exact title scores 8×words, so
    /// this admits single-word exact hits and stronger; paraphrase/overlap-only
    /// matches score below 8 and still delegate).
    pub verbatim_threshold: f64,

    // --- F2: exposure telemetry (matcher always on when injected; content append opt-in) ---
    /// `LOOM_EXPOSURE_APPEND` (0). When on, append a single
    /// `Not covered above: …` line to the answer content whenever served titles
    /// were dropped. The `exposure` telemetry block is emitted regardless.
    pub exposure_append: bool,

    // --- F3: thinking + budget control (defaults OFF = current behaviour) ---
    /// `LOOM_BACKEND_NO_THINK` (0). When on, an ENGAGED (scaffold-injected)
    /// delegation gets `chat_template_kwargs: {"enable_thinking": false}` IF the
    /// client did not set `chat_template_kwargs` themselves. Never applied to a
    /// non-engaged (passthrough) request.
    pub backend_no_think: bool,
    /// `LOOM_THINK_TOKEN_FLOOR` (default 0 = OFF; set to e.g. 1536 to enable).
    /// When delegating an ENGAGED request WITH thinking active (no-think off, or
    /// the client overrode `chat_template_kwargs`), raise a sub-floor INTEGER
    /// `max_tokens` the client sent up to this floor — think-tokens otherwise
    /// exhaust the budget on long scaffolds and truncate the answer to empty
    /// (paper reasoning-budget finding).
    ///
    /// Default 0 keeps F3 fully off (current behaviour preserved EXACTLY): the
    /// backend adapter's `LOOM_MIN_MAX_TOKENS` floor is the only token floor when
    /// F3 is unconfigured, so a deployment that disabled it (`LOOM_MIN_MAX_TOKENS=0`)
    /// is NOT silently re-floored. Profile A (the HP reference) sets 1536 — which
    /// simply matches the `LOOM_MIN_MAX_TOKENS` default it also runs, so the
    /// think-active path never truncates.
    pub think_token_floor: u64,

    // --- debug surfaces ---
    /// LOOM_SEMANTIC_DEBUG_SURFACE (0). The `/loom/search/semantic` route is the
    /// one labelled index-debug endpoint (RUST-ARCHITECTURE §9); it exposes bare
    /// IRI+score and NEVER feeds `/v1/chat/completions`. Default-OFF ⇒ 404 (audit
    /// finding 1). Turn on only for eval/debugging.
    pub semantic_debug_surface: bool,
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
            semantic_debug_surface: env_bool(
                "LOOM_SEMANTIC_DEBUG_SURFACE",
                d.semantic_debug_surface,
            ),
            verbatim_mode: env_bool("LOOM_VERBATIM_MODE", d.verbatim_mode),
            verbatim_threshold: env_parse("LOOM_VERBATIM_THRESHOLD", d.verbatim_threshold),
            exposure_append: env_bool("LOOM_EXPOSURE_APPEND", d.exposure_append),
            backend_no_think: env_bool("LOOM_BACKEND_NO_THINK", d.backend_no_think),
            think_token_floor: env_parse("LOOM_THINK_TOKEN_FLOOR", d.think_token_floor),
        }
    }

    /// The mirror/generation data directory — the parent of the scaffold index,
    /// exactly as the Python façade derives it (`os.path.dirname(INDEX)`).
    #[must_use]
    pub fn data_dir(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.index_path).parent().map_or_else(
            || std::path::PathBuf::from("."),
            std::path::Path::to_path_buf,
        )
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
            semantic_debug_surface: false, // default-OFF (audit finding 1)
            verbatim_mode: false,          // F1 OFF ⇒ current behaviour preserved
            verbatim_threshold: 8.0,       // exact-title-match floor (EXACT_TITLE_WEIGHT)
            exposure_append: false,        // F2 telemetry always on; content append opt-in
            backend_no_think: false,       // F3 OFF ⇒ current behaviour preserved
            think_token_floor: 0,          // OFF by default; Profile A sets 1536 explicitly
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
