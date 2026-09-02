//! LIVE contract test for the confidence-surfacing wire contract (ADR-138).
//!
//! This is the evaluator that would have caught the 2026-09-02 falsification:
//! the dream slot `confidence-injection` was REJECTED because nothing on the
//! wire reported whether the gate had engaged, so no evaluator could read the
//! per-request surface. This test reads it.
//!
//!     cargo test -q -p loom-facade --test contract_live
//!     LOOM_URL=http://192.168.2.132:8084 cargo test -q -p loom-facade --test contract_live
//!
//! Gated on reachability, not on `#[ignore]`: a node that is not up SKIPS with a
//! message (nightly evaluators must degrade, never fail red for being offline),
//! but a node that IS up is held to the whole contract.
//!
//! One test function, not three, on purpose: phase 3 counts requests on a SHARED
//! live node, so the probes must be sequenced rather than raced by the harness.

use std::time::Duration;

use serde_json::{json, Value};

/// The reference deployment: profile A on the HP, `:8084` DNAT'd to the LAN.
const DEFAULT_URL: &str = "http://127.0.0.1:8084";

/// Verified against the live HP facade on 2026-09-02: `top_score` 10.75 on the
/// lexical-additive scale (seeds `urn:ngm:class:rollup`, `…:blockchain`), i.e.
/// above `LOOM_STRONG_MATCH_SCORE` 8.0, so `confidence` clamps to 1.0.
const ON_ONTOLOGY: &str = "rollup in blockchain scaling";

/// Verified against the same node: `top_score` 0.0, `fusion_path` `NoMatch`,
/// zero seeds. The off-ontology arm of the bench protocol's A/B.
const OFF_ONTOLOGY: &str = "banana pancakes recipe";

/// The six fields the contract puts on every seed.
const SEED_FIELDS: [&str; 6] = [
    "iri",
    "score",
    "confidence",
    "quality",
    "provenance",
    "injected",
];

/// The closed set of `decision` values, all lowercase.
const DECISIONS: [&str; 4] = ["full", "scaled", "skipped", "verbatim"];

/// The closed set of `signal` values, all lowercase.
const SIGNALS: [&str; 3] = ["lexical", "semantic", "none"];

fn base_url() -> String {
    std::env::var("LOOM_URL").unwrap_or_else(|_| DEFAULT_URL.to_owned())
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("http client")
}

async fn health(c: &reqwest::Client, base: &str) -> Value {
    c.get(format!("{base}/health"))
        .send()
        .await
        .expect("GET /health")
        .json()
        .await
        .expect("/health is JSON")
}

async fn scaffold(c: &reqwest::Client, base: &str, prompt: &str) -> Value {
    let resp = c
        .post(format!("{base}/loom/scaffold"))
        .json(&json!({ "prompt": prompt }))
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST /loom/scaffold {prompt:?}: {e}"));
    assert!(
        resp.status().is_success(),
        "POST /loom/scaffold {prompt:?} → {}",
        resp.status()
    );
    resp.json().await.expect("/loom/scaffold is JSON")
}

/// Pull the `grounding` block out of a `/loom/scaffold` response, asserting the
/// contract's central promise: it is ALWAYS present, engaged or not.
fn grounding<'a>(body: &'a Value, prompt: &str) -> &'a Value {
    let g = body.get("grounding").unwrap_or_else(|| {
        panic!(
            "POST /loom/scaffold {prompt:?}: top-level `grounding` is MISSING. \
             The contract requires it on EVERY response — an absent block is the \
             exact hole the 2026-09-02 dream cycle recorded. Body keys: {:?}",
            body.as_object().map(|m| m.keys().collect::<Vec<_>>())
        )
    });
    assert!(
        g.is_object(),
        "POST /loom/scaffold {prompt:?}: `grounding` is {g}, expected an object"
    );
    g
}

fn str_field<'a>(g: &'a Value, key: &str, allowed: &[&str]) -> &'a str {
    let v = g
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("grounding.{key} missing or not a string: {g}"));
    assert_eq!(
        v,
        v.to_lowercase(),
        "grounding.{key} = {v:?} must be lowercase (the contract names lowercase enums)"
    );
    assert!(
        allowed.contains(&v),
        "grounding.{key} = {v:?} is not one of {allowed:?}"
    );
    v
}

fn f64_field(g: &Value, key: &str) -> f64 {
    g.get(key)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("grounding.{key} missing or not a number: {g}"))
}

