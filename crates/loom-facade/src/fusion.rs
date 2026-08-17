//! The retrieval-fusion pipeline (§6). This module holds NO policy — the gate
//! lives in `loom-scaffold` (`InjectionPolicy` / `LexicalIndex::assemble`). Here
//! we only sequence ports:
//!
//!   lexical primary → gate short-circuit (NO embed on a hit) → enabled+ready+
//!   generation-parity guard → embed → nearest → union-dedupe-by-IRI → the SAME
//!   gate via `assemble` → `Scaffold::empty` on no-match.
//!
//! Invariant I-P1 checkpoints, made structural:
//! - a lexical hit takes the hot path and NEVER calls the embedder (network-free);
//! - HNSW hits are candidate SEEDS handed BACK into `assemble` — the single gate
//!   that resolves each `Iri` to its `CanonicalUnit` markdown. No vector, cosine
//!   score or summary is ever served;
//! - a generation mismatch between the semantic and lexical projections skips the
//!   fallback (never-mixed-build), with a `tracing::warn`.
//!
//! DIVERGENCE from the §6 sketch (deliberate, EXP-007): the sketch uses `?` on
//! `embed`/`nearest`. Those are ACCELERATOR calls — an embedder or index error
//! must degrade to `NoMatch`, not 5xx the request (the fail-open rule). So they
//! are matched, not `?`-propagated. Only the lexical primary (`seeds`) and the
//! gate (`assemble`) propagate — the lexical index is the hard floor.

use loom_domain::{ConceptMatch, FusionPath, Iri, LoomError, Scaffold, ScaffoldOpts};

use crate::state::AppState;

/// Run the fusion pipeline for `query`, returning the served `Scaffold` (whose
/// `.block` is the per-IRI markdown, or empty on no-match).
///
/// # Errors
/// Propagates only the lexical floor's errors (`seeds`/`assemble` —
/// `IndexUnavailable`). Accelerator faults (embedder, HNSW) degrade to a
/// no-match `Scaffold`, never an `Err`.
pub async fn build_scaffold(
    state: &AppState,
    query: &str,
    opts: ScaffoldOpts,
) -> Result<Scaffold, LoomError> {
    // (1) LEXICAL PRIMARY — inverted-index match over the class titles. Hot,
    //     LLM-free, network-free. The lexical index is the floor: errors here
    //     propagate (500), unlike the accelerators below.
    let lexical = state.retriever.seeds(query, opts.max_seeds).await?;
    let top = lexical.first().map_or(0.0_f64, |m| f64::from(m.score));

    // (2) GATE — a lexical top at/above the inject floor assembles as today, with
    //     NO embedding call. This is the network-free hot path.
    if top >= state.policy.min_inject_score {
        return state
            .retriever
            .assemble(query, &lexical, opts.with_path(FusionPath::LexicalHit))
            .await;
    }

    // (3) SEMANTIC FALLBACK — only on a lexical miss / below-gate score, and only
    //     when enabled AND ready. Off by default (recall-gate governed).
    if state.semantic_fallback_enabled() && state.semantic.is_ready() {
        // Generation-parity guard: never fuse across builds (never-mixed).
        let semantic_gen = state.semantic.generation();
        let lexical_gen = state.retriever.generation();
        if semantic_gen != lexical_gen {
            tracing::warn!(
                semantic = %semantic_gen.id.0,
                lexical = %lexical_gen.id.0,
                "semantic index generation != lexical; skipping fallback (never-mixed-build)"
            );
        } else if let Some(candidates) =
            semantic_candidates(state, query, &lexical, opts.k_semantic).await
        {
            // (4) HANDED BACK INTO THE GATE — HNSW hits are candidate SEEDS, not an
            //     injection. The SAME policy decides whether/how much injects.
            if let Some(best) = candidates.first() {
                let normalised = normalise(state, best);
                // The fallback cosine gate is bench-set (`LOOM_SEMANTIC_MIN_INJECT`);
                // with no threshold configured, no semantic candidate may inject.
                if let Some(threshold) = state.config.semantic_min_inject {
                    if normalised >= threshold {
                        return state
                            .retriever
                            .assemble(
                                query,
                                &candidates,
                                opts.with_path(FusionPath::SemanticFallback),
                            )
                            .await;
                    }
                }
            }
        }
    }

    // (5) NO MATCH — empty scaffold; the caller falls back to the raw prompt.
    Ok(Scaffold::empty(
        FusionPath::NoMatch,
        state.retriever.generation(),
    ))
}

/// Embed the query and fetch the HNSW neighbours, union-deduped with the weak
/// lexical candidates. Returns `None` (degrade to no-match) on ANY accelerator
/// error — the fail-open rule (an embedder/index fault is not a 5xx).
async fn semantic_candidates(
    state: &AppState,
    query: &str,
    lexical: &[ConceptMatch],
    k_semantic: usize,
) -> Option<Vec<ConceptMatch>> {
    let qvec = match state.embedder.embed(query).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "embed failed on fallback; degrading to no-match");
            return None;
        }
    };
    let hits = match state.semantic.nearest(&qvec, k_semantic).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "hnsw nearest failed on fallback; degrading to no-match");
            return None;
        }
    };
    Some(union_dedupe_by_iri(lexical, &hits))
}

/// Union the two candidate lists, lexical first, dropping any semantic hit whose
/// `Iri` a lexical candidate already carries (dedupe by the addressing key).
#[must_use]
fn union_dedupe_by_iri(lexical: &[ConceptMatch], semantic: &[ConceptMatch]) -> Vec<ConceptMatch> {
    let mut seen: Vec<&Iri> = Vec::with_capacity(lexical.len() + semantic.len());
    let mut out: Vec<ConceptMatch> = Vec::with_capacity(lexical.len() + semantic.len());
    for m in lexical.iter().chain(semantic.iter()) {
        if seen.contains(&&m.iri) {
            continue;
        }
        seen.push(&m.iri);
        out.push(m.clone());
    }
    out
}

/// Map a candidate's raw score onto the comparable scale the fallback gate reads.
/// `LOOM_SEMANTIC_SCORE_SCALE` is the bench-tuned lexical↔cosine normalisation;
/// unset ⇒ identity (the domain `score_normalised` default).
fn normalise(state: &AppState, m: &ConceptMatch) -> f64 {
    let base = f64::from(m.score_normalised());
    state
        .config
        .semantic_score_scale
        .map_or(base, |scale| base * scale)
}
