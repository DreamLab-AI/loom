//! `confidence-check` — the dream-cycle evaluator for the confidence-surfacing
//! contract (ADR-138).
//!
//! Reads a `/health` payload on stdin (piped from curl, the same stdin contract
//! the retired `tests/graph_check.py` used so the entrypoint needs no inline
//! quoting — inline `python3 -c "…"` lost its inner quotes crossing the annexe
//! ssh `bash -lc` boundary, witnessed 2026-08-28).
//!
//!     curl -s --max-time 10 http://127.0.0.1:8084/health \
//!         | cargo run -q -p loom-facade --bin confidence-check
//!
//! It asserts the three blocks the contract adds to `/health` are PRESENT and
//! internally coherent. It deliberately does not assert a *value* of the gate
//! (injection on/off is an operator call); it asserts the operator can SEE the
//! gate, which is the falsification the 2026-09-02 dream cycle recorded: the
//! slot could not be evaluated because no confidence surface existed.
//!
//! It checks the payload TWICE, on purpose:
//!
//!  * against the facade's own [`HealthResponse`] struct, so a shape that has
//!    drifted from the type the node serves is caught as one clear error; and
//!  * field-by-field over `serde_json::Value`, which is what produces the
//!    readable "this block is MISSING" report an operator can act on. Serde
//!    stops at the first bad field; the `Value` pass reports every finding in
//!    one nightly run.
//!
//! The `Value` pass is also what keeps this bin useful against a node running
//! an OLDER build than the checker — the exact case the contract was written
//! for, since the HP container has to be redeployed before it can pass.
//! `graph`, `semantic` and `generation` stay untyped in `HealthResponse` itself
//! (they are re-serialised domain types), so `Value` is the only route to them.

use std::io::Read;
use std::process::ExitCode;

use loom_facade::routes::health::HealthResponse;
use serde_json::{Map, Value};

const OK: &str = "CONFIDENCE-SCAN-OK";
const FAIL: &str = "CONFIDENCE-SCAN-FAIL";

/// The score scales the contract names. `confidence` is only comparable within
/// a scale, so the scale is carried on the wire rather than assumed.
const SCALES: [&str; 2] = ["lexical-additive", "cosine"];

fn main() -> ExitCode {
    let mut raw = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
        eprintln!("{FAIL}: unreadable stdin: {e}");
        return ExitCode::FAILURE;
    }
    let health: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{FAIL}: /health is not JSON: {e}");
            eprintln!("{FAIL}: first 200 bytes: {}", &raw[..raw.len().min(200)]);
            return ExitCode::FAILURE;
        }
    };

    match scan(&health, &raw) {
        Ok(summary) => {
            println!("{OK} {summary}");
            ExitCode::SUCCESS
        }
        Err(findings) => {
            for f in &findings {
                eprintln!("{FAIL}: {f}");
            }
            eprintln!("{FAIL}: {} finding(s) in /health", findings.len());
            ExitCode::FAILURE
        }
    }
}

/// The `injection_policy` block, as far as it could be read.
#[derive(Default)]
struct Policy {
    injection: Option<bool>,
    strong: Option<f64>,
    min_score: Option<f64>,
    fraction: Option<f64>,
    scale: Option<String>,
}

/// The `confidence` rolling-window counters, as far as they could be read.
#[derive(Default)]
struct Counters {
    window: Option<f64>,
    requests: Option<f64>,
    engaged: Option<f64>,
    skipped: Option<f64>,
    verbatim: Option<f64>,
    mean: Option<f64>,
}

/// Run every contract check, collecting ALL findings so one nightly run reports
/// the whole gap rather than the first field that is missing.
fn scan(health: &Value, raw: &str) -> Result<String, Vec<String>> {
    let mut f: Vec<String> = Vec::new();
    // Shape gate: the body must still deserialise into the type the facade
    // serves. This catches a renamed or retyped field that the per-field pass
    // below would happily accept.
    if let Err(e) = serde_json::from_str::<HealthResponse>(raw) {
        f.push(format!(
            "/health does not deserialise into loom_facade::routes::health::HealthResponse: {e}"
        ));
    }
    let policy = scan_policy(health, &mut f);
    let verbatim_mode = scan_serving(health, policy.min_score, &mut f);
    let counters = scan_counters(health, &mut f);

    if f.is_empty() {
        Ok(summary(&policy, verbatim_mode, &counters))
    } else {
        Err(f)
    }
}

