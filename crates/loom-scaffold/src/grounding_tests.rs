//! The confidence-surfacing contract (PRD-026 FR-11), tested where it is
//! computed: the gate's four decisions, and the grounding `assemble_block`
//! hangs off every outcome — including the ones that inject nothing.

use std::path::Path;

use loom_domain::{
    FusionPath, GroundingSignal, InjectionDecision, LexicalIndex, ScaffoldOpts, ScoreScale,
    DEFAULT_MIN_INJECT_SCORE,
};

use crate::index::ScaffoldIndex;
use crate::match_::match_seeds;
use crate::policy::{decide, GatePolicy};
use crate::tuning::{MIN_INJECT_SCORE_DEFAULT, STRONG_MATCH_SCORE_DEFAULT};
use crate::{assemble_block, LexicalRetriever};

const PROMPT: &str = "Explain how a knowledge graph uses a graph database";

fn fixture_index() -> ScaffoldIndex {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = base.join("../..").join("tests/golden-python/fixture.json");
    let json = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    ScaffoldIndex::from_json_str(&json).expect("fixture parses")
}

fn gate(ci: bool, strong: f64, min: f64, frac: f64) -> GatePolicy {
    GatePolicy {
        confidence_injection: ci,
        strong_match_score: strong,
        min_inject_score: min,
        min_inject_fraction: frac,
    }
}

// --- the domain's mirrored default must not drift ---------------------------

#[test]
fn domain_default_threshold_matches_the_gate() {
    // `loom-domain` mirrors this constant so it can build an honest no-match
    // grounding without depending on this crate. Pin them together.
    assert!((DEFAULT_MIN_INJECT_SCORE - MIN_INJECT_SCORE_DEFAULT).abs() < f64::EPSILON);
    assert!(
        (GatePolicy::default().min_inject_score - DEFAULT_MIN_INJECT_SCORE).abs() < f64::EPSILON
    );
    assert!(
        (GatePolicy::default().strong_match_score - STRONG_MATCH_SCORE_DEFAULT).abs()
            < f64::EPSILON
    );
}

// --- decide(): the four branches --------------------------------------------

#[test]
fn decide_gate_off_is_always_full_budget() {
    let g = gate(false, 8.0, 2.0, 0.4);
    // Python baseline: injection off ⇒ full budget whatever the score, and even
    // when there is no score at all.
    assert_eq!(
        decide(Some(0.0), 1500, &g),
        (InjectionDecision::Full, Some(1500))
    );
    assert_eq!(
        decide(Some(19.5), 1500, &g),
        (InjectionDecision::Full, Some(1500))
    );
    assert_eq!(
        decide(None, 1500, &g),
        (InjectionDecision::Full, Some(1500))
    );
}

#[test]
fn decide_no_score_is_a_skip_when_gated() {
    let g = gate(true, 8.0, 2.0, 0.4);
    assert_eq!(decide(None, 1500, &g), (InjectionDecision::Skipped, None));
}

#[test]
fn decide_below_min_is_a_skip() {
    let g = gate(true, 8.0, 2.0, 0.4);
    assert_eq!(
        decide(Some(1.99), 1500, &g),
        (InjectionDecision::Skipped, None)
    );
    assert_eq!(
        decide(Some(0.0), 1500, &g),
        (InjectionDecision::Skipped, None)
    );
}

#[test]
fn decide_at_or_above_strong_is_full() {
    let g = gate(true, 8.0, 2.0, 0.4);
    assert_eq!(
        decide(Some(8.0), 1500, &g),
        (InjectionDecision::Full, Some(1500))
    );
    assert_eq!(
        decide(Some(19.5), 1500, &g),
        (InjectionDecision::Full, Some(1500))
    );
    // A non-positive strong_match_score guards the divide ⇒ frac 1.0 ⇒ Full.
    assert_eq!(
        decide(Some(5.0), 1500, &gate(true, 0.0, 2.0, 0.4)),
        (InjectionDecision::Full, Some(1500))
    );
}

#[test]
fn decide_midrange_is_scaled() {
    let g = gate(true, 8.0, 2.0, 0.4);
    assert_eq!(
        decide(Some(4.0), 1500, &g),
        (InjectionDecision::Scaled, Some(750))
    );
    assert_eq!(
        decide(Some(6.0), 1500, &g),
        (InjectionDecision::Scaled, Some(1125))
    );
    // The fraction FLOOR clamps up: 2.0/8.0 = 0.25 < 0.4 ⇒ 0.4, not 0.25.
    assert_eq!(
        decide(Some(2.0), 1500, &g),
        (InjectionDecision::Scaled, Some(600))
    );
}

