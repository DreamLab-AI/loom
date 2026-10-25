//! The **Jev option** for the façade (P1: LM-01-S1-01 intent/answerability gate,
//! LM-02-S1-01 evidence relevance and source-support scoring).
//!
//! An opt-in typed-decision step (System One `POST /v1/systemone`: Choice,
//! Score, Noul) that sits *beside* the existing chat path. With it off, or
//! unavailable for any reason, the code path and the response are exactly the
//! pre-change ones: every failure collapses to a [`Fallback`] reason, one log
//! line carrying only that reason, and the caller keeps what it already had.
//!
//! # Authority
//!
//! Jev never retrieves, never writes, never changes visibility or generation
//! identity:
//!
//! - **Gate** (`LOOM_JEV_GATE`): may only *withhold* an already-engaged scaffold
//!   (turning it into the same no-match scaffold the fusion pipeline returns on
//!   a miss). It runs only after retrieval, the corpus-generation guard and the
//!   `loom_options.scaffold = false` opt-out, all of which therefore take
//!   precedence. Withholding changes an outcome, so it is **inert until
//!   `LOOM_JEV_GATE_MIN_CONFIDENCE` is set**.
//! - **Rank** (`LOOM_JEV_RANK`): may only *reorder* the candidates and
//!   re-assemble them through the same gate (`LexicalIndex::assemble`). The
//!   re-assembly is kept only if the granted budget, the injection decision and
//!   the generation are unchanged; otherwise the original scaffold stands.
//! - **Support** (`LOOM_JEV_SUPPORT`): annotation only. Adds a `jev.support`
//!   block to the delegated answer; the answer text is never touched.
//!
//! # Data boundary
//!
//! The façade serves private corpus content, so it is a *local-only* consumer:
//! an endpoint that is not loopback/private/single-label is refused unless
//! `LOOM_JEV_ALLOW_CLOUD=1`. The agentbox loopback proxy is likewise refused
//! unless `AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL=1` (or `allow_cloud`). The
//! TypeSafe cloud default needs `allow_cloud` *and* `TYPESAFE_API_KEY`.
//!
//! This module performs no network I/O of its own: the transport is the
//! [`JevTransport`] trait, defaulting to [`ReqwestTransport`] over the HTTP
//! client the façade already depends on.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use loom_domain::{
    ConceptMatch, FusionPath, Grounding, InjectionDecision, LexicalIndex, Scaffold, ScaffoldOpts,
};

/// Version string of the gate question set (logged with every call).
pub const GATE_QUESTION_SET: &str = "loom.lm-01-s1-01.v1";
/// Version string of the evidence rank question set.
pub const RANK_QUESTION_SET: &str = "loom.lm-02-s1-01.rank.v1";
/// Version string of the answer-support question set.
pub const SUPPORT_QUESTION_SET: &str = "loom.lm-02-s1-01.support.v1";

/// Public System One URL, step 3 of endpoint resolution.
pub const TYPESAFE_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
/// Model used with the public endpoint and as the default model name.
pub const DEFAULT_MODEL: &str = "jev-latest";
/// Hard deadline for an interactive call. No retries on this path.
pub const DEFAULT_TIMEOUT_MS: u64 = 4000;

/// Maximum candidate records sent in one request.
const MAX_RECORDS: usize = 8;
/// Characters of a record's definition sent as evidence.
const RECORD_CHARS: usize = 400;
/// Characters of the served block / answer sent to the support check.
const SUPPORT_CHARS: usize = 6000;

/// The four routes of the gate's Choice question.
pub const ROUTE_GROUNDED: &str = "grounded_lookup";
/// Request mixes corpus lookup with other work.
pub const ROUTE_MIXED: &str = "mixed_request";
/// Request is not about the corpus at all.
pub const ROUTE_PASSTHROUGH: &str = "ordinary_passthrough";
/// Request is about the corpus but the candidates do not cover it.
pub const ROUTE_INSUFFICIENT: &str = "insufficient_evidence";

// --- fallback reasons --------------------------------------------------------

/// Why a Jev call produced nothing. Every variant means "use the pre-change
/// behaviour"; the string forms are the only thing ever logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// The master flag (or the feature's own flag) is off.
    FlagOff,
    /// No endpoint resolved.
    NoEndpoint,
    /// An endpoint resolved but is not local and `allow_cloud` is unset.
    NonLocalRefused,
    /// The deadline elapsed.
    Timeout,
    /// The request could not be sent or the connection failed.
    Transport,
    /// A non-2xx status.
    Http(u16),
    /// Body unparsable, expected answer IDs/keys missing, or a Choice key
    /// outside the offered set.
    Malformed,
    /// Confidence below the configured acceptance threshold.
    LowConfidence,
    /// The post-condition check on a re-assembly failed (rank only).
    InvariantBroken,
    /// Nothing to decide (too few candidates, empty answer).
    NotApplicable,
}

