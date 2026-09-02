//! EXP-007 — I-P1: no engine shape escapes as an answer. Fusion proofs via a
//! call-counting stub `VectorIndex`:
//! - fallback OFF ⇒ NO vector call even on a lexical miss;
//! - ON + lexical hit ⇒ NO vector call (the hot path stays network-free);
//! - ON + miss ⇒ semantic candidates flow ONLY through `assemble` — the served
//!   block is markdown resolved from the fixture unit, NEVER raw cosine scores;
//! - disabled-embedder error on fallback ⇒ `NoMatch` degrade, not 5xx.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::{call, generation_with_id, semantic_hit, StubVector, TestEnvBuilder};
use serde_json::json;

/// The fixture's `generated` stamp — the lexical generation id the semantic
/// index must match for the parity guard to admit a fallback (§6/EXP-009).
const FIXTURE_GEN: &str = "2026-08-09T00:00:00Z";

const HIT_PROMPT: &str = "Explain how a knowledge graph uses a graph database";
const MISS_PROMPT: &str = "best sourdough starter recipe";

#[tokio::test]
async fn fallback_off_never_calls_vector_on_miss() {
    // Vector is READY with hits, but the master switch is OFF.
    let vector = Arc::new(StubVector::new(
        true,
        generation_with_id(FIXTURE_GEN),
        vec![semantic_hit("knowledge-graph", 0.95)],
    ));
    let env = TestEnvBuilder::new()
        .with_vector(Arc::clone(&vector))
        .with_semantic_fallback(false, Some(0.7))
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS_PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["engaged"], json!(false));
    assert_eq!(body["fusion_path"], json!("NoMatch"));
    assert_eq!(
        env.vector_call_count(),
        0,
        "fallback OFF must not call the vector index"
    );
}

#[tokio::test]
async fn fallback_on_lexical_hit_never_calls_vector() {
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
        Some(json!({ "prompt": HIT_PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["fusion_path"], json!("LexicalHit"));
    assert_eq!(env.vector_call_count(), 0, "a lexical hit is network-free");
}

#[tokio::test]
async fn fallback_on_miss_flows_through_assemble_as_markdown() {
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
    assert_eq!(
        env.vector_call_count(),
        1,
        "the miss triggers exactly one ANN call"
    );
    assert_eq!(body["fusion_path"], json!("SemanticFallback"));
    assert_eq!(body["engaged"], json!(true));

    let block = body["scaffold"].as_str().unwrap();
    // Served as the per-IRI markdown resolved from the fixture unit (I-P1)…
    assert!(block.contains("## Knowledge Graph"), "block: {block}");
    assert!(block.starts_with("[ONTOLOGY CONTEXT]"));
    // …and NEVER the raw cosine score the index surfaced.
    assert!(
        !block.contains("0.95"),
        "raw score leaked into the served block: {block}"
    );
}

#[tokio::test]
async fn fallback_on_miss_below_threshold_does_not_inject() {
    // Semantic hit exists but scores below the bench gate ⇒ no injection.
    let vector = Arc::new(StubVector::new(
        true,
        generation_with_id(FIXTURE_GEN),
        vec![semantic_hit("knowledge-graph", 0.50)],
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
    assert_eq!(env.vector_call_count(), 1);
    assert_eq!(body["engaged"], json!(false));
    assert_eq!(body["fusion_path"], json!("NoMatch"));
}

#[tokio::test]
async fn disabled_embedder_on_fallback_degrades_not_5xx() {
    let vector = Arc::new(StubVector::new(
        true,
        generation_with_id(FIXTURE_GEN),
        vec![semantic_hit("knowledge-graph", 0.95)],
    ));
    let env = TestEnvBuilder::new()
        .with_vector(Arc::clone(&vector))
        .with_semantic_fallback(true, Some(0.7))
        .with_embed_fail(true) // the embedder is down
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS_PROMPT })),
    )
    .await;
    // Degrade to no-match, NOT a 5xx (fail-open on the accelerator).
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["engaged"], json!(false));
    assert_eq!(body["fusion_path"], json!("NoMatch"));
    // The embed failed before the ANN call, so the vector index was never hit.
    assert_eq!(env.vector_call_count(), 0);
}
