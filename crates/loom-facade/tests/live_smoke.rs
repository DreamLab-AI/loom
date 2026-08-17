//! LIVE smoke — the full façade over the REAL committed corpus (8,146 classes,
//! both reasoned TTLs, the `ontology-corpus.rvdb` HNSW artifact). `#[ignore]`d so
//! it never runs in the workspace regression gate; run explicitly for evidence:
//!
//!     cargo test -p loom-facade --test live_smoke -- --ignored --nocapture
//!
//! The real distill backend (`http://192.168.2.132:8084`) is DOWN in this
//! environment, so the chat path is delegated to a localhost wiremock — proving
//! the merge + annotate + delegate seam end-to-end without the model.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use common::call;
use loom_backend_openai::OpenAiBackend;
use loom_domain::{LexicalIndex, ModelBackend};
use loom_embed_xinference::XinferenceEmbedder;
use loom_facade::mirror::MirrorStore;
use loom_facade::state::AppState;
use loom_facade::{build_router, Config};
use loom_graph_oxigraph::OxigraphStore;
use loom_scaffold::policy::InjectionPolicy;
use loom_scaffold::LexicalRetriever;
use loom_vector_ruvector::HnswIndex;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Repo root, resolved from the crate manifest dir (CWD-independent).
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Build an `AppState` over the real corpus, delegating chat to `backend`.
fn live_state(backend: OpenAiBackend) -> AppState {
    let root = root();
    let index_path = root.join("app/data/scaffold-index.json");
    let prose_path = root.join("app/data/prose-index.json");
    let hnsw_path = root.join("data/ontology-corpus.rvdb");

    // Prose enrichment on, matching a real deployment.
    std::env::set_var("ONTOLOGY_PROSE_INDEX", &prose_path);
    let retriever =
        LexicalRetriever::load(Some(&index_path.to_string_lossy())).expect("real index loads");
    eprintln!("[live] lexical index: {} classes", retriever.class_count());

    let graph = OxigraphStore::load(root.join("app/data"));
    let semantic = HnswIndex::open(&hnsw_path);
    let embedder = XinferenceEmbedder::from_env();
    let generation = MirrorStore::new(&index_path.to_string_lossy());

    let config = Config {
        index_path: index_path.to_string_lossy().into_owned(),
        hnsw_artifact: hnsw_path.to_string_lossy().into_owned(),
        backend_url: backend.endpoint().to_owned(),
        ..Config::default()
    };

    AppState::new(
        Arc::new(retriever),
        Arc::new(semantic),
        Arc::new(graph),
        Arc::new(embedder),
        Arc::new(backend),
        Arc::new(generation),
        InjectionPolicy::default(),
        config,
    )
}

#[tokio::test]
#[ignore = "live: runs against the real committed corpus + a wiremock backend"]
async fn live_health_reports_real_corpus() {
    let state = live_state(OpenAiBackend::new("", Duration::from_secs(5), 1536));
    let (status, body) = call(build_router(state), "GET", "/health", None).await;
    eprintln!("[live] /health = {}", serde_json::to_string_pretty(&body).unwrap());
    assert_eq!(status, StatusCode::OK);

    assert_eq!(body["index_classes"], json!(8146), "expected 8,146 classes");
    assert_eq!(body["graph"]["available"], json!(true));
    let triples = body["graph"]["triples"].as_u64().unwrap();
    assert!(triples > 250_000, "expected ≈282k triples, got {triples}");
    assert_eq!(body["semantic"]["ready"], json!(true), "rvdb artifact present");
}

#[tokio::test]
#[ignore = "live: runs against the real committed corpus"]
async fn live_scaffold_engages() {
    let state = live_state(OpenAiBackend::new("", Duration::from_secs(5), 1536));
    let (status, body) = call(
        build_router(state),
        "POST",
        "/loom/scaffold",
        Some(json!({ "prompt": "Explain how a knowledge graph uses a graph database" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["engaged"], json!(true));
    let block = body["scaffold"].as_str().unwrap();
    eprintln!("[live] scaffold approx_tokens={}", body["approx_tokens"]);
    assert!(block.contains("Knowledge Graph"), "block head: {}", &block[..block.len().min(200)]);
}

#[tokio::test]
#[ignore = "live: runs against the real committed corpus"]
async fn live_sparql_count_works() {
    let state = live_state(OpenAiBackend::new("", Duration::from_secs(5), 1536));
    let (status, body) = call(
        build_router(state),
        "POST",
        "/loom/sparql",
        Some(json!({ "query": "SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let n: u64 = body["rows"][0][0].as_str().unwrap().parse().unwrap_or(0);
    eprintln!("[live] SPARQL COUNT(*) = {n}");
    assert!(n > 250_000, "expected ≈282k triples via SPARQL, got {n}");
}

#[tokio::test]
#[ignore = "live: real corpus + wiremock backend (real distill backend is down)"]
async fn live_chat_delegates_to_wiremock_and_annotates() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cmpl-live",
            "choices": [{ "message": { "role": "assistant", "content": "grounded answer" } }]
        })))
        .mount(&server)
        .await;

    let backend = OpenAiBackend::new(format!("{}/v1", server.uri()), Duration::from_secs(10), 1536);
    let state = live_state(backend);

    let (status, body) = call(
        build_router(state),
        "POST",
        "/v1/chat/completions",
        Some(json!({
            "messages": [{ "role": "user", "content": "Explain how a knowledge graph uses a graph database" }],
            "max_tokens": 256
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    eprintln!("[live] chat loom block = {}", body["loom"]);
    assert_eq!(body["loom"]["mode"], json!("scaffold"));
    assert_eq!(body["loom"]["fusion_path"], json!("LexicalHit"));
    assert!(body["loom"]["injected_tokens"].as_u64().unwrap() > 0);

    // The floor was applied by the adapter (256 → 1536) in the forwarded body.
    let reqs = server.received_requests().await.unwrap();
    let forwarded: serde_json::Value = reqs[0].body_json().unwrap();
    assert_eq!(forwarded["max_tokens"], json!(1536));
}

#[tokio::test]
#[ignore = "live: honesty path — retrieval-only node returns 503 on /v1/*"]
async fn live_retrieval_only_is_503() {
    let state = live_state(OpenAiBackend::new("", Duration::from_secs(5), 1536));
    let (status, _) = call(
        build_router(state),
        "POST",
        "/v1/chat/completions",
        Some(json!({ "messages": [{ "role": "user", "content": "hi" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
