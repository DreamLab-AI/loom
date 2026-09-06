//! EXP-014 — the grounding contract on EVERY response status (ADR-138 closeout).
//!
//! The review's acceptance condition: *"Test no-match, opt-out, semantic
//! fallback, verbatim, delegated success and backend failure. Define the required
//! grounding contract for each status."* Its finding was that grounding objects
//! were built for `/loom/scaffold` and for a successful chat, and that "non-200
//! backend paths lack the grounding contract".
//!
//! Why that gap is load-bearing rather than cosmetic: a consuming agent that
//! receives a bare 502 has the same information as one that receives an
//! ungrounded 200 — none. It cannot tell *the corpus had nothing to say* from
//! *the model was unreachable*, so it either trusts both or distrusts both. The
//! `status` + `corpus_backed` pair is what makes those two cases distinguishable
//! in one branch.
//!
//! Every test below asserts the SAME field set — [`REQUIRED_GROUNDING_FIELDS`] —
//! and then the status-specific meaning on top of it.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::{call, generation_with_id, semantic_hit, StubVector, TestEnvBuilder};
use loom_backend_openai::OpenAiBackend;
use loom_domain::REQUIRED_GROUNDING_FIELDS;
use serde_json::{json, Value};
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const HIT: &str = "Explain how a knowledge graph uses a graph database";
const MISS: &str = "best sourdough starter recipe";

fn backend_to(server: &MockServer) -> OpenAiBackend {
    OpenAiBackend::new(format!("{}/v1", server.uri()), Duration::from_secs(10), 0)
}

async fn ok_backend() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cmpl-1",
            "object": "chat.completion",
            "choices": [{ "message": { "role": "assistant", "content": "A knowledge graph uses a graph database." } }]
        })))
        .mount(&server)
        .await;
    server
}

/// THE contract assertion. Every grounding object, on every status, must carry
/// every required field — and the four the façade adds must be well-typed, not
/// merely present.
#[track_caller]
fn assert_contract(grounding: &Value, expected_status: &str) {
    let obj = grounding
        .as_object()
        .unwrap_or_else(|| panic!("grounding must be an object, got {grounding}"));
    let missing: Vec<_> = REQUIRED_GROUNDING_FIELDS
        .iter()
        .filter(|k| !obj.contains_key(**k))
        .collect();
    assert!(missing.is_empty(), "missing {missing:?} in {grounding}");

    assert_eq!(grounding["status"], json!(expected_status), "{grounding}");
    assert!(
        grounding["corpus_backed"].is_boolean(),
        "corpus_backed must be a bool: {grounding}"
    );
    assert!(
        grounding["generation"].is_string(),
        "the loaded generation must be named: {grounding}"
    );
    assert!(
        grounding["content_digest"]
            .as_str()
            .is_some_and(|d| d.len() == 64),
        "content_digest must be a sha256 hex: {grounding}"
    );
    assert!(
        grounding["degraded"].is_array(),
        "degraded must always be a list, empty when nothing degraded: {grounding}"
    );
    // The three axes the pre-contract shape conflated are still separate.
    assert!(grounding["signal"].is_string());
    assert!(grounding["score_scale"].is_string());
    assert!(grounding["confidence"].is_number());
}

// --- 1. no match --------------------------------------------------------------

#[tokio::test]
async fn no_match_reports_an_honest_zero_and_is_not_corpus_backed() {
    let server = ok_backend().await;
    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server))
        .with_commit_marker(true)
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": MISS }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let g = &body["loom"]["grounding"];
    assert_contract(g, "no-match");
    assert_eq!(g["engaged"], json!(false));
    assert_eq!(g["signal"], json!("none"));
    assert_eq!(g["decision"], json!("skipped"));
    assert_eq!(
        g["corpus_backed"],
        json!(false),
        "no evidence means the answer is not corpus-backed"
    );
    assert_eq!(g["top_score"], Value::Null);
    // The threshold reported is the one that actually judged the miss.
    assert_eq!(g["threshold"], json!(env.state.policy.min_inject_score));
}

