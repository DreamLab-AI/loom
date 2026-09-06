//! `BuildInfo` — the release receipt (ADR-137 closeout).
//!
//! "Bind the sibling RuVector revision, features, compiler, artefacts, dataset
//! and effective configuration in the release receipt." A Loom commit sha alone
//! does not identify a build: `ruvector-core` is a PATH dependency, so the
//! lockfile pins its version string and nothing about its revision. Two binaries
//! built from the same Loom commit against different sibling checkouts are
//! different builds, and until now nothing said so.
//!
//! Everything in [`BuildInfo`] is baked at compile time by `build.rs`. Nothing
//! is re-read at runtime: a receipt that resolved the sibling checkout when
//! asked would describe the machine answering, not the binary that was built.
//!
//! Three surfaces expose it, all from this one struct:
//! - `loom-facade --build-info` prints the receipt as JSON (the CI artefact);
//! - `/health.build` carries it beside the serving identity, so an operator can
//!   match a running node to a receipt without shell access;
//! - [`BuildInfo::with_effective_config`] adds the resolved runtime configuration
//!   so "which build, with which settings, over which generation" is one answer.

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// The compile-time identity of this binary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    /// The Loom checkout's HEAD at build time.
    pub loom_revision: String,
    /// The SIBLING RuVector checkout's HEAD at build time — the half of the
    /// source identity a Loom revision cannot carry.
    pub ruvector_revision: String,
    /// Where that sibling checkout was, so a receipt can be reproduced.
    pub ruvector_path: String,
    /// The explicit `ruvector-core` feature set from the workspace manifest.
    pub ruvector_features: Vec<String>,
    /// Cargo features this crate was compiled with.
    pub features: Vec<String>,
    /// `rustc --version` of the compiler that built this binary.
    pub rustc: String,
    /// `debug` / `release`.
    pub profile: String,
    /// Compilation target triple, and the host that produced it.
    pub target: String,
    pub host: String,
    /// The crate version.
    pub version: String,
}

impl BuildInfo {
    /// Read the receipt baked in by `build.rs`.
    #[must_use]
    pub fn current() -> Self {
        Self {
            loom_revision: env!("LOOM_BUILD_GIT_SHA").to_owned(),
            ruvector_revision: env!("LOOM_BUILD_RUVECTOR_SHA").to_owned(),
            ruvector_path: env!("LOOM_BUILD_RUVECTOR_PATH").to_owned(),
            ruvector_features: split_list(env!("LOOM_BUILD_RUVECTOR_FEATURES")),
            features: split_list(env!("LOOM_BUILD_FEATURES")),
            rustc: env!("LOOM_BUILD_RUSTC").to_owned(),
            profile: env!("LOOM_BUILD_PROFILE").to_owned(),
            target: env!("LOOM_BUILD_TARGET").to_owned(),
            host: env!("LOOM_BUILD_HOST").to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    /// Whether both halves of the source identity are known. A build made
    /// outside a git checkout reports `false` and must not be released — the
    /// receipt would not pin anything.
    #[must_use]
    pub fn source_identity_complete(&self) -> bool {
        self.loom_revision != "unknown" && self.ruvector_revision != "unknown"
    }

    /// The receipt plus the EFFECTIVE runtime configuration — what the release
    /// artefact records. Secrets are not part of `Config` (the backend URL is an
    /// endpoint, never a credential), so the whole resolved surface can be
    /// stated rather than sampled.
    #[must_use]
    pub fn with_effective_config(&self, config: &Config) -> BuildReceipt {
        BuildReceipt {
            build: self.clone(),
            effective_config: EffectiveConfig::of(config),
        }
    }
}

/// The build receipt: identity + the configuration it is running under.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildReceipt {
    pub build: BuildInfo,
    pub effective_config: EffectiveConfig,
}

/// The resolved §10 environment surface, as actually parsed.
///
/// A projection of [`Config`] rather than `Config` itself: this is a receipt
/// format that must stay stable for comparison across releases, while `Config`
/// is free to grow knobs. Field-for-field today; the two are allowed to diverge.
#[allow(clippy::struct_excessive_bools)] // a flat receipt of independent switches
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectiveConfig {
    pub deploy_profile: String,
    pub facade_port: u16,
    pub timeout_secs: u64,
    pub max_body_bytes: usize,
    pub backend_configured: bool,
    pub backend_url: String,
    pub min_max_tokens: u64,
    pub index_path: String,
    pub prose_path: String,
    pub budget: usize,
    pub default_max_seeds: usize,
    pub default_hops: usize,
    pub default_prose: bool,
    pub semantic_fallback: bool,
    pub hnsw_artifact: String,
    pub semantic_k: usize,
    pub semantic_min_inject: Option<f64>,
    pub semantic_score_scale: Option<f64>,
    pub semantic_debug_surface: bool,
    pub verbatim_mode: bool,
    pub verbatim_threshold: f64,
    pub exposure_append: bool,
    pub backend_no_think: bool,
    pub think_token_floor: u64,
}

impl EffectiveConfig {
    #[must_use]
    pub fn of(c: &Config) -> Self {
        Self {
            deploy_profile: c.deploy_profile.clone(),
            facade_port: c.facade_port,
            timeout_secs: c.timeout_secs,
            max_body_bytes: c.max_body_bytes,
            backend_configured: !c.backend_url.is_empty(),
            backend_url: c.backend_url.clone(),
            min_max_tokens: c.min_max_tokens,
            index_path: c.index_path.clone(),
            prose_path: c.prose_path.clone(),
            budget: c.budget,
            default_max_seeds: c.default_max_seeds,
            default_hops: c.default_hops,
            default_prose: c.default_prose,
            semantic_fallback: c.semantic_fallback,
            hnsw_artifact: c.hnsw_artifact.clone(),
            semantic_k: c.semantic_k,
            semantic_min_inject: c.semantic_min_inject,
            semantic_score_scale: c.semantic_score_scale,
            semantic_debug_surface: c.semantic_debug_surface,
            verbatim_mode: c.verbatim_mode,
            verbatim_threshold: c.verbatim_threshold,
            exposure_append: c.exposure_append,
            backend_no_think: c.backend_no_think,
            think_token_floor: c.think_token_floor,
        }
    }

