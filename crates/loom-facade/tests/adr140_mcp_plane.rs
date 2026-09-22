//! ADR-140 P0 — the agentic plane, end to end through the real router.
//!
//! What these tests are for: the unit tests in `loom-mcp` prove the adapter's
//! framing against a stub host. These prove the thing the ADR actually claims —
//! that the second plane reaches the SAME corpus through the SAME gate and
//! stamps the SAME grounding contract as `/v1/chat/completions`. A door that
//! answers from a different index, or without a generation, is the defect
//! ADR-140 §1.3 was written about, and it would pass the unit tests untouched.

mod common;

use axum::http::StatusCode;
use common::{call, TestEnvBuilder};
use loom_domain::REQUIRED_GROUNDING_FIELDS;
use serde_json::{json, Value};

fn rpc(id: u32, method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn tool(id: u32, name: &str, arguments: &Value) -> Value {
    rpc(
        id,
        "tools/call",
        &json!({ "name": name, "arguments": arguments }),
    )
}

/// Every key the ADR-138 contract requires, on a tool result, exactly as on an
/// HTTP answer.
fn assert_grounding_contract(body: &Value) {
    let grounding = &body["result"]["structuredContent"]["grounding"];
    assert!(grounding.is_object(), "no grounding object: {body}");
    for field in REQUIRED_GROUNDING_FIELDS {
        assert!(
            grounding.get(*field).is_some(),
            "grounding missing `{field}`: {grounding}"
        );
    }
}

#[tokio::test]
async fn initialize_and_tools_list_are_served_on_the_same_listener() {
    let env = TestEnvBuilder::new().build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(rpc(
            1,
            "initialize",
            &json!({ "protocolVersion": "2025-06-18" }),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["serverInfo"]["name"], "ontology-loom");

    let (status, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(rpc(2, "tools/list", &json!({}))),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["tools"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn the_manifest_names_the_generation_the_corpus_and_what_is_degraded() {
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(1, "loom.manifest", &json!({}))),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let manifest = &body["result"]["structuredContent"]["result"];
    assert!(manifest["generation"].is_object(), "{manifest}");
    assert!(manifest["profile"]["classes"].as_u64().unwrap() > 0);
    // The fixture carries no OKF trust keys, so every family but `terms` is an
    // honest zero rather than a silent absence (D4 in OKF vocabulary, ADR-141).
    assert_eq!(manifest["coverage"]["attested_computations"], 0);
    assert_eq!(manifest["coverage"]["verified_machine"], 0);
    assert_eq!(manifest["coverage"]["verified_human"], 0);
    assert_eq!(
        manifest["coverage"]["terms"],
        manifest["profile"]["classes"]
    );
    // The fixture has no ready HNSW, and the manifest must say so rather than
    // let an agent over-trust a lexical miss (Addendum A, conclusion 3).
    let degraded: Vec<&str> = manifest["degraded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(degraded.contains(&"semantic-hnsw"), "{degraded:?}");
    assert_eq!(manifest["tools"].as_array().unwrap().len(), 6);
    assert_grounding_contract(&body);
}

/// The manifest is O(1) in corpus size (D2): it must not grow a salience list
/// the size of the index just because it was asked to.
#[tokio::test]
async fn the_manifest_salience_list_is_bounded() {
    let env = TestEnvBuilder::new().build();
    let (_, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(1, "loom.manifest", &json!({ "salience": 100_000 }))),
    )
    .await;
    let salient = body["result"]["structuredContent"]["result"]["salient"]
        .as_array()
        .unwrap();
    assert!(
        salient.len() <= loom_mcp::schema::MAX_SALIENCE,
        "{}",
        salient.len()
    );
}

/// Invariant I-P1 through the real router: browse hands back addresses only.
#[tokio::test]
async fn browse_returns_addresses_and_resolve_returns_the_content() {
    let env = TestEnvBuilder::new().build();

    let (_, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(
            1,
            "loom.browse",
            &json!({ "query": "knowledge graph" }),
        )),
    )
    .await;
    let candidates = body["result"]["structuredContent"]["result"]["candidates"]
        .as_array()
        .expect("candidates array");
    assert!(!candidates.is_empty(), "fixture should match");
    let first = &candidates[0];
    assert!(first["iri"].as_str().is_some());
    for forbidden in ["block", "dfull", "definition", "landscape"] {
        assert!(
            first.get(forbidden).is_none(),
            "browse leaked `{forbidden}`: {first}"
        );
    }
    assert_grounding_contract(&body);

    let iri = first["iri"].as_str().unwrap().to_owned();
    let (_, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(2, "loom.resolve", &json!({ "iris": [iri] }))),
    )
    .await;
    let result = &body["result"]["structuredContent"]["result"];
    assert_eq!(result["engaged"], true, "{result}");
    assert!(
        result["block"]
            .as_str()
            .unwrap()
            .contains("[ONTOLOGY CONTEXT]"),
        "resolve must serve the canonical block"
    );
    assert_grounding_contract(&body);
    // Served from the corpus with no backend call — the same status the
    // injection plane uses for that decision.
    assert_eq!(
        body["result"]["structuredContent"]["grounding"]["status"],
        "verbatim"
    );
}

/// The whole point of D1: one gate. A resolve is budget-clamped exactly as an
/// injected scaffold is, and reports the same budget.
#[tokio::test]
async fn a_resolve_is_gated_and_budget_clamped_like_the_injection_plane() {
    let env = TestEnvBuilder::new().build();

    let (_, http) = call(
        env.router(),
        "POST",
        "/loom/scaffold",
        Some(json!({ "query": "knowledge graph" })),
    )
    .await;

    let (_, mcp) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(
            1,
            "loom.resolve",
            &json!({ "iris": ["knowledge-graph"] }),
        )),
    )
    .await;
    let result = &mcp["result"]["structuredContent"]["result"];

    assert_eq!(
        http["effective_budget"], result["effective_budget"],
        "the two planes must share one budget clamp"
    );
    assert_eq!(
        http["grounding"]["generation"],
        mcp["result"]["structuredContent"]["grounding"]["generation"],
        "the two planes must serve one generation"
    );
}