/// Assertions that hold on EVERY grounding block, engaged or skipped.
fn assert_shape(g: &Value) {
    str_field(g, "signal", &SIGNALS);
    str_field(g, "decision", &DECISIONS);
    str_field(g, "score_scale", &["lexical-additive", "cosine"]);

    let confidence = f64_field(g, "confidence");
    assert!(
        (0.0..=1.0).contains(&confidence),
        "grounding.confidence = {confidence} outside [0, 1] — confidence is a clamped ratio"
    );

    let threshold = f64_field(g, "threshold");
    assert!(
        threshold >= 0.0,
        "grounding.threshold = {threshold} must be non-negative"
    );

    assert!(
        g.get("engaged").and_then(Value::as_bool).is_some(),
        "grounding.engaged missing or not a boolean: {g}"
    );
    // `top_score` and `effective_budget` are nullable by contract, but the keys
    // must be present so a consumer never has to guess between "no match" and
    // "field not implemented".
    assert!(
        g.get("top_score").is_some(),
        "grounding.top_score key missing (null is a valid VALUE, absence is not)"
    );
    assert!(
        g.get("effective_budget").is_some(),
        "grounding.effective_budget key missing (null is a valid VALUE, absence is not)"
    );
    assert!(
        g.get("seeds").and_then(Value::as_array).is_some(),
        "grounding.seeds missing or not an array: {g}"
    );
}

#[tokio::test]
async fn grounding_contract_holds_on_the_live_facade() {
    let base = base_url();
    let c = client();

    // --- gate: skip (not fail) when the node is not up ----------------------
    let probe = c
        .get(format!("{base}/health"))
        .timeout(Duration::from_secs(5))
        .send()
        .await;
    if probe.is_err() {
        eprintln!(
            "[contract_live] SKIP — no facade at {base} \
             (set LOOM_URL to point at a running node; default {DEFAULT_URL})"
        );
        return;
    }

    // --- phase 0: /health carries the policy + counter surface --------------
    let before = health(&c, &base).await;
    for block in ["injection_policy", "serving", "confidence"] {
        assert!(
            before.get(block).is_some_and(Value::is_object),
            "/health.{block} is missing — run `cargo run -q -p loom-facade \
             --bin confidence-check` against this node for the full report"
        );
    }
    let requests_before = before["confidence"]["requests"]
        .as_u64()
        .expect("/health.confidence.requests is a non-negative integer");
    let strong_match = before["injection_policy"]["strong_match_score"]
        .as_f64()
        .expect("/health.injection_policy.strong_match_score is a number");

    // --- phase 1: an ON-ONTOLOGY prompt engages at full confidence ----------
    assert_on_ontology(&c, &base, strong_match).await;

    // --- phase 2: an OFF-ONTOLOGY prompt reports a skip, not a hole ---------
    assert_off_ontology(&c, &base).await;

    // --- phase 3: the two probes moved the /health counters -----------------
    let after = health(&c, &base).await;
    let requests_after = after["confidence"]["requests"]
        .as_u64()
        .expect("/health.confidence.requests is a non-negative integer");
    let delta = requests_after.saturating_sub(requests_before);
    // `>= 2`, not `== 2`: this runs against a SHARED node that may be serving
    // other traffic (and the counter is a rolling window that can saturate).
    // Two probes must be VISIBLE; exact equality would flake nightly.
    assert!(
        delta >= 2,
        "two /loom/scaffold probes must advance /health.confidence.requests by at \
         least 2 (before={requests_before}, after={requests_after}, delta={delta}) — \
         the counters are not wired to the request path"
    );
    eprintln!(
        "[contract_live] OK — {base}: requests {requests_before} → {requests_after}, \
         mean_confidence {}",
        after["confidence"]["mean_confidence"]
    );
}

/// Phase 1 — an exact-title lexical hit must engage at (near) full confidence,
/// and must show the seeds it engaged on.
async fn assert_on_ontology(c: &reqwest::Client, base: &str, strong_match: f64) {
    let body = scaffold(c, base, ON_ONTOLOGY).await;
    let g = grounding(&body, ON_ONTOLOGY);
    assert_shape(g);

    let decision = str_field(g, "decision", &DECISIONS);
    assert!(
        decision == "full" || decision == "verbatim",
        "{ON_ONTOLOGY:?} scored above the strong-match score ({strong_match}) so the \
         gate must grant the full budget (or serve verbatim), got decision={decision:?}"
    );
    assert_eq!(
        g["signal"], "lexical",
        "{ON_ONTOLOGY:?} is an exact-title lexical hit"
    );
    assert_eq!(
        g["engaged"], true,
        "{ON_ONTOLOGY:?} must engage the scaffold"
    );

    let confidence = f64_field(g, "confidence");
    assert!(
        confidence >= 0.9,
        "{ON_ONTOLOGY:?} scored top_score={} against strong_match_score={strong_match}, \
         so confidence should clamp near 1.0, got {confidence}",
        g["top_score"]
    );

    let top_score = g["top_score"]
        .as_f64()
        .expect("an engaged request has a numeric top_score");
    let expected = (top_score / strong_match).clamp(0.0, 1.0);
    assert!(
        (confidence - expected).abs() < 1e-6,
        "confidence must be clamp(top_score / strong_match_score, 0, 1): \
         {top_score} / {strong_match} = {expected}, got {confidence}"
    );

    let seeds = g["seeds"].as_array().expect("seeds array");
    assert!(
        !seeds.is_empty(),
        "{ON_ONTOLOGY:?} engaged, so it must report the seeds it engaged on"
    );
    assert_seeds(seeds);
}

