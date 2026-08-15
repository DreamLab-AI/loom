#!/usr/bin/env python3
"""Pure-Python EL-profile closure over the parsed JSON-LD corpus.

Computes the transitive subClassOf closure for every ontology class,
derives the *inferred* (non-direct) superclass set, and the relations a
class inherits from its ancestors. Also serialises the inferred triples
as Turtle (www/data/ontology-inferred.ttl).

Cycle-safe: ancestor discovery is a BFS with a visited set, so classes
that participate in a subClassOf cycle simply receive the closure of
their strongly connected component minus themselves.

Usage (module):
    from pipeline.reason import compute_closure, emit_inferred_ttl
    closure = compute_closure(pages)
    emit_inferred_ttl(pages, closure, Path("www/data/ontology-inferred.ttl"))
"""

from __future__ import annotations

import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from .jsonld_parser import PageData, WikilinkRef, parse_corpus

# The 12 relation types, (RelationSet attr, JSON key) — same set the page
# API emits in its "relationships" object (jsonld_to_page_api.py).
RELATION_TYPES: list[tuple[str, str]] = [
    ("has_part", "hasPart"),
    ("requires", "requires"),
    ("enables", "enables"),
    ("depends_on", "dependsOn"),
    ("implements", "implements"),
    ("contrasts_with", "contrastsWith"),
    ("bridges_to", "bridgesTo"),
    ("uses", "uses"),
    ("supports", "supports"),
    ("standardized_by", "standardizedBy"),
    ("part_of", "partOf"),
    ("related_to", "relatedTo"),
]

# Cap on inherited targets per relation type (keeps page API entries small).
INHERITED_RELATION_CAP = 8


def ref_slug(iri: str) -> str:
    """urn:ngm:class:<slug> → <slug> (last colon-separated segment)."""
    return iri.split(":")[-1]


def _is_thing(slug: str) -> bool:
    """owl:Thing (in any spelling) is never a useful ancestor."""
    return slug.lower() in ("thing", "owl-thing", "owl:thing")


@dataclass
class Closure:
    """Result of compute_closure().

    All maps are keyed by class slug (from the class IRI, not the page slug,
    though the two normally coincide).
    """
    # slug → ALL transitive superclass slugs, ordered by proximity
    # (BFS level; alphabetical within a level, direct parents keep
    # assertion order). Never contains self or owl:Thing.
    ancestors: dict[str, list[str]] = field(default_factory=dict)
    # slug → direct parent slugs (assertion order, deduped)
    direct_parents: dict[str, list[str]] = field(default_factory=dict)
    # slug → non-direct ancestors only (proximity order)
    inferred_superclasses: dict[str, list[str]] = field(default_factory=dict)
    # slug → {json relation key → inherited WikilinkRefs} (non-empty only)
    inherited_relations: dict[str, dict[str, list[WikilinkRef]]] = field(
        default_factory=dict)
    # slug → best-known label / IRI (corpus class first, then ref metadata)
    labels: dict[str, str] = field(default_factory=dict)
    iris: dict[str, str] = field(default_factory=dict)