impl Fallback {
    /// The log token for this reason. Never contains request content.
    #[must_use]
    pub fn as_str(&self) -> String {
        match self {
            Self::FlagOff => "flag-off".to_owned(),
            Self::NoEndpoint => "no-endpoint".to_owned(),
            Self::NonLocalRefused => "non-local-refused".to_owned(),
            Self::Timeout => "timeout".to_owned(),
            Self::Transport => "transport".to_owned(),
            Self::Http(c) => format!("http-{c}"),
            Self::Malformed => "malformed".to_owned(),
            Self::LowConfidence => "low-confidence".to_owned(),
            Self::InvariantBroken => "invariant-broken".to_owned(),
            Self::NotApplicable => "not-applicable".to_owned(),
        }
    }
}

fn log_fallback(stage: &str, set: &str, why: &Fallback) {
    // One line, reason only: no query, no evidence, no answer text.
    tracing::info!(stage, question_set = set, reason = %why.as_str(), "jev fallback");
}

// --- endpoint ----------------------------------------------------------------

/// A resolved System One endpoint.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Full URL of the `/v1/systemone` route.
    pub url: String,
    /// Model name sent in the request.
    pub model: String,
    /// Name of the env var holding a bearer token; `None` sends no
    /// `Authorization` header (explicit local endpoints and the agentbox proxy).
    pub bearer_env: Option<String>,
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The URL may embed a token in a query string; show the host only.
        f.debug_struct("Endpoint")
            .field("host", &host_of(&self.url))
            .field("model", &self.model)
            .field("bearer_env", &self.bearer_env)
            .finish()
    }
}

fn host_of(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
}

/// Whether `url` points somewhere private: loopback, RFC 1918 / link-local /
/// unique-local, `.local`/`.internal`/`.lan`, or a single-label name (a compose
/// service such as `loom` or `systemone`).
#[must_use]
pub fn is_local_url(url: &str) -> bool {
    use std::net::IpAddr;
    let Ok(u) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = u.host_str() else {
        return false;
    };
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        };
    }
    let d = bare.trim_end_matches('.').to_ascii_lowercase();
    d == "localhost"
        || !d.contains('.')
        || d.ends_with(".local")
        || d.ends_with(".internal")
        || d.ends_with(".lan")
        || d.ends_with(".localhost")
}

fn truthy(v: Option<String>) -> bool {
    matches!(v.as_deref(), Some(s) if !matches!(s, "" | "0" | "false" | "no"))
}

/// Resolve the endpoint (contract §2, first match wins) for a **local-only**
/// consumer. `get` reads an environment variable (injected so tests do not
/// mutate the process environment).
///
/// # Errors
/// [`Fallback::NoEndpoint`] when nothing resolves, [`Fallback::NonLocalRefused`]
/// when the first matching step is not local and `allow_cloud` is unset.
pub fn resolve_endpoint(
    get: &dyn Fn(&str) -> Option<String>,
    allow_cloud: bool,
) -> Result<Endpoint, Fallback> {
    let nonempty = |k: &str| get(k).filter(|v| !v.trim().is_empty());
    let model = nonempty("LOOM_JEV_MODEL");

    // 1. explicit config
    if let Some(url) = nonempty("LOOM_JEV_ENDPOINT") {
        let declared_local = truthy(get("LOOM_JEV_LOCAL"));
        if !(declared_local || is_local_url(&url) || allow_cloud) {
            return Err(Fallback::NonLocalRefused);
        }
        return Ok(Endpoint {
            url,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_owned()),
            bearer_env: nonempty("LOOM_JEV_KEY_ENV"),
        });
    }
    // 2. agentbox loopback proxy (holds the vendor key: no Authorization)
    if truthy(get("AGENTBOX_SYSTEM_ONE_PROXY")) {
        if let Some(url) = nonempty("AGENTBOX_SYSTEM_ONE_ENDPOINT") {
            if !(truthy(get("AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL")) || allow_cloud) {
                return Err(Fallback::NonLocalRefused);
            }
            return Ok(Endpoint {
                url,
                model: nonempty("AGENTBOX_SYSTEM_ONE_MODEL")
                    .or(model)
                    .unwrap_or_else(|| DEFAULT_MODEL.to_owned()),
                bearer_env: None,
            });
        }
    }
    // 3. public endpoint, only with a key, and for us only with allow_cloud
    if nonempty("TYPESAFE_API_KEY").is_some() {
        if !allow_cloud {
            return Err(Fallback::NonLocalRefused);
        }
        return Ok(Endpoint {
            url: TYPESAFE_ENDPOINT.to_owned(),
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_owned()),
            bearer_env: Some("TYPESAFE_API_KEY".to_owned()),
        });
    }
    Err(Fallback::NoEndpoint)
}

// --- transport ---------------------------------------------------------------

/// The one network seam. A test supplies a mock; production uses
/// [`ReqwestTransport`].
#[async_trait]
pub trait JevTransport: Send + Sync {
    /// POST `body` to `endpoint`, returning the parsed JSON body of a 2xx.
    ///
    /// # Errors
    /// A [`Fallback`] for a timeout, transport error, non-2xx or unparsable body.
    async fn post(
        &self,
        endpoint: &Endpoint,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value, Fallback>;
}

/// Default transport: one attempt, no retries, a hard deadline.
#[derive(Clone, Debug, Default)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[async_trait]
impl JevTransport for ReqwestTransport {
    async fn post(
        &self,
        endpoint: &Endpoint,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value, Fallback> {
        let mut req = self.client.post(&endpoint.url).timeout(timeout).json(body);
        if let Some(var) = &endpoint.bearer_env {
            match std::env::var(var) {
                Ok(k) if !k.is_empty() => req = req.bearer_auth(k),
                _ => return Err(Fallback::NoEndpoint),
            }
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                Fallback::Timeout
            } else {
                Fallback::Transport
            }
        })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Fallback::Http(status.as_u16()));
        }
        resp.json::<Value>().await.map_err(|e| {
            if e.is_timeout() {
                Fallback::Timeout
            } else {
                Fallback::Malformed
            }
        })
    }
}

