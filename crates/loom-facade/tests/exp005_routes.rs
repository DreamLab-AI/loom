//! EXP-005 — endpoint parity with the Python façade. Every route + alias
//! answers; `/health` carries the superset shape (including the confidence
//! contract's `injection_policy`/`serving`/`confidence` blocks) and
//! `/loom/scaffold` always carries `grounding`; `NoBackend`→503 on `/v1/*`;
//! `BadQuery`→400 on `/loom/sparql`; graph-absent degrades (scaffold still
//! serves, `health.graph.available=false`); unknown route → 404.

mod common;

use axum::http::StatusCode;
use common::{call, TestEnvBuilder};
use loom_domain::ScoreScale;
use loom_facade::routes::health::HealthResponse;
use serde_json::json;

const TTL: &str = r#"@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<urn:ngm:class:knowledge-graph> rdfs:label "Knowledge Graph" .
<urn:ngm:class:graph> rdfs:label "Graph" .
<urn:ngm:class:graph-database> rdfs:label "Graph Database" .
"#;

const PROMPT: &str = "Explain how a knowledge graph uses a graph database";

/// A prompt matching no fixture class title — the honest no-match probe.
const MISS_PROMPT: &str = "best sourdough starter recipe";

/// How many probes the confidence-window counter test fires.
const PROBES: usize = 5;

/// The nine keys every `grounding` object carries, engaged or not. Every one is
/// PRESENT even when its value is null — an absent key and a null key are the
/// same thing to `Value` indexing but not to a typed consumer, so presence is
/// asserted with `get()`, never by comparing an index to `json!(null)`.
const GROUNDING_KEYS: [&str; 9] = [
    "signal",
    "top_score",
    "score_scale",
    "confidence",
    "decision",
    "threshold",
    "effective_budget",
    "engaged",
    "seeds",
];

#[tokio::test]
async fn health_superset_shape() {
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);

    // Superset-shape keys (RUST-ARCHITECTURE §9).
    for key in [
        "ok",
        "facet",
        "mode",
        "backend",
        "backend_reachable",
        "index_classes",
        "graph",
        "semantic",
        "generation",
        "deploy_profile",
        "injection_policy",
        "serving",
        "confidence",
    ] {
        assert!(body.get(key).is_some(), "health missing key: {key}");
    }
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["facet"], json!("loom-facade"));
    assert_eq!(body["mode"], json!("scaffold"));
    assert_eq!(body["index_classes"], json!(7)); // the 7-class fixture
    assert_eq!(body["backend"], json!(null)); // retrieval-only
    assert_eq!(body["backend_reachable"], json!(null));
    // Graph absent (no ttl) ⇒ available:false but present in the shape.
    assert_eq!(body["graph"]["available"], json!(false));
    // Semantic not-ready stub.
    assert_eq!(body["semantic"]["ready"], json!(false));
    assert!(body["semantic"].get("generation").is_some());

    // --- the confidence contract's three new blocks -------------------------
    // The gate's configuration, verbatim from InjectionPolicy::default().
    let policy = &body["injection_policy"];
    assert_eq!(policy["confidence_injection"], json!(false));
    assert_eq!(policy["score_scale"], json!("lexical-additive"));
    for key in [
        "strong_match_score",
        "min_inject_score",
        "min_inject_fraction",
    ] {
        assert!(
            policy[key].as_f64().is_some(),
            "injection_policy.{key} not a number: {policy}"
        );
    }

    // The serving regime. Defaults: verbatim off, semantic fallback off, and an
    // UNSET semantic floor is null (not 0.0 — with it unset nothing may inject).
    let serving = &body["serving"];
    assert_eq!(serving["verbatim_mode"], json!(false));
    assert_eq!(serving["verbatim_threshold"], json!(8.0));
    assert_eq!(serving["semantic_fallback"], json!(false));
    assert_eq!(serving["semantic_min_inject"], json!(null));

    // The rolling window, empty on a fresh node but fully shaped.
    let confidence = &body["confidence"];
    assert_eq!(confidence["window"], json!(1000));
    for key in [
        "requests", "engaged", "skipped", "scaled", "full", "verbatim",
    ] {
        assert_eq!(confidence[key], json!(0), "fresh node confidence.{key}");
    }
    assert_eq!(confidence["mean_confidence"], json!(0.0));
}