def compute_closure(pages: list[PageData]) -> Closure:
    closure = Closure()

    # ── collect classes and the parent graph ──────────────────────────
    class_by_slug: dict[str, PageData] = {}
    parent_map: dict[str, list[str]] = {}

    for page in pages:
        oc = page.ontology_class
        if not oc or not oc.iri:
            continue
        slug = ref_slug(oc.iri)
        if slug in class_by_slug:
            continue  # first definition wins (corpus should be unique)
        class_by_slug[slug] = page
        closure.labels[slug] = oc.label or page.title or slug
        closure.iris[slug] = oc.iri

        parents: list[str] = []
        for ref in oc.sub_class_of:
            p_slug = ref_slug(ref.iri)
            if not p_slug or _is_thing(p_slug) or p_slug == slug:
                continue
            if p_slug not in parents:
                parents.append(p_slug)
            # remember ref-level metadata for ancestors outside the corpus
            closure.labels.setdefault(p_slug, ref.label or p_slug)
            closure.iris.setdefault(p_slug, ref.iri)
        parent_map[slug] = parents

    # ── transitive closure per class (cycle-safe BFS) ─────────────────
    for slug in class_by_slug:
        direct = parent_map.get(slug, [])
        visited = {slug}
        ordered: list[str] = []
        # level 1: direct parents in assertion order
        frontier = [p for p in direct if p not in visited]
        while frontier:
            for p in frontier:
                visited.add(p)
                ordered.append(p)
            nxt: list[str] = []
            for p in frontier:
                for gp in parent_map.get(p, []):
                    if gp not in visited and gp not in nxt:
                        nxt.append(gp)
            # alphabetical within a BFS level for determinism
            frontier = sorted(nxt)

        closure.direct_parents[slug] = list(direct)
        closure.ancestors[slug] = ordered
        direct_set = set(direct)
        closure.inferred_superclasses[slug] = [
            a for a in ordered if a not in direct_set
        ]

    # ── inherited relations (proximity order, capped, deduped) ────────
    for slug, page in class_by_slug.items():
        oc = page.ontology_class
        inherited: dict[str, list[WikilinkRef]] = {}
        for attr, json_key in RELATION_TYPES:
            own = {ref_slug(r.iri) for r in getattr(oc.relations, attr)}
            seen: set[str] = set(own)
            seen.add(slug)  # a class never inherits a relation to itself
            collected: list[WikilinkRef] = []
            for anc in closure.ancestors.get(slug, []):
                anc_page = class_by_slug.get(anc)
                if anc_page is None:
                    continue  # ancestor outside the corpus: nothing to inherit
                anc_refs = getattr(anc_page.ontology_class.relations, attr)
                # alphabetical by target slug within one ancestor
                for r in sorted(anc_refs, key=lambda r: ref_slug(r.iri)):
                    t = ref_slug(r.iri)
                    if t in seen:
                        continue
                    seen.add(t)
                    collected.append(r)
                    if len(collected) >= INHERITED_RELATION_CAP:
                        break
                if len(collected) >= INHERITED_RELATION_CAP:
                    break
            if collected:
                inherited[json_key] = collected
        if inherited:
            closure.inherited_relations[slug] = inherited

    return closure


def emit_inferred_ttl(pages: list[PageData], closure: Closure,
                      path: Path) -> int:
    """Write the inferred (non-direct) subClassOf pairs as Turtle.

    Returns the number of inferred rdfs:subClassOf triples written.
    Output is deterministic apart from the vc:generatedAt timestamp.
    """
    from rdflib import Graph, Literal, Namespace, URIRef
    from rdflib.namespace import OWL, RDF, RDFS, XSD

    VC = Namespace("https://narrativegoldmine.com/ns/v1#")

    g = Graph()
    g.bind("owl", OWL)
    g.bind("rdfs", RDFS)
    g.bind("xsd", XSD)
    g.bind("vc", VC)

    ontology_uri = URIRef("https://narrativegoldmine.com/ontology/inferred")
    g.add((ontology_uri, RDF.type, OWL.Ontology))
    g.add((ontology_uri, RDFS.label,
           Literal("NarrativeGoldmine Inferred Axioms", lang="en")))
    g.add((ontology_uri, VC.inferenceMethod,
           Literal("transitive-subclass-closure")))
    g.add((ontology_uri, VC.generatedAt,
           Literal(datetime.now(timezone.utc).isoformat(),
                   datatype=XSD.dateTime)))

    count = 0
    for slug in sorted(closure.inferred_superclasses):
        class_iri = closure.iris.get(slug, f"urn:ngm:class:{slug}")
        for anc in closure.inferred_superclasses[slug]:
            anc_iri = closure.iris.get(anc, f"urn:ngm:class:{anc}")
            g.add((URIRef(class_iri), RDFS.subClassOf, URIRef(anc_iri)))
            count += 1

    path.parent.mkdir(parents=True, exist_ok=True)
    g.serialize(destination=str(path), format="turtle")
    return count


def main():
    pages_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
        "mainKnowledgeGraph/pages")
    out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(
        "/tmp/ontology-inferred.ttl")

    pages = parse_corpus(pages_dir)
    closure = compute_closure(pages)
    n = emit_inferred_ttl(pages, closure, out_path)
    nontrivial = sum(1 for v in closure.inferred_superclasses.values() if v)
    print(f"Closure: {len(closure.ancestors)} classes, "
          f"{nontrivial} with inferred superclasses, "
          f"{n} inferred triples → {out_path}")


if __name__ == "__main__":
    main()