fn scan_policy(health: &Value, f: &mut Vec<String>) -> Policy {
    let Some(p) = block(health, "injection_policy", f) else {
        return Policy::default();
    };
    let out = Policy {
        injection: flag(p, "injection_policy", "confidence_injection", f),
        strong: number(p, "injection_policy", "strong_match_score", f),
        min_score: number(p, "injection_policy", "min_inject_score", f),
        fraction: number(p, "injection_policy", "min_inject_fraction", f),
        scale: text(p, "injection_policy", "score_scale", f),
    };

    if let (Some(lo), Some(hi)) = (out.min_score, out.strong) {
        if lo > hi {
            f.push(format!(
                "injection_policy.min_inject_score ({lo}) > strong_match_score ({hi}) — \
                 the skip floor sits above the full-budget ceiling, so every kept match \
                 is also a full-budget match and the gate cannot scale"
            ));
        }
    }
    if let Some(frac) = out.fraction {
        if !(frac > 0.0 && frac <= 1.0) {
            f.push(format!(
                "injection_policy.min_inject_fraction ({frac}) outside (0, 1] — \
                 it is a fraction of the budget, not a token count"
            ));
        }
    }
    if let Some(s) = &out.scale {
        if !SCALES.contains(&s.as_str()) {
            f.push(format!(
                "injection_policy.score_scale {s:?} is not one of {SCALES:?} — \
                 confidence is only comparable within a named scale"
            ));
        }
    }
    if let Some(hi) = out.strong {
        if hi <= 0.0 {
            f.push(format!(
                "injection_policy.strong_match_score ({hi}) must be > 0 — \
                 it is the confidence denominator"
            ));
        }
    }
    out
}

/// Returns `verbatim_mode` for the summary line.
fn scan_serving(health: &Value, min_score: Option<f64>, f: &mut Vec<String>) -> Option<bool> {
    let s = block(health, "serving", f)?;
    let verbatim_mode = flag(s, "serving", "verbatim_mode", f);
    let threshold = number(s, "serving", "verbatim_threshold", f);
    flag(s, "serving", "semantic_fallback", f);
    // `semantic_min_inject` is bench-set with no default: null is legitimate.
    nullable_number(s, "serving", "semantic_min_inject", f);

    if let (Some(true), Some(t), Some(lo)) = (verbatim_mode, threshold, min_score) {
        if t < lo {
            f.push(format!(
                "serving.verbatim_threshold ({t}) < injection_policy.min_inject_score ({lo}) — \
                 a scaffold too weak to inject would still be served verbatim"
            ));
        }
    }
    verbatim_mode
}

fn scan_counters(health: &Value, f: &mut Vec<String>) -> Counters {
    let Some(c) = block(health, "confidence", f) else {
        return Counters::default();
    };
    let out = Counters {
        window: counter(c, "confidence", "window", f),
        requests: counter(c, "confidence", "requests", f),
        engaged: counter(c, "confidence", "engaged", f),
        skipped: counter(c, "confidence", "skipped", f),
        verbatim: counter(c, "confidence", "verbatim", f),
        mean: number(c, "confidence", "mean_confidence", f),
    };
    let scaled = counter(c, "confidence", "scaled", f);
    let full = counter(c, "confidence", "full", f);

    if let Some(m) = out.mean {
        if !(0.0..=1.0).contains(&m) {
            f.push(format!(
                "confidence.mean_confidence ({m}) outside [0, 1] — confidence is a \
                 clamped ratio, not a raw score"
            ));
        }
    }
    if let (Some(r), Some(e), Some(s)) = (out.requests, out.engaged, out.skipped) {
        if e + s > r {
            f.push(format!(
                "confidence.engaged ({e}) + skipped ({s}) > requests ({r}) — \
                 a request is engaged or skipped, never both"
            ));
        }
    }
    // The decision counters partition the engaged requests.
    if let (Some(e), Some(sc), Some(fu), Some(vb)) = (out.engaged, scaled, full, out.verbatim) {
        if sc + fu + vb > e {
            f.push(format!(
                "confidence.scaled ({sc}) + full ({fu}) + verbatim ({vb}) > engaged ({e}) — \
                 the decision counters must partition the engaged requests"
            ));
        }
    }
    if let (Some(w), Some(r)) = (out.window, out.requests) {
        if r > w {
            f.push(format!(
                "confidence.requests ({r}) > window ({w}) — the counters are a \
                 rolling window, so requests cannot exceed it"
            ));
        }
    }
    out
}

