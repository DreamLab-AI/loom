//! The confidence-gated selective-injection policy — THE gate (Invariant I-P1 /
//! ADR-136 D3). [`decide`] is byte-identical to Python's `scaffold()` gate
//! branch. Computed in `f64` because Python is: the doc's `f32` sketch (§5.4)
//! loses to byte-parity here.
//!
//! [`decide`] is the surfacing form: it returns the DECISION alongside the
//! budget, so the served telemetry can say *why* a request got the budget it
//! got rather than leaving the caller to reverse-engineer it from a bare
//! `Option<usize>`. [`GatePolicy::effective_budget`] is the old
//! budget-only shape, kept as a thin wrapper for one release.

use loom_domain::{GateThresholds, InjectionDecision};

use crate::tuning::{
    MIN_INJECT_FRACTION_DEFAULT, MIN_INJECT_SCORE_DEFAULT, STRONG_MATCH_SCORE_DEFAULT,
};

/// Confidence-injection knobs. Defaults are Python-baseline (injection OFF),
/// each field env-overridable via `from_env` exactly as the Python module.
#[derive(Debug, Clone, Copy)]
pub struct GatePolicy {
    pub confidence_injection: bool, // LOOM_CONFIDENCE_INJECTION
    pub strong_match_score: f64,    // LOOM_STRONG_MATCH_SCORE (default EXACT_TITLE_WEIGHT)
    pub min_inject_score: f64,      // LOOM_MIN_INJECT_SCORE   (default MIN_SEED_SCORE)
    pub min_inject_fraction: f64,   // LOOM_MIN_INJECT_FRACTION (default 0.4)
}

/// The pre-grounding-contract name for [`GatePolicy`]. Retained for one release
/// so existing call sites keep compiling; prefer `GatePolicy` in new code.
pub type InjectionPolicy = GatePolicy;

impl Default for GatePolicy {
    fn default() -> Self {
        Self {
            confidence_injection: false,
            strong_match_score: STRONG_MATCH_SCORE_DEFAULT,
            min_inject_score: MIN_INJECT_SCORE_DEFAULT,
            min_inject_fraction: MIN_INJECT_FRACTION_DEFAULT,
        }
    }
}

/// The gate's two published thresholds, so `loom-domain` can describe a
/// grounding without depending on this crate (the hexagon points inward).
impl GateThresholds for GatePolicy {
    fn strong_match_score(&self) -> f64 {
        self.strong_match_score
    }

    fn min_inject_score(&self) -> f64 {
        self.min_inject_score
    }
}

impl GatePolicy {
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
    /// Thin wrapper over [`decide`], kept for one release. New callers should
    /// use `decide` and keep the [`InjectionDecision`] for the telemetry.
    #[must_use]
    pub fn effective_budget(&self, top_score: f64, requested: usize) -> Option<usize> {
        decide(Some(top_score), requested, self).1
    }
}

/// THE gate. Returns what the gate decided AND the budget that decision yields.
///
/// - injection off ⇒ `(Full, Some(requested))` — the Python baseline, full
///   budget whenever any seed matched at all.
/// - no score at all (nothing matched) ⇒ `(Skipped, None)`.
/// - `top_score < min_inject_score` ⇒ `(Skipped, None)` — the below-min skip.
/// - otherwise the budget is scaled by
///   `frac = min(1.0, max(min_inject_fraction, top_score / strong_match_score))`
///   and the result is `max(1, floor(requested * frac))`. `frac == 1.0` (a
///   match at or above `strong_match_score`, or a `min_inject_fraction` of 1.0,
///   or a non-positive `strong_match_score` that guards the divide) is reported
///   as `Full`; anything less is `Scaled`.
///
/// The arithmetic is byte-identical to the pre-existing `effective_budget`,
/// including the `max(1, …)` floor on every gated branch: only the returned
/// DECISION is new.
#[must_use]
pub fn decide(
    top_score: Option<f64>,
    requested: usize,
    gate: &GatePolicy,
) -> (InjectionDecision, Option<usize>) {
    if !gate.confidence_injection {
        return (InjectionDecision::Full, Some(requested));
    }
    let Some(top) = top_score else {
        return (InjectionDecision::Skipped, None);
    };
    if top < gate.min_inject_score {
        return (InjectionDecision::Skipped, None);
    }
    let frac = if gate.strong_match_score > 0.0 {
        let ratio = top / gate.strong_match_score;
        1.0_f64.min(gate.min_inject_fraction.max(ratio))
    } else {
        1.0
    };
    let scaled = ((requested as f64) * frac) as usize; // trunc toward zero == Python int()
    let decision = if frac >= 1.0 {
        InjectionDecision::Full
    } else {
        InjectionDecision::Scaled
    };
    (decision, Some(scaled.max(1)))
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}
