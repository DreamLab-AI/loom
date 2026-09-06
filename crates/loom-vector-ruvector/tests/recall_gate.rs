//! EXP-008 — the recall-gate integration test (RUST-ARCHITECTURE §8.4).
//!
//! The ground-truth wiring gate: proves the exported `ontology-corpus.rvdb`
//! surfaces the correct IRI for an in-domain query and cleanly separates it from
//! an off-ontology decoy. Governs whether `LOOM_SEMANTIC_FALLBACK` may ever
//! default to `1` (ADR-136 D3: benchmark-gated, default-off).
//!
//! Gated on `semantic-fallback`; `#[ignore]`d because it needs a live Xinference
//! and the exported artifact. Run for evidence with:
//!   `LOOM_HNSW_ARTIFACT`=<repo>/data/ontology-corpus.rvdb
//!   cargo test -p loom-vector-ruvector --features semantic-fallback
//!     --test `recall_gate` -- --ignored --nocapture
//!
//! MEASURED DIVERGENCE (2026-08-17, gen 2026-08-17T14:48:36Z, 8146 records):
//! the design floor in §8.4 is cosine ≥ 0.87 for `rgb-protocol` on the query
//! "rgb protocol". The stored vectors are concept-DOCUMENT embeddings, so a
//! short query lands at cos ≈ 0.82 — `rgb-protocol` is reliably in the top-2 but
//! does NOT clear 0.87. This test hard-asserts the wiring + separation
//! guarantees (which hold), and hard-asserts the design floor CONDITIONALLY
//! (audit finding 5): with `LOOM_SEMANTIC_FALLBACK=1` the floor is the FLIP-ON
//! precondition and the test fails RED below it; with the flag off (default) the
//! test asserts the gate is REPORTED red — a staleness tripwire that fails the
//! day recall improves, forcing the evidence + default to be refreshed. Override
//! the floor for a future bench-set value via `LOOM_SEMANTIC_RECALL_FLOOR`.

#![cfg(feature = "semantic-fallback")]

use loom_domain::ports::VectorIndex;
use loom_vector_ruvector::{HnswIndex, EMBEDDING_DIMENSIONS};

/// The design floor from §8.4 for the in-domain query (documented, not the hard
/// assert — see the module note on the document-embedding divergence).
const DESIGN_RECALL_FLOOR: f32 = 0.87;
/// Off-ontology decoy must stay strictly below the inject gate.
const DECOY_CEILING: f32 = 0.55;
/// Hard wiring floor: the correct IRI must clear this and beat the decoy by a
/// clear semantic margin. Empirically the query clears ~0.82; 0.75 is the
/// regression tripwire (well below measured, well above the decoy band).
const WIRING_FLOOR: f32 = 0.75;

/// bge-small-en-v1.5 via Xinference — the LOCKED embedding model (§11.3).
#[allow(clippy::cast_possible_truncation)]
async fn embed(text: &str) -> Vec<f32> {
    let base =
        std::env::var("XINFERENCE_URL").unwrap_or_else(|_| "http://xinference:9997/v1".to_owned());
    let url = format!("{base}/embeddings");
    let body = serde_json::json!({ "model": "bge-small-en-v1.5", "input": [text] });

    let resp: serde_json::Value = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .expect("Xinference request failed")
        .error_for_status()
        .expect("Xinference returned an error status")
        .json()
        .await
        .expect("Xinference response was not JSON");

    let vec: Vec<f32> = resp["data"][0]["embedding"]
        .as_array()
        .expect("embedding array missing in Xinference response")
        .iter()
        .map(|v| v.as_f64().expect("embedding element not a number") as f32)
        .collect();
    assert_eq!(
        vec.len(),
        EMBEDDING_DIMENSIONS,
        "embedder must return 384-dim"
    );
    vec
}

fn artifact_path() -> String {
    std::env::var("LOOM_HNSW_ARTIFACT").unwrap_or_else(|_| {
        format!(
            "{}/../../data/ontology-corpus.rvdb",
            env!("CARGO_MANIFEST_DIR")
        )
    })
}