/// One dense line an operator can read out of a nightly log.
fn summary(p: &Policy, verbatim_mode: Option<bool>, c: &Counters) -> String {
    let on = |b: Option<bool>| if b == Some(true) { "on" } else { "off" };
    let n = |v: Option<f64>| v.map_or_else(|| "?".to_owned(), |x| format!("{x}"));
    format!(
        "injection:{} scale:{} strong:{} min:{} frac:{} verbatim:{} \
         window:{} requests:{} engaged:{} skipped:{} verbatim_served:{} mean_confidence:{}",
        on(p.injection),
        p.scale.as_deref().unwrap_or("?"),
        n(p.strong),
        n(p.min_score),
        n(p.fraction),
        on(verbatim_mode),
        n(c.window),
        n(c.requests),
        n(c.engaged),
        n(c.skipped),
        n(c.verbatim),
        n(c.mean),
    )
}

// --- typed accessors over the untyped body ----------------------------------

fn block<'a>(health: &'a Value, key: &str, f: &mut Vec<String>) -> Option<&'a Map<String, Value>> {
    match health.get(key) {
        Some(Value::Object(m)) => Some(m),
        Some(other) => {
            f.push(format!("/health.{key} is {other} — expected an object"));
            None
        }
        None => {
            f.push(format!(
                "/health.{key} is MISSING — the confidence-surfacing contract \
                 (ADR-138) requires this block on every response"
            ));
            None
        }
    }
}

fn number(m: &Map<String, Value>, block: &str, key: &str, f: &mut Vec<String>) -> Option<f64> {
    let Some(v) = m.get(key) else {
        f.push(format!("{block}.{key} is MISSING"));
        return None;
    };
    match v.as_f64() {
        Some(n) if n.is_finite() => Some(n),
        _ => {
            f.push(format!("{block}.{key} is {v} — expected a finite number"));
            None
        }
    }
}

/// A number that may legitimately be `null` (an unset, no-default knob).
fn nullable_number(m: &Map<String, Value>, block: &str, key: &str, f: &mut Vec<String>) {
    match m.get(key) {
        None => f.push(format!("{block}.{key} is MISSING (null is a valid value)")),
        Some(Value::Null) => {}
        Some(v) if v.as_f64().is_some_and(f64::is_finite) => {}
        Some(v) => f.push(format!(
            "{block}.{key} is {v} — expected a finite number or null"
        )),
    }
}

/// A counter: finite, non-negative, integral.
fn counter(m: &Map<String, Value>, block: &str, key: &str, f: &mut Vec<String>) -> Option<f64> {
    let n = number(m, block, key, f)?;
    if n < 0.0 {
        f.push(format!(
            "{block}.{key} ({n}) is negative — counters only rise"
        ));
        return None;
    }
    if n.fract() != 0.0 {
        f.push(format!("{block}.{key} ({n}) is not a whole number"));
        return None;
    }
    Some(n)
}

fn flag(m: &Map<String, Value>, block: &str, key: &str, f: &mut Vec<String>) -> Option<bool> {
    match m.get(key) {
        Some(Value::Bool(b)) => Some(*b),
        Some(v) => {
            f.push(format!("{block}.{key} is {v} — expected a boolean"));
            None
        }
        None => {
            f.push(format!("{block}.{key} is MISSING"));
            None
        }
    }
}

fn text(m: &Map<String, Value>, block: &str, key: &str, f: &mut Vec<String>) -> Option<String> {
    match m.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(v) => {
            f.push(format!("{block}.{key} is {v} — expected a string"));
            None
        }
        None => {
            f.push(format!("{block}.{key} is MISSING"));
            None
        }
    }
}
