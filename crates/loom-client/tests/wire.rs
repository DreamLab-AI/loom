//! Wire-level behaviour against a mock façade.
//!
//! These are the tests that matter: every one of them encodes a failure that
//! reached production as a well-formed HTTP 200.

use std::time::Duration;

use loom_client::{ChatRequest, Error, LoomClient, LoomOptions, Message, ServedMode};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// A client pointed at `server`, with retries fast enough for a test.
fn client(server: &MockServer) -> LoomClient {
    LoomClient::builder(format!("{}/v1", server.uri()))
        .timeout(Duration::from_secs(5))
        .retry_backoff(Duration::from_millis(1))
        .build()
}

fn ask() -> ChatRequest {
    ChatRequest::new("test-model", vec![Message::user("hello")])
}

/// A normal, successful completion.
fn answer(content: &str) -> Value {
    json!({
        "model": "test-model",
        "choices": [{ "finish_reason": "stop", "message": { "content": content } }],
        "usage": { "prompt_tokens": 11, "completion_tokens": 22 }
    })
}

fn truncated() -> Value {
    json!({ "choices": [{ "finish_reason": "length", "message": { "content": "" } }] })
}

#[tokio::test]
async fn a_sub_floor_budget_is_raised_on_the_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer("ok")))
        .mount(&server)
        .await;

    client(&server).chat(ask().max_tokens(400)).await.unwrap();

    let sent: Value = server.received_requests().await.unwrap()[0]
        .body_json()
        .unwrap();
    assert_eq!(
        sent["max_tokens"], 1536,
        "a 400-token ask reaches a reasoning model as empty content"
    );
}

#[tokio::test]
async fn a_generous_budget_is_left_alone() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer("ok")))
        .mount(&server)
        .await;

    client(&server).chat(ask().max_tokens(12288)).await.unwrap();

    let sent: Value = server.received_requests().await.unwrap()[0]
        .body_json()
        .unwrap();
    assert_eq!(sent["max_tokens"], 12288);
}

#[tokio::test]
async fn truncation_retries_on_a_doubled_budget() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(truncated()))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer("complete")))
        .with_priority(2)
        .mount(&server)
        .await;

    let out = client(&server).chat(ask().max_tokens(2000)).await.unwrap();

    assert_eq!(out.content, "complete");
    assert_eq!(out.attempts, 2);
    let sent = server.received_requests().await.unwrap();
    let budget = |r: &Request| {
        r.body_json::<Value>().unwrap()["max_tokens"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(budget(&sent[0]), 2000);
    assert_eq!(
        budget(&sent[1]),
        4000,
        "the retry must raise the budget, not repeat it"
    );
}

#[tokio::test]
async fn a_permanently_truncated_answer_is_never_returned() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(truncated()))
        .mount(&server)
        .await;

    let err = client(&server)
        .chat(ask().max_tokens(2000))
        .await
        .unwrap_err();

    match err {
        Error::Truncated {
            attempts, budget, ..
        } => {
            assert_eq!(attempts, 3);
            assert_eq!(budget, 16000, "2000 doubled once per attempt");
        }
        other => panic!("expected Truncated, got {other:?}"),
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn scaffold_retrieval_is_refused_rather_than_parsed_as_an_answer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer(
            "## Grounding\n\nThis is ontology markdown; no model generation was performed.",
        )))
        .mount(&server)
        .await;

    let err = client(&server).chat(ask()).await.unwrap_err();

    assert!(matches!(err, Error::ScaffoldOnly { .. }), "got {err:?}");
    assert!(!err.is_transient(), "retrying a mode decision cannot help");
}

#[tokio::test]
async fn passthrough_that_was_ignored_is_an_error_not_an_answer() {
    let server = MockServer::start().await;
    let mut body = answer("grounded reply about the wrong sense of the word");
    body["loom"] = json!({ "served_mode": "grounded", "grounding": { "status": "ok" } });
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let err = client(&server)
        .chat(ask().options(LoomOptions::passthrough()))
        .await
        .unwrap_err();

    match err {
        Error::PassthroughRefused { served_mode, .. } => assert_eq!(served_mode, "grounded"),
        other => panic!("expected PassthroughRefused, got {other:?}"),
    }
}

#[tokio::test]
async fn passthrough_against_a_plain_openai_server_succeeds() {
    // No `loom` block at all: a llama.cpp server is passthrough by nature, and
    // demanding telemetry from it would break every non-façade deployment.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer("plain reply")))
        .mount(&server)
        .await;

    let out = client(&server)
        .chat(ask().options(LoomOptions::passthrough()))
        .await
        .unwrap();

    assert_eq!(out.content, "plain reply");
    assert_eq!(out.served_mode, ServedMode::Unreported);
}