    /// The knobs that are ALLOWED to differ between deploy profiles.
    ///
    /// The A/B parity test (ADR-137) needs a stated boundary: a profile may
    /// choose its port, its backend, its serving regime and its thinking
    /// controls, but it may NOT change what gets retrieved or what gets
    /// injected. Anything outside this list differing between two profiles is a
    /// parity failure, not a configuration choice.
    pub const PROFILE_DIVERGENT_KEYS: &'static [&'static str] = &[
        "deploy_profile",
        "facade_port",
        "backend_configured",
        "backend_url",
        "verbatim_mode",
        "verbatim_threshold",
        "backend_no_think",
        "think_token_floor",
        "exposure_append",
    ];

    /// The retrieval-affecting subset — the fields two profiles MUST agree on
    /// to be serving the same corpus the same way.
    #[must_use]
    pub fn retrieval_identity(&self) -> serde_json::Value {
        serde_json::json!({
            "index_path": self.index_path,
            "prose_path": self.prose_path,
            "budget": self.budget,
            "default_max_seeds": self.default_max_seeds,
            "default_hops": self.default_hops,
            "default_prose": self.default_prose,
            "semantic_fallback": self.semantic_fallback,
            "hnsw_artifact": self.hnsw_artifact,
            "semantic_k": self.semantic_k,
            "semantic_min_inject": self.semantic_min_inject,
            "semantic_score_scale": self.semantic_score_scale,
        })
    }
}

fn split_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "unknown")
        .map(std::borrow::ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_info_pins_both_checkouts() {
        let b = BuildInfo::current();
        assert!(
            b.source_identity_complete(),
            "both revisions must be known in a git checkout: loom={:?} ruvector={:?}",
            b.loom_revision,
            b.ruvector_revision
        );
        assert_eq!(b.loom_revision.len(), 40, "{:?}", b.loom_revision);
        assert_eq!(b.ruvector_revision.len(), 40, "{:?}", b.ruvector_revision);
        assert_ne!(
            b.loom_revision, b.ruvector_revision,
            "two independent checkouts must not report one sha"
        );
    }

    /// The explicit feature set the workspace manifest requests of the sibling
    /// crate — the second half of "a Loom revision alone does not pin the build".
    #[test]
    fn build_info_records_the_explicit_ruvector_features() {
        let b = BuildInfo::current();
        for want in ["hnsw", "storage", "simd", "parallel"] {
            assert!(
                b.ruvector_features.iter().any(|f| f == want),
                "missing {want} in {:?}",
                b.ruvector_features
            );
        }
    }

    #[test]
    fn build_info_records_the_compiler() {
        let b = BuildInfo::current();
        assert!(b.rustc.starts_with("rustc "), "{:?}", b.rustc);
    }

    #[test]
    fn effective_config_round_trips_through_json() {
        let c = Config::default();
        let e = EffectiveConfig::of(&c);
        let json = serde_json::to_string(&e).unwrap();
        let back: EffectiveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn retrieval_identity_excludes_every_profile_divergent_key() {
        let identity = EffectiveConfig::of(&Config::default()).retrieval_identity();
        let obj = identity.as_object().expect("object");
        for key in EffectiveConfig::PROFILE_DIVERGENT_KEYS {
            assert!(
                !obj.contains_key(*key),
                "{key} may differ between profiles and must not be part of retrieval identity"
            );
        }
    }
}