/// `Debug`-able, cloneable handle to a transport, so [`JevConfig`] stays a plain
/// `Config` field.
#[derive(Clone)]
pub struct TransportHandle(pub Arc<dyn JevTransport>);

impl fmt::Debug for TransportHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TransportHandle(..)")
    }
}

impl Default for TransportHandle {
    fn default() -> Self {
        Self(Arc::new(ReqwestTransport::default()))
    }
}

// --- configuration -----------------------------------------------------------

/// All Jev settings. `Default` is **off**.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct JevConfig {
    /// `LOOM_JEV` (0): master switch. Off ⇒ no request is ever built.
    pub enabled: bool,
    /// `LOOM_JEV_GATE`: LM-01 gate; on by default once the master switch is on.
    pub gate: bool,
    /// `LOOM_JEV_RANK`: LM-02 rank; on by default once the master switch is on.
    pub rank: bool,
    /// `LOOM_JEV_SUPPORT`: LM-02 support annotation; on by default once enabled.
    pub support: bool,
    /// Resolved endpoint, or the reason there is none.
    pub endpoint: Result<Endpoint, Fallback>,
    /// `LOOM_JEV_TIMEOUT_MS` (4000).
    pub timeout: Duration,
    /// `LOOM_JEV_GATE_MIN_CONFIDENCE`. `None` (default) ⇒ the gate **never
    /// withholds**: it records its decision as an annotation only.
    pub gate_min_confidence: Option<f64>,
    /// `LOOM_JEV_RANK_MIN_CONFIDENCE` (0.0, untuned): the least per-record Score
    /// confidence at which a reorder is accepted. Reorder is outcome-neutral on
    /// the budget, so the default accepts any well-formed answer.
    pub rank_min_confidence: f64,
    /// `LOOM_JEV_SUPPORT_THRESHOLD` (0.5): below this Noul probability the
    /// annotation flags the answer as not supported.
    pub support_threshold: f64,
    /// Transport; replace in tests.
    pub transport: TransportHandle,
}

impl Default for JevConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gate: true,
            rank: true,
            support: true,
            endpoint: Err(Fallback::FlagOff),
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            gate_min_confidence: None,
            rank_min_confidence: 0.0,
            support_threshold: 0.5,
            transport: TransportHandle::default(),
        }
    }
}

impl JevConfig {
    /// Read from an environment accessor. Off unless `LOOM_JEV` is truthy.
    #[must_use]
    pub fn from_lookup(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let d = Self::default();
        let enabled = truthy(get("LOOM_JEV"));
        if !enabled {
            return d;
        }
        let flag = |k: &str, dv: bool| match get(k) {
            None => dv,
            Some(v) => truthy(Some(v)),
        };
        let num = |k: &str| get(k).and_then(|v| v.parse::<f64>().ok());
        let allow_cloud = truthy(get("LOOM_JEV_ALLOW_CLOUD"));
        Self {
            enabled,
            gate: flag("LOOM_JEV_GATE", d.gate),
            rank: flag("LOOM_JEV_RANK", d.rank),
            support: flag("LOOM_JEV_SUPPORT", d.support),
            endpoint: resolve_endpoint(get, allow_cloud),
            timeout: get("LOOM_JEV_TIMEOUT_MS")
                .and_then(|v| v.parse::<u64>().ok())
                .map_or(d.timeout, Duration::from_millis),
            gate_min_confidence: num("LOOM_JEV_GATE_MIN_CONFIDENCE"),
            rank_min_confidence: num("LOOM_JEV_RANK_MIN_CONFIDENCE").unwrap_or(0.0),
            support_threshold: num("LOOM_JEV_SUPPORT_THRESHOLD").unwrap_or(0.5),
            transport: d.transport,
        }
    }

    /// Read from the process environment.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_lookup(&|k| std::env::var(k).ok())
    }

    fn ready(&self, feature_on: bool) -> Result<&Endpoint, Fallback> {
        if !self.enabled || !feature_on {
            return Err(Fallback::FlagOff);
        }
        self.endpoint.as_ref().map_err(Clone::clone)
    }
}

// --- wire: request builders --------------------------------------------------

