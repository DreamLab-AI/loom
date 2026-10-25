//! Jev option (LM-01-S1-01 gate, LM-02-S1-01 rank + support) on
//! `/v1/chat/completions`.
//!
//! Three obligations, per the rollout contract:
//!
//! 1. **Fallback equivalence** — flag off, no endpoint, transport error,
//!    timeout, HTTP error, malformed body and low confidence all leave BOTH the
//!    forwarded backend body and the response byte-identical to the pre-change
//!    (Jev-off) run. Equality is asserted against the baseline, not "no crash".
//! 2. **Wire shape** — what is sent matches the System One contract.
//! 3. **Accept path** — a confident mocked answer produces the intended gate /
//!    reorder / annotation, with the normal serving path still running after it.
//!
//! No live calls: the transport is a mock; the model backend is a wiremock.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use common::{call, TestEnvBuilder};
use loom_backend_openai::OpenAiBackend;
use loom_facade::jev::{
    Endpoint, Fallback, JevConfig, JevTransport, ReqwestTransport, TransportHandle,
    GATE_QUESTION_SET, ROUTE_GROUNDED, ROUTE_INSUFFICIENT, ROUTE_PASSTHROUGH,
};
use serde_json::{json, Value};
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PROMPT: &str = "Explain how a knowledge graph uses a graph database";

// --- mock transport ----------------------------------------------------------

type Script = dyn Fn(&Value) -> Result<Value, Fallback> + Send + Sync;

struct MockTransport {
    script: Box<Script>,
    seen: Mutex<Vec<Value>>,
}

impl MockTransport {
    fn new(f: impl Fn(&Value) -> Result<Value, Fallback> + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            script: Box::new(f),
            seen: Mutex::new(Vec::new()),
        })
    }
    fn calls(&self) -> Vec<Value> {
        self.seen.lock().unwrap().clone()
    }
}

#[async_trait]
impl JevTransport for MockTransport {
    async fn post(&self, _e: &Endpoint, body: &Value, _t: Duration) -> Result<Value, Fallback> {
        self.seen.lock().unwrap().push(body.clone());
        (self.script)(body)
    }
}

fn endpoint() -> Result<Endpoint, Fallback> {
    Ok(Endpoint {
        url: "http://127.0.0.1:1/v1/systemone".to_owned(),
        model: "jev-test".to_owned(),
        bearer_env: None,
    })
}

fn cfg(t: &Arc<MockTransport>) -> JevConfig {
    JevConfig {
        enabled: true,
        endpoint: endpoint(),
        transport: TransportHandle(t.clone()),
        ..JevConfig::default()
    }
}

fn gate_answer(route: &str, confidence: f64) -> Value {
    json!({"model":"jev-test","usage":{"input_tokens":1,"output_tokens":1},"answers":{
        "route":{"type":"choice","choice":route,"confidence":confidence,"probabilities":{route:1.0}},
        "answerable":{"type":"noul","noul":0.1}}})
}

/// Answer every `rel_*` question present in a rank request with `score_for(i)`,
/// and a `supported` question with `support`.
fn answer_for(req: &Value, score_for: &dyn Fn(usize) -> f64, support: f64) -> Value {
    let qs = req["questions"].as_object().unwrap();
    let mut answers = serde_json::Map::new();
    for k in qs.keys() {
        if let Some(i) = k.strip_prefix("rel_") {
            let i: usize = i.parse().unwrap();
            answers.insert(k.clone(), json!({"type":"score","score":score_for(i),"confidence":0.9,"legend":{},"probabilities":{}}));
        } else if k == "supported" {
            answers.insert(k.clone(), json!({"type":"noul","noul":support}));
        } else if k == "route" {
            answers.insert(
                k.clone(),
                json!({"type":"choice","choice":ROUTE_GROUNDED,"confidence":0.9}),
            );
        } else if k == "answerable" {
            answers.insert(k.clone(), json!({"type":"noul","noul":0.9}));
        }
    }
    json!({"model":"jev-test","usage":{"input_tokens":1,"output_tokens":1},"answers":answers})
}

// --- backend + harness -------------------------------------------------------

async fn mock_backend() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cmpl-fixed", "object": "chat.completion", "created": 1, "model": "m",
            "choices": [{"index":0,"message":{"role":"assistant","content":"A knowledge graph stores entities."},"finish_reason":"stop"}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        })))
        .mount(&server)
        .await;
    server
}