#[tokio::test]
async fn health_typed_response_deserialises() {
    // The evaluator consumes /health as a TYPED struct; prove the wire shape
    // round-trips through it, so a field rename breaks the build not a checker.
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    let typed: HealthResponse = serde_json::from_value(body).expect("health deserialises typed");
    assert!(typed.ok);
    assert_eq!(typed.facet, "loom-facade");
    assert_eq!(typed.index_classes, 7);
    assert_eq!(typed.backend, None);
    assert_eq!(
        typed.injection_policy.score_scale,
        ScoreScale::LexicalAdditive
    );
    assert_eq!(typed.confidence.window, 1000);
}

#[tokio::test]
async fn health_confidence_counters_advance_with_probes() {
    // N grounded requests advance `requests` by exactly N (the window is shared
    // through the Arc'd AppState, one sample per request), the decision counters
    // partition that total, and the mean stays a confidence — inside [0, 1].
    let env = TestEnvBuilder::new().build();
    for i in 0..PROBES {
        let prompt = if i % 2 == 0 { PROMPT } else { MISS_PROMPT };
        let (status, _) = call(
            env.router(),
            "POST",
            "/loom/scaffold",
            Some(json!({ "prompt": prompt })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (_, body) = call(env.router(), "GET", "/health", None).await;
    let c = &body["confidence"];
    assert_eq!(c["requests"], json!(PROBES), "one sample per request: {c}");
    // The four decision counters partition `requests` exactly.
    let sum: u64 = ["skipped", "scaled", "full", "verbatim"]
        .iter()
        .map(|k| c[*k].as_u64().expect("counter is a number"))
        .sum();
    assert_eq!(sum, PROBES as u64, "decisions partition requests: {c}");
    // engaged == requests - skipped, and the no-match probes DID skip.
    let skipped = c["skipped"].as_u64().unwrap();
    assert_eq!(c["engaged"].as_u64().unwrap(), PROBES as u64 - skipped);
    assert!(skipped > 0, "the sourdough probes must skip: {c}");
    // mean_confidence is a confidence, whatever the mix of decisions.
    let mean = c["mean_confidence"].as_f64().expect("mean is a number");
    assert!(
        (0.0..=1.0).contains(&mean),
        "mean_confidence out of [0,1]: {mean}"
    );
}

#[tokio::test]
async fn scaffold_grounding_contract_shape() {
    // The `grounding` block is the contract: lowercase enum names on the wire
    // (they sit beside `loom.mode`/`served_mode`, not the CamelCase fusion_path
    // audit alias), and every seed carries the full six-field provenance row.
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let g = &body["grounding"];
    assert!(g.is_object(), "grounding must be an object: {g}");
    for key in GROUNDING_KEYS {
        assert!(g.get(key).is_some(), "grounding missing key {key}: {g}");
    }
    // Lowercase enum names (the kebab score scale included).
    assert_eq!(g["signal"], json!("lexical"));
    assert_eq!(g["score_scale"], json!("lexical-additive"));
    assert_eq!(g["decision"], json!("full")); // gate off by default ⇒ full budget
    assert_eq!(g["engaged"], json!(true));
    let confidence = g["confidence"].as_f64().expect("confidence is a number");
    assert!(
        (0.0..=1.0).contains(&confidence),
        "confidence: {confidence}"
    );

    // Per-seed rows: the six contract fields, with at least one injected seed.
    let seeds = g["seeds"].as_array().expect("seeds array");
    assert!(!seeds.is_empty(), "an engaged scaffold has seeds: {g}");
    for seed in seeds {
        for key in [
            "iri",
            "score",
            "confidence",
            "quality",
            "provenance",
            "injected",
        ] {
            assert!(seed.get(key).is_some(), "seed missing {key}: {seed}");
        }
        assert!(seed["iri"]
            .as_str()
            .is_some_and(|s| s.starts_with("urn:ngm:class:")));
        assert!(seed["injected"].is_boolean());
    }
    assert!(
        seeds.iter().any(|s| s["injected"] == json!(true)),
        "an engaged block injected at least one seed: {g}"
    );

    // The pre-contract aliases survive one release alongside it. `top_score`
    // is compared numerically: the flat alias is the f32 the Scaffold carries,
    // the contract field the f64 the gate scored on.
    let alias_top = body["top_score"].as_f64().expect("alias top_score");
    let contract_top = g["top_score"].as_f64().expect("grounding top_score");
    assert!(
        (alias_top - contract_top).abs() < 1e-4,
        "alias {alias_top} vs contract {contract_top}"
    );
    assert_eq!(body["effective_budget"], g["effective_budget"]);
    assert_eq!(body["engaged"], g["engaged"]);
    assert_eq!(body["fusion_path"], json!("LexicalHit"));
    assert!(body["seeds"].as_array().is_some_and(|s| !s.is_empty()));
}

#[tokio::test]
async fn scaffold_no_match_grounding_is_present_not_null() {
    // The honest zero: nothing matched, and the block SAYS so rather than
    // vanishing. A consumer can tell "found nothing" from "never asked".
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS_PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let g = &body["grounding"];
    assert!(g.is_object(), "grounding present on a no-match: {body}");
    // Every key is PRESENT even on a total miss — `get()`, not index-vs-null,
    // because `Value` indexing cannot tell an absent key from a null one and a
    // typed consumer (the confidence-check bin) very much can.
    for key in GROUNDING_KEYS {
        assert!(
            g.get(key).is_some(),
            "no-match grounding missing key {key}: {g}"
        );
    }
    assert_eq!(g["signal"], json!("none"));
    assert_eq!(g["decision"], json!("skipped"));
    assert_eq!(g["engaged"], json!(false));
    assert_eq!(g["confidence"], json!(0.0));
    assert_eq!(g["seeds"], json!([]));
    // `top_score`/`effective_budget` are OPTIONS in the contract: a miss has no
    // score and no budget, and says so with an explicit null rather than a 0 a
    // reader could mistake for a real measurement.
    assert!(g["top_score"].is_null(), "top_score null on a miss: {g}");
    assert!(
        g["effective_budget"].is_null(),
        "effective_budget null on a miss: {g}"
    );
}

#[tokio::test]
async fn generation_and_alias_answer() {
    let env = TestEnvBuilder::new().build();
    for uri in ["/loom/generation", "/generation"] {
        let (status, body) = call(env.router(), "GET", uri, None).await;
        assert_eq!(status, StatusCode::OK, "route {uri}");
        // Pre-manifest fallback: the scaffold-index stamp.
        assert_eq!(body["source"], json!("ScaffoldIndex"), "route {uri}");
        assert_eq!(body["class_count"], json!(7));
    }
}

#[tokio::test]
async fn scaffold_and_alias_engage() {
    let env = TestEnvBuilder::new().build();
    for uri in ["/loom/scaffold", "/scaffold"] {
        let (status, body) =
            call(env.router(), "POST", uri, Some(json!({ "prompt": PROMPT }))).await;
        assert_eq!(status, StatusCode::OK, "route {uri}");
        assert_eq!(body["engaged"], json!(true), "route {uri}");
        let block = body["scaffold"].as_str().unwrap();
        assert!(block.contains("Knowledge Graph"), "block: {block}");
        assert!(block.starts_with("[ONTOLOGY CONTEXT]"));
        // Audit surface over Python: seeds + fusion_path.
        assert_eq!(body["fusion_path"], json!("LexicalHit"), "route {uri}");
        assert!(body["seeds"].as_array().is_some_and(|s| !s.is_empty()));
    }
}

#[tokio::test]
async fn scaffold_irrelevant_is_empty() {
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS_PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["engaged"], json!(false));
    assert_eq!(body["scaffold"], json!(""));
    assert_eq!(body["fusion_path"], json!("NoMatch"));
}

#[tokio::test]
async fn scaffold_missing_prompt_is_400() {
    let env = TestEnvBuilder::new().build();
    let (status, _) = call(env.router(), "POST", "/scaffold", Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn nobackend_503_on_v1() {
    let env = TestEnvBuilder::new().build(); // retrieval-only (empty DISTILL_BACKEND_URL)
    let (chat, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": "hi" }] })),
    )
    .await;
    assert_eq!(chat, StatusCode::SERVICE_UNAVAILABLE);

    let (models, _) = call(env.router(), "GET", "/v1/models", None).await;
    assert_eq!(models, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn sparql_badquery_is_400() {
    let env = TestEnvBuilder::new().with_graph_ttl(TTL).build();
    for uri in ["/loom/sparql", "/sparql"] {
        let (status, _) = call(
            env.router(),
            "POST",
            uri,
            Some(json!({ "query": "INSERT DATA { <urn:x> <urn:y> <urn:z> }" })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "route {uri}");
    }
}

#[tokio::test]
async fn sparql_select_works_with_graph() {
    let env = TestEnvBuilder::new().with_graph_ttl(TTL).build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/sparql",
        Some(json!({ "query": "SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["rows"].as_array().is_some_and(|r| !r.is_empty()),
        "body: {body}"
    );
}

#[tokio::test]
async fn sparql_missing_query_is_400() {
    let env = TestEnvBuilder::new().with_graph_ttl(TTL).build();
    let (status, _) = call(env.router(), "POST", "/loom/sparql", Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_and_alias_answer() {
    let env = TestEnvBuilder::new().with_graph_ttl(TTL).build();
    for uri in ["/loom/search", "/search"] {
        let (status, body) = call(env.router(), "POST", uri, Some(json!({ "q": "graph" }))).await;
        assert_eq!(status, StatusCode::OK, "route {uri}");
        assert!(body.as_array().is_some(), "search returns a list: {body}");
    }
}

#[tokio::test]
async fn search_missing_q_is_400() {
    let env = TestEnvBuilder::new().with_graph_ttl(TTL).build();
    let (status, _) = call(env.router(), "POST", "/search", Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn semantic_search_disabled_by_default_is_404() {
    // Audit finding 1: the labelled index-debug surface is default-OFF, so a
    // bare IRI+score shape can never be reached unless explicitly enabled.
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/search/semantic",
        Some(json!({ "q": "knowledge graph" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("semantic debug surface disabled"));
}

#[tokio::test]
async fn semantic_search_enabled_reports_not_ready() {
    // Enabled + vector stub not-ready ⇒ the debug surface says so honestly, 200.
    let env = TestEnvBuilder::new()
        .with_semantic_debug_surface(true)
        .build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/search/semantic",
        Some(json!({ "q": "knowledge graph" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ready"], json!(false));
    assert_eq!(body["results"], json!([]));
}

#[tokio::test]
async fn semantic_search_enabled_and_ready_returns_labelled_hits() {
    use common::{generation_with_id, semantic_hit, StubVector};
    use std::sync::Arc;
    // A ready index with canned hits + a working embedder ⇒ bare IRI+score list
    // (labelled as the index, never markdown — I-P1 safe).
    let vector = Arc::new(StubVector::new(
        true,
        generation_with_id("gen-a"),
        vec![semantic_hit("knowledge-graph", 0.91)],
    ));
    let env = TestEnvBuilder::new()
        .with_semantic_debug_surface(true)
        .with_vector(vector)
        .build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/search/semantic",
        Some(json!({ "q": "knowledge graph", "k": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ready"], json!(true));
    let results = body["results"].as_array().expect("results array");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["iri"], json!("urn:ngm:class:knowledge-graph"));
    assert!(
        results[0]["score"].as_f64().is_some(),
        "bare cosine score present"
    );
    // The debug surface never emits an assembled markdown block.
    assert!(body.get("scaffold").is_none());
}

#[tokio::test]
async fn graph_absent_degrades_but_scaffold_serves() {
    let env = TestEnvBuilder::new().build(); // no ttl
                                             // Health reports the degrade honestly.
    let (_, health) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(health["graph"]["available"], json!(false));
    // Scaffold still serves (lexical floor is independent of the graph).
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": PROMPT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["engaged"], json!(true));
}

#[tokio::test]
async fn unknown_route_is_404() {
    let env = TestEnvBuilder::new().build();
    let (status, _) = call(env.router(), "GET", "/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
