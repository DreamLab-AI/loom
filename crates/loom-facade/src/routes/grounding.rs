//! Surfacing the grounding contract on both answer routes.
//!
//! The contract itself is built where the evidence lives: `loom-scaffold`'s
//! `assemble` knows which seeds survived the budget clamp and against which gate
//! they were judged, so it stamps the [`Grounding`] onto the `Scaffold`. This
//! module does the remaining façade-side work — carrying that object onto the
//! wire on EVERY request, and restamping it for the one decision the scaffold
//! cannot see (the verbatim serve, which is a façade serving-mode choice made
//! after assembly).
//!
//! The invariant this module exists to hold: `grounding` is ALWAYS an object,
//! never `null`. A consumer must be able to distinguish "the Loom looked and
//! found nothing" from "the Loom was never asked", and an absent field cannot
//! express that difference. The no-match case is therefore a fully-populated
//! honest zero — `signal: "none"`, `decision: "skipped"`, `engaged: false`.
//!
//! # The per-status contract (ADR-138 closeout)
//!
//! The review found the object present on `/loom/scaffold` and on a successful
//! chat, and absent from "non-200 backend paths". That gap matters more than it
//! sounds: a consumer that receives a bare 502 cannot tell whether the corpus
//! had nothing or the model was down, so it treats a retrieval success with a
//! dead backend exactly like a retrieval miss.
//!
//! [`envelope`] therefore builds ONE shape for all six statuses the closeout
//! enumerates — no-match, opt-out, semantic fallback, verbatim, delegated
//! success and backend failure — adding four keys to the domain object that only
//! the façade can know:
//!
//! - `status` — which of the six paths produced this response;
//! - `corpus_backed` — the single predicate a consuming agent should branch on,
//!   true only when the scaffold engaged AND the path can carry evidence;
//! - `generation` + `content_digest` — the LOADED serving identity (ADR-135), so
//!   the answer and the bytes it came from are named together rather than
//!   requiring a second call to `/health` that might race a promotion;
//! - `degraded` — the accelerators that were unavailable for this request.
//!
//! [`REQUIRED_GROUNDING_FIELDS`] is the contract those keys satisfy, and the
//! integration tests assert it on every status.

use serde_json::{json, Map, Value};

use loom_domain::{
    FusionPath, Grounding, GroundingStatus, InjectionDecision, LoomError, Scaffold, ServedMode,
    REQUIRED_GROUNDING_FIELDS,
};

use crate::error::api_error_parts;
use crate::serving;
use crate::state::AppState;

/// The grounding for one request.
///
/// `scaffold` is `None` only when no scaffold was attempted at all (no user
/// message, or a lexical-floor error the chat path swallowed). That case has
/// nothing honest to report beyond the threshold in force, so it reports the
/// gate's own `min_inject_score` rather than the domain's compiled-in default.
#[must_use]
pub fn build(st: &AppState, scaffold: Option<&Scaffold>) -> Grounding {
    scaffold.map_or_else(
        || Grounding::none(st.policy.min_inject_score),
        |s| s.grounding.clone(),
    )
}

/// Restamp a grounding for the F1 verbatim serve.
///
/// The retrieval axis is untouched — a verbatim answer still arrived through a
/// lexical hit — but the DECISION and the threshold it was judged against both
/// change: the bar cleared was the verbatim threshold, not the injection floor.
/// Reporting `full` and `min_inject_score` here would name the wrong gate.
#[must_use]
pub fn as_verbatim(mut grounding: Grounding, verbatim_threshold: f64) -> Grounding {
    grounding.decision = InjectionDecision::Verbatim;
    grounding.with_threshold(verbatim_threshold)
}