struct Run {
    status: StatusCode,
    body: Value,
    forwarded: Value,
}

async fn run(jev: Option<JevConfig>, req_body: Value) -> Run {
    let server = mock_backend().await;
    let mut b = TestEnvBuilder::new().with_backend(OpenAiBackend::new(
        format!("{}/v1", server.uri()),
        Duration::from_secs(10),
        1536,
    ));
    if let Some(j) = jev {
        b = b.with_jev(j);
    }
    let env = b.build();
    let (status, body) = call(env.router(), "POST", "/v1/chat/completions", Some(req_body)).await;
    let reqs = server.received_requests().await.unwrap();
    let forwarded = reqs
        .first()
        .map(|r| serde_json::from_slice::<Value>(&r.body).unwrap())
        .unwrap_or(Value::Null);
    Run {
        status,
        body: norm(body),
        forwarded,
    }
}

/// Each run activates its own bundle; `activated_at` has one-second resolution
/// and is the only field that legitimately differs between two runs.
fn norm(mut v: Value) -> Value {
    if let Some(g) = v.pointer_mut("/loom/generation") {
        if let Some(m) = g.as_object_mut() {
            m.remove("activated_at");
        }
    }
    v
}

fn chat() -> Value {
    json!({ "messages": [{ "role": "user", "content": PROMPT }] })
}

fn strip_jev(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.remove("jev");
    }
    v
}

fn system_text(forwarded: &Value) -> String {
    forwarded["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "system")
        .map(|m| m["content"].as_str().unwrap_or("").to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

// --- 1. fallback equivalence -------------------------------------------------

#[tokio::test]
async fn every_fallback_equals_the_pre_change_run() {
    let baseline = run(None, chat()).await;
    assert_eq!(baseline.status, StatusCode::OK);
    assert!(
        system_text(&baseline.forwarded).contains("Knowledge Graph"),
        "scaffold engaged in baseline"
    );

    // (name, config). Each must reproduce `baseline` exactly.
    let scenarios: Vec<(&str, JevConfig, Option<Arc<MockTransport>>)> = {
        let mut v: Vec<(&str, JevConfig, Option<Arc<MockTransport>>)> = Vec::new();

        let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_PASSTHROUGH, 1.0)));
        let mut off = cfg(&t);
        off.enabled = false;
        v.push(("flag-off", off, Some(t)));

        let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_PASSTHROUGH, 1.0)));
        let mut none = cfg(&t);
        none.endpoint = Err(Fallback::NoEndpoint);
        v.push(("no-endpoint", none, Some(t)));

        let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_PASSTHROUGH, 1.0)));
        let mut refused = cfg(&t);
        refused.endpoint = Err(Fallback::NonLocalRefused);
        v.push(("non-local-refused", refused, Some(t)));

        for (name, err) in [
            ("transport", Fallback::Transport),
            ("timeout", Fallback::Timeout),
            ("http-500", Fallback::Http(500)),
            ("http-429", Fallback::Http(429)),
            ("malformed-from-transport", Fallback::Malformed),
        ] {
            let t = MockTransport::new(move |_| Err(err.clone()));
            let mut c = cfg(&t);
            c.gate_min_confidence = Some(0.5);
            v.push((name, c, Some(t)));
        }

        // Well-formed JSON, wrong shape: missing expected answer IDs.
        let t = MockTransport::new(|_| {
            Ok(json!({"model":"m","usage":{},"answers":{"other":{"type":"noul","noul":0.9}}}))
        });
        let mut c = cfg(&t);
        c.gate_min_confidence = Some(0.5);
        v.push(("missing-answer-ids", c, Some(t)));

        // Choice key outside the offered set.
        let t = MockTransport::new(|_| Ok(gate_answer("not-a-route", 1.0)));
        let mut c = cfg(&t);
        c.gate_min_confidence = Some(0.5);
        v.push(("choice-outside-set", c, Some(t)));

        // Low confidence against a configured acceptance threshold.
        let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_PASSTHROUGH, 0.2)));
        let mut c = cfg(&t);
        c.gate_min_confidence = Some(0.8);
        c.rank = false;
        c.support = false;
        v.push(("low-confidence", c, Some(t)));

        // Default threshold ("accept nothing"): a confident passthrough verdict
        // is recorded but never withholds.
        let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_PASSTHROUGH, 1.0)));
        let mut c = cfg(&t);
        c.rank = false;
        c.support = false;
        v.push(("default-threshold-accepts-nothing", c, Some(t)));
        v
    };

    for (name, config, t) in scenarios {
        let got = run(Some(config), chat()).await;
        assert_eq!(got.status, baseline.status, "{name}: status");
        assert_eq!(
            got.forwarded, baseline.forwarded,
            "{name}: backend request must be identical"
        );
        // The default-threshold case may add an annotation; nothing else may differ.
        assert_eq!(
            strip_jev(got.body.clone()),
            baseline.body,
            "{name}: response"
        );
        if name != "default-threshold-accepts-nothing" {
            assert!(
                got.body.get("jev").is_none(),
                "{name}: no annotation on a fallback"
            );
        }
        if name == "flag-off" || name == "no-endpoint" || name == "non-local-refused" {
            assert!(
                t.unwrap().calls().is_empty(),
                "{name}: nothing was built or sent"
            );
        }
    }
}

