//! The promotion binary builds an artefact the serving reader qualifies.
//!
//! Promotion runs as a child process, exactly as in production, because
//! `ruvector-core` holds every database it opens for the life of the process.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use loom_domain::ports::VectorIndex;
use loom_vector_ruvector::artifact::{ARTIFACT_FILE, RECORDS_FILE, SIDECAR_FILE};
use loom_vector_ruvector::{HnswIndex, EMBEDDING_DIMENSIONS};
use serde_json::json;
use sha2::{Digest, Sha256};

const GEN: &str = "visionGraph@abc1234";

fn unit(seed: usize) -> Vec<f32> {
    let mut v = vec![0.0_f32; EMBEDDING_DIMENSIONS];
    v[seed % EMBEDDING_DIMENSIONS] = 1.0;
    v[(seed * 7 + 3) % EMBEDDING_DIMENSIONS] = 0.5;
    v
}

fn bundle(dir: &Path, n: usize) {
    let mut text = String::new();
    for i in 0..n {
        let rec = json!({
            "key": format!("urn:ngm:class:c{i}"),
            "metadata": { "generation": GEN },
            "embedding": unit(i),
        });
        text.push_str(&rec.to_string());
        text.push('\n');
    }
    std::fs::write(dir.join(RECORDS_FILE), &text).unwrap();
    let digest = Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::new(), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        });
    let marker = json!({
        "id": GEN, "commit": "abc1234", "content_digest": "c".repeat(64),
        "class_count": n, "page_count": n, "vocabulary_version": 1,
        "artifacts": [ { "name": RECORDS_FILE, "sha256": digest, "bytes": text.len() } ],
    });
    std::fs::write(dir.join(".generation.json"), marker.to_string()).unwrap();
}

#[tokio::test]
async fn the_reader_qualifies_what_the_promotion_binary_builds() {
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), 12);

    let out = Command::new(env!("CARGO_BIN_EXE_promote_vault_build"))
        .args(["--data", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let index = HnswIndex::open(dir.path().join(ARTIFACT_FILE));
    assert!(index.is_ready(), "{:?}", index.qualification().reasons());
    assert_eq!(
        index.generation().id.0,
        GEN,
        "the served semantic generation is the bundle's"
    );
    let hits = index.nearest(&unit(3), 1).await.unwrap();
    assert_eq!(hits[0].iri.as_str(), "urn:ngm:class:c3");
    assert!(dir.path().join(SIDECAR_FILE).is_file());
}

#[test]
fn a_refusal_exits_2_and_names_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    bundle(dir.path(), 2);
    std::fs::write(dir.path().join(RECORDS_FILE), "{}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_promote_vault_build"))
        .args(["--data", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("does not match the digest"));
}