/// A record offered to the model as evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// Stable source identifier (the class slug).
    pub id: String,
    /// Unit title.
    pub title: String,
    /// Definition excerpt, truncated to a bounded length.
    pub excerpt: String,
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Resolve seeds to records through the retriever's own `resolve` (the only
/// address→content path), preserving seed order and capping the count.
#[must_use]
pub fn records_for(retriever: &dyn LexicalIndex, seeds: &[ConceptMatch]) -> Vec<Record> {
    seeds
        .iter()
        .filter_map(|m| {
            retriever.resolve(&m.iri).map(|u| Record {
                id: m.iri.slug().to_owned(),
                title: u.title,
                excerpt: truncate_chars(&u.definition, RECORD_CHARS),
            })
        })
        .take(MAX_RECORDS)
        .collect()
}

fn records_state(records: &[Record]) -> Value {
    let mut m = Map::new();
    for (i, r) in records.iter().enumerate() {
        m.insert(
            format!("r{i}"),
            json!({ "source_id": r.id, "title": r.title, "excerpt": r.excerpt }),
        );
    }
    Value::Object(m)
}

/// LM-01: build the gate request (one Choice, one Noul over shared state).
#[must_use]
pub fn build_gate_request(model: &str, query: &str, records: &[Record]) -> Value {
    json!({
        "model": model,
        "state": {
            "question": query,
            "corpus": "A curated ontology of defined technical concepts; each record is one concept with a title and a short definition.",
            "candidate_records": records_state(records),
        },
        "questions": {
            "route": {
                "type": "choice",
                "instructions": "Decide how the user's `question` relates to the corpus described in `corpus`, given the `candidate_records` that keyword retrieval returned. Pick the single best route.",
                "criteria": {
                    ROUTE_GROUNDED: "The question asks what the corpus says about a concept, and at least one candidate record is about that concept.",
                    ROUTE_MIXED: "The question combines a corpus lookup with other work (writing, coding, reasoning) and at least one candidate record is relevant to the lookup part.",
                    ROUTE_PASSTHROUGH: "The question is not about the corpus's concepts at all; the candidate records matched on incidental words only.",
                    ROUTE_INSUFFICIENT: "The question is about the corpus's subject area, but none of the candidate records contains what is needed to answer it."
                }
            },
            "answerable": {
                "type": "noul",
                "instructions": "The `candidate_records` together contain enough information to answer the user's `question`."
            }
        }
    })
}

/// LM-02: one Score per record (fan-out over shared state).
#[must_use]
pub fn build_rank_request(model: &str, query: &str, records: &[Record]) -> Value {
    let mut questions = Map::new();
    for i in 0..records.len() {
        questions.insert(
            format!("rel_{i}"),
            json!({
                "type": "score",
                "instructions": format!(
                    "How directly does the excerpt of record `r{i}` in `records` help answer the user's `question`? Judge only that record's own title and excerpt."
                ),
                "criteria": [
                    "Unrelated to the question, or misleading for it",
                    "Related background that does not help answer the question",
                    "Partly answers the question or supplies a needed definition",
                    "Directly answers the question"
                ]
            }),
        );
    }
    json!({
        "model": model,
        "state": { "question": query, "records": records_state(records) },
        "questions": Value::Object(questions),
    })
}

/// LM-02: one Noul — are the answer's claims supported by the served evidence?
#[must_use]
pub fn build_support_request(model: &str, query: &str, evidence: &str, answer: &str) -> Value {
    json!({
        "model": model,
        "state": {
            "question": query,
            "evidence": truncate_chars(evidence, SUPPORT_CHARS),
            "answer": truncate_chars(answer, SUPPORT_CHARS),
        },
        "questions": {
            "supported": {
                "type": "noul",
                "instructions": "Every factual claim made in `answer` is stated by, or follows directly from, the text in `evidence`. Claims that need outside knowledge make this false."
            }
        }
    })
}

// --- wire: response parsing --------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Answer {
    Choice { choice: String, confidence: f64 },
    Score { score: f64, confidence: f64 },
    Noul { noul: f64 },
}

#[derive(Debug, Deserialize)]
struct WireResponse {
    answers: HashMap<String, Answer>,
}

fn unit(x: f64) -> bool {
    x.is_finite() && (0.0..=1.0).contains(&x)
}

fn parse_answers(v: Value) -> Result<HashMap<String, Answer>, Fallback> {
    let r: WireResponse = serde_json::from_value(v).map_err(|_| Fallback::Malformed)?;
    Ok(r.answers)
}

/// The gate's parsed verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct GateVerdict {
    /// One of the `ROUTE_*` keys.
    pub route: String,
    /// System One confidence of the Choice, in `[0, 1]`.
    pub confidence: f64,
    /// Probability that the candidates answer the question.
    pub answerable: f64,
}

/// Parse a gate response.
///
/// # Errors
/// [`Fallback::Malformed`] on missing IDs, wrong answer types, a route outside
/// the offered set, or out-of-range numbers.
pub fn parse_gate(v: Value) -> Result<GateVerdict, Fallback> {
    let mut a = parse_answers(v)?;
    let (Some(Answer::Choice { choice, confidence }), Some(Answer::Noul { noul })) =
        (a.remove("route"), a.remove("answerable"))
    else {
        return Err(Fallback::Malformed);
    };
    let offered = [
        ROUTE_GROUNDED,
        ROUTE_MIXED,
        ROUTE_PASSTHROUGH,
        ROUTE_INSUFFICIENT,
    ];
    if !offered.contains(&choice.as_str()) || !unit(confidence) || !unit(noul) {
        return Err(Fallback::Malformed);
    }
    Ok(GateVerdict {
        route: choice,
        confidence,
        answerable: noul,
    })
}