#[tokio::test]
async fn low_confidence_rank_and_support_fall_back_too() {
    let baseline = run(None, chat()).await;
    let t = MockTransport::new(|req| {
        // Reverse ranking with LOW confidence, and a support answer out of range.
        let mut a = answer_for(req, &|i| 3.0 - i as f64, 0.9);
        if let Some(m) = a["answers"].as_object_mut() {
            for v in m.values_mut() {
                if v["type"] == "score" {
                    v["confidence"] = json!(0.1);
                }
                if v["type"] == "noul" {
                    v["noul"] = json!(7.0); // out of range ⇒ malformed
                }
            }
        }
        Ok(a)
    });
    let mut c = cfg(&t);
    c.gate = false;
    c.rank_min_confidence = 0.5;
    let got = run(Some(c), chat()).await;
    assert_eq!(got.forwarded, baseline.forwarded);
    assert_eq!(got.body, baseline.body);
    assert!(
        !t.calls().is_empty(),
        "the rank and support questions were asked"
    );
}

// --- 2. wire shape -----------------------------------------------------------

#[tokio::test]
async fn gate_request_is_a_valid_system_one_body() {
    let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_GROUNDED, 0.9)));
    let mut c = cfg(&t);
    c.rank = false;
    c.support = false;
    let _ = run(Some(c), chat()).await;
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    let r = &calls[0];
    assert_eq!(r["model"], "jev-test");
    assert!(r["state"]["question"]
        .as_str()
        .unwrap()
        .contains("knowledge graph"));
    assert!(r["state"]["candidate_records"].as_object().unwrap().len() >= 2);
    assert!(
        r["questions"]["route"]["criteria"].is_object(),
        "Choice criteria is a map"
    );
    assert!(
        r["questions"]["answerable"]["criteria"].is_null(),
        "Noul has no criteria here"
    );
    // Only keys the OpenAPI request schema defines at the top level.
    let keys: Vec<_> = r.as_object().unwrap().keys().cloned().collect();
    assert!(
        keys.iter()
            .all(|k| ["model", "state", "questions"].contains(&k.as_str())),
        "{keys:?}"
    );
    assert!(!GATE_QUESTION_SET.is_empty());
}

// --- 3. accept paths ---------------------------------------------------------

#[tokio::test]
async fn confident_passthrough_withholds_the_scaffold_when_a_threshold_is_set() {
    let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_PASSTHROUGH, 0.95)));
    let mut c = cfg(&t);
    c.gate_min_confidence = Some(0.8);
    c.rank = false;
    c.support = false;
    let got = run(Some(c), chat()).await;
    assert_eq!(got.status, StatusCode::OK);
    assert!(
        !system_text(&got.forwarded).contains("Knowledge Graph"),
        "scaffold withheld"
    );
    assert_eq!(
        got.forwarded["messages"].as_array().unwrap().len(),
        1,
        "raw prompt delegated"
    );
    assert_eq!(got.body["jev"]["gate"]["applied"], json!(true));
    assert_eq!(got.body["jev"]["gate"]["route"], json!(ROUTE_PASSTHROUGH));
    assert_eq!(got.body["loom"]["grounding"]["engaged"], json!(false));
}

