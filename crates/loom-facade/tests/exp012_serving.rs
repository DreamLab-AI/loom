//! EXP-012/013/014 — findings-driven serving features on `/v1/chat/completions`:
//! F1 verbatim serving, F2 exposure telemetry, F3 thinking + budget control. All
//! config-gated; defaults preserve current behaviour (proven by the existing
//! EXP-006 suite still passing unchanged).
//!
//! Proof technique for F1 "no backend call": the default builder wires a
//! retrieval-only backend (empty `DISTILL_BACKEND_URL`). If verbatim serves, the
//! request returns 200 WITHOUT a backend; if it falls through to delegate, the
//! backend surfaces `NoBackend` → 503. So 200-vs-503 cleanly witnesses which path
//! ran. Delegate-path features (F2/F3) use a wiremock backend and inspect the
//! forwarded body / annotated response.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::{call, TestEnvBuilder};
use loom_backend_openai::OpenAiBackend;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Engages the lexical gate strongly: "knowledge graph" is an exact 2-word title
/// n-gram (`top_score` ≈ 16 ≥ the 8.0 verbatim threshold).
const PROMPT: &str = "Explain how a knowledge graph uses a graph database";
/// Matches no fixture class title → no scaffold → NOT engaged.
const MISS_PROMPT: &str = "zzzz qqqq wwww vvvv";

fn backend_to(server: &MockServer, floor: u64) -> OpenAiBackend {
    OpenAiBackend::new(
        format!("{}/v1", server.uri()),
        Duration::from_secs(10),
        floor,
    )
}

// ---------------------------------------------------------------------------
// F1 — VERBATIM SERVING
// ---------------------------------------------------------------------------

#[tokio::test]
async fn f1_verbatim_serves_without_backend() {
    // Retrieval-only backend (default): a 200 here PROVES no backend was called.
    let env = TestEnvBuilder::new().with_verbatim(true, 8.0).build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "verbatim served without a backend");
    assert_eq!(body["model"], json!("loom-verbatim"));
    assert_eq!(body["object"], json!("chat.completion"));
    assert_eq!(body["choices"][0]["finish_reason"], json!("stop"));
    assert_eq!(body["usage"]["total_tokens"], json!(0));
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(
        content.contains("Served verbatim"),
        "provenance header present"
    );
    assert!(
        content.contains("Knowledge Graph"),
        "canonical markdown served"
    );
    assert!(
        !content.contains("[ONTOLOGY CONTEXT]"),
        "wrapper stripped from verbatim content"
    );
    // Telemetry marks the served mode + exposure block.
    assert_eq!(body["loom"]["served_mode"], json!("verbatim"));
    assert_eq!(body["loom"]["fusion_path"], json!("LexicalHit"));
    assert!(body["loom"]["exposure"]["targets"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn f1_opt_out_bypasses_verbatim() {
    // loom_options.verbatim=false forces the delegate path → NoBackend → 503.
    let env = TestEnvBuilder::new().with_verbatim(true, 8.0).build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": PROMPT }],
            "loom_options": { "verbatim": false }
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "opt-out → delegate → 503"
    );
}

#[tokio::test]
async fn f1_multi_turn_bypasses_verbatim() {
    // A prior assistant turn ⇒ not a delivery-lookup shape ⇒ delegate ⇒ 503.
    let env = TestEnvBuilder::new().with_verbatim(true, 8.0).build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [
                { "role": "user", "content": "hi" },
                { "role": "assistant", "content": "hello" },
                { "role": "user", "content": PROMPT }
            ]
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "multi-turn → delegate"
    );
}

#[tokio::test]
async fn f1_streaming_bypasses_verbatim() {
    let env = TestEnvBuilder::new().with_verbatim(true, 8.0).build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": PROMPT }],
            "stream": true
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "streaming → delegate"
    );
}

#[tokio::test]
async fn f1_threshold_boundary_gates_verbatim() {
    // Threshold above the achievable top_score ⇒ delegate ⇒ 503.
    let high = TestEnvBuilder::new().with_verbatim(true, 999.0).build();
    let (status, _) = call(
        high.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "below threshold → delegate"
    );

    // A low threshold clears ⇒ verbatim serves (200, no backend).
    let low = TestEnvBuilder::new().with_verbatim(true, 2.0).build();
    let (status2, body2) = call(
        low.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(body2["model"], json!("loom-verbatim"));
}

#[tokio::test]
async fn f1_default_off_preserves_delegation() {
    // Verbatim mode OFF (default) ⇒ engaged request still delegates ⇒ 503.
    let env = TestEnvBuilder::new().build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "F1 off ⇒ delegate");
}

// ---------------------------------------------------------------------------
// F2 — EXPOSURE TELEMETRY
// ---------------------------------------------------------------------------