#[test]
fn decide_is_byte_identical_to_effective_budget() {
    // The wrapper kept for one release must not have changed a single number.
    for ci in [true, false] {
        let g = gate(ci, 8.0, 2.0, 0.4);
        for top in [0.0, 1.99, 2.0, 4.0, 6.0, 8.0, 19.5] {
            for requested in [0_usize, 1, 2, 60, 1500] {
                assert_eq!(
                    g.effective_budget(top, requested),
                    decide(Some(top), requested, &g).1,
                    "top={top} requested={requested} ci={ci}"
                );
            }
        }
    }
}

// --- the grounding on every outcome -----------------------------------------

#[test]
fn grounding_present_on_a_full_lexical_hit() {
    let idx = fixture_index();
    let seeds = match_seeds(&idx, PROMPT, 4);
    let g = GatePolicy::default();
    let out = assemble_block(&idx, &seeds, 1500, 1, false, None, &g);

    let gr = &out.grounding;
    assert_eq!(gr.signal, GroundingSignal::Lexical);
    assert_eq!(gr.score_scale, ScoreScale::LexicalAdditive);
    assert_eq!(gr.decision, InjectionDecision::Full);
    assert_eq!(gr.effective_budget, Some(1500));
    assert!(gr.engaged);
    assert_eq!(gr.top_score, Some(out.top_score));
    // A strong exact-title hit is unbounded on the raw scale but saturates the
    // normalised confidence — that is the whole point of the second number.
    assert!(out.top_score > STRONG_MATCH_SCORE_DEFAULT);
    assert!((gr.confidence - 1.0).abs() < f64::EPSILON);
    assert!((gr.threshold - MIN_INJECT_SCORE_DEFAULT).abs() < f64::EPSILON);
    assert_eq!(gr.seeds.len(), seeds.len());
    assert!(gr.seeds.iter().all(|s| s.injected));
    assert!(gr.seeds[0].iri.starts_with("urn:ngm:class:"));
    assert_eq!(gr.seeds[0].provenance, "lexical");
    // The flat aliases still agree with the typed form.
    assert_eq!(out.effective_budget, gr.effective_budget.unwrap_or(0));
}

#[test]
fn grounding_present_on_no_match() {
    let idx = fixture_index();
    let g = GatePolicy::default();
    let out = assemble_block(&idx, &[], 1500, 1, false, None, &g);

    let gr = &out.grounding;
    assert_eq!(gr.signal, GroundingSignal::None);
    assert_eq!(gr.decision, InjectionDecision::Skipped);
    assert_eq!(gr.top_score, None);
    assert_eq!(gr.effective_budget, None);
    assert!((gr.confidence - 0.0).abs() < f64::EPSILON);
    assert!(!gr.engaged);
    assert!(gr.seeds.is_empty());
}

#[test]
fn grounding_reports_a_gated_skip_with_its_evidence() {
    let idx = fixture_index();
    // A seed that scores below a deliberately high min_inject_score.
    let seeds = vec![("knowledge-graph".to_owned(), 3.0)];
    let g = gate(true, 8.0, 12.0, 0.4);
    let out = assemble_block(&idx, &seeds, 1500, 1, false, None, &g);

    assert!(out.block.is_empty());
    let gr = &out.grounding;
    assert_eq!(gr.decision, InjectionDecision::Skipped);
    assert_eq!(gr.effective_budget, None);
    assert!(!gr.engaged);
    // The skip still SHOWS its working: the signal, the score, the bar it lost
    // to, and the seed it was about.
    assert_eq!(gr.signal, GroundingSignal::Lexical);
    assert_eq!(gr.top_score, Some(3.0));
    assert!((gr.threshold - 12.0).abs() < f64::EPSILON);
    assert!((gr.confidence - 0.375).abs() < f64::EPSILON);
    assert_eq!(gr.seeds.len(), 1);
    assert!(!gr.seeds[0].injected, "a skipped seed was not served");
}

#[test]
fn grounding_scaled_decision_carries_the_reduced_budget() {
    let idx = fixture_index();
    let seeds = vec![("knowledge-graph".to_owned(), 4.0)];
    let g = gate(true, 8.0, 2.0, 0.4);
    let out = assemble_block(&idx, &seeds, 1500, 1, false, None, &g);

    assert_eq!(out.grounding.decision, InjectionDecision::Scaled);
    assert_eq!(out.grounding.effective_budget, Some(750));
    assert!((out.grounding.confidence - 0.5).abs() < f64::EPSILON);
}

// --- per-seed `injected` after a real clamp ---------------------------------