#[tokio::test]
async fn insufficient_evidence_also_withholds_and_grounded_keeps() {
    let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_INSUFFICIENT, 0.9)));
    let mut c = cfg(&t);
    c.gate_min_confidence = Some(0.5);
    c.rank = false;
    c.support = false;
    let got = run(Some(c), chat()).await;
    assert!(!system_text(&got.forwarded).contains("Knowledge Graph"));

    let baseline = run(None, chat()).await;
    let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_GROUNDED, 0.99)));
    let mut c = cfg(&t);
    c.gate_min_confidence = Some(0.5);
    c.rank = false;
    c.support = false;
    let kept = run(Some(c), chat()).await;
    assert_eq!(
        kept.forwarded, baseline.forwarded,
        "grounded ⇒ scaffold untouched"
    );
}

#[tokio::test]
async fn explicit_scaffold_false_takes_precedence_and_jev_is_never_asked() {
    let t = MockTransport::new(|_| Ok(gate_answer(ROUTE_GROUNDED, 1.0)));
    let c = cfg(&t);
    let req = json!({
        "messages": [{ "role": "user", "content": PROMPT }],
        "loom_options": { "scaffold": false }
    });
    let got = run(Some(c), req).await;
    assert_eq!(got.status, StatusCode::OK);
    assert!(t.calls().is_empty(), "opt-out never reaches Jev");
}

#[tokio::test]
async fn rank_reorders_within_the_same_budget_and_keeps_every_source() {
    let baseline = run(None, chat()).await;
    let base_sys = system_text(&baseline.forwarded);

    // Prefer the LAST-retrieved candidate: the reverse of retrieval order.
    let t = MockTransport::new(|req| Ok(answer_for(req, &|i| 0.5 * i as f64, 0.9)));
    let mut c = cfg(&t);
    c.gate = false;
    c.support = false;
    let got = run(Some(c), chat()).await;
    assert_eq!(got.status, StatusCode::OK);

    let ranked = &got.body["jev"]["rank"];
    assert_eq!(ranked["applied"], json!(true), "reorder kept: {ranked}");
    assert_eq!(
        ranked["order"],
        json!(["graph", "graph-database", "knowledge-graph"])
    );
    let sys = system_text(&got.forwarded);
    assert_ne!(sys, base_sys, "the block is assembled in the new order");
    // Same granted budget, decision and generation as the retrieval gate gave.
    for k in ["effective_budget", "decision"] {
        assert_eq!(
            got.body["loom"]["grounding"][k], baseline.body["loom"]["grounding"][k],
            "{k}"
        );
    }
    // Same sources, now led by the model's preferred record.
    for title in ["Knowledge Graph", "Graph Database"] {
        assert_eq!(sys.contains(title), base_sys.contains(title), "{title}");
    }
    let first = |s: &str| {
        ["Knowledge Graph", "Graph Database", "# Graph"]
            .iter()
            .filter_map(|t| s.find(t).map(|i| (i, *t)))
            .min()
            .map(|(_, t)| t)
    };
    assert_ne!(first(&sys), first(&base_sys), "the lead record changed");
    // The fan-out is one ordered-list Score per record.
    let reqs = t.calls();
    assert_eq!(reqs[0]["questions"].as_object().unwrap().len(), 3);
    assert!(reqs[0]["questions"]["rel_0"]["criteria"].is_array());
}

#[tokio::test]
async fn support_annotates_without_touching_the_answer() {
    let baseline = run(None, chat()).await;
    let t = MockTransport::new(|req| Ok(answer_for(req, &|_| 1.0, 0.12)));
    let mut c = cfg(&t);
    c.gate = false;
    c.rank = false;
    let got = run(Some(c), chat()).await;
    assert_eq!(got.forwarded, baseline.forwarded);
    assert_eq!(
        got.body["choices"], baseline.body["choices"],
        "answer text untouched"
    );
    assert_eq!(
        got.body["jev"]["support"]["flagged_unsupported"],
        json!(true)
    );
    assert!(
        (got.body["jev"]["support"]["supported_probability"]
            .as_f64()
            .unwrap()
            - 0.12)
            .abs()
            < 1e-9
    );
    assert!(
        got.body["jev"]["support"]["source_ids"]
            .as_array()
            .unwrap()
            .len()
            >= 2
    );
    // The check saw the exact answer text.
    let sent = &t.calls()[0];
    assert_eq!(
        sent["state"]["answer"],
        json!("A knowledge graph stores entities.")
    );
}

