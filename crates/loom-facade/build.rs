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

    // Track the metadata that actually supplied each revision, including a
    // linked worktree's shared branch ref. HEAD alone does not move on commit.
    for root in [&loom_root, &ruvector_root] {
        for path in git_revision(root).watched {
            println!("cargo::rerun-if-changed={}", path.display());
        }
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

/// Resolve normal and linked worktree revisions without a Git executable.
fn git_head(root: &Path) -> String {
    git_revision(root).sha
}

struct GitRevision {
    sha: String,
    watched: Vec<PathBuf>,
}

fn git_revision(root: &Path) -> GitRevision {
    let dotgit = root.join(".git");
    let mut revision = GitRevision {
        sha: "unknown".to_owned(),
        watched: Vec::new(),
    };
    let git_dir = if dotgit.is_dir() {
        dotgit
    } else {
        revision.watched.push(dotgit.clone());
        let Ok(pointer) = std::fs::read_to_string(&dotgit) else {
            return revision;
        };
        let Some(path) = pointer
            .trim()
            .strip_prefix("gitdir: ")
            .filter(|s| !s.is_empty())
        else {
            return revision;
        };
        root.join(path)
    };
    let common_pointer = git_dir.join("commondir");
    let common = std::fs::read_to_string(&common_pointer).ok().map_or_else(
        || git_dir.clone(),
        |path| {
            revision.watched.push(common_pointer);
            git_dir.join(path.trim())
        },
    );
    let head_path = git_dir.join("HEAD");
    revision.watched.push(head_path.clone());
    let Ok(head) = std::fs::read_to_string(head_path) else {
        return revision;
    };
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref: ") else {
        if valid_sha(head) {
            head.clone_into(&mut revision.sha);
        }
        return revision;
    };
    // Ref names are relative Git paths; malformed metadata cannot redirect a
    // build outside either Git metadata directory.
    if !reference.starts_with("refs/")
        || !Path::new(reference)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return revision;
    }
    for metadata in [&git_dir, &common] {
        let loose = metadata.join(reference);
        if let Ok(sha) = std::fs::read_to_string(&loose) {
            revision.watched.push(loose);
            if valid_sha(sha.trim()) {
                sha.trim().clone_into(&mut revision.sha);
            }
            return revision;
        }
        // A packed branch may later acquire a loose ref. Track the closest
        // existing parent so that creation triggers Cargo, without tracking a
        // permanently absent file and rebuilding on every invocation.
        if let Some(parent) = loose.ancestors().skip(1).find(|p| p.is_dir()) {
            revision.watched.push(parent.to_path_buf());
        }
    }
    for metadata in [&git_dir, &common] {
        let path = metadata.join("packed-refs");
        if let Ok(packed) = std::fs::read_to_string(&path) {
            revision.watched.push(path);
            if let Some(sha) = packed.lines().find_map(|line| {
                let (sha, name) = line.split_once(' ')?;
                (name == reference && valid_sha(sha)).then(|| sha.to_owned())
            }) {
                revision.sha = sha;
                return revision;
            }
        }
    }
    revision
}

fn valid_sha(sha: &str) -> bool {
    matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| b.is_ascii_hexdigit())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const FIRST: &str = "1111111111111111111111111111111111111111";
    const SECOND: &str = "2222222222222222222222222222222222222222";

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn linked_fixture() -> (TempDir, PathBuf, PathBuf, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("checkout");
        let common = dir.path().join("repository/.git");
        let admin = common.join("worktrees/checkout");
        write(
            &root.join(".git"),
            "gitdir: ../repository/.git/worktrees/checkout\n",
        );
        write(&admin.join("commondir"), "../..\n");
        (dir, root, common, admin)
    }

    #[test]
    fn regular_checkout_tracks_a_loose_branch_revision() {
        let dir = TempDir::new().unwrap();
        let git = dir.path().join(".git");
        write(&git.join("HEAD"), "ref: refs/heads/main\n");
        write(&git.join("refs/heads/main"), FIRST);
        assert_eq!(git_head(dir.path()), FIRST);
        assert!(git_revision(dir.path())
            .watched
            .contains(&git.join("refs/heads/main")));
        write(&git.join("refs/heads/main"), SECOND);
        assert_eq!(git_head(dir.path()), SECOND);
    }

    #[test]
    fn linked_worktree_resolves_and_tracks_its_shared_branch() {
        let (_dir, root, common, admin) = linked_fixture();
        write(&admin.join("HEAD"), "ref: refs/heads/integration\n");
        write(&common.join("refs/heads/integration"), FIRST);
        assert_eq!(git_head(&root), FIRST);
        assert!(git_revision(&root).watched.iter().any(|path| {
            fs::canonicalize(path).ok()
                == fs::canonicalize(common.join("refs/heads/integration")).ok()
        }));
        write(&common.join("refs/heads/integration"), SECOND);
        assert_eq!(git_head(&root), SECOND);
    }

    #[test]
    fn linked_worktree_resolves_a_packed_shared_branch() {
        let (_dir, root, common, admin) = linked_fixture();
        write(&admin.join("HEAD"), "ref: refs/heads/integration\n");
        write(
            &common.join("packed-refs"),
            &format!("# pack-refs\n{FIRST} refs/heads/integration\n"),
        );
        assert_eq!(git_head(&root), FIRST);
        assert!(git_revision(&root).watched.iter().any(|path| {
            fs::canonicalize(path).ok() == fs::canonicalize(common.join("packed-refs")).ok()
        }));
    }

    #[test]
    fn linked_worktree_resolves_a_detached_head_with_an_absolute_gitdir() {
        let (_dir, root, _common, admin) = linked_fixture();
        write(
            &root.join(".git"),
            &format!("gitdir: {}\n", admin.display()),
        );
        write(&admin.join("HEAD"), FIRST);
        assert_eq!(git_head(&root), FIRST);
    }

    #[test]
    fn malformed_head_never_becomes_a_release_revision() {
        let dir = TempDir::new().unwrap();
        write(&dir.path().join(".git/HEAD"), "not-a-commit\n");
        assert_eq!(git_head(dir.path()), "unknown");
    }

    #[test]
    fn binary_receipt_pins_both_selected_checkout_revisions() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let loom = manifest.parent().unwrap().parent().unwrap();
        let ruvector = loom.parent().unwrap().join("ruvector");
        let receipt = loom_facade::build_info::BuildInfo::current();
        assert!(receipt.source_identity_complete());
        assert_eq!(receipt.loom_revision, git_head(loom));
        assert_eq!(receipt.ruvector_revision, git_head(&ruvector));
    }
}
