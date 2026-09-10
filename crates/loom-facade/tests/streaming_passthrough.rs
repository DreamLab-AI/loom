//! Agent-client transport contract: opaque SSE and typed multimodal/tool input.
mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use common::TestEnvBuilder;
use loom_backend_openai::OpenAiBackend;
use serde_json::{json, Value};
use std::time::Duration;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn request(server: &MockServer, body: Value) -> axum::response::Response {
    let env = TestEnvBuilder::new()
        .with_backend(OpenAiBackend::new(
            format!("{}/v1", server.uri()),
            Duration::from_secs(10),
            1536,
        ))
        .build();
    env.router()
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn tools_images_usage_and_finish_reasons_pass_through_verbatim() {
    let server = MockServer::start().await;
    let events = concat!(
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"tool1\",\"type\":\"function\",\"function\":{\"name\":\"read_skill\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{}\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"completion_tokens\":8}}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(events, "text/event-stream; charset=utf-8"),
        )
        .mount(&server)
        .await;
    let mut body = json!({
        "model":"qwen", "stream":true, "max_tokens":17,
        "stream_options":{"include_usage":true},
        "loom_options":{"scaffold":false},
        "messages":[
            {"role":"user","content":[{"type":"text","text":"Assess the page"},{"type":"image_url","image_url":{"url":"data:image/png;base64,aGVsbG8=","detail":"high"}}]},
            {"role":"assistant","content":null,"tool_calls":[{"id":"prior","type":"function","function":{"name":"read_skill","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"prior","content":"Skill instructions"}
        ],
        "tools":[{"type":"function","function":{"name":"read_skill","parameters":{"type":"object"}}}],
        "tool_choice":"auto", "parallel_tool_calls":false
    });
    let response = request(&server, body.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-loom-served-mode"], "passthrough");
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        events.as_bytes()
    );
    body.as_object_mut().unwrap().remove("loom_options");
    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].body_json::<Value>().unwrap(), body);
}

#[tokio::test]
async fn upstream_errors_remain_labelled_json_before_sse_starts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({"error":"busy"})))
        .mount(&server)
        .await;
    let response = request(
        &server,
        json!({"stream":true,"loom_options":{"scaffold":false},"messages":[]}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body["upstream_status"], 429);
    assert_eq!(body["error"], "backend_http");
}

#[tokio::test]
async fn json_success_is_rejected_instead_of_masquerading_as_sse() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[]})))
        .mount(&server)
        .await;
    let response = request(
        &server,
        json!({"stream":true,"loom_options":{"scaffold":false},"messages":[]}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn first_chunk_arrives_before_completion_and_client_drop_cancels_upstream() {
    use futures_util::Stream;
    use http_body_util::BodyExt;
    use std::{
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Poll},
    };
    struct HangingStream {
        first: bool,
        dropped: Arc<AtomicBool>,
    }
    impl Stream for HangingStream {
        type Item = Result<axum::body::Bytes, std::io::Error>;
        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if self.first {
                self.first = false;
                Poll::Ready(Some(Ok(axum::body::Bytes::from_static(b"data: first\n\n"))))
            } else {
                Poll::Pending
            }
        }
    }
    impl Drop for HangingStream {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let state = dropped.clone();
    let upstream = axum::Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(move || {
            let dropped = state.clone();
            async move {
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(HangingStream {
                        first: true,
                        dropped,
                    }),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let env = TestEnvBuilder::new()
        .with_backend(OpenAiBackend::new(
            format!("http://{addr}/v1"),
            Duration::from_secs(10),
            1536,
        ))
        .build();
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        env.router().oneshot(
            Request::post("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"stream":true,"loom_options":{"scaffold":false},"messages":[]})
                        .to_string(),
                ))
                .unwrap(),
        ),
    )
    .await
    .expect("headers should arrive before upstream completion")
    .unwrap();
    let mut body = response.into_body();
    let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
        .await
        .expect("first SSE bytes should not wait for completion")
        .unwrap()
        .unwrap();
    assert_eq!(frame.into_data().unwrap().as_ref(), b"data: first\n\n");
    assert!(!dropped.load(Ordering::SeqCst));
    drop(body);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("client cancellation must drop upstream response body");
    server.abort();
}
