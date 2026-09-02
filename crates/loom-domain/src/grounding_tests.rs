//! Unit tests for `grounding` — split out of `grounding.rs` to keep that
//! file under the 500-line ceiling.

use crate::grounding::*;

struct Gate {
    strong: f64,
    min: f64,
}

impl GateThresholds for Gate {
    fn strong_match_score(&self) -> f64 {
        self.strong
    }
    fn min_inject_score(&self) -> f64 {
        self.min
    }
}

fn gate() -> Gate {
    Gate {
        strong: 8.0,
        min: 2.0,
    }
}

fn seed(iri: &str, score: f64, injected: bool) -> SeedGrounding {
    SeedGrounding {
        iri: iri.to_owned(),
        score,
        confidence: confidence_for(Some(score), 8.0),
        quality: None,
        provenance: "lexical".to_owned(),
        injected,
    }
}

// --- confidence_for: the clamp at both ends and the honest middle -------

#[test]
fn confidence_is_zero_without_a_score() {
    assert!((confidence_for(None, 8.0) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn confidence_is_the_plain_ratio_in_range() {
    // 4.0 of a strong-match 8.0 is exactly half the confidence.
    assert!((confidence_for(Some(4.0), 8.0) - 0.5).abs() < f64::EPSILON);
    assert!((confidence_for(Some(2.0), 8.0) - 0.25).abs() < f64::EPSILON);
}

#[test]
fn confidence_clamps_to_one_above_strong_match() {
    // The lexical score is UNBOUNDED — 19.5 must not report 2.4.
    assert!((confidence_for(Some(19.5), 8.0) - 1.0).abs() < f64::EPSILON);
    assert!((confidence_for(Some(8.0), 8.0) - 1.0).abs() < f64::EPSILON);
}

#[test]
fn confidence_clamps_to_zero_below_nothing() {
    assert!((confidence_for(Some(-3.0), 8.0) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn confidence_guards_the_divide() {
    // strong <= 0 would divide by zero; the gate guards it, so do we.
    assert!((confidence_for(Some(5.0), 0.0) - 0.0).abs() < f64::EPSILON);
    assert!((confidence_for(Some(5.0), -1.0) - 0.0).abs() < f64::EPSILON);
    assert!((confidence_for(Some(f64::NAN), 8.0) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn semantic_confidence_passes_the_cosine_through() {
    let s = GroundingSignal::Semantic;
    assert!((s.confidence_of(Some(0.83), 8.0) - 0.83).abs() < f64::EPSILON);
    // A cosine is already bounded, but a bad adapter is not trusted.
    assert!((s.confidence_of(Some(1.4), 8.0) - 1.0).abs() < f64::EPSILON);
    assert!((s.confidence_of(None, 8.0) - 0.0).abs() < f64::EPSILON);
    assert_eq!(s.score_scale(), ScoreScale::Cosine);
}

#[test]
fn none_signal_never_claims_confidence() {
    let s = GroundingSignal::None;
    assert!((s.confidence_of(Some(19.5), 8.0) - 0.0).abs() < f64::EPSILON);
    assert_eq!(s.score_scale(), ScoreScale::LexicalAdditive);
}

// --- Grounding::none shape ---------------------------------------------

#[test]
fn none_grounding_is_an_honest_zero() {
    let g = Grounding::none(2.0);
    assert_eq!(g.signal, GroundingSignal::None);
    assert_eq!(g.top_score, None);
    assert_eq!(g.score_scale, ScoreScale::LexicalAdditive);
    assert!((g.confidence - 0.0).abs() < f64::EPSILON);
    assert_eq!(g.decision, InjectionDecision::Skipped);
    assert!((g.threshold - 2.0).abs() < f64::EPSILON);
    assert_eq!(g.effective_budget, None);
    assert!(!g.engaged);
    assert!(g.seeds.is_empty());
}

// --- from_parts derives, it does not trust ------------------------------

#[test]
fn from_parts_derives_confidence_and_threshold() {
    let g = Grounding::from_parts(
        GroundingSignal::Lexical,
        Some(4.0),
        ScoreScale::LexicalAdditive,
        &gate(),
        InjectionDecision::Scaled,
        Some(600),
        vec![seed("urn:ngm:class:knowledge-graph", 4.0, true)],
    );
    assert!((g.confidence - 0.5).abs() < f64::EPSILON);
    assert!((g.threshold - 2.0).abs() < f64::EPSILON);
    assert_eq!(g.effective_budget, Some(600));
    assert!(g.engaged, "a Scaled decision is an engagement");
}

#[test]
fn from_parts_marks_a_skip_disengaged() {
    let g = Grounding::from_parts(
        GroundingSignal::Lexical,
        Some(1.5),
        ScoreScale::LexicalAdditive,
        &gate(),
        InjectionDecision::Skipped,
        None,
        Vec::new(),
    );
    assert!(!g.engaged);
}

#[test]
fn with_engaged_corrects_an_empty_block() {
    let g = Grounding::from_parts(
        GroundingSignal::Lexical,
        Some(19.5),
        ScoreScale::LexicalAdditive,
        &gate(),
        InjectionDecision::Full,
        Some(1500),
        Vec::new(),
    )
    .with_engaged(false);
    assert!(!g.engaged, "the clamp trimmed everything; that is no serve");
}

#[test]
fn with_threshold_carries_the_verbatim_bar() {
    let g = Grounding::none(2.0).with_threshold(0.92);
    assert!((g.threshold - 0.92).abs() < f64::EPSILON);
}

#[test]
fn with_signal_restates_every_confidence_on_the_new_scale() {
    let g = Grounding::from_parts(
        GroundingSignal::Lexical,
        Some(0.83),
        ScoreScale::LexicalAdditive,
        &gate(),
        InjectionDecision::Full,
        Some(1500),
        vec![
            seed("urn:ngm:class:a", 0.83, true),
            seed("urn:ngm:class:b", 0.6, false),
        ],
    )
    .with_signal(GroundingSignal::Semantic, 8.0);
    assert_eq!(g.signal, GroundingSignal::Semantic);
    assert_eq!(g.score_scale, ScoreScale::Cosine);
    assert!((g.confidence - 0.83).abs() < f64::EPSILON);
    assert!((g.seeds[0].confidence - 0.83).abs() < f64::EPSILON);
    assert!((g.seeds[1].confidence - 0.6).abs() < f64::EPSILON);
}

// --- the wire form ------------------------------------------------------

#[test]
fn enums_serialise_lowercase_and_kebab() {
    let j = |v: &serde_json::Value| serde_json::to_string(v).unwrap();
    assert_eq!(
        j(&serde_json::to_value(GroundingSignal::Lexical).unwrap()),
        "\"lexical\""
    );
    assert_eq!(
        j(&serde_json::to_value(GroundingSignal::Semantic).unwrap()),
        "\"semantic\""
    );
    assert_eq!(
        j(&serde_json::to_value(GroundingSignal::None).unwrap()),
        "\"none\""
    );
    for (d, want) in [
        (InjectionDecision::Full, "\"full\""),
        (InjectionDecision::Scaled, "\"scaled\""),
        (InjectionDecision::Skipped, "\"skipped\""),
        (InjectionDecision::Verbatim, "\"verbatim\""),
    ] {
        assert_eq!(j(&serde_json::to_value(d).unwrap()), want);
    }
    assert_eq!(
        j(&serde_json::to_value(ScoreScale::LexicalAdditive).unwrap()),
        "\"lexical-additive\""
    );
    assert_eq!(
        j(&serde_json::to_value(ScoreScale::Cosine).unwrap()),
        "\"cosine\""
    );
}

#[test]
fn grounding_round_trips_through_json() {
    let g = Grounding::from_parts(
        GroundingSignal::Lexical,
        Some(19.5),
        ScoreScale::LexicalAdditive,
        &gate(),
        InjectionDecision::Full,
        Some(1500),
        vec![seed("urn:ngm:class:knowledge-graph", 19.5, true)],
    );
    let text = serde_json::to_string(&g).expect("grounding serialises");
    let back: Grounding = serde_json::from_str(&text).expect("grounding deserialises");
    assert_eq!(back.signal, GroundingSignal::Lexical);
    assert_eq!(back.decision, InjectionDecision::Full);
    assert_eq!(back.score_scale, ScoreScale::LexicalAdditive);
    assert_eq!(back.seeds.len(), 1);
    assert!(back.seeds[0].injected);

    // The field NAMES are the contract; pin them.
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    for key in [
        "signal",
        "top_score",
        "score_scale",
        "confidence",
        "decision",
        "threshold",
        "effective_budget",
        "engaged",
        "seeds",
    ] {
        assert!(v.get(key).is_some(), "missing grounding field {key}");
    }
    for key in [
        "iri",
        "score",
        "confidence",
        "quality",
        "provenance",
        "injected",
    ] {
        assert!(v["seeds"][0].get(key).is_some(), "missing seed field {key}");
    }
}

#[test]
fn seed_grounding_from_match_converts_both_scales() {
    use crate::model::{ConceptMatch, Iri, MatchProvenance};

    let lexical = ConceptMatch {
        iri: Iri::from_slug("knowledge-graph"),
        score: 19.5,
        provenance: MatchProvenance::Lexical,
    };
    let s = SeedGrounding::from_match(&lexical, GroundingSignal::Lexical, 8.0, Some(0.9), true);
    assert_eq!(s.iri, "urn:ngm:class:knowledge-graph");
    assert!((s.score - 19.5).abs() < 1e-4);
    assert!(
        (s.confidence - 1.0).abs() < f64::EPSILON,
        "unbounded score saturates"
    );
    assert!((s.quality.unwrap() - 0.9).abs() < 1e-6);
    assert_eq!(s.provenance, "lexical");
    assert!(s.injected);

    let semantic = ConceptMatch {
        iri: Iri::from_slug("graph-database"),
        score: 0.83,
        provenance: MatchProvenance::SemanticHnsw,
    };
    let s = SeedGrounding::from_match(&semantic, GroundingSignal::Semantic, 8.0, None, false);
    // A cosine reports itself, NOT cosine/strong_match_score.
    assert!((s.confidence - 0.83).abs() < 1e-6);
    assert_eq!(s.provenance, "semantic-hnsw");
    assert_eq!(s.quality, None);
    assert!(!s.injected);
}

#[test]
fn decision_enums_are_hashable_for_counters() {
    use std::collections::HashMap;

    // The facade keys its /health rolling-window counters off these.
    let mut counts: HashMap<InjectionDecision, usize> = HashMap::new();
    *counts.entry(InjectionDecision::Full).or_default() += 1;
    *counts.entry(InjectionDecision::Full).or_default() += 1;
    *counts.entry(InjectionDecision::Skipped).or_default() += 1;
    assert_eq!(counts[&InjectionDecision::Full], 2);
    assert_eq!(counts[&InjectionDecision::Skipped], 1);

    let mut signals: HashMap<GroundingSignal, usize> = HashMap::new();
    *signals.entry(GroundingSignal::Semantic).or_default() += 1;
    assert_eq!(signals[&GroundingSignal::Semantic], 1);

    let mut scales: HashMap<ScoreScale, usize> = HashMap::new();
    *scales.entry(ScoreScale::Cosine).or_default() += 1;
    assert_eq!(scales[&ScoreScale::Cosine], 1);
}