#[test]
fn seed_injected_reflects_clamp_survivors() {
    let idx = fixture_index();
    let seeds = match_seeds(&idx, PROMPT, 4);
    assert!(seeds.len() >= 2, "fixture must yield multiple seeds");
    let g = GatePolicy::default();

    // Full budget: everything survives.
    let full = assemble_block(&idx, &seeds, 1500, 1, false, None, &g);
    let survivors =
        |o: &crate::ScaffoldOutcome| o.grounding.seeds.iter().filter(|s| s.injected).count();
    assert_eq!(survivors(&full), seeds.len());

    // Shrink the budget until the clamp starts trimming from the END. The
    // grounding must then mark exactly the trimmed tail as not injected —
    // "selected" and "served" are different facts.
    let mut saw_partial = false;
    for budget in [120_usize, 200, 320, 480, 700] {
        let out = assemble_block(&idx, &seeds, budget, 1, false, None, &g);
        let kept = survivors(&out);
        assert_eq!(
            out.grounding.seeds.len(),
            seeds.len(),
            "every selected seed is reported, served or not"
        );
        // Survivors are always a PREFIX of the seed list (the clamp trims tails).
        for (i, s) in out.grounding.seeds.iter().enumerate() {
            assert_eq!(
                s.injected,
                i < kept,
                "injected must be a prefix at {budget}"
            );
        }
        if out.block.is_empty() {
            assert_eq!(kept, 0, "an empty block injected nothing");
            assert!(!out.grounding.engaged);
        } else if kept < seeds.len() {
            saw_partial = true;
            assert!(out.grounding.engaged);
        }
    }
    assert!(
        saw_partial,
        "expected at least one budget to trim some but not all sections"
    );
}

#[test]
fn seed_quality_comes_from_the_index() {
    let idx = fixture_index();
    let seeds = match_seeds(&idx, PROMPT, 4);
    let out = assemble_block(&idx, &seeds, 1500, 1, false, None, &GatePolicy::default());
    for (g, (slug, _)) in out.grounding.seeds.iter().zip(&seeds) {
        assert_eq!(g.quality, idx.get(slug).and_then(|e| e.q));
    }
}

// --- through the port: provenance + the semantic remap ----------------------

#[tokio::test]
async fn assemble_stamps_lexical_grounding_onto_the_scaffold() {
    let retriever = LexicalRetriever::from_index(fixture_index());
    let seeds = retriever.seeds(PROMPT, 4).await.expect("seeds");
    let opts = ScaffoldOpts::default().with_path(FusionPath::LexicalHit);
    let scaffold = retriever
        .assemble(PROMPT, &seeds, opts)
        .await
        .expect("assemble");

    let gr = &scaffold.grounding;
    assert_eq!(gr.signal, GroundingSignal::Lexical);
    assert_eq!(gr.score_scale, ScoreScale::LexicalAdditive);
    assert!(gr.engaged);
    assert!(gr.seeds.iter().all(|s| s.provenance == "lexical"));
    // The flat aliases mirror the typed contract.
    assert_eq!(scaffold.effective_budget, gr.effective_budget.unwrap_or(0));
    assert!((f64::from(scaffold.top_score) - gr.top_score.unwrap_or(0.0)).abs() < 1e-4);
}

#[tokio::test]
async fn semantic_fallback_is_restated_on_the_cosine_scale() {
    use loom_domain::{ConceptMatch, Iri, MatchProvenance};

    let retriever = LexicalRetriever::from_index(fixture_index());
    // HNSW hands back IRI-keyed candidates whose scores are cosines in [0,1].
    let candidates = vec![
        ConceptMatch {
            iri: Iri::from_slug("knowledge-graph"),
            score: 0.83,
            provenance: MatchProvenance::SemanticHnsw,
        },
        ConceptMatch {
            iri: Iri::from_slug("graph-database"),
            score: 0.61,
            provenance: MatchProvenance::SemanticHnsw,
        },
    ];
    let opts = ScaffoldOpts::default().with_path(FusionPath::SemanticFallback);
    let scaffold = retriever
        .assemble(PROMPT, &candidates, opts)
        .await
        .expect("assemble");

    let gr = &scaffold.grounding;
    assert_eq!(gr.signal, GroundingSignal::Semantic);
    assert_eq!(gr.score_scale, ScoreScale::Cosine);
    // A 0.83 cosine is 0.83 confident — NOT 0.83/8.0 read on the lexical scale.
    assert!((gr.confidence - 0.83).abs() < 1e-6);
    assert!((gr.seeds[0].confidence - 0.83).abs() < 1e-6);
    assert!((gr.seeds[1].confidence - 0.61).abs() < 1e-6);
    assert!(gr.seeds.iter().all(|s| s.provenance == "semantic-hnsw"));
}
