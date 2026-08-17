//! The confidence-gated selective-injection policy — THE gate (Invariant I-P1 /
//! ADR-136 D3). `effective_budget` is byte-identical to Python's `scaffold()`
//! gate branch. Computed in `f64` because Python is: the doc's `f32` sketch
//! (§5.4) loses to byte-parity here.

use crate::tuning::{
    MIN_INJECT_FRACTION_DEFAULT, MIN_INJECT_SCORE_DEFAULT, STRONG_MATCH_SCORE_DEFAULT,
};

/// Confidence-injection knobs. Defaults are Python-baseline (injection OFF),
/// each field env-overridable via `from_env` exactly as the Python module.
#[derive(Debug, Clone, Copy)]
pub struct InjectionPolicy {
    pub confidence_injection: bool, // LOOM_CONFIDENCE_INJECTION
    pub strong_match_score: f64,    // LOOM_STRONG_MATCH_SCORE (default EXACT_TITLE_WEIGHT)
    pub min_inject_score: f64,      // LOOM_MIN_INJECT_SCORE   (default MIN_SEED_SCORE)
    pub min_inject_fraction: f64,   // LOOM_MIN_INJECT_FRACTION (default 0.4)
}

impl Default for InjectionPolicy {
    fn default() -> Self {
        Self {
            confidence_injection: false,
            strong_match_score: STRONG_MATCH_SCORE_DEFAULT,
            min_inject_score: MIN_INJECT_SCORE_DEFAULT,
            min_inject_fraction: MIN_INJECT_FRACTION_DEFAULT,
        }
    }
}

impl InjectionPolicy {
    /// Read the four env vars Python reads at import. Unset ⇒ Python defaults
    /// (injection off; behaviour byte-identical to the ungated path).
    #[must_use]
    pub fn from_env() -> Self {
        let d = Self::default();
        let ci = std::env::var("LOOM_CONFIDENCE_INJECTION").ok();
        let confidence_injection = match ci.as_deref() {
            None | Some("0" | "false" | "no" | "") => false,
            Some(_) => true,
        };
        Self {
            confidence_injection,
            strong_match_score: env_f64("LOOM_STRONG_MATCH_SCORE", d.strong_match_score),
            min_inject_score: env_f64("LOOM_MIN_INJECT_SCORE", d.min_inject_score),
            min_inject_fraction: env_f64("LOOM_MIN_INJECT_FRACTION", d.min_inject_fraction),
        }
    }

    /// Given the top candidate score + the requested budget, decide the
    /// EFFECTIVE budget, or `None` to skip injection entirely.
    ///
    /// - injection off ⇒ `Some(requested)` (Python baseline: full budget).
    /// - `top_score < min_inject_score` ⇒ `None` (below-min skip).
    /// - else `Some(max(1, floor(requested * frac)))` where
    ///   `frac = min(1.0, max(min_inject_fraction, top_score/strong_match_score))`.
    #[must_use]
    pub fn effective_budget(&self, top_score: f64, requested: usize) -> Option<usize> {
        if !self.confidence_injection {
            return Some(requested);
        }
        if top_score < self.min_inject_score {
            return None;
        }
        let frac = if self.strong_match_score > 0.0 {
            let ratio = top_score / self.strong_match_score;
            1.0_f64.min(self.min_inject_fraction.max(ratio))
        } else {
            1.0
        };
        let scaled = ((requested as f64) * frac) as usize; // trunc toward zero == Python int()
        Some(scaled.max(1))
    }
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}