#[tokio::test]
async fn scaffold_route_no_match_carries_the_same_contract() {
    let env = TestEnvBuilder::new().with_commit_marker(true).build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": MISS })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_contract(&body["grounding"], "no-match");
    assert_eq!(body["engaged"], json!(false));
}

// --- 2. opt-out ---------------------------------------------------------------

/// A verbatim serve the CALLER declined. Reported as `opt-out`, not folded into
/// a plain delegation: the node would have answered from the scaffold, and a
/// benchmark that cannot see the caller's choice will mis-attribute the latency
/// and the copy fidelity to the serving mode.
#[tokio::test]
async fn opt_out_is_distinguishable_from_an_ordinary_delegation() {
    let server = ok_backend().await;
    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server))
        .with_verbatim(true, 8.0)
        .with_commit_marker(true)
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": HIT }],
            "loom_options": { "verbatim": false }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let g = &body["loom"]["grounding"];
    assert_contract(g, "opt-out");
    assert_eq!(g["engaged"], json!(true));
    assert_eq!(
        g["corpus_backed"],
        json!(true),
        "the scaffold was still injected, so the answer IS corpus-backed"
    );
    assert_eq!(
        body["loom"]["served_mode"],
        json!("delegated"),
        "the delivery axis says delegated; the status axis says why"
    );
}

// --- 3. semantic fallback -----------------------------------------------------

/// The fallback's scores are cosines, not lexical-additive sums. The status and
/// the scale must move together or a consumer will threshold `0.83` against a
/// bar meant for `19.5`.
#[tokio::test]
async fn semantic_fallback_reports_its_own_status_and_score_scale() {
    let server = ok_backend().await;
    let generation = generation_with_id("2026-08-09T00:00:00Z");
    let vector = Arc::new(StubVector::new(
        true,
        generation,
        vec![semantic_hit("knowledge-graph", 0.83)],
    ));

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server))
        .with_vector(vector)
        .with_semantic_fallback(true, Some(0.5))
        .with_commit_marker(true)
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": MISS }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let g = &body["loom"]["grounding"];
    assert_contract(g, "semantic-fallback");
    assert_eq!(g["signal"], json!("semantic"));
    assert_eq!(
        g["score_scale"],
        json!("cosine"),
        "a semantic answer must not be labelled on the lexical scale"
    );
    assert_eq!(g["corpus_backed"], json!(true));
    assert_eq!(body["loom"]["fusion_path"], json!("SemanticFallback"));
    assert!(env.vector_call_count() > 0, "the fallback must have run");
}

// --- 4. verbatim --------------------------------------------------------------

#[tokio::test]
async fn verbatim_serve_carries_the_contract_and_names_its_own_threshold() {
    let env = TestEnvBuilder::new()
        .with_verbatim(true, 8.0)
        .with_commit_marker(true)
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": HIT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let g = &body["loom"]["grounding"];
    assert_contract(g, "verbatim");
    assert_eq!(g["decision"], json!("verbatim"));
    assert_eq!(
        g["threshold"],
        json!(8.0),
        "the bar cleared was the verbatim threshold, not min_inject_score"
    );
    assert_eq!(g["corpus_backed"], json!(true));
    assert_eq!(body["loom"]["served_mode"], json!("verbatim"));
}

// --- 5. delegated success -----------------------------------------------------

#[tokio::test]
async fn delegated_success_is_corpus_backed_when_engaged() {
    let server = ok_backend().await;
    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server))
        .with_commit_marker(true)
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": HIT }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let g = &body["loom"]["grounding"];
    assert_contract(g, "delegated");
    assert_eq!(g["engaged"], json!(true));
    assert_eq!(g["signal"], json!("lexical"));
    assert_eq!(g["score_scale"], json!("lexical-additive"));
    assert_eq!(g["corpus_backed"], json!(true));
    assert_eq!(body["loom"]["served_mode"], json!("delegated"));
    assert!(
        g["seeds"].as_array().is_some_and(|s| !s.is_empty()),
        "per-seed detail rides along: {g}"
    );
}

// --- 6. backend failure -------------------------------------------------------

