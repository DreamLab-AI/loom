//! Capture the RELEASE IDENTITY at compile time (ADR-137 closeout).
//!
//! The finding: "the workspace consumes `../ruvector/crates/ruvector-core` by
//! path, so a Loom revision alone does not pin the build". A path dependency has
//! no lockfile entry to pin — `Cargo.lock` records `ruvector-core 0.1.0 (path)`
//! and nothing about which commit that path was on. So the identity has to be
//! read from the sibling checkout at build time and baked in.
//!
//! Everything emitted here is a `cargo::rustc-env` the `build_info` module reads
//! back. Nothing is looked up at runtime: a receipt that re-read the sibling
//! checkout when asked would describe the machine it is running on, not the
//! binary that is running.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    // crates/loom-facade → crates → <loom> → <workspace parent>
    let loom_root = manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest.clone(), Path::to_path_buf);
    let ruvector_root = loom_root
        .parent()
        .map_or_else(|| loom_root.join("ruvector"), |p| p.join("ruvector"));

    emit("LOOM_BUILD_GIT_SHA", &git_head(&loom_root));
    emit("LOOM_BUILD_RUVECTOR_SHA", &git_head(&ruvector_root));
    emit(
        "LOOM_BUILD_RUVECTOR_PATH",
        &ruvector_root.display().to_string(),
    );
    emit("LOOM_BUILD_RUSTC", &rustc_version());
    emit("LOOM_BUILD_PROFILE", &env("PROFILE"));
    emit("LOOM_BUILD_TARGET", &env("TARGET"));
    emit("LOOM_BUILD_HOST", &env("HOST"));
    // The features THIS crate was compiled with (CARGO_FEATURE_* → kebab list).
    emit("LOOM_BUILD_FEATURES", &enabled_features());
    // The explicit ruvector-core feature set, read from the workspace manifest
    // so the receipt states what was actually requested rather than repeating a
    // constant that could drift from Cargo.toml.
    emit(
        "LOOM_BUILD_RUVECTOR_FEATURES",
        &ruvector_features(&loom_root.join("Cargo.toml")),
    );

    // Re-run when either checkout's HEAD moves, or the manifest changes.
    for root in [&loom_root, &ruvector_root] {
        println!("cargo::rerun-if-changed={}", root.join(".git/HEAD").display());
    }
    println!(
        "cargo::rerun-if-changed={}",
        loom_root.join("Cargo.toml").display()
    );
    println!("cargo::rerun-if-changed=build.rs");
}

fn emit(key: &str, value: &str) {
    println!("cargo::rustc-env={key}={value}");
}

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| "unknown".to_owned())
}

/// Resolve `<root>/.git/HEAD` to a commit sha without shelling out to git (the
/// build may run in an image with no git binary). Handles both a detached HEAD
/// and a symbolic ref, and falls back to `packed-refs` for a packed branch.
fn git_head(root: &Path) -> String {
    let git_dir = root.join(".git");
    let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) else {
        return "unknown".to_owned();
    };
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref: ") else {
        // Detached HEAD: the file already holds the sha.
        return head.to_owned();
    };
    if let Ok(sha) = std::fs::read_to_string(git_dir.join(reference)) {
        return sha.trim().to_owned();
    }
    // Packed refs.
    std::fs::read_to_string(git_dir.join("packed-refs"))
        .ok()
        .and_then(|packed| {
            packed
                .lines()
                .filter(|l| !l.starts_with('#') && !l.starts_with('^'))
                .find_map(|l| {
                    let (sha, name) = l.split_once(' ')?;
                    (name.trim() == reference).then(|| sha.trim().to_owned())
                })
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

fn rustc_version() -> String {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        )
}

/// `CARGO_FEATURE_FOO=1` → `foo`, comma-joined and sorted.
fn enabled_features() -> String {
    let mut names: Vec<String> = std::env::vars()
        .filter_map(|(k, _)| {
            k.strip_prefix("CARGO_FEATURE_")
                .map(|f| f.to_lowercase().replace('_', "-"))
        })
        .collect();
    names.sort();
    names.join(",")
}

/// Extract the `features = [...]` list from the workspace manifest's
/// `ruvector-core` dependency line. A deliberately small, format-specific read:
/// the manifest line is a single-line array in this workspace, and pulling in a
/// TOML parser as a build dependency to read one array is not worth the cost.
fn ruvector_features(manifest: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return "unknown".to_owned();
    };
    text.lines()
        .find(|l| l.trim_start().starts_with("ruvector-core"))
        .and_then(|line| {
            let start = line.find("features = [")? + "features = [".len();
            let rest = &line[start..];
            let end = rest.find(']')?;
            Some(
                rest[..end]
                    .split(',')
                    .map(|f| f.trim().trim_matches('"').to_owned())
                    .filter(|f| !f.is_empty())
                    .collect::<Vec<_>>()
                    .join(","),
            )
        })
        .unwrap_or_else(|| "unknown".to_owned())
}