// --- the real transport ------------------------------------------------------

fn ep(server: &MockServer, bearer_env: Option<&str>) -> Endpoint {
    Endpoint {
        url: format!("{}/v1/systemone", server.uri()),
        model: "jev-test".into(),
        bearer_env: bearer_env.map(str::to_owned),
    }
}

#[tokio::test]
async fn reqwest_transport_maps_each_failure_to_its_reason() {
    let t = ReqwestTransport::default();
    let body =
        json!({"model":"m","state":"s","questions":{"q":{"type":"noul","instructions":"i"}}});

    let ok = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers":{}})))
        .expect(1)
        .mount(&ok)
        .await;
    assert_eq!(
        t.post(&ep(&ok, None), &body, Duration::from_secs(2)).await,
        Ok(json!({"answers":{}}))
    );
    // No Authorization header without a bearer env (loopback proxy posture).
    assert!(ok.received_requests().await.unwrap()[0]
        .headers
        .get("authorization")
        .is_none());

    for (code, want) in [
        (500u16, Fallback::Http(500)),
        (429, Fallback::Http(429)),
        (529, Fallback::Http(529)),
        (401, Fallback::Http(401)),
    ] {
        let s = MockServer::start().await;
        // expect(1): a 429/529 is NOT retried on the interactive path.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(code))
            .expect(1)
            .mount(&s)
            .await;
        assert_eq!(
            t.post(&ep(&s, None), &body, Duration::from_secs(2)).await,
            Err(want)
        );
    }

    let junk = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&junk)
        .await;
    assert_eq!(
        t.post(&ep(&junk, None), &body, Duration::from_secs(2))
            .await,
        Err(Fallback::Malformed)
    );

    let slow = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({}))
                .set_delay(Duration::from_millis(600)),
        )
        .mount(&slow)
        .await;
    assert_eq!(
        t.post(&ep(&slow, None), &body, Duration::from_millis(100))
            .await,
        Err(Fallback::Timeout)
    );

    // Connection refused ⇒ transport.
    let dead = Endpoint {
        url: "http://127.0.0.1:9/v1/systemone".into(),
        model: "m".into(),
        bearer_env: None,
    };
    assert_eq!(
        t.post(&dead, &body, Duration::from_secs(2)).await,
        Err(Fallback::Transport)
    );
}

#[tokio::test]
async fn reqwest_transport_sends_bearer_only_when_configured_and_present() {
    let t = ReqwestTransport::default();
    let body = json!({"model":"m","state":"s","questions":{}});
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header_exists("authorization"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"answers":{}})))
        .mount(&s)
        .await;
    std::env::set_var("LOOM_JEV_TEST_KEY_PRESENT", "k-123");
    assert!(t
        .post(
            &ep(&s, Some("LOOM_JEV_TEST_KEY_PRESENT")),
            &body,
            Duration::from_secs(2)
        )
        .await
        .is_ok());
    let auth = s.received_requests().await.unwrap()[0]
        .headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(auth, "Bearer k-123");
    // A named-but-unset key ⇒ no request at all, reason no-endpoint.
    let before = s.received_requests().await.unwrap().len();
    assert_eq!(
        t.post(
            &ep(&s, Some("LOOM_JEV_TEST_KEY_ABSENT")),
            &body,
            Duration::from_secs(2)
        )
        .await,
        Err(Fallback::NoEndpoint)
    );
    assert_eq!(s.received_requests().await.unwrap().len(), before);
}

/// A hung transport cannot hold the request past the configured deadline.
#[tokio::test]
async fn a_transport_that_ignores_its_deadline_is_still_cut_off() {
    struct Hang;
    #[async_trait]
    impl JevTransport for Hang {
        async fn post(&self, _: &Endpoint, _: &Value, _: Duration) -> Result<Value, Fallback> {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Ok(json!({}))
        }
    }
    let baseline = run(None, chat()).await;
    let mut c = JevConfig {
        enabled: true,
        endpoint: endpoint(),
        transport: TransportHandle(Arc::new(Hang)),
        ..JevConfig::default()
    };
    c.timeout = Duration::from_millis(150);
    c.gate_min_confidence = Some(0.1);
    let started = std::time::Instant::now();
    let got = run(Some(c), chat()).await;
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(got.forwarded, baseline.forwarded);
    assert_eq!(got.body, baseline.body);
}
