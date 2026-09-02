//! `generation-check` — the SSOT generation-drift guard. Rust port of the
//! retired `tests/generation_drift_check.py`.
//!
//! Reads a `/health` payload on stdin and compares the served top-level
//! `generation` (source: `ScaffoldIndex`) against `semantic.generation`
//! (source: `MirrorManifest`). Emits a DRIFT line per inconsistency.
//!
//! Warn-only by default (exit 0) so nightly evaluators degrade gracefully;
//! pass `--strict` to exit 1 on drift.
//!
//!     curl -s --max-time 10 http://127.0.0.1:8084/health \
//!         | cargo run -q -p loom-facade --bin generation-check
//!
//! The never-mixed-build law this guards is ADR-135 D2.1 / ADR-136 D4: two
//! units with different `GenerationId` must never be served together.
//!
//! Timestamp handling: the workspace carries no date crate on purpose (the
//! release binary is a static single artifact), so the RFC 3339 subset the
//! mirror actually emits is parsed inline — `YYYY-MM-DDTHH:MM:SS[.fff]` with a
//! `Z` or `±HH:MM` offset, both witnessed live on the HP node.

use std::io::Read;
use std::process::ExitCode;

use serde_json::Value;

fn main() -> ExitCode {
    let strict = std::env::args().skip(1).any(|a| a == "--strict");

    let mut raw = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
        println!("DRIFT-CHECK ERROR: unreadable stdin: {e}");
        return if strict {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }
    let health: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            println!("DRIFT-CHECK ERROR: unreadable /health payload: {e}");
            return if strict {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            };
        }
    };

    let drift = findings(&health);
    if drift.is_empty() {
        println!("DRIFT-CHECK: OK — served generation matches promoted mirror generation");
        return ExitCode::SUCCESS;
    }
    for d in &drift {
        println!("DRIFT: {d}");
    }
    println!(
        "DRIFT-CHECK: {} finding(s) — facade may be serving a stale generation",
        drift.len()
    );
    if strict {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The four drift comparisons, in the order the Python emitted them.
fn findings(health: &Value) -> Vec<String> {
    let served = health.get("generation").unwrap_or(&Value::Null);
    let semantic = health
        .get("semantic")
        .and_then(|s| s.get("generation"))
        .unwrap_or(&Value::Null);
    let index_classes = health.get("index_classes").and_then(Value::as_i64);

    let mut drift = Vec::new();

    let sc = served.get("class_count").and_then(Value::as_i64);
    let mc = semantic.get("class_count").and_then(Value::as_i64);
    if let (Some(s), Some(m)) = (sc, mc) {
        if s != m {
            drift.push(format!(
                "class_count served={s} promoted={m} (delta={:+})",
                m - s
            ));
        }
    }

    let sg_raw = served.get("generated_at").and_then(Value::as_str);
    let mg_raw = semantic.get("generated_at").and_then(Value::as_str);
    if let (Some(sg), Some(mg)) = (sg_raw.and_then(epoch), mg_raw.and_then(epoch)) {
        if sg < mg {
            drift.push(format!(
                "served generation {} is older than promoted mirror {}",
                sg_raw.unwrap_or("?"),
                mg_raw.unwrap_or("?"),
            ));
        }
    }

    if served.get("verified_single_generation") == Some(&Value::Bool(false))
        && semantic.get("verified_single_generation") == Some(&Value::Bool(true))
    {
        drift.push(
            "served index is unverified while a verified mirror generation exists".to_owned(),
        );
    }

    if let (Some(ic), Some(m)) = (index_classes, mc) {
        if ic != m {
            drift.push(format!("index_classes={ic} != promoted class_count={m}"));
        }
    }

    drift
}

/// Parse the RFC 3339 subset the mirror emits into whole seconds since the
/// epoch. Returns `None` for anything it does not recognise, which makes the
/// comparison skip rather than report false drift.
fn epoch(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64 = s.get(8..10)?.parse().ok()?;
    let hour: i64 = s.get(11..13)?.parse().ok()?;
    let minute: i64 = s.get(14..16)?.parse().ok()?;
    let second: i64 = s.get(17..19)?.parse().ok()?;

    // Everything after the seconds: an optional fraction (ignored — the tests
    // compare to whole seconds) then an optional Z / ±HH:MM offset.
    let tail = s.get(19..).unwrap_or("");
    let tail = tail.strip_prefix('.').map_or(tail, |frac| {
        let digits = frac.len() - frac.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        &frac[digits..]
    });
    let offset_secs = match tail {
        "" | "Z" | "z" => 0,
        t => {
            let sign: i64 = match t.as_bytes().first() {
                Some(b'+') => 1,
                Some(b'-') => -1,
                _ => return None,
            };
            let oh: i64 = t.get(1..3)?.parse().ok()?;
            let om: i64 = t.get(4..6)?.parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
    };

    Some(
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second
            - offset_secs,
    )
}

/// Howard Hinnant's `days_from_civil`: days since 1970-01-01 for a proleptic
/// Gregorian date. Exact for every date the mirror can stamp.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::{epoch, findings};
    use serde_json::json;

    #[test]
    fn parses_both_stamp_shapes_the_mirror_emits() {
        // Witnessed live on the HP node, 2026-09-02.
        let fractional = epoch("2026-08-22T08:19:43.776950+00:00").expect("fractional + offset");
        let zulu = epoch("2026-08-17T14:54:45Z").expect("zulu");
        assert!(zulu < fractional, "{zulu} should precede {fractional}");
        assert_eq!(epoch("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            epoch("2026-01-01T01:00:00+01:00"),
            epoch("2026-01-01T00:00:00Z")
        );
        assert_eq!(epoch("not a date"), None);
    }

    #[test]
    fn clean_health_reports_no_drift() {
        let health = json!({
            "index_classes": 8146,
            "generation": {
                "class_count": 8146,
                "generated_at": "2026-08-22T08:19:43.776950+00:00",
                "verified_single_generation": true
            },
            "semantic": { "generation": {
                "class_count": 8146,
                "generated_at": "2026-08-17T14:54:45Z",
                "verified_single_generation": true
            }}
        });
        assert!(findings(&health).is_empty());
    }

    #[test]
    fn stale_served_generation_is_drift() {
        let health = json!({
            "index_classes": 8000,
            "generation": {
                "class_count": 8000,
                "generated_at": "2026-08-01T00:00:00Z",
                "verified_single_generation": false
            },
            "semantic": { "generation": {
                "class_count": 8146,
                "generated_at": "2026-08-17T14:54:45Z",
                "verified_single_generation": true
            }}
        });
        let d = findings(&health);
        assert_eq!(d.len(), 4, "expected all four comparisons to fire: {d:?}");
    }
}