#[tokio::test]
async fn f2_exposure_block_reports_drops() {
    let server = MockServer::start().await;
    // Answer mentions "knowledge graph" but omits "graph database".
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "role": "assistant", "content": "A knowledge graph is a structure." } }]
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
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let exposure = &body["loom"]["exposure"];
    assert!(
        exposure["targets"].as_u64().unwrap() >= 2,
        "multiple titles served"
    );
    assert!(
        exposure["delivered"].as_u64().unwrap() >= 1,
        "at least KG restated"
    );
    let dropped: Vec<String> = exposure["dropped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert!(
        dropped.iter().any(|t| t == "Graph Database"),
        "served-but-omitted title reported as dropped: {dropped:?}"
    );
    assert_eq!(body["loom"]["served_mode"], json!("delegated"));
}

#[tokio::test]
async fn f2_append_adds_not_covered_line() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "role": "assistant", "content": "A knowledge graph is a structure." } }]
        })))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .with_exposure_append(true)
        .build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": PROMPT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.starts_with("A knowledge graph is a structure."));
    assert!(
        content.contains("Not covered above:"),
        "append line present: {content}"
    );
}

#[tokio::test]
async fn f2_no_exposure_when_not_engaged() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "role": "assistant", "content": "no idea" } }]
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
        Some(json!({ "messages": [{ "role": "user", "content": MISS_PROMPT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Not engaged ⇒ exposure is null, served_mode still delegated.
    assert!(
        body["loom"]["exposure"].is_null(),
        "no exposure without injection"
    );
    assert_eq!(body["loom"]["served_mode"], json!("delegated"));
}

// ---------------------------------------------------------------------------
// F3 — THINKING + BUDGET CONTROL
// ---------------------------------------------------------------------------

#[tokio::test]
async fn f3_no_think_injects_kwargs_when_engaged() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .with_thinking(true, 1536)
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
    assert_eq!(
        forwarded["chat_template_kwargs"]["enable_thinking"],
        json!(false),
        "no-think injected on engaged request"
    );
}

#[tokio::test]
async fn f3_no_think_not_injected_when_not_engaged() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .with_thinking(true, 1536)
        .build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": MISS_PROMPT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let reqs = server.received_requests().await.unwrap();
    let forwarded: Value = reqs[0].body_json().unwrap();
    assert!(
        forwarded.get("chat_template_kwargs").is_none(),
        "no-think NEVER applied to a passthrough request"
    );
}

#[tokio::test]
async fn f3_client_override_keeps_thinking_and_floors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    // Backend floor 0 so the ONLY floor in play is F3's think-token floor.
    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .with_thinking(true, 1536)
        .build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": PROMPT }],
            "chat_template_kwargs": { "enable_thinking": true },
            "max_tokens": 256
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let reqs = server.received_requests().await.unwrap();
    let forwarded: Value = reqs[0].body_json().unwrap();
    // Client kwargs untouched; thinking active ⇒ sub-floor ask raised to 1536.
    assert_eq!(
        forwarded["chat_template_kwargs"]["enable_thinking"],
        json!(true)
    );
    assert_eq!(
        forwarded["max_tokens"],
        json!(1536),
        "think-token floor applied"
    );
}

#[tokio::test]
async fn f3_default_off_leaves_engaged_max_tokens_untouched() {
    // Audit finding 1 remediation: with ALL F3 env unset (no_think off, floor 0)
    // AND the backend floor disabled, an ENGAGED request's sub-floor max_tokens is
    // NOT raised — F3 is truly off by default, current behaviour preserved exactly.
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
        Some(json!({
            "messages": [{ "role": "user", "content": PROMPT }],
            "max_tokens": 256
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let reqs = server.received_requests().await.unwrap();
    let forwarded: Value = reqs[0].body_json().unwrap();
    assert_eq!(
        forwarded["max_tokens"],
        json!(256),
        "F3 default-off must not raise an engaged request's max_tokens"
    );
    assert!(
        forwarded.get("chat_template_kwargs").is_none(),
        "no-think default-off adds nothing"
    );
}

#[tokio::test]
async fn f3_floor_not_applied_to_passthrough() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "choices": [] })))
        .mount(&server)
        .await;

    // Engaged=false (miss prompt), backend floor 0. F3 must not raise max_tokens.
    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server, 0))
        .with_thinking(false, 1536)
        .build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": MISS_PROMPT }],
            "max_tokens": 256
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let reqs = server.received_requests().await.unwrap();
    let forwarded: Value = reqs[0].body_json().unwrap();
    assert_eq!(
        forwarded["max_tokens"],
        json!(256),
        "passthrough max_tokens untouched"
    );
}
