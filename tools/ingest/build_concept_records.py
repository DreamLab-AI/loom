#!/usr/bin/env python3
"""Build the CONCEPT-INDEX records for RuVector ingest (ADR-136 D3 / task #17).

One record per ontology class — the seed-finding surface a semantic query matches
against. The embedded text is the human-scrutible unit's *header*: title +
definition + verbalised typed relations + taxonomy + the dfull summary, so a vector
query lands on the right IRI (validated: an RGB record built this way scored 0.875
for a Bitcoin/RGB query while an "AI Core" decoy scored 0.447).

PURE + deterministic: reads the build-derived projections (scaffold-index.json +
prose-index.json — themselves single-source per ADR-136 D4) and emits records. No
network, no embedding, no infra write. The writer (embed via Xinference + store via
the governed memory_store MCP into namespace `ontology-corpus`) is a separate step
so this half stays testable and reviewable.

Every record is keyed by IRI slug and carries the corpus `generation` stamp, so the
close-the-loop step (re-embed on promotion) is a cheap per-IRI diff.

Usage:
    python3 tools/ingest/build_concept_records.py \
        --scaffold app/data/scaffold-index.json \
        --prose app/data/prose-index.json \
        --out uplift-results/ingest/concept-records.jsonl
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Relation types in the order they read most naturally when verbalised.
REL_ORDER = ["hasPart", "requires", "enables", "dependsOn", "implements", "uses",
             "partOf", "relatedTo", "bridgesTo", "supports", "standardizedBy",
             "contrastsWith"]
NAMESPACE = "ontology-corpus"
MAX_REL_TARGETS = 8   # cap per relation type: enough signal, no dilution


def _titles(slugs, title_of, cap=MAX_REL_TARGETS):
    """Map target slugs to human titles (fallback: the slug), de-duped, capped."""
    out, seen = [], set()
    for s in slugs:
        t = title_of.get(s, s.replace("-", " "))
        if t not in seen:
            seen.add(t)
            out.append(t)
        if len(out) >= cap:
            break
    return out


def verbalise(slug, cls, dfull, title_of) -> str:
    """The embedded text: title · definition · taxonomy · relations · dfull summary.
    Kept as readable prose so the vector points at exactly the legible unit."""
    title = cls.get("t") or slug.replace("-", " ")
    parts = [f"{title}."]
    if cls.get("d"):
        parts.append(cls["d"].strip())

    # Taxonomy (direct parents first — the strongest structural signal).
    parents = _titles(cls.get("sup", []), title_of, cap=6)
    if parents:
        parts.append("Is a kind of: " + ", ".join(parents) + ".")

    # Typed relations, in reading order, empty types omitted.
    rel = cls.get("rel", {})
    rel_phrases = []
    for key in REL_ORDER:
        tgts = _titles(rel.get(key, []), title_of)
        if tgts:
            rel_phrases.append(f"{key}: {', '.join(tgts)}")
    if rel_phrases:
        parts.append("Relations — " + "; ".join(rel_phrases) + ".")

    # The dfull research-prose summary (when the class has one).
    if dfull:
        parts.append(dfull.strip())

    return " ".join(parts)


def build_records(scaffold_path: Path, prose_path: Path):
    sdoc = json.load(open(scaffold_path))
    classes = sdoc.get("classes", {})
    generation = sdoc.get("generated")
    prose = json.load(open(prose_path)).get("pages", {})
    title_of = {slug: (c.get("t") or slug) for slug, c in classes.items()}

    records = []
    for slug in sorted(classes):
        cls = classes[slug]
        dfull = (prose.get(slug) or {}).get("dfull", "") if slug in prose else ""
        text = verbalise(slug, cls, dfull, title_of)
        records.append({
            "id": slug,
            "namespace": NAMESPACE,
            "text": text,
            "metadata": {
                "slug": slug,
                "title": cls.get("t") or slug,
                "domain": cls.get("dom") or None,
                "maturity": cls.get("m") or None,
                "quality": cls.get("q"),
                "has_prose": bool(dfull),
                "n_parents": len(cls.get("sup", [])),
                "n_relations": sum(len(v) for v in cls.get("rel", {}).values()),
                "generation": generation,
            },
        })
    return records, generation


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--scaffold", default="app/data/scaffold-index.json", type=Path)
    ap.add_argument("--prose", default="app/data/prose-index.json", type=Path)
    ap.add_argument("--out", default="uplift-results/ingest/concept-records.jsonl", type=Path)
    ap.add_argument("--sample", type=int, default=1, help="print N sample records to stderr")
    args = ap.parse_args(argv)

    records, generation = build_records(args.scaffold, args.prose)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w") as f:
        for r in records:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    lens = [len(r["text"]) for r in records]
    with_prose = sum(1 for r in records if r["metadata"]["has_prose"])
    lens.sort()
    print(f"built {len(records)} concept records (generation={generation})", file=sys.stderr)
    print(f"  with dfull prose: {with_prose}/{len(records)}", file=sys.stderr)
    print(f"  text chars: min={lens[0]} median={lens[len(lens)//2]} "
          f"p95={lens[int(len(lens)*0.95)]} max={lens[-1]}", file=sys.stderr)
    print(f"  → {args.out}", file=sys.stderr)
    for r in records[:args.sample]:
        print(f"\n--- sample: {r['id']} (meta: {r['metadata']}) ---\n{r['text'][:600]}",
              file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