/// Parse a rank response into `(score, confidence)` per record, in record order.
///
/// # Errors
/// [`Fallback::Malformed`] unless every `rel_i` is present and well-formed.
pub fn parse_rank(v: Value, n: usize) -> Result<Vec<(f64, f64)>, Fallback> {
    let mut a = parse_answers(v)?;
    (0..n)
        .map(|i| match a.remove(&format!("rel_{i}")) {
            Some(Answer::Score { score, confidence })
                if score.is_finite() && (0.0..=3.0).contains(&score) && unit(confidence) =>
            {
                Ok((score, confidence))
            }
            _ => Err(Fallback::Malformed),
        })
        .collect()
}

/// Parse a support response into the probability that the answer is supported.
///
/// # Errors
/// [`Fallback::Malformed`] if `supported` is absent, mistyped or out of range.
pub fn parse_support(v: Value) -> Result<f64, Fallback> {
    match parse_answers(v)?.remove("supported") {
        Some(Answer::Noul { noul }) if unit(noul) => Ok(noul),
        _ => Err(Fallback::Malformed),
    }
}

// --- decisions (pure) --------------------------------------------------------

/// What the gate does with a verdict. Pure, so it can be tested exhaustively.
#[derive(Clone, Debug, PartialEq)]
pub enum GateAction {
    /// Keep the scaffold exactly as retrieval built it.
    Keep,
    /// Withhold the scaffold (same shape as a fusion no-match).
    Withhold,
}

/// Decide from a verdict. Only a route of passthrough or insufficient evidence,
/// at or above `min_confidence`, withholds; with `min_confidence = None` nothing
/// is ever withheld.
#[must_use]
pub fn gate_action(v: &GateVerdict, min_confidence: Option<f64>) -> GateAction {
    let Some(min) = min_confidence else {
        return GateAction::Keep;
    };
    let against = v.route == ROUTE_PASSTHROUGH || v.route == ROUTE_INSUFFICIENT;
    if against && v.confidence >= min {
        GateAction::Withhold
    } else {
        GateAction::Keep
    }
}