/// Every seed carries all six contract fields, with the nullable ones typed.
fn assert_seeds(seeds: &[Value]) {
    for (i, seed) in seeds.iter().enumerate() {
        for field in SEED_FIELDS {
            assert!(
                seed.get(field).is_some(),
                "seeds[{i}].{field} missing — the contract puts all six of \
                 {SEED_FIELDS:?} on every seed. Seed: {seed}"
            );
        }
        let sc = seed["confidence"]
            .as_f64()
            .unwrap_or_else(|| panic!("seeds[{i}].confidence is not a number: {seed}"));
        assert!(
            (0.0..=1.0).contains(&sc),
            "seeds[{i}].confidence = {sc} outside [0, 1]"
        );
        assert!(
            seed["injected"].is_boolean(),
            "seeds[{i}].injected must be a boolean: {seed}"
        );
        // `quality` is nullable (not every class carries a curated grade), but
        // the key must exist.
        assert!(
            seed["quality"].is_null() || seed["quality"].is_number(),
            "seeds[{i}].quality must be a number or null: {seed}"
        );
        // `grounding.seeds[].provenance` is the LOWERCASE wire string
        // ("lexical" / "semantic-hnsw"). The legacy flat `seeds` alias array on
        // /loom/scaffold carries the CamelCase `MatchProvenance` form instead —
        // reading the wrong array is the easy mistake, so pin the case here.
        let prov = seed["provenance"]
            .as_str()
            .unwrap_or_else(|| panic!("seeds[{i}].provenance is not a string: {seed}"));
        assert_eq!(
            prov,
            prov.to_lowercase(),
            "seeds[{i}].provenance = {prov:?} must be lowercase — CamelCase here \
             means the legacy alias array leaked into grounding.seeds"
        );
    }
    assert!(
        seeds.iter().any(|s| s["injected"] == json!(true)),
        "an engaged request injected at least one seed, but none is flagged injected"
    );
}

/// Phase 2 — an off-ontology prompt must report a SKIP. The contract's point is
/// that this is a reported decision, not an absent block.
async fn assert_off_ontology(c: &reqwest::Client, base: &str) {
    let body = scaffold(c, base, OFF_ONTOLOGY).await;
    let g = grounding(&body, OFF_ONTOLOGY);
    assert_shape(g);

    assert_eq!(
        g["decision"], "skipped",
        "{OFF_ONTOLOGY:?} is off-ontology: the gate must report a skip"
    );
    assert_eq!(
        g["signal"], "none",
        "{OFF_ONTOLOGY:?} matched nothing, so there is no signal"
    );
    assert_eq!(
        g["engaged"], false,
        "{OFF_ONTOLOGY:?} must not engage the scaffold"
    );
    let confidence = f64_field(g, "confidence");
    assert!(
        confidence.abs() < f64::EPSILON,
        "{OFF_ONTOLOGY:?} must report confidence 0, got {confidence}"
    );
    assert!(
        g["seeds"].as_array().is_some_and(Vec::is_empty),
        "{OFF_ONTOLOGY:?} matched nothing, so seeds must be empty: {}",
        g["seeds"]
    );

    // A miss reports honest ABSENCE, not a zero. These two are `Option` on the
    // wire and must be `null` here.
    //
    // Do not "fix" these to `== 0.0` from a live observation: the flat legacy
    // aliases at the TOP level of the /loom/scaffold body (`top_score`, an f32,
    // and `effective_budget`, a usize) genuinely do read 0 on a miss, and they
    // coexist with these for one release. The two disagree by design — 0 is a
    // score, null is "there was no score". Asserting 0 here would pass against
    // a node with no grounding block at all, which is exactly the state this
    // whole contract exists to detect.
    assert!(
        g["top_score"].is_null(),
        "{OFF_ONTOLOGY:?} matched nothing, so grounding.top_score must be null, \
         not a number: {} (the flat top-level alias is the one that reads 0.0)",
        g["top_score"]
    );
    assert!(
        g["effective_budget"].is_null(),
        "{OFF_ONTOLOGY:?} was skipped, so grounding.effective_budget must be \
         null, not a number: {}",
        g["effective_budget"]
    );
}
