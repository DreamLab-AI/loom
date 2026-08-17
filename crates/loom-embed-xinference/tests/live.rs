//! Live smoke test against a real Xinference (`XINFERENCE_URL`, default
//! `http://xinference:9997/v1`). `#[ignore]`d so it never runs in a network-free
//! CI leg; run explicitly with:
//!
//! ```text
//! cargo test -p loom-embed-xinference -- --ignored
//! ```
//!
//! Verifies the ops-law lock end-to-end: the endpoint is live, the model behind
//! it returns a 384-wide vector, and (a bge property) that vector is unit-norm.

use loom_embed_xinference::{EmbeddingProvider, XinferenceEmbedder, DIMENSIONS};

#[tokio::test]
#[ignore = "requires a live XINFERENCE_URL"]
async fn live_embed_is_384_unit_norm() {
    let embedder = XinferenceEmbedder::from_env();

    // Model/dim locks are constants, but assert them so a config drift shows here.
    assert_eq!(embedder.model_id(), "bge-small-en-v1.5");
    assert_eq!(embedder.dimensions(), DIMENSIONS);

    let v = embedder
        .embed("rgb protocol")
        .await
        .expect("live embed of 'rgb protocol' should succeed");

    assert_eq!(v.len(), DIMENSIONS, "expected 384-dim, got {}", v.len());

    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 1e-3,
        "bge embeddings are unit-norm; got L2 norm {norm}"
    );
}
