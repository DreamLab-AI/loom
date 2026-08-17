//! EXP-attest: the chained attestation ledger — growth, tamper detection,
//! determinism, and `GateVerdict` serialisation.

use std::fs;
use std::sync::Arc;

use loom_attest_proofgate::{ChainedLedger, FixedClock, LedgerEntry};
use loom_domain::{AttestationLedger, GateVerdict, Iri};

fn verdict(predicate: &str, passed: bool) -> GateVerdict {
    GateVerdict {
        predicate: predicate.to_owned(),
        passed,
        detail: Some(format!("{predicate} detail")),
        subject: Some(Iri::from_slug(predicate)),
    }
}

fn ledger_at(path: std::path::PathBuf, ts: i64) -> ChainedLedger {
    ChainedLedger::with_path_and_clock(path, Arc::new(FixedClock(ts)))
}

fn read_lines(path: &std::path::Path) -> Vec<LedgerEntry> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn chain_grows_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let ledger = ledger_at(path.clone(), 1_700_000_000);

    let id0 = ledger.attest(&verdict("p0", true)).await.unwrap();
    let id1 = ledger.attest(&verdict("p1", false)).await.unwrap();
    let id2 = ledger.attest(&verdict("p2", true)).await.unwrap();

    let entries = read_lines(&path);
    assert_eq!(entries.len(), 3, "three appends → three lines");
    assert_eq!(entries[0].seq, 0);
    assert_eq!(entries[1].seq, 1);
    assert_eq!(entries[2].seq, 2);

    // Genesis root, then each prev links to the prior entry_sha256.
    assert_eq!(entries[0].prev_sha256, "");
    assert_eq!(entries[1].prev_sha256, entries[0].entry_sha256);
    assert_eq!(entries[2].prev_sha256, entries[1].entry_sha256);

    // The returned ids ARE the entry hashes, and they are distinct.
    assert_eq!(id0.0, entries[0].entry_sha256);
    assert_eq!(id1.0, entries[1].entry_sha256);
    assert_eq!(id2.0, entries[2].entry_sha256);
    assert_ne!(id0.0, id1.0);
    assert_ne!(id1.0, id2.0);

    assert!(ledger.verify_chain().await.unwrap(), "fresh chain is intact");
}

#[tokio::test]
async fn verify_chain_true_on_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = ledger_at(dir.path().join("absent.jsonl"), 1);
    assert!(
        ledger.verify_chain().await.unwrap(),
        "an empty (never-written) chain is intact"
    );
}

#[tokio::test]
async fn verify_chain_false_after_byte_flip_mid_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let ledger = ledger_at(path.clone(), 1_700_000_000);

    ledger.attest(&verdict("p0", true)).await.unwrap();
    ledger.attest(&verdict("p1", false)).await.unwrap();
    ledger.attest(&verdict("p2", true)).await.unwrap();
    assert!(ledger.verify_chain().await.unwrap());

    // Tamper with the MIDDLE entry's hashed `predicate` field, keeping the JSON
    // structurally valid so it still parses — only the re-hash should diverge.
    let raw = fs::read_to_string(&path).unwrap();
    let tampered = raw.replacen("\"predicate\":\"p1\"", "\"predicate\":\"pX\"", 1);
    assert_ne!(raw, tampered, "the flip must actually change a byte");
    fs::write(&path, tampered).unwrap();

    assert!(
        !ledger.verify_chain().await.unwrap(),
        "a flipped byte mid-chain must fail verification"
    );
}

#[tokio::test]
async fn attest_is_deterministic_given_fixed_inputs() {
    let dir = tempfile::tempdir().unwrap();
    // Two independent genesis ledgers, same fixed clock, same first verdict →
    // identical entry hash. The hash is a pure function of (prev, ts, fields).
    let a = ledger_at(dir.path().join("a.jsonl"), 42);
    let b = ledger_at(dir.path().join("b.jsonl"), 42);

    let id_a = a.attest(&verdict("class_count_parity", true)).await.unwrap();
    let id_b = b.attest(&verdict("class_count_parity", true)).await.unwrap();
    assert_eq!(id_a, id_b, "deterministic given fixed inputs");

    // A different clock ⇒ a different hash (ts is part of the canonical form).
    let c = ledger_at(dir.path().join("c.jsonl"), 43);
    let id_c = c.attest(&verdict("class_count_parity", true)).await.unwrap();
    assert_ne!(id_a, id_c, "ts participates in the hash");
}

#[test]
fn gate_verdict_serialises_with_expected_shape() {
    let v = GateVerdict {
        predicate: "no_mixed_generation".to_owned(),
        passed: false,
        detail: Some("two generations present".to_owned()),
        subject: Some(Iri::from_slug("photosynthesis")),
    };
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["predicate"], "no_mixed_generation");
    assert_eq!(json["passed"], false);
    assert_eq!(json["detail"], "two generations present");
    assert_eq!(json["subject"], "urn:ngm:class:photosynthesis");
}