/// Serialise a grounding for the wire under the full per-status contract.
///
/// Degrades to an explicit no-match object rather than `null` on the (impossible
/// for this type) serialise error — the contract is that `grounding` is always
/// an object, and a serialisation accident must not be the one case that breaks
/// it.
#[must_use]
pub fn envelope(st: &AppState, grounding: &Grounding, status: GroundingStatus) -> Value {
    let mut obj = match serde_json::to_value(grounding) {
        Ok(Value::Object(m)) => m,
        _ => fallback_object(grounding),
    };
    let identity = st.generation.reported_identity();

    obj.insert("status".to_owned(), json!(status.as_str()));
    // The consumer-facing predicate. Engagement alone is not enough: a delegated
    // request whose backend then failed engaged the scaffold and still has no
    // answer to back.
    obj.insert(
        "corpus_backed".to_owned(),
        json!(grounding.engaged && status.may_be_corpus_backed()),
    );
    obj.insert(
        "generation".to_owned(),
        json!(identity.generation.id.0.clone()),
    );
    obj.insert(
        "content_digest".to_owned(),
        json!(identity.content_digest.clone()),
    );
    obj.insert("degraded".to_owned(), json!(degradations(st, status)));
    Value::Object(obj)
}

/// Which accelerators were unavailable for this request. An empty list is the
/// honest "nothing was degraded", and is always present so a consumer never has
/// to distinguish absent-from-empty.
fn degradations(st: &AppState, status: GroundingStatus) -> Vec<String> {
    let mut out = Vec::new();
    if !st.graph.status().available {
        out.push("graph-unavailable".to_owned());
    }
    if st.config.semantic_fallback && !st.semantic.is_ready() {
        // Name WHY, not just that: an unqualified artefact and an absent one are
        // different operational problems (ADR-137 closeout).
        let q = st.semantic.qualification();
        out.push(format!(
            "semantic-unavailable: {}",
            q.first_rejection()
                .map_or_else(|| "not ready".to_owned(), ToString::to_string)
        ));
    }
    if !st.generation.identity().atomicity_verified {
        out.push("bundle-unattested".to_owned());
    }
    if !st.generation.disk_matches_loaded() {
        out.push("pending-generation-on-disk".to_owned());
    }
    if status == GroundingStatus::BackendFailure {
        out.push("backend-failure".to_owned());
    }
    out
}

/// The last-resort object, used only if the domain type somehow fails to
/// serialise. Carries the domain half of the contract; [`envelope`] adds the
/// façade half on top exactly as it would for a real one.
fn fallback_object(grounding: &Grounding) -> Map<String, Value> {
    let Value::Object(m) = json!({
        "signal": "none",
        "top_score": Value::Null,
        "score_scale": "lexical-additive",
        "confidence": 0.0,
        "decision": "skipped",
        "threshold": grounding.threshold,
        "effective_budget": Value::Null,
        "engaged": false,
        "seeds": [],
    }) else {
        unreachable!("json! literal above is an object")
    };
    m
}

/// Assert the contract on a built envelope — used by the router's own debug
/// assertions and by the integration tests, so the field list cannot drift from
/// [`REQUIRED_GROUNDING_FIELDS`] without something failing.
#[must_use]
pub fn missing_contract_fields(envelope: &Value) -> Vec<&'static str> {
    let Some(obj) = envelope.as_object() else {
        return REQUIRED_GROUNDING_FIELDS.to_vec();
    };
    REQUIRED_GROUNDING_FIELDS
        .iter()
        .filter(|k| !obj.contains_key(**k))
        .copied()
        .collect()
}

/// The status a chat request took, from the facts the router holds.
#[must_use]
pub fn chat_status(
    engaged: bool,
    fusion_path: FusionPath,
    verbatim_served: bool,
    verbatim_declined: bool,
) -> GroundingStatus {
    if verbatim_served {
        return GroundingStatus::Verbatim;
    }
    if !engaged {
        return GroundingStatus::NoMatch;
    }
    if verbatim_declined {
        return GroundingStatus::OptOut;
    }
    if fusion_path == FusionPath::SemanticFallback {
        return GroundingStatus::SemanticFallback;
    }
    GroundingStatus::Delegated
}

/// The status a `/loom/scaffold` request took. There is no delegation on this
/// route, so the axis is only which retriever answered.
#[must_use]
pub fn scaffold_status(scaffold: Option<&Scaffold>) -> GroundingStatus {
    match scaffold {
        Some(s) if s.block.is_empty() => GroundingStatus::NoMatch,
        Some(s) if s.fusion_path == FusionPath::SemanticFallback => {
            GroundingStatus::SemanticFallback
        }
        Some(_) => GroundingStatus::Delegated,
        None => GroundingStatus::NoMatch,
    }
}

