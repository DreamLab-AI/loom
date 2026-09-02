//! `GET /health` — the operator's one-stop honesty surface, and the contract the
//! confidence-check evaluator deserialises.
//!
//! It is a SUPERSET of the Python façade's shape: every key the reference served
//! is still there and still means the same thing; the additions
//! (`injection_policy`, `serving`, `confidence`) answer the question the old
//! block could not — "is this node grounding anything, and how confidently?".
//! Without them a deployment can only discover a mis-set gate by diffing answers.
//!
//! [`HealthResponse`] is a typed, round-trippable struct rather than an ad-hoc
//! `json!` literal so an evaluator can `serde_json::from_slice` it and get a
//! compile error when the shape moves, instead of a silent `None`. The three
//! adapter-status members stay `Value`: they are re-serialised domain types
//! (`GraphStatus`, `Generation`) whose own shape is owned elsewhere, and pinning
//! them here would make this struct a second, drifting definition of them.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use loom_domain::ScoreScale;

use crate::state::{AppState, ConfidenceStats};

/// The full `/health` body. Public + `Deserialize` so out-of-process checkers
/// (the confidence-check bin, deploy smoke tests) can consume it typed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub facet: String,
    pub mode: String,
    /// The backend endpoint, or `null` on a retrieval-only node.
    pub backend: Option<String>,
    /// `null` when there is no backend configured (Python parity).
    pub backend_reachable: Option<bool>,
    pub index_classes: usize,
    /// `GraphStatus` — `{available, triples, loaded_files, error}`.
    pub graph: Value,
    /// `{ready, generation}` for the HNSW artifact.
    pub semantic: Value,
    /// The `Generation` descriptor this node is serving.
    pub generation: Value,
    pub deploy_profile: String,
    pub injection_policy: InjectionPolicyBlock,
    pub serving: ServingBlock,
    pub confidence: ConfidenceStats,
}

/// The confidence gate's configuration, verbatim from `InjectionPolicy`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InjectionPolicyBlock {
    pub confidence_injection: bool,
    pub strong_match_score: f64,
    pub min_inject_score: f64,
    pub min_inject_fraction: f64,
    /// Always `"lexical-additive"`: the gate's thresholds live on the lexical
    /// matcher's additive scale (`EXACT_TITLE_WEIGHT` per exactly-matched title
    /// word), whatever a given request's `signal` turned out to be. Naming the
    /// scale here stops a reader treating `min_inject_score: 2.0` as a
    /// probability.
    pub score_scale: ScoreScale,
}

/// The serving-regime configuration: when this node answers from the scaffold
/// itself, and whether the semantic fallback may run.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ServingBlock {
    pub verbatim_mode: bool,
    pub verbatim_threshold: f64,
    pub semantic_fallback: bool,
    /// Cosine floor for a semantic candidate to inject. `null` ⇒ unset, and with
    /// it unset NO semantic candidate may inject (bench-set, no default).
    pub semantic_min_inject: Option<f64>,
}

pub(super) async fn health(State(st): State<AppState>) -> Response {
    let backend_configured = !st.config.backend_url.is_empty();
    // `backend_reachable` is null when there is no backend (Python parity).
    let backend_reachable = if backend_configured {
        Some(st.backend.reachable().await)
    } else {
        None
    };
    let backend = if backend_configured {
        Some(st.backend.endpoint().to_owned())
    } else {
        None
    };

    let semantic = json!({
        "ready": st.semantic.is_ready(),
        "generation": super::to_value(&st.semantic.generation()),
    });

    Json(HealthResponse {
        ok: true,
        facet: "loom-facade".to_owned(),
        mode: "scaffold".to_owned(),
        backend,
        backend_reachable,
        index_classes: st.retriever.class_count(),
        graph: super::to_value(&st.graph.status()),
        semantic,
        generation: super::to_value(&st.generation.current()),
        deploy_profile: st.config.deploy_profile.clone(),
        injection_policy: InjectionPolicyBlock {
            confidence_injection: st.policy.confidence_injection,
            strong_match_score: st.policy.strong_match_score,
            min_inject_score: st.policy.min_inject_score,
            min_inject_fraction: st.policy.min_inject_fraction,
            score_scale: ScoreScale::LexicalAdditive,
        },
        serving: ServingBlock {
            verbatim_mode: st.config.verbatim_mode,
            verbatim_threshold: st.config.verbatim_threshold,
            semantic_fallback: st.config.semantic_fallback,
            semantic_min_inject: st.config.semantic_min_inject,
        },
        confidence: st.confidence.snapshot(),
    })
    .into_response()
}
