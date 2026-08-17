//! EXP-005 — endpoint parity with the Python façade. Every route + alias
//! answers; `/health` carries the superset shape; `NoBackend`→503 on `/v1/*`;
//! `BadQuery`→400 on `/loom/sparql`; graph-absent degrades (scaffold still
//! serves, `health.graph.available=false`); unknown route → 404.

mod common;

use axum::http::StatusCode;
use common::{call, TestEnvBuilder};
use serde_json::json;

const TTL: &str = r#"@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
<urn:ngm:class:knowledge-graph> rdfs:label "Knowledge Graph" .
<urn:ngm:class:graph> rdfs:label "Graph" .
<urn:ngm:class:graph-database> rdfs:label "Graph Database" .
"#;

const PROMPT: &str = "Explain how a knowledge graph uses a graph database";

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
        let (status, body) = call(
            env.router(),
            "POST",
            uri,
            Some(json!({ "prompt": PROMPT })),
        )
        .await;
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
        Some(json!({ "prompt": "best sourdough starter recipe" })),
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
    assert!(body["rows"].as_array().is_some_and(|r| !r.is_empty()), "body: {body}");
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
        let (status, body) = call(
            env.router(),
            "POST",
            uri,
            Some(json!({ "q": "graph" })),
        )
        .await;
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
    let env = TestEnvBuilder::new().with_semantic_debug_surface(true).build();
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
    assert!(results[0]["score"].as_f64().is_some(), "bare cosine score present");
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
