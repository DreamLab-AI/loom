//! Protocol-level tests. The host here is a stub: these assert the ADAPTER's
//! contract (framing, schemas, clamping, grounding presence, I-P1), not
//! retrieval, which is tested where it lives.

use super::*;

struct StubHost;

fn grounding() -> Value {
    json!({ "status": "engaged", "corpus_backed": true, "generation": "2026-08-22T08:19:43Z" })
}

#[async_trait]
impl ToolHost for StubHost {
    async fn manifest(&self, salience: usize) -> Result<ToolOutcome, LoomError> {
        Ok(ToolOutcome::new(
            json!({ "salience_used": salience }),
            grounding(),
        ))
    }
    async fn browse(
        &self,
        query: &str,
        kind: UnitKind,
        n: usize,
    ) -> Result<ToolOutcome, LoomError> {
        Ok(ToolOutcome::new(
            json!({
                "query": query, "kind": kind.as_str(), "n": n,
                "candidates": [ { "iri": "urn:ngm:class:knowledge-graph", "score": 19.5, "provenance": "lexical" } ]
            }),
            grounding(),
        ))
    }
    async fn resolve(&self, iris: &[Iri]) -> Result<ToolOutcome, LoomError> {
        Ok(ToolOutcome::new(
            json!({ "count": iris.len(), "block": "## Knowledge Graph\n…" }),
            grounding(),
        ))
    }
    async fn sparql(&self, _query: &str) -> Result<ToolOutcome, LoomError> {
        Err(LoomError::BadQuery("write forms are refused".into()))
    }
    async fn neighbours(&self, _iri: &Iri, limit: usize) -> Result<ToolOutcome, LoomError> {
        Ok(ToolOutcome::new(
            json!({ "limit_used": limit }),
            grounding(),
        ))
    }
    async fn paths(&self, _f: &Iri, _t: &Iri, max_hops: usize) -> Result<ToolOutcome, LoomError> {
        Ok(ToolOutcome::new(
            json!({ "max_hops_used": max_hops }),
            grounding(),
        ))
    }
}

async fn call(args: Value) -> Value {
    dispatch(
        &StubHost,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": args }),
    )
    .await
    .expect("a request always answers")
}

#[tokio::test]
async fn initialize_echoes_a_supported_version_and_falls_back_otherwise() {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                      "params": { "protocolVersion": "2024-11-05" } });
    let r = dispatch(&StubHost, &req).await.unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");

    let req = json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize",
                      "params": { "protocolVersion": "1999-01-01" } });
    let r = dispatch(&StubHost, &req).await.unwrap();
    assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSION);
}

#[tokio::test]
async fn a_notification_produces_no_response() {
    let req = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert!(dispatch(&StubHost, &req).await.is_none());
}

#[tokio::test]
async fn tools_list_publishes_exactly_the_p0_contract() {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let r = dispatch(&StubHost, &req).await.unwrap();
    let names: Vec<&str> = r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            "loom.manifest",
            "loom.browse",
            "loom.resolve",
            "loom.sparql",
            "loom.neighbours",
            "loom.paths"
        ]
    );
}

/// Invariant I-P1 on the agentic plane: browse hands back addresses, and the
/// adapter has no path by which unit content could ride along.
#[tokio::test]
async fn browse_returns_addresses_and_never_content() {
    let r =
        call(json!({ "name": "loom.browse", "arguments": { "query": "knowledge graph" } })).await;
    let result = &r["result"]["structuredContent"]["result"];
    let candidate = &result["candidates"][0];
    assert!(candidate.get("iri").is_some());
    assert!(candidate.get("score").is_some());
    for forbidden in [
        "block",
        "body",
        "dfull",
        "markdown",
        "definition",
        "content",
    ] {
        assert!(
            candidate.get(forbidden).is_none(),
            "browse candidate leaked `{forbidden}` — I-P1 requires addresses only"
        );
    }
}

#[tokio::test]
async fn every_successful_tool_result_carries_a_grounding_envelope() {
    for args in [
        json!({ "name": "loom.manifest", "arguments": {} }),
        json!({ "name": "loom.browse", "arguments": { "query": "x" } }),
        json!({ "name": "loom.resolve", "arguments": { "iris": ["knowledge-graph"] } }),
        json!({ "name": "loom.neighbours", "arguments": { "iri": "knowledge-graph" } }),
        json!({ "name": "loom.paths", "arguments": { "from": "a", "to": "b" } }),
    ] {
        let r = call(args.clone()).await;
        assert!(
            r["result"]["structuredContent"]["grounding"].is_object(),
            "no grounding on {args}"
        );
    }
}

#[tokio::test]
async fn numeric_arguments_are_clamped_to_their_schema_ceiling() {
    let r = call(json!({ "name": "loom.manifest", "arguments": { "salience": 100_000 } })).await;
    assert_eq!(
        r["result"]["structuredContent"]["result"]["salience_used"],
        schema::MAX_SALIENCE
    );

    let r = call(json!({ "name": "loom.browse", "arguments": { "query": "x", "n": 9_999 } })).await;
    assert_eq!(
        r["result"]["structuredContent"]["result"]["n"],
        schema::MAX_BROWSE_N
    );
}

#[tokio::test]
async fn resolve_caps_the_address_list() {
    let many: Vec<String> = (0..100).map(|i| format!("slug-{i}")).collect();
    let r = call(json!({ "name": "loom.resolve", "arguments": { "iris": many } })).await;
    assert_eq!(
        r["result"]["structuredContent"]["result"]["count"],
        schema::MAX_RESOLVE_IRIS
    );
}

#[tokio::test]
async fn a_bad_argument_is_invalid_params_and_a_retrieval_failure_is_not() {
    let r = call(json!({ "name": "loom.browse", "arguments": { "query": "   " } })).await;
    assert_eq!(r["error"]["code"], -32602);

    let r = call(json!({ "name": "loom.sparql", "arguments": { "query": "DELETE {}" } })).await;
    assert_eq!(r["error"]["code"], -32000);
}

#[tokio::test]
async fn unknown_methods_and_tools_are_method_not_found() {
    let req = json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" });
    assert_eq!(
        dispatch(&StubHost, &req).await.unwrap()["error"]["code"],
        -32601
    );

    let r = call(json!({ "name": "loom.delete_everything", "arguments": {} })).await;
    assert_eq!(r["error"]["code"], -32601);
}
