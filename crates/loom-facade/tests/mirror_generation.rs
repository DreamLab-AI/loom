//! Behavioural tests for the `mirror.sh` generation verifier — the SSOT-boundary
//! guarantee that the Loom never serves a mixed-build set (ADR-136 D4 /
//! ADR-135 D2.1). Rust port of the retired `tests/test_mirror_generation.py`.
//!
//! `app/mirror.sh` STAYS: it is the promote mechanism the Rust Loom reads (the
//! Rust node implements only the READ side of the generation contract). The
//! verifier under test is the inline python block inside `app/mirror.sh`, so
//! this test still drives `python3` as a subprocess — what the port removes is
//! the pytest harness, not the subject. The block is EXTRACTED from the shipped
//! script (never copied) so the test cannot drift from the code it guards.
//!
//!     cargo test -q -p loom-facade --test mirror_generation
//!
//! Skips with a message when `python3` is absent, so a nightly evaluator on a
//! python-free image degrades rather than failing red.
//!
//! Asserted, unchanged from the Python:
//!   * a mixed-build candidate (stamps spanning > `GEN_TOL`) is REJECTED (exit 2)
//!     and the live set is left untouched — never a partial promotion;
//!   * a consistent fresh build is PROMOTED atomically (exit 0) with a
//!     `.generation.json` manifest written;
//!   * an all-current run (nothing downloaded) is a clean no-op (exit 0);
//!   * a failed fetch with no prior copy is REJECTED (exit 2).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};
use tempfile::TempDir;

const ARTIFACTS: [&str; 4] = [
    "scaffold-index.json",
    "prose-index.json",
    "ontology.ttl",
    "ontology-inferred.ttl",
];

/// Repo root, resolved from the crate manifest dir (CWD-independent), matching
/// `live_smoke.rs`.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn mirror_sh() -> PathBuf {
    root().join("app/mirror.sh")
}

