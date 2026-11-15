//! ADR-2024: source attribution and review metadata survive consumer loading.
use loom_domain::{Iri, LexicalIndex};
use loom_scaffold::{index::ScaffoldIndex, LexicalRetriever};

#[test]
fn legacy_processing_does_not_invent_content_authorship_or_review() {
    let index = ScaffoldIndex::from_json_str(r#"{"version":1,"classes":{"a":{"t":"A"}}}"#).unwrap();
    let unit = LexicalRetriever::from_index(index)
        .resolve(&Iri::from_slug("a"))
        .unwrap();
    assert_eq!(unit.provenance.generated.by.as_str(), "process:vault");
    assert!(unit.provenance.content_attribution.is_none());
    assert!(unit.provenance.review_binding.is_none());
    assert_eq!(unit.provenance.verified, []);
}

#[test]
fn explicit_attribution_and_stale_human_stamp_are_preserved() {
    let index = ScaffoldIndex::from_json_str(r#"{"version":1,"classes":{"a":{"t":"A","generated":{"by":"agent:writer/1","at":"2026-11-15"},"verified":[{"by":"human:reviewer","at":"2026-11-15","keys":["definition"],"content_digest":"sha256-v1:old"}],"review_binding":{"current":0,"stale":1,"unbound":0}}}}"#).unwrap();
    let unit = LexicalRetriever::from_index(index)
        .resolve(&Iri::from_slug("a"))
        .unwrap();
    assert_eq!(
        unit.provenance.content_attribution.unwrap().by.as_str(),
        "agent:writer/1"
    );
    let status = unit.provenance.review_binding.unwrap();
    assert_eq!((status.current, status.stale, status.unbound), (0, 1, 0));
    let stamp = serde_json::to_value(&unit.provenance.verified[0]).unwrap();
    assert_eq!(stamp["keys"], serde_json::json!(["definition"]));
    assert_eq!(stamp["content_digest"], "sha256-v1:old");
}

#[test]
fn an_empty_actor_is_not_a_human_verifier() {
    assert!(!loom_domain::Actor::new("human:").is_human());
    assert!(!loom_domain::Actor::new("agent:worker/1").is_human());
    assert!(loom_domain::Actor::new("human:reviewer").is_human());
}
