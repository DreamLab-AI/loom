//! EXP-006 — chat delegation semantics against a wiremock backend. Scaffold
//! built from the LAST user message, merged into the system message (or inserted
//! at 0), the `loom:{…}` annotation attached on the 200, and a backend failure
//! propagated as 502.
//!
//! The `[ONTOLOGY CONTEXT]` block is asserted PRESENT in the forwarded body and
//! MERGED (not duplicated) — wiremock captures the exact request the façade
//! delegated.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::{call, TestEnvBuilder};
use loom_backend_openai::OpenAiBackend;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PROMPT: &str = "Explain how a knowledge graph uses a graph database";

/// A backend pointing at a wiremock server, with the floor disabled (0) so the
/// forwarded body is asserted without floor noise where that is not the point.
fn backend_to(server: &MockServer, floor: u64) -> OpenAiBackend {
    OpenAiBackend::new(
        format!("{}/v1", server.uri()),
        Duration::from_secs(10),
        floor,
    )
}

#[tokio::test]
async fn scaffold_merged_into_system_message_and_annotated() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cmpl-1",
            "object": "chat.completion",
            "choices": [{ "message": { "role": "assistant", "content": "ok" } }]
        })))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [
                { "role": "system", "content": "You are helpful." },
                { "role": "user", "content": PROMPT }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The 200 carries the loom annotation block.
    let loom = &body["loom"];
    assert_eq!(loom["mode"], json!("scaffold"));
    assert_eq!(loom["fusion_path"], json!("LexicalHit"));
    assert!(loom["injected_tokens"].as_u64().unwrap() > 0);
    assert!(loom.get("grounding").is_some());
    assert!(loom.get("generation").is_some());

    // Inspect the exact body the façade forwarded to the model.
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    let forwarded: Value = reqs[0].body_json().unwrap();
    let msgs = forwarded["messages"].as_array().unwrap();
    // Merged into the EXISTING system message (still 2 messages, not 3).
    assert_eq!(msgs.len(), 2, "scaffold merged, not inserted: {forwarded}");
    let sys = msgs[0]["content"].as_str().unwrap();
    assert!(sys.starts_with("You are helpful."), "existing preserved: {sys}");
    // The ontology block is present EXACTLY once (merged, not duplicated).
    assert_eq!(sys.matches("[ONTOLOGY CONTEXT]").count(), 1, "sys: {sys}");
    assert!(sys.contains("Knowledge Graph"));
}

#[tokio::test]
async fn scaffold_inserted_at_zero_when_no_system_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .build();

    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let reqs = server.received_requests().await.unwrap();
    let forwarded: Value = reqs[0].body_json().unwrap();
    let msgs = forwarded["messages"].as_array().unwrap();
    // A fresh system message was inserted at position 0 (now 2 messages).
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["role"], json!("system"));
    let sys = msgs[0]["content"].as_str().unwrap();
    assert!(sys.contains("[ONTOLOGY CONTEXT]"));
    assert_eq!(msgs[1]["role"], json!("user"));
}

#[tokio::test]
async fn max_tokens_floored_by_adapter() {
    // The floor lives in the backend adapter; the façade must NOT re-floor. With
    // floor=1536 a sub-floor ask is raised in the FORWARDED body.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 1536))
        .build();

    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": PROMPT }],
            "max_tokens": 256,
            "stream": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let reqs = server.received_requests().await.unwrap();
    let forwarded: Value = reqs[0].body_json().unwrap();
    assert_eq!(forwarded["max_tokens"], json!(1536), "sub-floor raised");
    assert!(forwarded.get("stream").is_none(), "stream stripped");
}

#[tokio::test]
async fn backend_failure_propagates_502() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("model exploded"))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    // §7 table: BackendHttp → 502, upstream status + body attached (labelled).
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], json!("backend_http"));
    assert_eq!(body["upstream_status"], json!(500));
    assert!(body["detail"].as_str().unwrap().contains("model exploded"));
}
