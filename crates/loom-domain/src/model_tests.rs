//! Unit tests for `model` — split out of `model.rs` to keep that file
//! under the 500-line ceiling.

use crate::grounding::{GroundingSignal, InjectionDecision};
use crate::model::*;

#[test]
fn iri_slug_roundtrip() {
    let iri = Iri::from_slug("knowledge-graph");
    assert_eq!(iri.as_str(), "urn:ngm:class:knowledge-graph");
    assert_eq!(iri.slug(), "knowledge-graph");
    // from_slug ∘ slug is stable on the canonical form.
    assert_eq!(Iri::from_slug(iri.slug()), iri);
}

#[test]
fn iri_bare_slug_tolerance() {
    // A bare slug (no `:`) resolves to itself — the _ref_to_slug leniency.
    let bare = Iri::new("rgb-protocol");
    assert_eq!(bare.slug(), "rgb-protocol");
    // Full urn and bare slug agree on the join key.
    assert_eq!(Iri::from_slug("rgb-protocol").slug(), bare.slug());
}

#[test]
fn iri_serde_roundtrip() {
    let iri = Iri::from_slug("colour-channel");
    let json = serde_json::to_string(&iri).unwrap();
    assert_eq!(json, "\"urn:ngm:class:colour-channel\"");
    let back: Iri = serde_json::from_str(&json).unwrap();
    assert_eq!(back, iri);
}

#[test]
fn relation_kind_predicate_roundtrip() {
    let predicates = [
        ("has-part", RelationKind::HasPart),
        ("requires", RelationKind::Requires),
        ("enables", RelationKind::Enables),
        ("depends-on", RelationKind::DependsOn),
        ("implements", RelationKind::Implements),
        ("uses", RelationKind::Uses),
        ("part-of", RelationKind::PartOf),
        ("related-to", RelationKind::RelatedTo),
        ("bridges-to", RelationKind::BridgesTo),
        ("supports", RelationKind::Supports),
        ("standardized-by", RelationKind::StandardizedBy),
        ("contrasts-with", RelationKind::ContrastsWith),
    ];
    for (wire, variant) in predicates {
        // string → variant
        let parsed = RelationKind::from_predicate(wire);
        assert_eq!(parsed, variant, "from_predicate({wire})");
        // variant → string
        assert_eq!(variant.as_predicate(), wire, "as_predicate for {wire}");
        // serde round-trip through JSON keeps the plain string form.
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{wire}\""));
        let back: RelationKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn relation_kind_other_tail() {
    let k: RelationKind = serde_json::from_str("\"mentions\"").unwrap();
    assert_eq!(k, RelationKind::Other("mentions".to_owned()));
    // Other serialises as a bare string, NOT a tagged {"Other": …} object.
    assert_eq!(serde_json::to_string(&k).unwrap(), "\"mentions\"");
}

#[test]
fn scaffold_empty_shape() {
    let gen = Generation {
        id: GenerationId("build-abc".to_owned()),
        source: GenerationSource::ScaffoldIndex,
        generated_at: None,
        commit_sha: None,
        promoted_at: None,
        cluster_span_seconds: None,
        artifacts: Vec::new(),
        verified_single_generation: false,
        class_count: None,
    };
    // Deliberately exercising the deprecated constructor: it is still published
    // for one release, so its no-match shape stays under test.
    #[allow(deprecated)]
    let s = Scaffold::empty(FusionPath::NoMatch, gen);
    assert!(s.block.is_empty());
    assert!(!s.engaged);
    assert_eq!(s.approx_tokens, 0);
    assert!(s.seeds.is_empty());
    assert_eq!(s.effective_budget, 0);
    assert_eq!(s.fusion_path, FusionPath::NoMatch);
    assert_eq!(s.generation, GenerationId("build-abc".to_owned()));
    // Grounding is present on the no-match case too — an honest zero, never an
    // absent field the consumer has to infer from.
    assert_eq!(s.grounding.signal, GroundingSignal::None);
    assert_eq!(s.grounding.decision, InjectionDecision::Skipped);
    assert_eq!(s.grounding.top_score, None);
    assert_eq!(s.grounding.effective_budget, None);
    assert!(!s.grounding.engaged);
    assert!(s.grounding.seeds.is_empty());
    // The flat aliases agree with the typed form they mirror.
    assert_eq!(
        s.effective_budget,
        s.grounding.effective_budget.unwrap_or(0)
    );
}

#[test]
fn generation_id_equality() {
    let a = GenerationId("sha-1||b1".to_owned());
    let b = GenerationId("sha-1||b1".to_owned());
    let c = GenerationId("sha-2||b2".to_owned());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn generation_equality_is_identity() {
    // Two descriptors with the same id but different metadata are equal;
    // different ids are not (the never-mixed-build parity guard).
    let base = |id: &str, count: Option<usize>| Generation {
        id: GenerationId(id.to_owned()),
        source: GenerationSource::BuildManifest,
        generated_at: None,
        commit_sha: None,
        promoted_at: None,
        cluster_span_seconds: None,
        artifacts: Vec::new(),
        verified_single_generation: true,
        class_count: count,
    };
    assert_eq!(base("g1", Some(10)), base("g1", Some(9999)));
    assert_ne!(base("g1", Some(10)), base("g2", Some(10)));
}

#[test]
fn served_mode_serialises_lowercase() {
    assert_eq!(
        serde_json::to_string(&ServedMode::Delegated).unwrap(),
        "\"delegated\""
    );
    assert_eq!(
        serde_json::to_string(&ServedMode::Verbatim).unwrap(),
        "\"verbatim\""
    );
}

#[test]
fn exposure_report_shape() {
    let r = ExposureReport {
        targets: 3,
        delivered: 2,
        dropped: vec!["Graph Database".to_owned()],
    };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["targets"], 3);
    assert_eq!(v["delivered"], 2);
    assert_eq!(v["dropped"], serde_json::json!(["Graph Database"]));
    // Default is the honest empty report.
    let d = ExposureReport::default();
    assert_eq!(d.targets, 0);
    assert!(d.dropped.is_empty());
}

#[test]
fn concept_match_score_normalised_is_identity() {
    let m = ConceptMatch {
        iri: Iri::from_slug("x"),
        score: 0.87,
        provenance: MatchProvenance::SemanticHnsw,
    };
    assert!((m.score_normalised() - 0.87).abs() < f32::EPSILON);
}

#[test]
fn match_provenance_wire_spelling() {
    // `SeedGrounding::provenance` is a plain string; these are the two values
    // it may hold.
    assert_eq!(MatchProvenance::Lexical.as_str(), "lexical");
    assert_eq!(MatchProvenance::SemanticHnsw.as_str(), "semantic-hnsw");
}
