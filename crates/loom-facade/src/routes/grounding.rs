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

use serde_json::{json, Value};

use loom_domain::{FusionPath, Grounding, InjectionDecision, Scaffold, ServedMode};

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

/// Serialise a grounding for the wire, degrading to an explicit no-match object
/// rather than `null` on the (impossible for this type) serialise error — the
/// contract is that `grounding` is always an object.
#[must_use]
pub fn to_json(grounding: &Grounding) -> Value {
    serde_json::to_value(grounding).unwrap_or_else(|_| {
        json!({
            "signal": "none",
            "top_score": Value::Null,
            "score_scale": "lexical-additive",
            "confidence": 0.0,
            "decision": "skipped",
            "threshold": grounding.threshold,
            "effective_budget": Value::Null,
            "engaged": false,
            "seeds": [],
        })
    })
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
    let gen = st.generation.current();
    let content = serving::verbatim_content(&s.block, &gen.id.0);
    // F2: the served content IS the answer, so exposure is honest here too.
    let exposure = serving::compute_exposure(st.retriever.as_ref(), s, &content);
    let block = loom_block(
        ServedMode::Verbatim,
        est_block_tokens(&s.block),
        grounding,
        fusion_path,
        &super::to_value(&exposure),
        &super::to_value(&gen),
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
                &super::to_value(&st.generation.current()),
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
