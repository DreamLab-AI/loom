//! EXP-009 — generation-parity guard (never-mixed-build). Fusion skips the
//! semantic fallback (lexical-only degrade) when the semantic index generation
//! != the lexical generation; the observable proxy is `fusion_path` +
//! zero vector calls (the ANN is never reached). Also: `GenerationStore` prefers
//! build-manifest → mirror-manifest (`.generation.json`) → scaffold-index.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{call, generation_with_id, semantic_hit, StubVector, TestEnvBuilder};
use loom_domain::{GenerationSource, GenerationStore};
use loom_facade::mirror::MirrorStore;
use serde_json::json;
use tempfile::TempDir;

const FIXTURE_GEN: &str = "2026-08-09T00:00:00Z";
const MISS_PROMPT: &str = "best sourdough starter recipe";

#[tokio::test]
async fn generation_mismatch_skips_fallback() {
    // Semantic index is a DIFFERENT generation than the lexical index.
    let vector = Arc::new(StubVector::new(
        true,
        generation_with_id("SOME-OTHER-BUILD"),
        vec![semantic_hit("knowledge-graph", 0.95)],
    ));
    let env = TestEnvBuilder::new()
        .with_vector(Arc::clone(&vector))
        .with_semantic_fallback(true, Some(0.7))
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS_PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Guard trips: lexical-only degrade, and the ANN is NEVER reached.
    assert_eq!(body["fusion_path"], json!("NoMatch"));
    assert_eq!(body["engaged"], json!(false));
    assert_eq!(
        env.vector_call_count(),
        0,
        "generation mismatch must skip the fallback before any ANN call"
    );
}

#[tokio::test]
async fn generation_match_admits_fallback() {
    // Same generation ⇒ the guard admits the fallback and the ANN runs.
    let vector = Arc::new(StubVector::new(
        true,
        generation_with_id(FIXTURE_GEN),
        vec![semantic_hit("knowledge-graph", 0.95)],
    ));
    let env = TestEnvBuilder::new()
        .with_vector(Arc::clone(&vector))
        .with_semantic_fallback(true, Some(0.7))
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS_PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["fusion_path"], json!("SemanticFallback"));
    assert_eq!(env.vector_call_count(), 1);
}

// --- GenerationStore source precedence --------------------------------------

fn write(dir: &TempDir, name: &str, content: &str) {
    std::fs::write(dir.path().join(name), content).unwrap();
}

#[tokio::test]
async fn generation_store_prefers_build_manifest() {
    let dir = TempDir::new().unwrap();
    write(&dir, "scaffold-index.json", r#"{"generated":"S","counts":{"classes":7}}"#);
    write(
        &dir,
        ".generation.json",
        r#"{"generation":"M","artifacts":{}}"#,
    );
    write(
        &dir,
        "build-manifest.json",
        r#"{"commitSha":"abc123","buildId":"b1","generatedAt":"2026-08-17T00:00:00Z"}"#,
    );
    let store = MirrorStore::new(&dir.path().join("scaffold-index.json").to_string_lossy());
    let g = store.current();
    assert_eq!(g.source, GenerationSource::BuildManifest);
    assert_eq!(g.commit_sha.as_deref(), Some("abc123"));
    assert_eq!(g.id.0, "abc123"); // prefers commitSha for identity
}

#[tokio::test]
async fn generation_store_falls_to_mirror_manifest() {
    let dir = TempDir::new().unwrap();
    write(&dir, "scaffold-index.json", r#"{"generated":"S","counts":{"classes":7}}"#);
    write(
        &dir,
        ".generation.json",
        r#"{"generation":"2026-08-17T09:00:00Z","promoted_at":"2026-08-17T09:05:00Z","artifacts":{}}"#,
    );
    let store = MirrorStore::new(&dir.path().join("scaffold-index.json").to_string_lossy());
    let g = store.current();
    assert_eq!(g.source, GenerationSource::MirrorManifest);
    assert_eq!(g.id.0, "2026-08-17T09:00:00Z");
    assert_eq!(g.promoted_at.as_deref(), Some("2026-08-17T09:05:00Z"));
    assert!(g.verified_single_generation);
}

#[tokio::test]
async fn generation_store_falls_to_scaffold_index() {
    let dir = TempDir::new().unwrap();
    write(&dir, "scaffold-index.json", r#"{"generated":"2026-08-09T00:00:00Z","counts":{"classes":7}}"#);
    let store = MirrorStore::new(&dir.path().join("scaffold-index.json").to_string_lossy());
    let g = store.current();
    assert_eq!(g.source, GenerationSource::ScaffoldIndex);
    assert_eq!(g.id.0, "2026-08-09T00:00:00Z");
    assert_eq!(g.class_count, Some(7));
    assert!(!g.verified_single_generation);
}

#[tokio::test]
async fn verify_atomicity_flags_a_tampered_artifact() {
    let dir = TempDir::new().unwrap();
    // A manifest recording a sha that does NOT match the on-disk artifact.
    write(&dir, "scaffold-index.json", r#"{"generated":"S","counts":{"classes":7}}"#);
    write(&dir, "ontology-corpus.rvdb", "pretend-vector-bytes");
    write(
        &dir,
        ".generation.json",
        r#"{"generation":"G","artifacts":{"ontology-corpus.rvdb":{"sha256":"deadbeef","bytes":20}}}"#,
    );
    let store = MirrorStore::new(&dir.path().join("scaffold-index.json").to_string_lossy());
    let err = store.verify_atomicity().await.unwrap_err();
    assert!(
        matches!(err, loom_domain::LoomError::GenerationDrift(_)),
        "expected GenerationDrift, got {err:?}"
    );
}

#[tokio::test]
async fn verify_atomicity_ok_without_manifest() {
    let dir = TempDir::new().unwrap();
    write(&dir, "scaffold-index.json", r#"{"generated":"S","counts":{"classes":7}}"#);
    let store = MirrorStore::new(&dir.path().join("scaffold-index.json").to_string_lossy());
    // No .generation.json ⇒ nothing promoted to verify ⇒ Ok (fail-open).
    assert!(store.verify_atomicity().await.is_ok());
}
