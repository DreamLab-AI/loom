#!/usr/bin/env python3
"""Emit www/data/scaffold-index.json — a compact one-file index of every
public ontology class for scaffold/agent consumers.

Schema contract (version 1):
{
  "version": 1,
  "generated": "<ISO8601>",
  "counts": {"classes": <int>},
  "classes": {
    "<slug>": {
      "t": "<Title>",
      "d": "<definition, truncated to 400 chars>",
      "dom": "<domain|''>",
      "q": <qualityScore float|null>,
      "m": "<maturity|''>",
      "sup": ["<direct parent slug>", ...],
      "isup": ["<inferred (non-direct) ancestor slug>", ...],
      "rel": {"hasPart": [...], ...},   # empty lists omitted
      "bl": ["<backlink slug>", ...]    # max 20
    }
  }
}
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

from .jsonld_parser import PageData
from .reason import Closure, ref_slug

DEFINITION_CAP = 400
BACKLINK_CAP = 20

# Contract key order for the "rel" object.
REL_KEY_ORDER: list[tuple[str, str]] = [
    ("has_part", "hasPart"),
    ("requires", "requires"),
    ("enables", "enables"),
    ("depends_on", "dependsOn"),
    ("implements", "implements"),
    ("uses", "uses"),
    ("part_of", "partOf"),
    ("related_to", "relatedTo"),
    ("bridges_to", "bridgesTo"),
    ("supports", "supports"),
    ("standardized_by", "standardizedBy"),
    ("contrasts_with", "contrastsWith"),
]


def emit_scaffold_index(pages: list[PageData], closure: Closure,
                        backlinks: dict[str, list[str]], path: Path) -> dict:
    """Write scaffold-index.json; returns the emitted document."""
    classes: dict[str, dict] = {}

    candidates = [
        p for p in pages
        if p.is_public and p.ontology_class and p.ontology_class.iri
        and p.ontology_class.entity_type == "Class"
    ]

    for page in sorted(candidates, key=lambda p: ref_slug(p.ontology_class.iri)):
        oc = page.ontology_class
        slug = ref_slug(oc.iri)
        if slug in classes:
            continue  # first definition wins

        rel: dict[str, list[str]] = {}
        for attr, key in REL_KEY_ORDER:
            targets: list[str] = []
            for r in getattr(oc.relations, attr):
                t = ref_slug(r.iri)
                if t and t not in targets:
                    targets.append(t)
            if targets:  # empty lists MUST be omitted
                rel[key] = targets

        classes[slug] = {
            "t": oc.label or page.title,
            "d": (oc.definition or "")[:DEFINITION_CAP],
            "dom": oc.domain or "",
            "q": oc.quality_score if oc.quality_score else None,
            "m": oc.maturity or "",
            "sup": closure.direct_parents.get(slug, []),
            "isup": closure.inferred_superclasses.get(slug, []),
            "rel": rel,
            "bl": backlinks.get(page.slug, [])[:BACKLINK_CAP],
        }

    doc = {
        "version": 1,
        "generated": datetime.now(timezone.utc).isoformat(),
        "counts": {"classes": len(classes)},
        "classes": classes,
    }

    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(doc, f, separators=(",", ":"))
    return doc
