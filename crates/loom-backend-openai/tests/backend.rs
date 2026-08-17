//! EXP-006 (backend half): the `OpenAiBackend` delegate against a mocked
//! OpenAI-compatible upstream. Asserts the ported façade semantics — the
//! `max_tokens` floor (raise, preserve, disable), `stream` stripping,
//! `max_completion_tokens` flooring, non-2xx mapping, `/models` passthrough,
//! and the `reachable` probe.

use std::time::Duration;

use loom_domain::{LoomError, ModelBackend};
use loom_backend_openai::OpenAiBackend;
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a backend pointed at the mock server, whose `DISTILL_BACKEND_URL`
/// carries the `/v1` suffix exactly like the real deployment.
fn backend(server: &MockServer, floor: u64) -> OpenAiBackend {
    OpenAiBackend::new(
        format!("{}/v1", server.uri()),
        Duration::from_secs(30),
        floor,
    )
}

const OK_COMPLETION: fn() -> serde_json::Value = || {
    json!({
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "hi"}}]
    })
};

#[tokio::test]
async fn floor_raises_sub_floor_max_tokens() {
    let server = MockServer::start().await;
    // The upstream only answers 200 when it receives the FLOORED body; a
    // mismatch falls through to wiremock's default 404, failing the assert.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_json(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 1536
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(OK_COMPLETION()))
        .mount(&server)
        .await;

    let out = backend(&server, 1536)
        .chat(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 256
        }))
        .await
        .expect("floored request should be accepted (256 → 1536)");
    assert_eq!(out.status, 200);
    assert_eq!(out.body["choices"][0]["message"]["content"], "hi");
}

#[tokio::test]
async fn higher_ask_is_preserved() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_json(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 4096
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(OK_COMPLETION()))
        .mount(&server)
        .await;

    // Floor is 1536 but the caller asked for 4096 → 4096 must survive.
    backend(&server, 1536)
        .chat(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 4096
        }))
        .await
        .expect("higher ask (4096) must be preserved, not lowered");
}

#[tokio::test]
async fn floor_disabled_at_zero_leaves_body_untouched() {
    let server = MockServer::start().await;
    // Floor 0 → no flooring and NO default insertion; 256 passes through as-is.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_json(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 256
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(OK_COMPLETION()))
        .mount(&server)
        .await;

    backend(&server, 0)
        .chat(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 256
        }))
        .await
        .expect("floor=0 disables flooring; 256 stays 256");
}

#[tokio::test]
async fn stream_is_stripped() {
    let server = MockServer::start().await;
    // The floored body must NOT contain `stream`; with floor 0 the only rewrite
    // is the strip, so an exact body without `stream` proves it was popped.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_json(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(OK_COMPLETION()))
        .mount(&server)
        .await;

    backend(&server, 0)
        .chat(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "stream": true
        }))
        .await
        .expect("`stream` must be stripped before delegation");
}

#[tokio::test]
async fn max_completion_tokens_is_floored_too() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_json(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_completion_tokens": 1536
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(OK_COMPLETION()))
        .mount(&server)
        .await;

    backend(&server, 1536)
        .chat(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_completion_tokens": 100
        }))
        .await
        .expect("max_completion_tokens (100 → 1536) must be floored");
}

#[tokio::test]
async fn absent_token_fields_get_default_floor_inserted() {
    let server = MockServer::start().await;
    // Neither field present + floor active → façade inserts max_tokens = floor.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_json(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}],
            "max_tokens": 1536
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(OK_COMPLETION()))
        .mount(&server)
        .await;

    backend(&server, 1536)
        .chat(json!({
            "model": "m",
            "messages": [{"role": "user", "content": "q"}]
        }))
        .await
        .expect("no token field + active floor → max_tokens defaulted to floor");
}

#[tokio::test]
async fn non_2xx_maps_to_backend_http() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(502).set_body_string("upstream boom"))
        .mount(&server)
        .await;

    let err = backend(&server, 1536)
        .chat(json!({"model": "m", "messages": []}))
        .await
        .expect_err("502 must surface as a labelled BackendHttp error");
    match err {
        LoomError::BackendHttp { status, body } => {
            assert_eq!(status, 502);
            assert_eq!(body, "upstream boom");
        }
        other => panic!("expected BackendHttp, got {other:?}"),
    }
}

#[tokio::test]
async fn models_passes_through() {
    let server = MockServer::start().await;
    let catalogue = json!({
        "object": "list",
        "data": [{"id": "qwen3.8-27b", "object": "model"}]
    });
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalogue.clone()))
        .mount(&server)
        .await;

    let out = backend(&server, 1536).models().await.expect("models passthrough");
    assert_eq!(out, catalogue);
}

#[tokio::test]
async fn reachable_true_on_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .mount(&server)
        .await;

    assert!(backend(&server, 1536).reachable().await, "2xx /models → reachable");
}

#[tokio::test]
async fn reachable_false_on_5xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    assert!(
        !backend(&server, 1536).reachable().await,
        "non-2xx probe → not reachable (mirrors urlopen raising on HTTPError)"
    );
}

#[tokio::test]
async fn empty_endpoint_is_retrieval_only() {
    // No DISTILL_BACKEND_URL: chat/models surface NoBackend, reachable is false,
    // and endpoint() returns the raw (empty) string.
    let be = OpenAiBackend::new("", Duration::from_secs(5), 1536);
    assert_eq!(be.endpoint(), "");
    assert!(!be.reachable().await);
    assert!(matches!(
        be.chat(json!({"messages": []})).await,
        Err(LoomError::NoBackend)
    ));
    assert!(matches!(be.models().await, Err(LoomError::NoBackend)));
}