#[tokio::test]
async fn the_two_switches_go_on_the_wire_separately() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer("ok")))
        .mount(&server)
        .await;

    let c = client(&server);
    c.chat(ask().options(LoomOptions::declining_verbatim()))
        .await
        .unwrap();
    c.chat(ask().options(LoomOptions::passthrough()))
        .await
        .unwrap();
    c.chat(ask()).await.unwrap();

    let sent = server.received_requests().await.unwrap();
    let opts = |i: usize| sent[i].body_json::<Value>().unwrap()["loom_options"].clone();
    assert_eq!(
        opts(0),
        json!({ "verbatim": false }),
        "grounding must survive"
    );
    assert_eq!(opts(1), json!({ "verbatim": false, "scaffold": false }));
    assert_eq!(opts(2), Value::Null, "no switch set means no field sent");
}

#[tokio::test]
async fn a_gateway_five_hundred_is_retried_and_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(524))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer("recovered")))
        .with_priority(2)
        .mount(&server)
        .await;

    let out = client(&server).chat(ask()).await.unwrap();

    assert_eq!(out.content, "recovered");
    assert_eq!(out.attempts, 2);
}

#[tokio::test]
async fn a_four_hundred_fails_immediately_without_retrying() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(422).set_body_string("bad request"))
        .mount(&server)
        .await;

    let err = client(&server).chat(ask()).await.unwrap_err();

    assert!(
        matches!(err, Error::Http { status: 422, .. }),
        "got {err:?}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "a client error will not improve on a second try"
    );
}

#[tokio::test]
async fn reasoning_and_telemetry_are_surfaced_not_swallowed() {
    let server = MockServer::start().await;
    let mut body = answer("the answer");
    body["choices"][0]["message"]["reasoning_content"] = json!("the working");
    body["loom"] = json!({ "served_mode": "grounded", "grounding": { "status": "hit" } });
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let out = client(&server).chat(ask()).await.unwrap();

    assert_eq!(out.content, "the answer");
    assert_eq!(out.reasoning.as_deref(), Some("the working"));
    assert_eq!(out.served_mode, ServedMode::Grounded);
    assert_eq!(out.grounding_status.as_deref(), Some("hit"));
    assert_eq!(out.usage.completion_tokens, Some(22));
    assert!(out.was_grounded());
}

#[tokio::test]
async fn health_reads_the_root_not_the_v1_base() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&server)
        .await;

    assert!(client(&server).healthy(Duration::from_secs(2)).await);
}

#[tokio::test]
async fn health_is_false_when_the_facade_says_it_is_not_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": false })))
        .mount(&server)
        .await;

    assert!(!client(&server).healthy(Duration::from_secs(2)).await);
}

#[tokio::test]
async fn health_without_an_ok_field_counts_as_healthy() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "up" })))
        .mount(&server)
        .await;

    assert!(client(&server).healthy(Duration::from_secs(2)).await);
}

#[tokio::test]
async fn first_model_id_reads_both_the_openai_and_llama_cpp_shapes() {
    for (body, want) in [
        (json!({ "data": [{ "id": "qwen3.8-27B" }] }), "qwen3.8-27B"),
        (json!({ "models": [{ "name": "gemma" }] }), "gemma"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        assert_eq!(client(&server).first_model_id().await.unwrap(), want);
    }
}

#[tokio::test]
async fn resolve_base_picks_the_first_healthy_candidate() {
    let dead = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&dead)
        .await;
    let live = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&live)
        .await;

    let candidates = vec![format!("{}/v1", dead.uri()), format!("{}/v1", live.uri())];
    let chosen = loom_client::resolve_base(&candidates, Duration::from_secs(2)).await;

    assert_eq!(chosen.as_deref(), Some(candidates[1].as_str()));
}

#[tokio::test]
async fn resolve_base_is_none_when_nothing_answers() {
    let dead = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&dead)
        .await;

    let candidates = vec![format!("{}/v1", dead.uri())];
    assert!(
        loom_client::resolve_base(&candidates, Duration::from_secs(2))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn an_empty_choices_array_is_an_empty_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .up_to_n_times(3)
        .mount(&server)
        .await;

    let err = client(&server).chat(ask()).await.unwrap_err();
    assert!(matches!(err, Error::Empty { .. }), "got {err:?}");
}

#[tokio::test]
async fn an_error_object_inside_a_two_hundred_is_not_an_answer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": { "message": "context length exceeded" }
        })))
        .mount(&server)
        .await;

    let err = client(&server).chat(ask()).await.unwrap_err();
    assert!(matches!(err, Error::Malformed { .. }), "got {err:?}");
}
