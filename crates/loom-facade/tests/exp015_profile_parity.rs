//! EXP-015 — Profile A/B serving parity over one loaded generation (ADR-137).
//!
//! The review's acceptance condition: *"Verify profile A/B serving parity and
//! recovery against the same loaded generation."* Its finding was narrower than
//! it sounds — "code for both deployment profiles does not prove either is
//! activated" — but the risk it points at is real: the two compose files differ
//! in nine environment variables, and nothing established which of those
//! differences are ALLOWED to change an answer.
//!
//! This file states that boundary and then tests it. A deploy profile may choose
//! its port, its backend, its serving regime (verbatim) and its thinking
//! controls. It may NOT change what is retrieved, what is injected, or how
//! confidence is reported. So:
//!
//! - **Retrieval parity is exact.** For the same query over the same loaded
//!   generation, both profiles must produce a byte-identical scaffold block, the
//!   same seeds, the same score, and the same grounding evidence.
//! - **Delivery divergence is bounded.** Profile A may serve a high-confidence
//!   delivery lookup verbatim where B delegates — that is the whole point of the
//!   profile — but the EVIDENCE both report about that answer must be identical.
//!
//! Both environments are activated over ONE shared data directory — literally
//! the same bytes on the same disk — so they reach the same generation and the
//! same content digest. The test asserts that first, because a parity claim over
//! two different corpora is meaningless.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use axum::Router;
use common::{call, TestEnvBuilder};
use loom_backend_openai::OpenAiBackend;
use loom_facade::build_info::EffectiveConfig;
use loom_facade::BuildInfo;
use serde_json::{json, Value};
use std::path::Path;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The queries the parity sweep runs: a strong exact-title hit, a weaker
/// multi-word hit, and a clean miss — one per gate outcome.
const QUERIES: &[&str] = &[
    "Explain how a knowledge graph uses a graph database",
    "graph database",
    "best sourdough starter recipe",
];

async fn ok_backend() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cmpl-1",
            "object": "chat.completion",
            "choices": [{ "message": { "role": "assistant", "content": "answer" } }]
        })))
        .mount(&server)
        .await;
    server
}

fn backend_to(server: &MockServer, floor: u64) -> OpenAiBackend {
    OpenAiBackend::new(
        format!("{}/v1", server.uri()),
        Duration::from_secs(10),
        floor,
    )
}

/// Profile A, as `deploy/compose.profile-a.yml` sets it: verbatim serving on at
/// the exact-title threshold, no-think on, the 1536 think-token floor.
fn profile_a(server: &MockServer, data_dir: &Path) -> common::TestEnv {
    TestEnvBuilder::new()
        .with_data_dir(data_dir)
        .with_profile("a")
        .with_backend(backend_to(server, 1536))
        .with_verbatim(true, 8.0)
        .with_thinking(true, 1536)
        .with_commit_marker(true)
        .build()
}

/// Profile B, as `deploy/compose.profile-b.yml` sets it: the serving-regime
/// switches all commented out, i.e. library defaults.
fn profile_b(server: &MockServer, data_dir: &Path) -> common::TestEnv {
    TestEnvBuilder::new()
        .with_data_dir(data_dir)
        .with_profile("b")
        .with_backend(backend_to(server, 1536))
        .with_commit_marker(true)
        .build()
}

// --- the premise: one loaded generation --------------------------------------

#[tokio::test]
async fn both_profiles_activate_the_same_loaded_generation_and_content() {
    let server = ok_backend().await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );

    let ia = a.state.generation.identity();
    let ib = b.state.generation.identity();
    assert_eq!(
        ia.generation.id, ib.generation.id,
        "parity is only meaningful over one generation"
    );
    assert_eq!(
        ia.content_digest, ib.content_digest,
        "…and over one set of bytes: identical generation strings are not enough"
    );
    assert!(ia.atomicity_verified && ib.atomicity_verified);
}

