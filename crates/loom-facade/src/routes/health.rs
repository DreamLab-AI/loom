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

use loom_domain::{ScoreScale, ServingIdentity};

use crate::build_info::BuildInfo;
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
    /// `{ready, generation, qualification}` for the HNSW artifact. The
    /// qualification block is the ADR-137 closeout surface: effective
    /// dimensions, effective metric, declared embedding model, and every reason
    /// the artefact was rejected. `ready` is DERIVED from it, so the two can
    /// never disagree.
    pub semantic: Value,
    /// The `Generation` this node is SERVING — the identity captured when the
    /// bundle was activated, not a fresh read of the data directory (ADR-135
    /// closeout). `serving_bundle` below carries the disk view beside it.
    pub generation: Value,
    /// The activated bundle: its immutable identity, its lifecycle phase, and
    /// how the disk currently compares to it.
    pub serving_bundle: ServingBundleBlock,
    /// The compile-time release identity, including the SIBLING RuVector
    /// revision a Loom commit cannot pin on its own (ADR-137 closeout).
    pub build: BuildInfo,
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

/// The serving-bundle block: which bundle is loaded, how far through its
/// lifecycle it is, and whether the data directory has moved on without it.
///
/// `disk_matches_loaded: false` is the review's mismatch made visible: a
/// promotion has landed that this process has NOT activated, and the answer is a
/// restart, not a reload. Reporting it here means an operator sees the pending
/// change instead of inferring it from a generation string that used to advance
/// on its own.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServingBundleBlock {
    pub identity: ServingIdentity,
    /// What the data directory reports right now (`Generation`).
    pub disk_generation: Value,
    pub disk_matches_loaded: bool,
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

    let qualification = st.semantic.qualification();
    let semantic = json!({
        "ready": st.semantic.is_ready(),
        "generation": super::to_value(&st.semantic.generation()),
        "qualification": super::to_value(&qualification),
        // The metric a score from THIS artefact may honestly be labelled with.
        // `null` when the artefact is unqualified — an unqueryable artefact has
        // no score scale, rather than a default one.
        "score_metric": qualification.served_metric().map(loom_domain::VectorMetric::as_str),
        "rejections": qualification.reasons(),
    });

    let identity = st.generation.reported_identity();
    let serving_bundle = ServingBundleBlock {
        disk_generation: super::to_value(&st.generation.disk_generation()),
        disk_matches_loaded: st.generation.disk_matches_loaded(),
        identity: identity.clone(),
    };

    Json(HealthResponse {
        ok: true,
        facet: "loom-facade".to_owned(),
        mode: "scaffold".to_owned(),
        backend,
        backend_reachable,
        index_classes: st.retriever.class_count(),
        graph: super::to_value(&st.graph.status()),
        semantic,
        generation: super::to_value(&identity.generation),
        serving_bundle,
        build: BuildInfo::current(),
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