#[tokio::test]
async fn a_miss_is_an_honest_no_match_not_an_error() {
    let env = TestEnvBuilder::new().build();
    let (status, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(
            1,
            "loom.resolve",
            &json!({ "iris": ["definitely-not-a-class-xyzzy"] }),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let grounding = &body["result"]["structuredContent"]["grounding"];
    assert_eq!(grounding["status"], "no-match");
    assert_eq!(grounding["corpus_backed"], false);
    assert_eq!(
        body["result"]["structuredContent"]["result"]["requested"][0]["found"],
        false
    );
}

/// A navigation is neither an answer nor a failed lookup, and the contract has
/// to be able to say so (the `navigated` status added with this plane).
#[tokio::test]
async fn navigation_tools_report_the_navigated_status_and_are_not_corpus_backed() {
    let env = TestEnvBuilder::new().build();
    for args in [
        tool(1, "loom.browse", &json!({ "query": "knowledge graph" })),
        tool(2, "loom.neighbours", &json!({ "iri": "knowledge-graph" })),
        tool(
            3,
            "loom.paths",
            &json!({ "from": "knowledge-graph", "to": "ontology" }),
        ),
        tool(4, "loom.manifest", &json!({})),
    ] {
        let (_, body) = call(env.router(), "POST", "/mcp", Some(args.clone())).await;
        let grounding = &body["result"]["structuredContent"]["grounding"];
        assert_eq!(grounding["status"], "navigated", "for {args}");
        assert_eq!(grounding["corpus_backed"], false, "for {args}");
    }
}

#[tokio::test]
async fn neighbours_exposes_the_reasoned_projection() {
    let env = TestEnvBuilder::new().build();
    let (_, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(tool(
            1,
            "loom.neighbours",
            &json!({ "iri": "knowledge-graph" }),
        )),
    )
    .await;
    let result = &body["result"]["structuredContent"]["result"];
    assert_eq!(result["found"], true, "{result}");
    // `ancestors` is the inferred closure — the thing a raw-markdown door
    // cannot compute, and the reason this plane exists on Loom rather than
    // beside it (ADR-140 Addendum A).
    assert!(result.get("ancestors").is_some());
    assert!(result.get("is_a").is_some());
    assert!(result.get("relations").is_some());
}

#[tokio::test]
async fn a_get_on_the_mcp_endpoint_declines_the_stream_honestly() {
    let env = TestEnvBuilder::new().build();
    let (status, _) = call(env.router(), "GET", "/mcp", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn a_batch_of_notifications_is_accepted_with_no_body() {
    let env = TestEnvBuilder::new().build();
    let (status, _) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(json!([{ "jsonrpc": "2.0", "method": "notifications/initialized" }])),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
}