#[tokio::test]
#[ignore = "requires live Xinference + exported ontology-corpus artifact (EXP-008)"]
async fn recall_gate() {
    let path = artifact_path();
    eprintln!("EXP-008 recall_gate: artifact = {path}");
    let idx = HnswIndex::open(&path);
    assert!(
        idx.is_ready(),
        "artifact not ready at {path} — run the exporter first"
    );
    eprintln!("generation = {:?}\n", idx.generation());

    // --- Axis 1a: in-domain recall ------------------------------------------
    let q = embed("rgb protocol").await;
    let hits = idx.nearest(&q, 5).await.expect("nearest failed");
    eprintln!("query 'rgb protocol' top-5:");
    for h in &hits {
        eprintln!("  {:<38} cos={:.4}", h.iri.slug(), h.score);
    }
    let rgb = hits
        .iter()
        .find(|h| h.iri.slug() == "rgb-protocol")
        .expect("rgb-protocol must appear in the top-5 (wiring correctness)");
    let rgb_score = rgb.score;

    // --- Axis 1b: off-ontology decoy ----------------------------------------
    let decoy = embed("best sourdough starter recipe").await;
    let decoy_hits = idx.nearest(&decoy, 5).await.expect("nearest failed");
    let decoy_top = decoy_hits.first().map_or(0.0, |h| h.score);
    eprintln!(
        "\ndecoy 'best sourdough starter recipe' top cos = {decoy_top:.4} (slug={})",
        decoy_hits.first().map_or("<none>", |h| h.iri.slug())
    );

    // --- Axis 2 (informational): paraphrase OOV recovery --------------------
    let para = embed("colour-channel protocol").await;
    let para_hits = idx.nearest(&para, 5).await.expect("nearest failed");
    let para_has_rgb = para_hits.iter().any(|h| h.iri.slug() == "rgb-protocol");
    eprintln!(
        "paraphrase 'colour-channel protocol' → rgb-protocol in top-5: {para_has_rgb} (top={} cos={:.4})",
        para_hits.first().map_or("<none>", |h| h.iri.slug()),
        para_hits.first().map_or(0.0, |h| h.score)
    );

    // --- HARD gate: wiring correctness + off-ontology separation ------------
    assert!(
        rgb_score >= WIRING_FLOOR,
        "rgb-protocol cosine {rgb_score:.4} below wiring floor {WIRING_FLOOR}"
    );
    assert!(
        decoy_top < DECOY_CEILING,
        "off-ontology decoy cosine {decoy_top:.4} must stay below {DECOY_CEILING}"
    );
    assert!(
        rgb_score > decoy_top + 0.15,
        "in-domain ({rgb_score:.4}) must clearly beat the decoy ({decoy_top:.4})"
    );

    // --- DESIGN-FLOOR gate (ADR-136 D3): hard-asserted, env-conditioned -------
    // The floor is the PRECONDITION for default-on. Override for a future
    // bench-set floor via LOOM_SEMANTIC_RECALL_FLOOR.
    let floor = std::env::var("LOOM_SEMANTIC_RECALL_FLOOR")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(DESIGN_RECALL_FLOOR);
    let design_floor_met = rgb_score >= floor;
    // Match Config's env truthiness (0/false/no/unset ⇒ off).
    let flip_on = matches!(
        std::env::var("LOOM_SEMANTIC_FALLBACK").ok().as_deref(),
        Some("1" | "true" | "yes")
    );
    eprintln!("\nEXP-008 numbers: rgb-protocol cos={rgb_score:.4}, decoy cos={decoy_top:.4}");
    eprintln!(
        "DESIGN FLOOR (>= {floor}): {} — LOOM_SEMANTIC_FALLBACK={}",
        if design_floor_met {
            "MET"
        } else {
            "NOT MET (gate RED)"
        },
        if flip_on { "1" } else { "0" }
    );

    if flip_on {
        // FLIP-ON precondition: enabling the fallback REQUIRES clearing the floor.
        assert!(
            design_floor_met,
            "FLIP-ON PRECONDITION FAILED: rgb-protocol cos={rgb_score:.4} < floor {floor} — \
             LOOM_SEMANTIC_FALLBACK must not be on until recall clears the floor"
        );
    } else {
        // Default-off: the gate MUST currently be RED. This documents red as the
        // present truth AND is a staleness tripwire — the day recall clears the
        // floor this assert fails, forcing EXP-008 evidence + the default to be
        // refreshed (audit finding 5).
        assert!(
            !design_floor_met,
            "recall now clears the floor (cos={rgb_score:.4} >= {floor}): refresh EXP-008 \
             evidence and reconsider the LOOM_SEMANTIC_FALLBACK default"
        );
    }
}
