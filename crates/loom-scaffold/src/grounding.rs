//! Building the per-seed half of the confidence contract.
//!
//! The domain owns the [`Grounding`] shape and its arithmetic; this module owns
//! the one thing only the scaffold knows — which seeds were actually SERVED,
//! and what curated quality the index holds for each of them.

use loom_domain::{Grounding, GroundingSignal, InjectionDecision, ScoreScale, SeedGrounding};

use crate::index::ScaffoldIndex;
use crate::policy::GatePolicy;

/// The provenance recorded for seeds assembled through the lexical path. The
/// fusion layer overwrites this per seed when the candidates came from HNSW.
pub const LEXICAL_PROVENANCE: &str = "lexical";

/// Describe each `(slug, score)` seed, marking the first `kept` as injected.
///
/// Sections are serialised one-per-seed in seed order and the budget clamp only
/// trims from the end, so `kept` partitions the list exactly: `0..kept` were
/// served, the rest were selected but did not survive the budget.
#[must_use]
pub fn seed_groundings(
    idx: &ScaffoldIndex,
    seeds: &[(String, f64)],
    gate: &GatePolicy,
    kept: usize,
) -> Vec<SeedGrounding> {
    seeds
        .iter()
        .enumerate()
        .map(|(i, (slug, score))| SeedGrounding {
            iri: loom_domain::Iri::from_slug(slug).as_str().to_owned(),
            score: *score,
            confidence: GroundingSignal::Lexical
                .confidence_of(Some(*score), gate.strong_match_score),
            quality: idx.get(slug).and_then(|e| e.q),
            provenance: LEXICAL_PROVENANCE.to_owned(),
            injected: i < kept,
        })
        .collect()
}

/// Assemble the lexical-scale grounding for one assembly.
///
/// `engaged` is passed in rather than inferred, because only the caller knows
/// whether the clamp left a non-empty block behind.
#[must_use]
pub fn lexical_grounding(
    idx: &ScaffoldIndex,
    seeds: &[(String, f64)],
    gate: &GatePolicy,
    decision: InjectionDecision,
    effective_budget: Option<usize>,
    kept: usize,
    engaged: bool,
) -> Grounding {
    if seeds.is_empty() {
        return Grounding::none(gate.min_inject_score);
    }
    Grounding::from_parts(
        GroundingSignal::Lexical,
        Some(seeds[0].1),
        ScoreScale::LexicalAdditive,
        gate,
        decision,
        effective_budget,
        seed_groundings(idx, seeds, gate, kept),
    )
    .with_engaged(engaged)
}