/// Extract the exact inline python block from `mirror.sh` (between `<<'PY'` and
/// the closing `PY` line).
fn verifier_src() -> String {
    let text = std::fs::read_to_string(mirror_sh()).expect("read app/mirror.sh");
    let start = text
        .find("<<'PY'\n")
        .expect("could not locate the `<<'PY'` heredoc opener in mirror.sh")
        + "<<'PY'\n".len();
    let rest = &text[start..];
    let end = rest
        .find("\nPY\n")
        .expect("could not locate the closing `PY` line in mirror.sh");
    rest[..end].to_owned()
}

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Days since the epoch for a proleptic Gregorian date (Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Microseconds since the epoch for a `YYYY-MM-DDTHH:MM:SS` UTC stamp. Integer
/// throughout: the sub-second offsets below are exact, and no float rounding
/// can drift a stamp across the verifier's `GEN_TOL` boundary.
fn epoch(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> i64 {
    (days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s) * 1_000_000
}

/// Render microseconds-since-epoch back to the `+00:00` ISO form the verifier
/// parses.
fn iso(epoch_micros: i64) -> String {
    let secs = epoch_micros.div_euclid(1_000_000);
    let micros = epoch_micros.rem_euclid(1_000_000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{micros:06}+00:00")
}

/// Inverse of `days_from_civil`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Write a consistent artifact set whose stamps cluster around `base`
/// (microseconds since the epoch), the +0.3s / +0.9s / +4.9s spread the Python
/// used — inside the 300s `GEN_TOL`, so the set reads as ONE build.
fn write_set(dir: &Path, base: i64) {
    std::fs::create_dir_all(dir).expect("mkdir set");
    let offsets = [300_000_i64, 900_000, 4_900_000];
    std::fs::write(
        dir.join("scaffold-index.json"),
        json!({ "version": 1, "generated": iso(base + offsets[0]), "counts": { "classes": 8146 } })
            .to_string(),
    )
    .expect("write scaffold-index");
    std::fs::write(
        dir.join("prose-index.json"),
        json!({ "version": 1, "generated": iso(base + offsets[1]), "counts": { "pages": 5854 } })
            .to_string(),
    )
    .expect("write prose-index");
    std::fs::write(
        dir.join("ontology-inferred.ttl"),
        format!(
            "@prefix vc: <x:> .\nvc:o vc:generatedAt \"{}\" .\n",
            iso(base + offsets[2])
        ),
    )
    .expect("write inferred ttl");
    std::fs::write(
        dir.join("ontology.ttl"),
        "@prefix vc: <x:> .\nvc:o a vc:Ontology .\n",
    )
    .expect("write ttl");
}

/// Drive the extracted verifier exactly as `mirror.sh` does.
fn run(verifier: &Path, data: &Path, stage: &Path, downloaded: &[&str], failed: &str) -> Output {
    Command::new("python3")
        .arg(verifier)
        .arg(data)
        .arg(stage)
        .arg("300")
        .arg("https://test")
        .arg(downloaded.join(" "))
        .arg(failed)
        .output()
        .expect("run the extracted verifier")
}

/// A test environment: a tempdir holding the extracted verifier plus `data/`
/// and `data/.stage/`.
struct Env {
    _dir: TempDir,
    verifier: PathBuf,
    data: PathBuf,
    stage: PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let verifier = dir.path().join("verify.py");
        std::fs::write(&verifier, verifier_src()).expect("write verifier");
        let data = dir.path().join("data");
        let stage = data.join(".stage");
        Self {
            _dir: dir,
            verifier,
            data,
            stage,
        }
    }

    fn run(&self, downloaded: &[&str], failed: &str) -> Output {
        run(&self.verifier, &self.data, &self.stage, downloaded, failed)
    }
}

/// 2026-08-15T13:22:45Z — the Python's live-set base.
fn live_base() -> i64 {
    epoch(2026, 8, 15, 13, 22, 45)
}

/// 2026-08-16T20:00:00Z — the Python's fresh-build base.
fn fresh_base() -> i64 {
    epoch(2026, 8, 16, 20, 0, 0)
}

fn skip_if_no_python(name: &str) -> bool {
    if python3_available() {
        return false;
    }
    eprintln!("[{name}] SKIP — python3 not on PATH; the verifier under test is the inline python block in app/mirror.sh");
    true
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn rejects_mixed_build() {
    if skip_if_no_python("rejects_mixed_build") {
        return;
    }
    let env = Env::new();
    write_set(&env.data, live_base()); // consistent live set
    std::fs::create_dir_all(&env.stage).expect("mkdir stage");
    // A freshly-"downloaded" scaffold from a DIFFERENT build (5 days off).
    std::fs::write(
        env.stage.join("scaffold-index.json"),
        json!({
            "version": 1,
            "generated": "2026-08-10T09:00:00+00:00",
            "counts": { "classes": 8146 }
        })
        .to_string(),
    )
    .expect("write mixed scaffold");

    let out = env.run(&["scaffold-index.json"], "");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected REJECT(2), stderr: {}",
        stderr_of(&out)
    );
    assert!(
        stderr_of(&out).to_lowercase().contains("mixed build"),
        "stderr: {}",
        stderr_of(&out)
    );

    // Live set untouched; no manifest written — never a partial promotion.
    let live: Value = serde_json::from_str(
        &std::fs::read_to_string(env.data.join("scaffold-index.json")).expect("read live scaffold"),
    )
    .expect("live scaffold is JSON");
    assert!(
        live["generated"]
            .as_str()
            .is_some_and(|s| s.starts_with("2026-08-15")),
        "live scaffold must be kept, not the mixed one: {}",
        live["generated"]
    );
    assert!(!env.data.join(".generation.json").exists());
}

#[test]
fn promotes_consistent_new_build() {
    if skip_if_no_python("promotes_consistent_new_build") {
        return;
    }
    let env = Env::new();
    write_set(&env.data, live_base()); // old live set
    write_set(&env.stage, fresh_base()); // consistent new build, all fresh

    let out = env.run(&ARTIFACTS, "");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected PROMOTE(0), stderr: {}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).contains("PROMOTED"),
        "stdout: {}",
        stdout_of(&out)
    );

    let man: Value = serde_json::from_str(
        &std::fs::read_to_string(env.data.join(".generation.json")).expect("read manifest"),
    )
    .expect("manifest is JSON");
    assert!(
        man["generation"]
            .as_str()
            .is_some_and(|s| s.starts_with("2026-08-16")),
        "manifest generation: {}",
        man["generation"]
    );
    let artifacts = man["artifacts"].as_object().expect("manifest artifacts");
    for name in ARTIFACTS {
        let entry = artifacts
            .get(name)
            .unwrap_or_else(|| panic!("manifest missing artifact {name}"));
        assert!(
            entry.get("sha256").is_some(),
            "artifact {name} has no sha256: {entry}"
        );
    }
    assert_eq!(artifacts.len(), ARTIFACTS.len());

    // Live scaffold is now the new generation.
    let live: Value = serde_json::from_str(
        &std::fs::read_to_string(env.data.join("scaffold-index.json")).expect("read live scaffold"),
    )
    .expect("live scaffold is JSON");
    assert!(
        live["generated"]
            .as_str()
            .is_some_and(|s| s.starts_with("2026-08-16")),
        "live scaffold: {}",
        live["generated"]
    );
}

#[test]
fn current_when_nothing_downloaded() {
    if skip_if_no_python("current_when_nothing_downloaded") {
        return;
    }
    let env = Env::new();
    write_set(&env.data, live_base());
    std::fs::create_dir_all(&env.stage).expect("mkdir stage");

    let out = env.run(&[], "");
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected no-op(0), stderr: {}",
        stderr_of(&out)
    );
    assert!(
        stdout_of(&out).to_lowercase().contains("current"),
        "stdout: {}",
        stdout_of(&out)
    );
    // No promotion => no manifest.
    assert!(!env.data.join(".generation.json").exists());
}

#[test]
fn rejects_failed_fetch_with_no_prior() {
    if skip_if_no_python("rejects_failed_fetch_with_no_prior") {
        return;
    }
    let env = Env::new();
    write_set(&env.data, live_base());
    std::fs::create_dir_all(&env.stage).expect("mkdir stage");

    let out = env.run(&[], "ontology.ttl");
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected REJECT(2), stderr: {}",
        stderr_of(&out)
    );
    assert!(
        stderr_of(&out).to_lowercase().contains("unreachable"),
        "stderr: {}",
        stderr_of(&out)
    );
}
