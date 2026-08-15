#!/usr/bin/env python3
"""Behavioural tests for pipeline.conflicts — the importable ConflictReport API.

Run (from the logseq repo root, `pipeline` importable as a package):
    PYTHONPATH=<pytest-libs> python -m pytest pipeline/tests/test_conflicts.py -q

Covers, with in-memory corpora (no on-disk fixtures needed):
  * detect_conflicts() imports + runs on constructed PageData objects and still
    returns a list[Conflict] with the frozen as_dict() element shape.
  * Each detector fires on its minimal reproducing corpus: SUBCLASS_CYCLE,
    RELATION_CONTRADICTION, DUPLICATE_CONCEPT, TYPE_CONFLICT, and a clean case.
  * analyse(...) → ConflictReport is JSON round-trippable and its blocking()/
    ok()/exit_code honour the severity_gate without touching sys.exit.
"""

from __future__ import annotations

import json
from pathlib import Path

from pipeline.conflicts import (
    Conflict,
    ConflictReport,
    analyse,
    detect_conflicts,
    summarise,
)
from pipeline.jsonld_parser import (
    OntologyEntity,
    PageData,
    RelationSet,
    WikilinkRef,
)


# ─────────────────────────── in-memory corpus builders ───────────────────────

def _page(entity: OntologyEntity) -> PageData:
    slug = entity.iri.split(":")[-1]
    return PageData(
        path=Path(f"{slug}.md"),
        page_iri=entity.iri,
        slug=slug,
        title=entity.label,
        is_public=True,
        schema_version=2,
        ontology_class=entity,
    )


def _cls(iri, label, *, sub_class_of=(), contrasts_with=(), definition="",
         entity_type="Class", domain="d") -> OntologyEntity:
    return OntologyEntity(
        iri=iri,
        label=label,
        entity_type=entity_type,
        domain=domain,
        definition=definition,
        sub_class_of=[WikilinkRef(i, i) for i in sub_class_of],
        relations=RelationSet(contrasts_with=[WikilinkRef(i, i) for i in contrasts_with]),
    )


def _subclass_cycle_corpus():
    a = _cls("ex:A", "A", sub_class_of=["ex:B"])
    b = _cls("ex:B", "B", sub_class_of=["ex:A"])
    return [_page(a), _page(b)]


def _relation_contradiction_corpus():
    c = _cls("ex:C", "C", sub_class_of=["ex:T"], contrasts_with=["ex:T"])
    t = _cls("ex:T", "T")  # a real Class parent → no incidental TYPE_CONFLICT
    return [_page(c), _page(t)]


def _duplicate_concept_corpus():
    # Two distinct IRIs sharing a normalised label "graph node".
    a = _cls("ex:D1", "Graph Node", definition="a vertex")
    b = _cls("ex:D2", "graph  node", definition="a different thing")
    return [_page(a), _page(b)]


def _type_conflict_corpus():
    # Parent P is an Individual → a Class subclassing it is a type mismatch.
    child = _cls("ex:Ch", "Child", sub_class_of=["ex:P"])
    parent = _cls("ex:P", "Parent", entity_type="Individual")
    return [_page(child), _page(parent)]


def _clean_corpus():
    root = _cls("ex:Root", "Root")
    leaf = _cls("ex:Leaf", "Leaf", sub_class_of=["ex:Root"])
    return [_page(root), _page(leaf)]


# ───────────────────────────── detect_conflicts seam ─────────────────────────

def test_detect_conflicts_importable_and_typed():
    conflicts = detect_conflicts(_subclass_cycle_corpus())
    assert isinstance(conflicts, list)
    assert all(isinstance(c, Conflict) for c in conflicts)
    # frozen as_dict() element shape preserved for the --json CLI contract
    for c in conflicts:
        d = c.as_dict()
        assert set(d.keys()) == {"kind", "severity", "subjects", "detail"}
        assert isinstance(d["subjects"], list)


def test_subclass_cycle_detected():
    conflicts = detect_conflicts(_subclass_cycle_corpus())
    kinds = [c.kind for c in conflicts]
    assert "SUBCLASS_CYCLE" in kinds
    cyc = next(c for c in conflicts if c.kind == "SUBCLASS_CYCLE")
    assert cyc.severity == "high"
    assert set(cyc.subjects) == {"ex:A", "ex:B"}


def test_relation_contradiction_detected():
    conflicts = detect_conflicts(_relation_contradiction_corpus())
    contradictions = [c for c in conflicts if c.kind == "RELATION_CONTRADICTION"]
    assert len(contradictions) == 1
    c = contradictions[0]
    assert c.severity == "medium"
    assert c.subjects == ["ex:C", "ex:T"]
    # no incidental TYPE_CONFLICT because ex:T is a real Class
    assert not any(x.kind == "TYPE_CONFLICT" for x in conflicts)


def test_duplicate_concept_detected():
    conflicts = detect_conflicts(_duplicate_concept_corpus())
    dupes = [c for c in conflicts if c.kind == "DUPLICATE_CONCEPT"]
    assert len(dupes) == 1
    c = dupes[0]
    assert c.severity == "high"
    assert set(c.subjects) == {"ex:D1", "ex:D2"}
    # differing definitions escalate the detail text
    assert "DIFFERING" in c.detail


def test_type_conflict_detected():
    conflicts = detect_conflicts(_type_conflict_corpus())
    tcs = [c for c in conflicts if c.kind == "TYPE_CONFLICT"]
    assert len(tcs) == 1
    c = tcs[0]
    assert c.severity == "medium"
    assert c.subjects == ["ex:Ch", "ex:P"]


def test_clean_corpus_has_no_conflicts():
    assert detect_conflicts(_clean_corpus()) == []


# ───────────────────────────── ConflictReport API ────────────────────────────

def test_report_json_round_trippable():
    report = analyse(_subclass_cycle_corpus())
    assert isinstance(report, ConflictReport)
    d = report.to_dict()
    # full round-trip through json
    reloaded = json.loads(json.dumps(d))
    assert reloaded == d
    assert reloaded["severity_gate"] == "high"
    assert reloaded["summary"] == summarise(report.conflicts)
    # element shape matches the CLI's as_dict()
    for elem in reloaded["conflicts"]:
        assert set(elem.keys()) == {"kind", "severity", "subjects", "detail"}
    # to_json() is a string that parses back to the same dict
    assert json.loads(report.to_json()) == d


def test_high_severity_blocks_at_default_gate():
    report = analyse(_subclass_cycle_corpus())  # default gate "high"
    assert report.exit_code == 1
    assert report.ok() is False
    assert len(report.blocking()) == 1
    assert report.blocking()[0].kind == "SUBCLASS_CYCLE"


def test_medium_conflict_passes_high_gate_but_blocks_medium_gate():
    pages = _relation_contradiction_corpus()  # only a medium conflict
    high = analyse(pages, severity_gate="high")
    assert high.exit_code == 0
    assert high.ok() is True
    assert high.blocking() == []          # medium < high → not blocking
    # but the conflict IS present in the report
    assert any(c.kind == "RELATION_CONTRADICTION" for c in high.conflicts)

    medium = analyse(pages, severity_gate="medium")
    assert medium.exit_code == 1
    assert medium.ok() is False
    assert len(medium.blocking()) == 1


def test_clean_report_is_ok():
    report = analyse(_clean_corpus())
    assert report.conflicts == []
    assert report.ok() is True
    assert report.exit_code == 0
    assert report.summary["total"] == 0