/// The `loom` telemetry block shared by both chat delivery paths.
fn loom_block(
    served_mode: ServedMode,
    injected_tokens: usize,
    grounding: &Value,
    fusion_path: FusionPath,
    exposure: &Value,
    generation: &Value,
) -> Value {
    json!({
        "mode": "scaffold",
        "served_mode": served_mode,
        "injected_tokens": injected_tokens,
        "grounding": grounding,
        "fusion_path": fusion_path,
        "exposure": exposure,
        "generation": generation,
    })
}

/// Build the F1 verbatim 200 body: the scaffold served as the answer, with
/// `served_mode: verbatim` + exposure telemetry. No backend is called.
#[must_use]
pub fn verbatim_response(
    st: &AppState,
    s: &Scaffold,
    grounding: &Value,
    fusion_path: FusionPath,
) -> Value {
    let identity = st.generation.reported_identity();
    let gen = &identity.generation;
    let content = serving::verbatim_content(&s.block, &gen.id.0);
    // F2: the served content IS the answer, so exposure is honest here too.
    let exposure = serving::compute_exposure(st.retriever.as_ref(), s, &content);
    let block = loom_block(
        ServedMode::Verbatim,
        est_block_tokens(&s.block),
        grounding,
        fusion_path,
        &super::to_value(&exposure),
        &super::to_value(&identity),
    );
    serving::verbatim_completion(&content, &block)
}

/// Attach the `loom` telemetry block to a delegated 200 response, computing F2
/// exposure (and optionally appending the "Not covered" line) when engaged.
pub fn annotate_delegated(
    st: &AppState,
    out: &mut Value,
    scaffold: Option<&Scaffold>,
    grounding: &Value,
    fusion_path: FusionPath,
    injected: usize,
) {
    let exposure = match scaffold {
        Some(s) if !s.block.is_empty() => {
            let answer = serving::answer_text(out);
            let report = serving::compute_exposure(st.retriever.as_ref(), s, &answer);
            // LOOM_EXPOSURE_APPEND: append a single "Not covered" line on drops.
            if st.config.exposure_append {
                serving::append_not_covered(out, &report);
            }
            super::to_value(&report)
        }
        _ => Value::Null,
    };
    if let Value::Object(ref mut map) = out {
        map.insert(
            "loom".to_owned(),
            loom_block(
                ServedMode::Delegated,
                injected,
                grounding,
                fusion_path,
                &exposure,
                &super::to_value(&st.generation.reported_identity()),
            ),
        );
    }
}

/// Estimate token count of a served block for the `injected_tokens` telemetry on
/// the verbatim path (no message-merge delta is available there). Mirrors the
/// façade's `chars/4` heuristic used on the delegate path.
fn est_block_tokens(block: &str) -> usize {
    block.chars().count().div_ceil(4)
}

/// Build the chat path's FAILURE response with the grounding contract attached.
///
/// The §7 status mapping is unchanged — this is the same `(code, body)` the
/// error module produces — but the body now also carries the `loom` block, so a
/// 502 states what retrieval found before the model let go of it. Without this,
/// a consuming agent's only signal on a dead backend is the absence of an
/// answer, which is indistinguishable from an ungrounded one.
#[must_use]
pub fn backend_failure_response(
    st: &AppState,
    err: LoomError,
    grounding: &Grounding,
    fusion_path: FusionPath,
    injected_tokens: usize,
) -> (axum::http::StatusCode, Value) {
    let (code, mut body) = api_error_parts(err);
    let envelope = envelope(st, grounding, GroundingStatus::BackendFailure);
    debug_assert!(
        missing_contract_fields(&envelope).is_empty(),
        "failure path must satisfy the grounding contract"
    );
    if let Value::Object(ref mut map) = body {
        map.insert(
            "loom".to_owned(),
            loom_block(
                ServedMode::Failed,
                injected_tokens,
                &envelope,
                fusion_path,
                &Value::Null,
                &super::to_value(&st.generation.reported_identity()),
            ),
        );
    }
    (code, body)
}
