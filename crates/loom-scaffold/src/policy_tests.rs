//! EXP-003 — confidence-gate math parity. Split out of `tests.rs` for file-size
//! discipline. These pin `effective_budget` (the wrapper kept for one release)
//! against Python's `scaffold()` gate branch, number for number; the DECISION
//! half of the same gate is covered in `grounding_tests`.

use crate::policy::InjectionPolicy;

// --- EXP-003: confidence-gate math parity -----------------------------------

fn policy(ci: bool, strong: f64, min: f64, frac: f64) -> InjectionPolicy {
    InjectionPolicy {
        confidence_injection: ci,
        strong_match_score: strong,
        min_inject_score: min,
        min_inject_fraction: frac,
    }
}

#[test]
fn gate_off_returns_full_budget_regardless_of_score() {
    let p = policy(false, 8.0, 2.0, 0.4);
    // injection off ⇒ Python baseline: full budget whenever any seed matched.
    assert_eq!(p.effective_budget(0.0, 1500), Some(1500));
    assert_eq!(p.effective_budget(19.5, 1500), Some(1500));
    assert_eq!(p.effective_budget(2.0, 1500), Some(1500));
}

#[test]
fn gate_on_below_min_skips() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // top below MIN_INJECT_SCORE ⇒ skip injection entirely.
    assert_eq!(p.effective_budget(1.99, 1500), None);
    assert_eq!(p.effective_budget(0.0, 1500), None);
}

#[test]
fn gate_on_at_or_above_strong_full_budget() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // ratio >= 1.0 ⇒ frac clamped to 1.0 ⇒ full budget.
    assert_eq!(p.effective_budget(8.0, 1500), Some(1500));
    assert_eq!(p.effective_budget(19.5, 1500), Some(1500));
}

#[test]
fn gate_on_midrange_scales_budget() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // top=4.0, strong=8.0 ⇒ frac=0.5 (exact) ⇒ 1500*0.5 = 750.
    assert_eq!(p.effective_budget(4.0, 1500), Some(750));
    // top=6.0 ⇒ frac=0.75 (exact) ⇒ 1125.
    assert_eq!(p.effective_budget(6.0, 1500), Some(1125));
}

#[test]
fn gate_on_fraction_floor_clamps() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // top=2.0 (== min) ⇒ ratio 0.25 < 0.4 ⇒ clamp UP to 0.4, not 0.25.
    let eff = p.effective_budget(2.0, 1500).unwrap();
    // identical f64 path to Python's int(1500*0.4); NOT the un-clamped 375.
    assert_eq!(eff, (1500.0_f64 * 0.4) as usize);
    assert_ne!(eff, (1500.0_f64 * 0.25) as usize);
}

#[test]
fn gate_on_strong_zero_avoids_div() {
    let p = policy(true, 0.0, 2.0, 0.4);
    // strong_match_score <= 0 ⇒ frac stays 1.0 (Python guards the divide).
    assert_eq!(p.effective_budget(5.0, 1500), Some(1500));
}

#[test]
fn gate_on_min_one_budget_floor() {
    let p = policy(true, 8.0, 0.1, 0.4);
    // tiny requested budget ⇒ max(1, floor) keeps at least 1 token.
    assert_eq!(p.effective_budget(1.0, 1), Some(1));
    assert_eq!(p.effective_budget(1.0, 2), Some(1)); // int(2*0.4)=0 → max(1,0)=1
}