/// THE gap the closeout names. A 502 must still say what retrieval found, and
/// must say that the answer is not corpus-backed even though retrieval SUCCEEDED
/// — because there is no answer at all.
#[tokio::test]
async fn backend_failure_carries_the_contract_and_is_never_corpus_backed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("model exploded"))
        .mount(&server)
        .await;

    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server))
        .with_commit_marker(true)
        .build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": HIT }] })),
    )
    .await;

    // §7 mapping is unchanged: an upstream failure is a 502.
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], json!("backend_http"));
    assert_eq!(body["upstream_status"], json!(500));

    let g = &body["loom"]["grounding"];
    assert_contract(g, "backend-failure");
    assert_eq!(
        g["engaged"],
        json!(true),
        "retrieval genuinely succeeded — that fact must survive the failure"
    );
    assert_eq!(
        g["corpus_backed"],
        json!(false),
        "there is no answer to be backed by the corpus"
    );
    assert!(
        g["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d == "backend-failure"),
        "the degradation must be named: {g}"
    );
    assert_eq!(body["loom"]["served_mode"], json!("failed"));
}

/// A retrieval-only node asked to delegate: same contract, 503, and the same
/// "retrieval worked, delivery did not" shape.
#[tokio::test]
async fn absent_backend_is_a_labelled_failure_with_the_contract_attached() {
    let env = TestEnvBuilder::new().with_commit_marker(true).build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": HIT }] })),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], json!("no_backend"));
    let g = &body["loom"]["grounding"];
    assert_contract(g, "backend-failure");
    assert_eq!(g["corpus_backed"], json!(false));
}

// --- cross-status invariants ---------------------------------------------------

/// `corpus_backed` must be the ONE predicate a consumer branches on, and it must
/// never be true where either half is missing — no evidence, or no answer.
#[tokio::test]
async fn corpus_backed_is_true_exactly_when_evidence_and_answer_both_exist() {
    let server = ok_backend().await;
    let cases: Vec<(&str, Value, bool)> = vec![
        (
            "hit + working backend",
            json!({ "messages": [{ "role": "user", "content": HIT }] }),
            true,
        ),
        (
            "miss + working backend",
            json!({ "messages": [{ "role": "user", "content": MISS }] }),
            false,
        ),
    ];
    for (label, body, expected) in cases {
        let env = TestEnvBuilder::new()
            .with_backend(backend_to(&server))
            .with_commit_marker(true)
            .build();
        let (_, out) = call(env.router(), "POST", "/v1/chat/completions", Some(body)).await;
        assert_eq!(
            out["loom"]["grounding"]["corpus_backed"],
            json!(expected),
            "{label}"
        );
    }
}

/// A degraded accelerator is NAMED, so an operator reading one response knows
/// which capability was missing without correlating with `/health`.
#[tokio::test]
async fn degradations_are_named_on_the_response_that_suffered_them() {
    let server = ok_backend().await;
    // Semantic fallback enabled with an artefact that failed its contract.
    let vector = Arc::new(StubVector::unqualified(
        generation_with_id("2026-08-09T00:00:00Z"),
        loom_domain::VectorMetric::Euclidean,
    ));
    let env = TestEnvBuilder::new()
        .with_backend(backend_to(&server))
        .with_vector(vector)
        .with_semantic_fallback(true, Some(0.5))
        .with_commit_marker(true)
        .build();

    let (_, body) = call(
        env.router(),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": MISS }] })),
    )
    .await;

    let degraded = body["loom"]["grounding"]["degraded"]
        .as_array()
        .expect("degraded list")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("; ");
    assert!(
        degraded.contains("semantic-unavailable") && degraded.contains("metric"),
        "the reason must name the failing axis, not just the outcome: {degraded}"
    );
}

/// An unattested (marker-less) bundle says so on every response, so a consumer
/// never mistakes a development checkout for a publisher-attested generation.
#[tokio::test]
async fn an_unattested_bundle_is_declared_on_the_response() {
    let env = TestEnvBuilder::new().with_commit_marker(false).build();
    let (_, body) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": HIT })),
    )
    .await;
    let degraded = body["grounding"]["degraded"].as_array().unwrap();
    assert!(
        degraded.iter().any(|d| d == "bundle-unattested"),
        "{degraded:?}"
    );
}
