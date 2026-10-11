//! Migration test for ontomark step 10 (`ontomark/cli-publish`), EXP-10 rows LM-06 and LM-09 (decision D9).
//!
//! Red until the Python generation-marker check is deleted (the Rust
//! `loom-facade::mirror` check is the only one), the reload timer no longer runs
//! Python, and the publishing path builds with a pinned `ontomark` release
//! instead of `vault`. Reads files only; never imports ontomark.

use std::fs;
use std::path::Path;

fn path(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn read(rel: &str) -> String {
    fs::read_to_string(path(rel)).unwrap_or_default()
}

#[test]
fn lm06_python_marker_check_is_deleted_and_rust_check_remains() {
    for (row, f) in [
        ("LM-06", "scripts/reload-published-generation.py"),
        ("LM-06", "tests/test_reload_published_generation.py"),
    ] {
        assert!(
            !path(f).exists(),
            "{row}: {f} must be deleted (the Rust mirror.rs check is the only one)"
        );
    }
    assert!(
        path("crates/loom-facade/src/mirror.rs").exists(),
        "LM-06: crates/loom-facade/src/mirror.rs (the surviving marker check) is missing"
    );
}

#[test]
fn lm06_reload_timer_runs_no_python() {
    let unit = read("deploy/loom-generation-reload.service");
    let exec = unit
        .lines()
        .find(|l| l.starts_with("ExecStart="))
        .unwrap_or("");
    assert!(
        !exec.contains("python") && !exec.contains(".py"),
        "LM-06: deploy/loom-generation-reload.service ExecStart still runs the Python reloader: {exec}"
    );
    assert!(
        exec.contains("reload"),
        "LM-06: ExecStart must run a Rust reload binary, got: {exec}"
    );
}

#[test]
fn lm09_promote_error_text_names_ontomark_build() {
    let artifact = read("crates/loom-vector-ruvector/src/artifact.rs");
    assert!(
        !artifact.contains("`vault build --with-rvdb`"),
        "LM-09: crates/loom-vector-ruvector/src/artifact.rs:184 still tells the operator to run `vault build --with-rvdb`"
    );
    assert!(
        artifact.contains("ontomark build --with-rvdb"),
        "LM-09: artifact.rs error text must say `ontomark build --with-rvdb`"
    );
}

#[test]
fn lm09_publishing_runbook_builds_with_a_pinned_ontomark_release() {
    let doc = read("docs/published-generation-reload.md");
    assert!(
        doc.contains("ontomark build"),
        "LM-09: docs/published-generation-reload.md must build the bundle with `ontomark build`"
    );
    assert!(
        doc.lines()
            .any(|l| l.to_lowercase().contains("ontomark") && l.to_lowercase().contains("tag")),
        "LM-09: docs/published-generation-reload.md must pin `ontomark` to a release tag"
    );
    for stale in [
        "cargo build --release -p vault",
        "vault-target/release/vault",
        "reload-published-generation.py",
    ] {
        assert!(
            !doc.contains(stale),
            "LM-06/LM-09: docs/published-generation-reload.md still mentions `{stale}`"
        );
    }
}
