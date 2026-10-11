//! LM-06 belongs to the Rust reload migration, independently of the pending
//! LM-04/LM-09 publishing contracts on the parent preparation branch.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn lm06_python_implementation_and_duplicate_tests_are_removed() {
    for path in [
        "scripts/reload-published-generation.py",
        "tests/test_reload_published_generation.py",
    ] {
        assert!(!root().join(path).exists(), "LM-06: {path} survives");
    }
    let ci = fs::read_to_string(root().join(".github/workflows/rust-contracts.yml")).unwrap();
    assert!(ci.contains("test --workspace --locked"));
    assert!(!ci.contains("test_reload_published_generation.py"));
}

#[test]
fn lm06_service_runs_the_registered_rust_binary() {
    // Cargo resolves this only for a real registered binary; the fixture suite
    // also executes it to prove its lock, exit codes and receipt behaviour.
    let binary = Path::new(env!("CARGO_BIN_EXE_reload_published_generation"));
    assert!(binary.is_file());
    let unit = fs::read_to_string(root().join("deploy/loom-generation-reload.service")).unwrap();
    let commands: Vec<_> = unit
        .lines()
        .filter_map(|line| line.strip_prefix("ExecStart="))
        .collect();
    assert_eq!(commands.len(), 1);
    let words: Vec<_> = commands[0].split_whitespace().collect();
    assert_eq!(
        words.first().copied(),
        Some("%h/githubs/loom/target/release/reload_published_generation")
    );
    for flag in ["--data-dir", "--receipt", "--url", "--"] {
        assert!(words.contains(&flag), "LM-06: missing {flag}");
    }
    assert!(!words.iter().any(|word| {
        word.to_ascii_lowercase().contains("python")
            || Path::new(word)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
    }));
}