/// Retrieval-affecting configuration must be identical. The knobs that may
/// differ are enumerated once, in `EffectiveConfig::PROFILE_DIVERGENT_KEYS`, so
/// a new switch is a deliberate addition to that list rather than a silent
/// parity break.
#[tokio::test]
async fn profiles_differ_only_in_the_declared_divergent_keys() {
    let server = ok_backend().await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );

    let ea = EffectiveConfig::of(&a.state.config);
    let eb = EffectiveConfig::of(&b.state.config);
    assert_eq!(
        ea.retrieval_identity(),
        eb.retrieval_identity(),
        "retrieval configuration must be profile-invariant"
    );

    let (va, vb) = (json!(ea), json!(eb));
    let differing: Vec<String> = va
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, v)| vb.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect();
    for key in &differing {
        assert!(
            EffectiveConfig::PROFILE_DIVERGENT_KEYS.contains(&key.as_str()),
            "{key} differs between profiles but is not a declared divergent key"
        );
    }
    assert!(
        differing.iter().any(|k| k == "verbatim_mode"),
        "the fixture must actually exercise a divergence: {differing:?}"
    );
}

/// Both profiles are the same binary, so the release receipt must be identical.
/// A profile is a runtime configuration, never a different build.
#[test]
fn the_release_receipt_is_profile_independent() {
    let build = BuildInfo::current();
    assert!(build.source_identity_complete());
    let a = build.with_effective_config(&loom_facade::Config {
        deploy_profile: "a".to_owned(),
        ..loom_facade::Config::default()
    });
    let b = build.with_effective_config(&loom_facade::Config {
        deploy_profile: "b".to_owned(),
        ..loom_facade::Config::default()
    });
    assert_eq!(a.build, b.build, "one binary, one build identity");
    assert_ne!(
        a.effective_config.deploy_profile,
        b.effective_config.deploy_profile
    );
}

// --- retrieval parity ---------------------------------------------------------

/// The core assertion: over one loaded generation, retrieval and injection are
/// byte-identical across profiles, for every gate outcome.
#[tokio::test]
async fn scaffold_output_is_byte_identical_across_profiles() {
    let server = ok_backend().await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );

    for query in QUERIES {
        let ra = scaffold(a.router(), query).await;
        let rb = scaffold(b.router(), query).await;

        assert_eq!(
            ra["scaffold"], rb["scaffold"],
            "served block must be identical for {query:?}"
        );
        assert_eq!(ra["engaged"], rb["engaged"], "{query:?}");
        assert_eq!(ra["seeds"], rb["seeds"], "{query:?}");
        assert_eq!(ra["top_score"], rb["top_score"], "{query:?}");
        assert_eq!(ra["effective_budget"], rb["effective_budget"], "{query:?}");
        assert_eq!(ra["fusion_path"], rb["fusion_path"], "{query:?}");
        assert_eq!(
            ra["grounding"], rb["grounding"],
            "the grounding evidence must be profile-invariant for {query:?}"
        );
        assert_eq!(ra["generation"], rb["generation"], "{query:?}");
    }
}

/// The gate's own configuration is not a profile choice. Profile A's compose
/// file sets all four values explicitly to the code defaults precisely so the
/// uplift A/B varies only the master switch — this pins that intent.
#[tokio::test]
async fn the_injection_gate_is_reported_identically_by_both_profiles() {
    let server = ok_backend().await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );

    let (_, ha) = call(a.router(), "GET", "/health", None).await;
    let (_, hb) = call(b.router(), "GET", "/health", None).await;

    assert_eq!(ha["injection_policy"], hb["injection_policy"]);
    assert_eq!(ha["index_classes"], hb["index_classes"]);
    assert_eq!(ha["generation"], hb["generation"]);
    assert_eq!(
        ha["serving_bundle"]["identity"]["content_digest"],
        hb["serving_bundle"]["identity"]["content_digest"]
    );
    assert_eq!(ha["build"], hb["build"], "one binary");
    // …and the declared divergence is visible where it belongs.
    assert_eq!(ha["serving"]["verbatim_mode"], json!(true));
    assert_eq!(hb["serving"]["verbatim_mode"], json!(false));
    assert_eq!(ha["deploy_profile"], json!("a"));
    assert_eq!(hb["deploy_profile"], json!("b"));
}