/// Stable descending order by Jev score (ties keep retrieval order). Returns the
/// permutation of original indices, or `None` if it equals the identity.
#[must_use]
pub fn rank_order(scores: &[(f64, f64)]) -> Option<Vec<usize>> {
    let mut idx: Vec<usize> = (0..scores.len()).collect();
    idx.sort_by(|&a, &b| {
        scores[b]
            .0
            .partial_cmp(&scores[a].0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if idx.iter().enumerate().all(|(i, &j)| i == j) {
        None
    } else {
        Some(idx)
    }
}

// --- orchestration -----------------------------------------------------------

/// What the Jev stages did for one request, for the optional `jev` response key.
#[derive(Clone, Debug, Default)]
pub struct JevReport {
    gate: Option<Value>,
    rank: Option<Value>,
    support: Option<Value>,
}

impl JevReport {
    /// The `jev` JSON block, or `None` if nothing was computed (so a fallback
    /// leaves the response byte-identical).
    #[must_use]
    pub fn to_json(&self) -> Option<Value> {
        if self.gate.is_none() && self.rank.is_none() && self.support.is_none() {
            return None;
        }
        let mut m = Map::new();
        for (k, v) in [
            ("gate", &self.gate),
            ("rank", &self.rank),
            ("support", &self.support),
        ] {
            if let Some(v) = v {
                m.insert(k.to_owned(), v.clone());
            }
        }
        Some(Value::Object(m))
    }
}

async fn call(cfg: &JevConfig, ep: &Endpoint, body: &Value) -> Result<Value, Fallback> {
    // Belt and braces: the transport is also given the deadline, but an
    // implementation that ignores it cannot hold the request past it.
    match tokio::time::timeout(cfg.timeout, cfg.transport.0.post(ep, body, cfg.timeout)).await {
        Ok(r) => r,
        Err(_) => Err(Fallback::Timeout),
    }
}

/// Apply the gate (LM-01) to an engaged scaffold.
///
/// Returns the scaffold to use: the input unchanged on any fallback or `Keep`,
/// or the fusion-style no-match scaffold on `Withhold`.
pub async fn apply_gate(
    cfg: &JevConfig,
    retriever: &dyn LexicalIndex,
    min_inject_score: f64,
    query: &str,
    scaffold: Scaffold,
    report: &mut JevReport,
) -> Scaffold {
    if !scaffold.engaged || scaffold.block.is_empty() {
        return scaffold;
    }
    let ep = match cfg.ready(cfg.gate) {
        Ok(ep) => ep,
        Err(why) => {
            if why != Fallback::FlagOff {
                log_fallback("gate", GATE_QUESTION_SET, &why);
            }
            return scaffold;
        }
    };
    let records = records_for(retriever, &scaffold.seeds);
    if records.is_empty() {
        return scaffold;
    }
    let req = build_gate_request(&ep.model, query, &records);
    let verdict = match call(cfg, ep, &req).await.and_then(parse_gate) {
        Ok(v) => v,
        Err(why) => {
            log_fallback("gate", GATE_QUESTION_SET, &why);
            return scaffold;
        }
    };
    let action = gate_action(&verdict, cfg.gate_min_confidence);
    let withheld = action == GateAction::Withhold;
    // Below a configured threshold the verdict is a low-confidence fallback:
    // nothing changes and nothing is annotated, so the response is identical.
    if let Some(min) = cfg.gate_min_confidence {
        if verdict.confidence < min {
            log_fallback("gate", GATE_QUESTION_SET, &Fallback::LowConfidence);
            return scaffold;
        }
    }
    report.gate = Some(json!({
        "question_set": GATE_QUESTION_SET,
        "route": verdict.route,
        "confidence": verdict.confidence,
        "answerable": verdict.answerable,
        "applied": withheld,
    }));
    if withheld {
        Scaffold::empty_with_grounding(
            FusionPath::NoMatch,
            retriever.generation(),
            Grounding::none(min_inject_score),
        )
    } else {
        scaffold
    }
}

/// Apply evidence ranking (LM-02) to an engaged scaffold: reorder the seeds and
/// re-assemble through the same gate, keeping the result only if the granted
/// budget, the injection decision and the generation are unchanged.
pub async fn apply_rank(
    cfg: &JevConfig,
    retriever: &dyn LexicalIndex,
    query: &str,
    opts: &ScaffoldOpts,
    scaffold: Scaffold,
    report: &mut JevReport,
) -> Scaffold {
    if !scaffold.engaged || scaffold.seeds.len() < 2 {
        return scaffold;
    }
    let ep = match cfg.ready(cfg.rank) {
        Ok(ep) => ep,
        Err(why) => {
            if why != Fallback::FlagOff {
                log_fallback("rank", RANK_QUESTION_SET, &why);
            }
            return scaffold;
        }
    };
    // Rank exactly the records sent: seeds beyond the cap keep their tail order.
    let head: Vec<ConceptMatch> = scaffold.seeds.iter().take(MAX_RECORDS).cloned().collect();
    let records = records_for(retriever, &head);
    if records.len() != head.len() {
        return scaffold; // an unresolved seed: do not guess
    }
    let req = build_rank_request(&ep.model, query, &records);
    let scores = match call(cfg, ep, &req)
        .await
        .and_then(|v| parse_rank(v, records.len()))
    {
        Ok(s) => s,
        Err(why) => {
            log_fallback("rank", RANK_QUESTION_SET, &why);
            return scaffold;
        }
    };
    if scores.iter().any(|(_, c)| *c < cfg.rank_min_confidence) {
        log_fallback("rank", RANK_QUESTION_SET, &Fallback::LowConfidence);
        return scaffold;
    }
    let Some(order) = rank_order(&scores) else {
        report.rank = Some(json!({
            "question_set": RANK_QUESTION_SET, "applied": false, "scores": scores_json(&records, &scores),
        }));
        return scaffold;
    };
    let mut reordered: Vec<ConceptMatch> = order.iter().map(|&i| head[i].clone()).collect();
    reordered.extend(scaffold.seeds.iter().skip(MAX_RECORDS).cloned());

    let candidate = match retriever
        .assemble(
            query,
            &reordered,
            opts.clone().with_path(scaffold.fusion_path),
        )
        .await
    {
        Ok(s) => s,
        Err(_) => {
            log_fallback("rank", RANK_QUESTION_SET, &Fallback::InvariantBroken);
            return scaffold;
        }
    };
    let same_gate = candidate.engaged
        && candidate.effective_budget == scaffold.effective_budget
        && candidate.generation == scaffold.generation
        && decision_of(&candidate) == decision_of(&scaffold);
    if !same_gate {
        log_fallback("rank", RANK_QUESTION_SET, &Fallback::InvariantBroken);
        return scaffold;
    }
    report.rank = Some(json!({
        "question_set": RANK_QUESTION_SET,
        "applied": true,
        "order": order.iter().map(|&i| records[i].id.clone()).collect::<Vec<_>>(),
        "scores": scores_json(&records, &scores),
    }));
    candidate
}

fn decision_of(s: &Scaffold) -> InjectionDecision {
    s.grounding.decision
}

fn scores_json(records: &[Record], scores: &[(f64, f64)]) -> Value {
    Value::Array(
        records
            .iter()
            .zip(scores)
            .map(|(r, (s, c))| json!({ "source_id": r.id, "score": s, "confidence": c }))
            .collect(),
    )
}

/// Annotate a delegated answer (LM-02 support). Never alters the answer.
pub async fn check_support(
    cfg: &JevConfig,
    query: &str,
    scaffold: &Scaffold,
    answer: &str,
    report: &mut JevReport,
) {
    if !scaffold.engaged || scaffold.block.is_empty() || answer.trim().is_empty() {
        return;
    }
    let ep = match cfg.ready(cfg.support) {
        Ok(ep) => ep,
        Err(why) => {
            if why != Fallback::FlagOff {
                log_fallback("support", SUPPORT_QUESTION_SET, &why);
            }
            return;
        }
    };
    let req = build_support_request(&ep.model, query, &scaffold.block, answer);
    match call(cfg, ep, &req).await.and_then(parse_support) {
        Ok(p) => {
            report.support = Some(json!({
                "question_set": SUPPORT_QUESTION_SET,
                "supported_probability": p,
                "flagged_unsupported": p < cfg.support_threshold,
                "source_ids": scaffold.seeds.iter().map(|m| m.iri.slug().to_owned()).collect::<Vec<_>>(),
            }));
        }
        Err(why) => log_fallback("support", SUPPORT_QUESTION_SET, &why),
    }
}

// --- unit tests (pure parts) -------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn default_is_off_and_without_endpoint() {
        let c = JevConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.endpoint, Err(Fallback::FlagOff));
        assert!(c.gate_min_confidence.is_none());
        let from_empty = JevConfig::from_lookup(&env(&[]));
        assert!(!from_empty.enabled);
    }

    #[test]
    fn local_classification() {
        for u in [
            "http://127.0.0.1:8097/v1/systemone",
            "http://localhost:8097",
            "http://192.168.2.132:8084/v1/systemone",
            "http://10.1.2.3/x",
            "http://systemone:8097/v1/systemone",
            "http://[::1]:80/x",
            "http://box.local/x",
        ] {
            assert!(is_local_url(u), "{u}");
        }
        for u in [
            "https://api.typesafe.ai/v1/systemone",
            "http://8.8.8.8/x",
            "http://example.com/x",
            "not a url",
        ] {
            assert!(!is_local_url(u), "{u}");
        }
    }

    #[test]
    fn endpoint_resolution_order_and_local_only() {
        // 1. explicit local wins over everything
        let e = resolve_endpoint(
            &env(&[
                ("LOOM_JEV_ENDPOINT", "http://127.0.0.1:1/v1/systemone"),
                ("TYPESAFE_API_KEY", "k"),
            ]),
            false,
        )
        .unwrap();
        assert_eq!(e.url, "http://127.0.0.1:1/v1/systemone");
        assert_eq!(e.model, DEFAULT_MODEL);
        assert!(e.bearer_env.is_none());
        // explicit non-local refused unless declared local / allow_cloud
        let remote = [("LOOM_JEV_ENDPOINT", "https://s1.example.com/v1/systemone")];
        assert_eq!(
            resolve_endpoint(&env(&remote), false),
            Err(Fallback::NonLocalRefused)
        );
        assert!(resolve_endpoint(&env(&remote), true).is_ok());
        let declared = [
            ("LOOM_JEV_ENDPOINT", "https://s1.example.com/v1/systemone"),
            ("LOOM_JEV_LOCAL", "1"),
        ];
        assert!(resolve_endpoint(&env(&declared), false).is_ok());
        // 2. proxy needs the local declaration and sends no auth
        let proxy = [
            ("AGENTBOX_SYSTEM_ONE_PROXY", "1"),
            (
                "AGENTBOX_SYSTEM_ONE_ENDPOINT",
                "http://127.0.0.1:9/v1/systemone",
            ),
            ("AGENTBOX_SYSTEM_ONE_MODEL", "jev-x"),
        ];
        assert_eq!(
            resolve_endpoint(&env(&proxy), false),
            Err(Fallback::NonLocalRefused)
        );
        let mut local_proxy = proxy.to_vec();
        local_proxy.push(("AGENTBOX_SYSTEM_ONE_BACKEND_LOCAL", "1"));
        let e = resolve_endpoint(&env(&local_proxy), false).unwrap();
        assert_eq!((e.model.as_str(), e.bearer_env), ("jev-x", None));
        // 3. public default: key AND allow_cloud
        let key = [("TYPESAFE_API_KEY", "k")];
        assert_eq!(
            resolve_endpoint(&env(&key), false),
            Err(Fallback::NonLocalRefused)
        );
        let e = resolve_endpoint(&env(&key), true).unwrap();
        assert_eq!(e.url, TYPESAFE_ENDPOINT);
        assert_eq!(e.bearer_env.as_deref(), Some("TYPESAFE_API_KEY"));
        // 4. nothing
        assert_eq!(resolve_endpoint(&env(&[]), true), Err(Fallback::NoEndpoint));
    }

    #[test]
    fn debug_never_prints_url_path_or_key() {
        let e = Endpoint {
            url: "https://h.example/v1/systemone?token=SECRET".into(),
            model: "m".into(),
            bearer_env: Some("TYPESAFE_API_KEY".into()),
        };
        let d = format!("{e:?}");
        assert!(!d.contains("SECRET"));
    }

    fn recs(n: usize) -> Vec<Record> {
        (0..n)
            .map(|i| Record {
                id: format!("c{i}"),
                title: format!("T{i}"),
                excerpt: format!("d{i}"),
            })
            .collect()
    }

    #[test]
    fn gate_request_wire_shape() {
        let r = build_gate_request("jev-latest", "what is x?", &recs(2));
        assert_eq!(r["model"], "jev-latest");
        let route = &r["questions"]["route"];
        assert_eq!(route["type"], "choice");
        // Choice criteria is a MAP including the 'none'-style options.
        let crit = route["criteria"].as_object().expect("map");
        for k in [
            ROUTE_GROUNDED,
            ROUTE_MIXED,
            ROUTE_PASSTHROUGH,
            ROUTE_INSUFFICIENT,
        ] {
            assert!(crit.contains_key(k), "{k}");
        }
        assert_eq!(r["questions"]["answerable"]["type"], "noul");
        assert!(r["questions"]["answerable"].get("criteria").is_none());
        assert_eq!(r["state"]["candidate_records"]["r1"]["source_id"], "c1");
        // Instructions are self-contained (name the state fields they use).
        assert!(route["instructions"]
            .as_str()
            .unwrap()
            .contains("`question`"));
    }

    #[test]
    fn rank_request_wire_shape() {
        let r = build_rank_request("m", "q", &recs(3));
        let qs = r["questions"].as_object().unwrap();
        assert_eq!(qs.len(), 3);
        for i in 0..3 {
            let q = &qs[&format!("rel_{i}")];
            assert_eq!(q["type"], "score");
            // Score criteria is an ORDERED LIST.
            assert_eq!(q["criteria"].as_array().unwrap().len(), 4);
            assert!(q["instructions"]
                .as_str()
                .unwrap()
                .contains(&format!("`r{i}`")));
        }
    }

    #[test]
    fn support_request_is_bounded() {
        let long = "x".repeat(20_000);
        let r = build_support_request("m", "q", &long, &long);
        assert_eq!(
            r["state"]["evidence"].as_str().unwrap().chars().count(),
            SUPPORT_CHARS
        );
        assert_eq!(r["questions"]["supported"]["type"], "noul");
    }

    #[test]
    fn parse_gate_rejects_out_of_set_and_missing() {
        let ok = json!({"model":"m","usage":{},"answers":{
            "route":{"type":"choice","choice":ROUTE_GROUNDED,"confidence":0.8,"probabilities":{}},
            "answerable":{"type":"noul","noul":0.9}}});
        assert_eq!(parse_gate(ok).unwrap().route, ROUTE_GROUNDED);
        let outside = json!({"answers":{
            "route":{"type":"choice","choice":"banana","confidence":0.8},
            "answerable":{"type":"noul","noul":0.9}}});
        assert_eq!(parse_gate(outside), Err(Fallback::Malformed));
        let missing =
            json!({"answers":{"route":{"type":"choice","choice":ROUTE_MIXED,"confidence":0.8}}});
        assert_eq!(parse_gate(missing), Err(Fallback::Malformed));
        let wrong_type = json!({"answers":{
            "route":{"type":"noul","noul":0.5},"answerable":{"type":"noul","noul":0.9}}});
        assert_eq!(parse_gate(wrong_type), Err(Fallback::Malformed));
        assert_eq!(parse_gate(json!("junk")), Err(Fallback::Malformed));
    }

    #[test]
    fn gate_action_is_inert_without_threshold() {
        let v = |route: &str, c: f64| GateVerdict {
            route: route.into(),
            confidence: c,
            answerable: 0.1,
        };
        assert_eq!(
            gate_action(&v(ROUTE_PASSTHROUGH, 1.0), None),
            GateAction::Keep
        );
        assert_eq!(
            gate_action(&v(ROUTE_PASSTHROUGH, 0.9), Some(0.8)),
            GateAction::Withhold
        );
        assert_eq!(
            gate_action(&v(ROUTE_INSUFFICIENT, 0.9), Some(0.8)),
            GateAction::Withhold
        );
        assert_eq!(
            gate_action(&v(ROUTE_PASSTHROUGH, 0.5), Some(0.8)),
            GateAction::Keep
        );
        assert_eq!(
            gate_action(&v(ROUTE_GROUNDED, 1.0), Some(0.0)),
            GateAction::Keep
        );
        assert_eq!(
            gate_action(&v(ROUTE_MIXED, 1.0), Some(0.0)),
            GateAction::Keep
        );
    }

    #[test]
    fn rank_order_is_stable_and_none_for_identity() {
        assert_eq!(rank_order(&[(3.0, 1.0), (2.0, 1.0), (1.0, 1.0)]), None);
        assert_eq!(
            rank_order(&[(1.0, 1.0), (3.0, 1.0), (3.0, 1.0)]),
            Some(vec![1, 2, 0])
        );
        assert_eq!(rank_order(&[(2.0, 1.0), (2.0, 1.0)]), None);
    }

    #[test]
    fn parse_rank_requires_every_id() {
        let ok = json!({"answers":{
            "rel_0":{"type":"score","score":2.5,"confidence":0.7},
            "rel_1":{"type":"score","score":0.1,"confidence":0.7}}});
        assert_eq!(parse_rank(ok, 2).unwrap().len(), 2);
        let short = json!({"answers":{"rel_0":{"type":"score","score":2.5,"confidence":0.7}}});
        assert_eq!(parse_rank(short, 2), Err(Fallback::Malformed));
        let range = json!({"answers":{"rel_0":{"type":"score","score":9.0,"confidence":0.7}}});
        assert_eq!(parse_rank(range, 1), Err(Fallback::Malformed));
    }

    #[test]
    fn empty_report_is_none() {
        assert!(JevReport::default().to_json().is_none());
    }
}
