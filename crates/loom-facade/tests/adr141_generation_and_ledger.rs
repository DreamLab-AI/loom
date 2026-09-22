//! ADR-141 — the Loom consumes the vault build.
//!
//! Three properties, each of which was a decision rather than a refactor:
//!
//! 1. **Two marker shapes, one reader.** A `vault build` `.generation.json`
//!    (`visionGraph@<sha>`, artefact list) and the legacy mirror marker
//!    (ISO stamp, artefact map) both activate, and the served identity has ONE
//!    shape whichever was read. The live node still carries the legacy marker.
//! 2. **Staleness is a reported fact, never a refusal.** A generation past its
//!    OKF `stale_after` still answers; `/health` says `generation.stale: true`
//!    and the MCP manifest names `generation-stale` among the degraded
//!    accelerators.
//! 3. **The governance ledger is chain-verified and survives a restart.**

mod common;

use axum::http::StatusCode;
use serde_json::{json, Value};

use common::{call, TestEnvBuilder};

fn rpc(id: u64, method: &str, params: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

// --- 1. generation identity --------------------------------------------------

#[tokio::test]
async fn a_vault_build_marker_activates_and_names_the_local_generation() {
    let env = TestEnvBuilder::new()
        .with_vault_marker("abc1234", None)
        .build();
    let (status, body) = call(env.router(), "GET", "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["generation"]["id"], "visionGraph@abc1234");
    assert_eq!(body["generation"]["source"], "VaultBuild");
    assert_eq!(body["generation"]["commit_sha"], "abc1234");
    assert_eq!(body["generation"]["class_count"], 7);
    assert_eq!(body["generation"]["page_count"], 7);
    assert_eq!(body["generation"]["vocabulary_version"], 1);
    // A build-declared digest is carried beside the digest this process
    // computed over the bytes it read; both being present is what lets the two
    // be compared at all.
    assert!(body["generation"]["content_digest"].is_string());
    assert!(body["serving_bundle"]["identity"]["content_digest"].is_string());
    assert_eq!(
        body["serving_bundle"]["identity"]["atomicity_verified"],
        true
    );
}

#[tokio::test]
async fn the_legacy_mirror_marker_still_activates() {
    // The reader is a superset; the live HP bundle is this shape until the
    // first vault-build promotion, and refusing it would take the node down to
    // gain nothing.
    let env = TestEnvBuilder::new().with_commit_marker(true).build();
    let (status, body) = call(env.router(), "GET", "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["generation"]["source"], "MirrorManifest");
    assert_eq!(body["generation"]["id"], "2026-09-05T00:00:00Z");
    assert_eq!(
        body["serving_bundle"]["identity"]["atomicity_verified"],
        true
    );
    // The keys the new shape adds are present and null, not absent: a consumer
    // reads one shape whichever marker the node activated.
    assert!(body["generation"]["stale_after"].is_null());
    assert_eq!(body["generation"]["stale"], false);
}

// --- 2. stale_after ----------------------------------------------------------

#[tokio::test]
async fn a_fresh_generation_is_not_stale() {
    let env = TestEnvBuilder::new()
        .with_vault_marker("fresh01", Some("2999-01-01"))
        .build();
    let (_, body) = call(env.router(), "GET", "/health", None).await;

    assert_eq!(body["generation"]["stale_after"], "2999-01-01");
    assert_eq!(body["generation"]["stale"], false);
}

#[tokio::test]
async fn a_generation_past_its_stale_after_is_reported_stale_and_still_served() {
    let env = TestEnvBuilder::new()
        .with_vault_marker("stale01", Some("2020-01-01"))
        .build();

    let (status, health) = call(env.router(), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK, "a stale node still answers");
    assert_eq!(health["ok"], true);
    assert_eq!(health["generation"]["stale"], true);
    assert_eq!(health["generation"]["stale_after"], "2020-01-01");

    // The agentic plane learns it from the manifest's degraded list, where an
    // agent already looks for reasons to discount an answer.
    let (status, body) = call(
        env.router(),
        "POST",
        "/mcp",
        Some(rpc(
            1,
            "tools/call",
            &json!({ "name": "loom.manifest", "arguments": {} }),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let manifest = &body["result"]["structuredContent"]["result"];
    let degraded: Vec<&str> = manifest["degraded"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(degraded.contains(&"generation-stale"), "{degraded:?}");

    // The grounding contract is UNCHANGED by staleness: the corpus answered,
    // and how old it is belongs in the manifest, not in the retrieval status.
    let grounding = &body["result"]["structuredContent"]["grounding"];
    assert_eq!(grounding["status"], "navigated");
}

// --- 3. the governance ledger ------------------------------------------------

fn decision(case: &str, outcome: &str) -> Value {
    json!({
        "case_id": case,
        "digest": "sha256:deadbeef",
        "outcome": outcome,
        "signer": "human:npub1testsigner",
        "at": "2026-09-22T12:00:00Z",
    })
}

#[tokio::test]
async fn a_signed_decision_is_ledgered_and_the_chain_verifies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("ledger.jsonl");
    let env = TestEnvBuilder::new().with_ledger_path(&ledger).build();

    let (status, body) = call(
        env.router(),
        "POST",
        "/loom/attest",
        Some(decision("case-1", "promote")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let entry_id = body["entry_id"].as_str().expect("entry_id").to_owned();
    assert_eq!(body["chain_head"], entry_id.as_str());
    assert_eq!(entry_id.len(), 64, "a sha-256 chain hash");

    let (status, verify) = call(env.router(), "GET", "/loom/attest/verify", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(verify["ok"], true);
    assert_eq!(verify["length"], 1);

    // Every outcome is recorded, not only the admitting one: a rejection that
    // left no trace would make the ledger unable to explain an absence.
    for (case, outcome) in [
        ("case-2", "demote"),
        ("case-3", "reject"),
        ("case-4", "expired"),
    ] {
        let (status, _) = call(
            env.router(),
            "POST",
            "/loom/attest",
            Some(decision(case, outcome)),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{outcome}");
    }
    let (_, verify) = call(env.router(), "GET", "/loom/attest/verify", None).await;
    assert_eq!(verify["ok"], true);
    assert_eq!(verify["length"], 4);
}

#[tokio::test]
async fn the_chain_survives_a_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("ledger.jsonl");

    let first = TestEnvBuilder::new().with_ledger_path(&ledger).build();
    let (status, _) = call(
        first.router(),
        "POST",
        "/loom/attest",
        Some(decision("case-restart", "promote")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    drop(first);

    // A reload is a process boundary (ADR-135 closeout), so the ledger must be
    // a file, not process state.
    let second = TestEnvBuilder::new().with_ledger_path(&ledger).build();
    let (_, verify) = call(second.router(), "GET", "/loom/attest/verify", None).await;
    assert_eq!(verify["ok"], true);
    assert_eq!(verify["length"], 1);

    let (status, body) = call(
        second.router(),
        "POST",
        "/loom/attest",
        Some(decision("case-restart-2", "promote")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, verify) = call(second.router(), "GET", "/loom/attest/verify", None).await;
    assert_eq!(verify["ok"], true, "chained across the restart: {body}");
    assert_eq!(verify["length"], 2);
}

#[tokio::test]
async fn a_decision_without_a_case_or_digest_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env = TestEnvBuilder::new()
        .with_ledger_path(&dir.path().join("ledger.jsonl"))
        .build();

    let (status, _) = call(
        env.router(),
        "POST",
        "/loom/attest",
        Some(json!({
            "case_id": "  ",
            "digest": "sha256:deadbeef",
            "outcome": "promote",
            "signer": "human:npub1testsigner",
            "at": "2026-09-22T12:00:00Z",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // An unknown outcome is a deserialisation failure, not a silent default:
    // the ledger must never record a decision nobody made.
    let (status, _) = call(
        env.router(),
        "POST",
        "/loom/attest",
        Some(decision("case-x", "maybe")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, verify) = call(env.router(), "GET", "/loom/attest/verify", None).await;
    assert_eq!(verify["length"], 0);
}