// --- bounded delivery divergence ----------------------------------------------

/// Profile A serves the high-confidence delivery lookup verbatim; B delegates.
/// That is the intended difference — and the EVIDENCE each reports about the
/// answer must still be the same, so a benchmark can attribute a latency or
/// fidelity difference to the serving regime rather than to retrieval.
#[tokio::test]
async fn delivery_mode_diverges_while_the_evidence_stays_identical() {
    let server = ok_backend().await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );
    let query = QUERIES[0];

    let (sa, ba) = chat(a.router(), query).await;
    let (sb, bb) = chat(b.router(), query).await;
    assert_eq!(sa, StatusCode::OK);
    assert_eq!(sb, StatusCode::OK);

    // The declared divergence.
    assert_eq!(ba["loom"]["served_mode"], json!("verbatim"));
    assert_eq!(bb["loom"]["served_mode"], json!("delegated"));
    assert_eq!(ba["loom"]["grounding"]["status"], json!("verbatim"));
    assert_eq!(bb["loom"]["grounding"]["status"], json!("delegated"));

    // Everything about the EVIDENCE is the same.
    let (ga, gb) = (&ba["loom"]["grounding"], &bb["loom"]["grounding"]);
    for field in [
        "signal",
        "score_scale",
        "top_score",
        "confidence",
        "seeds",
        "engaged",
        "corpus_backed",
        "generation",
        "content_digest",
        "degraded",
    ] {
        assert_eq!(ga[field], gb[field], "grounding.{field} must match");
    }
    assert_eq!(ba["loom"]["fusion_path"], bb["loom"]["fusion_path"]);
}

/// A miss delegates on BOTH profiles: the verbatim regime is confidence-gated,
/// so it cannot introduce a divergence where there is no evidence to serve.
#[tokio::test]
async fn a_miss_behaves_identically_on_both_profiles() {
    let server = ok_backend().await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );
    let query = QUERIES[2];

    let (sa, ba) = chat(a.router(), query).await;
    let (sb, bb) = chat(b.router(), query).await;
    assert_eq!(sa, sb);
    assert_eq!(ba["loom"]["served_mode"], json!("delegated"));
    assert_eq!(bb["loom"]["served_mode"], json!("delegated"));
    assert_eq!(ba["loom"]["grounding"], bb["loom"]["grounding"]);
}

// --- recovery parity ------------------------------------------------------------

/// Recovery from a dead backend must be identical in KIND across profiles: the
/// same status, the same contract, the same evidence. Profile A's verbatim
/// regime does not apply to a miss, so both profiles reach the backend and both
/// must fail the same way.
#[tokio::test]
async fn backend_failure_recovery_is_identical_across_profiles() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("model down"))
        .mount(&server)
        .await;
    let shared = TempDir::new().expect("shared data dir");
    let (a, b) = (
        profile_a(&server, shared.path()),
        profile_b(&server, shared.path()),
    );
    let query = QUERIES[2]; // a miss: neither profile can serve it verbatim

    let (sa, ba) = chat(a.router(), query).await;
    let (sb, bb) = chat(b.router(), query).await;

    assert_eq!(sa, StatusCode::BAD_GATEWAY);
    assert_eq!(sb, StatusCode::BAD_GATEWAY);
    assert_eq!(ba["error"], bb["error"]);
    assert_eq!(ba["upstream_status"], bb["upstream_status"]);
    assert_eq!(
        ba["loom"]["grounding"], bb["loom"]["grounding"],
        "the failure contract must be profile-invariant"
    );
    assert_eq!(ba["loom"]["served_mode"], json!("failed"));
    assert_eq!(bb["loom"]["served_mode"], json!("failed"));
}

// --- helpers -------------------------------------------------------------------

async fn scaffold(router: Router, prompt: &str) -> Value {
    let (status, body) = call(
        router,
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": prompt })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "scaffold {prompt:?}");
    body
}

async fn chat(router: Router, prompt: &str) -> (StatusCode, Value) {
    call(
        router,
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": prompt }] })),
    )
    .await
}
