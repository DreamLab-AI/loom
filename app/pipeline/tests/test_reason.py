#!/usr/bin/env python3
"""Behavioural tests for pipeline.reason + pipeline.scaffold_index.

Run:
    <venv python> -m pytest pipeline/tests/test_reason.py -q

Covers:
  * chain closure A<B<C ⇒ A.inferred == [C]
  * cycle safety (2-cycle and 3-cycle terminate; SCC-minus-self semantics)
  * owl:Thing and self-parents never appear in the closure
  * inherited relations: own-assertion exclusion, self exclusion,
    proximity-then-alpha ordering, cap at 8 per relation type
  * emit_inferred_ttl: inferred pairs only, ontology header annotations
  * scaffold-index.json shape per the version-1 contract
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from pipeline.backlinks import build_backlink_index
from pipeline.jsonld_parser import (
    OntologyEntity,
    PageData,
    RelationSet,
    WikilinkRef,
)
from pipeline.jsonld_to_page_api import build_page_api
from pipeline.reason import (
    INHERITED_RELATION_CAP,
    compute_closure,
    emit_inferred_ttl,
    ref_slug,
)
from pipeline.scaffold_index import emit_scaffold_index


def _iri(slug: str) -> str:
    return f"urn:ngm:class:{slug}"


def _page(slug, label=None, domain="ai", subclass=(), relations=None,
          public=True, definition=None, quality=0.0, maturity="draft",
          wikilinks=(), entity_type="Class"):
    label = label or slug.replace("-", " ").title()
    rs = RelationSet()
    for attr, targets in (relations or {}).items():
        setattr(rs, attr, [WikilinkRef(iri=_iri(t), label=t) for t in targets])
    oc = OntologyEntity(
        iri=_iri(slug), label=label, entity_type=entity_type, domain=domain,
        definition=definition if definition is not None else f"def {label}",
        sub_class_of=[WikilinkRef(iri=_iri(s) if ":" not in s else s, label=s)
                      for s in subclass],
        quality_score=quality, maturity=maturity, relations=rs,
    )
    return PageData(
        path=Path(f"{slug}.md"), page_iri=f"urn:visionflow:page:{slug}",
        slug=slug, title=label, is_public=public, schema_version=2,
        wikilinks=[WikilinkRef(iri=_iri(w), label=w) for w in wikilinks],
        ontology_class=oc,
    )


# ───────────────────────── transitive closure ─────────────────────────

def test_chain_closure_a_b_c():
    """A subClassOf B subClassOf C ⇒ inferred(A) == [C]."""
    pages = [
        _page("a", subclass=["b"]),
        _page("b", subclass=["c"]),
        _page("c"),
    ]
    cl = compute_closure(pages)
    assert cl.ancestors["a"] == ["b", "c"]  # proximity order
    assert cl.direct_parents["a"] == ["b"]
    assert cl.inferred_superclasses["a"] == ["c"]
    assert cl.inferred_superclasses["b"] == []
    assert cl.inferred_superclasses["c"] == []


def test_diamond_direct_parent_not_inferred():
    """A parent reachable both directly and transitively stays direct-only."""
    pages = [
        _page("a", subclass=["b", "c"]),
        _page("b", subclass=["c"]),
        _page("c"),
    ]
    cl = compute_closure(pages)
    assert set(cl.ancestors["a"]) == {"b", "c"}
    assert cl.inferred_superclasses["a"] == []  # c is asserted directly


def test_cycle_two_nodes_terminates():
    pages = [
        _page("x", subclass=["y"]),
        _page("y", subclass=["x"]),
    ]
    cl = compute_closure(pages)  # must not hang
    # SCC minus self: each sees the other, never itself
    assert cl.ancestors["x"] == ["y"]
    assert cl.ancestors["y"] == ["x"]
    assert cl.inferred_superclasses["x"] == []
    assert cl.inferred_superclasses["y"] == []


def test_cycle_three_nodes_scc_minus_self():
    pages = [
        _page("a", subclass=["b"]),
        _page("b", subclass=["c"]),
        _page("c", subclass=["a"]),
    ]
    cl = compute_closure(pages)
    assert cl.ancestors["a"] == ["b", "c"]
    assert "a" not in cl.ancestors["a"]
    assert cl.inferred_superclasses["a"] == ["c"]
    assert cl.inferred_superclasses["b"] == ["a"]
    assert cl.inferred_superclasses["c"] == ["b"]


def test_owl_thing_and_self_excluded():
    pages = [
        _page("a", subclass=["owl:Thing", "a", "b"]),
        _page("b", subclass=["owl:Thing"]),
    ]
    cl = compute_closure(pages)
    assert cl.direct_parents["a"] == ["b"]
    assert cl.ancestors["a"] == ["b"]
    assert cl.ancestors["b"] == []
    for slugs in cl.ancestors.values():
        assert "thing" not in [s.lower() for s in slugs]


def test_dangling_ancestor_included_in_closure():
    """A parent outside the corpus is still a legitimate ancestor slug."""
    pages = [_page("a", subclass=["ghost"])]
    cl = compute_closure(pages)
    assert cl.ancestors["a"] == ["ghost"]
    assert cl.inferred_superclasses["a"] == []  # direct, not inferred


# ───────────────────────── inherited relations ─────────────────────────

def test_inherited_relation_exclusion_and_cap():
    """Child inherits parent's `uses` targets minus what it already asserts,
    minus itself, capped at INHERITED_RELATION_CAP, alpha within ancestor."""
    parent_uses = [f"t{i:02d}" for i in range(10)] + ["own-target", "child"]
    pages = [
        _page("child", subclass=["parent"],
              relations={"uses": ["own-target"]}),
        _page("parent", relations={"uses": parent_uses}),
    ]
    cl = compute_closure(pages)
    inherited = cl.inherited_relations["child"]["uses"]
    slugs = [ref_slug(r.iri) for r in inherited]

    assert "own-target" not in slugs          # already asserted by child
    assert "child" not in slugs               # never inherit self
    assert len(slugs) == INHERITED_RELATION_CAP == 8
    assert slugs == sorted(slugs)             # alpha within one ancestor
    assert slugs == [f"t{i:02d}" for i in range(8)]


def test_inherited_relation_proximity_order():
    """Nearer ancestors contribute before farther ones."""
    pages = [
        _page("a", subclass=["b"]),
        _page("b", subclass=["c"], relations={"enables": ["zz-near"]}),
        _page("c", relations={"enables": ["aa-far"]}),
    ]
    cl = compute_closure(pages)
    slugs = [ref_slug(r.iri) for r in cl.inherited_relations["a"]["enables"]]
    # b (proximity 1) before c (proximity 2) despite alpha order
    assert slugs == ["zz-near", "aa-far"]


def test_inherited_relations_empty_types_omitted():
    pages = [
        _page("a", subclass=["b"]),
        _page("b", relations={"uses": ["x"]}),
        _page("x"),
    ]
    cl = compute_closure(pages)
    assert set(cl.inherited_relations["a"].keys()) == {"uses"}
    # a class with nothing to inherit has no entry at all
    assert "b" not in cl.inherited_relations


# ───────────────────────── inferred Turtle ─────────────────────────

def test_emit_inferred_ttl(tmp_path):
    rdflib = pytest.importorskip("rdflib")
    from rdflib.namespace import RDFS

    pages = [
        _page("a", subclass=["b"]),
        _page("b", subclass=["c"]),
        _page("c"),
    ]
    cl = compute_closure(pages)
    out = tmp_path / "ontology-inferred.ttl"
    n = emit_inferred_ttl(pages, cl, out)
    assert n == 1
    assert out.exists()

    g = rdflib.Graph()
    g.parse(str(out), format="turtle")
    A, B, C = (rdflib.URIRef(_iri(s)) for s in "abc")
    assert (A, RDFS.subClassOf, C) in g       # inferred pair present
    assert (A, RDFS.subClassOf, B) not in g   # direct pair NOT emitted

    # ontology header annotations
    VC = rdflib.Namespace("https://narrativegoldmine.com/ns/v1#")
    onto = rdflib.URIRef("https://narrativegoldmine.com/ontology/inferred")
    assert (onto, VC.inferenceMethod,
            rdflib.Literal("transitive-subclass-closure")) in g
    assert next(g.objects(onto, VC.generatedAt), None) is not None


# ───────────────────────── scaffold index ─────────────────────────

def _scaffold_corpus():
    return [
        _page("a", label="Alpha", domain="ai", subclass=["b"],
              relations={"uses": ["x"]}, quality=0.9, maturity="established",
              definition="d" * 500, wikilinks=["b"]),
        _page("b", label="Beta", subclass=["c"], wikilinks=["a"]),
        _page("c", label="Gamma", domain=""),
        _page("x", label="Xray"),
        _page("secret", public=False),
        _page("indiv", entity_type="Individual"),
    ]


def test_scaffold_index_shape(tmp_path):
    pages = _scaffold_corpus()
    cl = compute_closure(pages)
    backlinks = build_backlink_index(pages)
    out = tmp_path / "scaffold-index.json"
    emit_scaffold_index(pages, cl, backlinks, out)

    doc = json.loads(out.read_text())
    assert doc["version"] == 1
    assert isinstance(doc["generated"], str) and "T" in doc["generated"]
    # public Classes only: a, b, c, x (secret private; indiv is an Individual)
    assert doc["counts"]["classes"] == 4
    assert set(doc["classes"].keys()) == {"a", "b", "c", "x"}

    a = doc["classes"]["a"]
    assert a["t"] == "Alpha"
    assert len(a["d"]) == 400                  # truncated to 400 chars
    assert a["dom"] == "ai"
    assert a["q"] == 0.9
    assert a["m"] == "established"
    assert a["sup"] == ["b"]
    assert a["isup"] == ["c"]
    assert a["rel"] == {"uses": ["x"]}         # empty rel types omitted
    assert "hasPart" not in a["rel"]
    assert a["bl"] == ["b"]                    # b wikilinks to a

    c = doc["classes"]["c"]
    assert c["dom"] == ""
    assert c["q"] is None                      # 0.0 score → null
    assert c["rel"] == {}
    assert c["sup"] == [] and c["isup"] == []


def test_scaffold_backlinks_capped_at_20(tmp_path):
    target = _page("hub")
    sources = [_page(f"src-{i:02d}", wikilinks=["hub"]) for i in range(25)]
    pages = [target] + sources
    cl = compute_closure(pages)
    backlinks = build_backlink_index(pages)
    out = tmp_path / "scaffold-index.json"
    doc = emit_scaffold_index(pages, cl, backlinks, out)
    assert len(doc["classes"]["hub"]["bl"]) == 20


# ───────────────────────── page API enrichment ─────────────────────────

def test_page_api_closure_enrichment(tmp_path):
    pages = [
        _page("a", subclass=["b"]),
        _page("b", subclass=["c"], relations={"uses": ["x"]}),
        _page("c"),
        _page("x"),
    ]
    cl = compute_closure(pages)
    api_dir = tmp_path / "api" / "pages"
    build_page_api(pages, api_dir, closure=cl)

    a = json.loads((api_dir / "a.json").read_text())
    assert a["inferredSuperClasses"] == [
        {"id": _iri("c"), "label": "C", "slug": "c"}
    ]
    assert a["inheritedRelations"]["uses"] == [
        {"id": _iri("x"), "label": "x"}
    ]
    # inherited relations omit empty types
    assert "hasPart" not in a["inheritedRelations"]

    # backwards compatible without a closure
    api_dir2 = tmp_path / "api2" / "pages"
    build_page_api(pages, api_dir2)
    a2 = json.loads((api_dir2 / "a.json").read_text())
    assert "inferredSuperClasses" not in a2
    assert "inheritedRelations" not in a2
