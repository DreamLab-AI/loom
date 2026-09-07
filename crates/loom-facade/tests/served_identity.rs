//! Retrieval identity is carried by every successful read response.
mod common;
use axum::{body::Body, http::Request};
use common::{call, TestEnvBuilder};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn graph_reads_report_the_same_loaded_identity_as_generation() {
    let env = TestEnvBuilder::new()
        .with_commit_marker(true)
        .with_graph_ttl("<urn:test:class> <http://www.w3.org/2000/01/rdf-schema#label> \"Class\" .")
        .build();
    let (_, generation) = call(env.router(), "GET", "/loom/generation", None).await;
    assert!(generation["embedding"].is_object());
    assert!(generation["semantic_generation"].is_object());
    for (path, query) in [
        ("/loom/search", json!({"q": "Class"})),
        (
            "/loom/sparql",
            json!({"query": "SELECT ?s WHERE { ?s ?p ?o } LIMIT 1"}),
        ),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&query).unwrap()))
            .unwrap();
        let response = env.router().oneshot(request).await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            response.headers()["x-loom-generation"],
            generation["id"].as_str().unwrap()
        );
        assert_eq!(
            response.headers()["x-loom-content-digest"],
            generation["identity"]["content_digest"].as_str().unwrap()
        );
        assert_eq!(response.headers()["x-loom-atomicity-verified"], "true");
    }
}

#[tokio::test]
async fn disk_change_does_not_relabel_the_loaded_graph() {
    let env = TestEnvBuilder::new()
        .with_commit_marker(true)
        .with_graph_ttl("<urn:test:class> <http://www.w3.org/2000/01/rdf-schema#label> \"Class\" .")
        .build();
    let (_, before) = call(env.router(), "GET", "/loom/generation", None).await;
    std::fs::write(&env.state.config.index_path, b"changed after activation").unwrap();
    let (_, after) = call(env.router(), "GET", "/loom/generation", None).await;
    assert_eq!(before["identity"], after["identity"]);
    assert_eq!(after["drift"]["ok"], false);
    let request = Request::builder()
        .method("POST")
        .uri("/loom/search")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"q":"Class"}"#))
        .unwrap();
    let response = env.router().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers()["x-loom-content-digest"],
        before["identity"]["content_digest"].as_str().unwrap()
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(String::from_utf8(bytes.to_vec())
        .unwrap()
        .contains("urn:test:class"));
}
