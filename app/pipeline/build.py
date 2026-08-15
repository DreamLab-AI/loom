#!/usr/bin/env python3
"""Orchestrate the full JSON-LD publishing pipeline.

Usage:
    python -m pipeline.build [pages_dir] [output_dir]

Runs all stages: parse → filter → turtle + webvowl + page_api + search_index
"""

import json
import sys
import time
from pathlib import Path

from .backlinks import build_backlink_index
from .jsonld_parser import parse_corpus
from .jsonld_to_turtle import build_graph
from .jsonld_to_webvowl import build_webvowl
from .jsonld_to_page_api import build_page_api
from .jsonld_to_search import build_search_index
from .emit_graph_tiers import emit_graph_tiers
from .reason import compute_closure, emit_inferred_ttl
from .scaffold_index import emit_scaffold_index
from .validate import validate_corpus


def build(pages_dir: Path, output_dir: Path):
    t0 = time.time()

    # Stage 1: Parse
    print(f"[1/9] Parsing {pages_dir}...")
    pages = parse_corpus(pages_dir)
    with_oc = sum(1 for p in pages if p.ontology_class)
    public = sum(1 for p in pages if p.is_public)
    print(f"       {len(pages)} pages ({with_oc} OntologyClass, {public} public)")

    # Stage 2: Validate
    print("[2/9] Validating...")
    report = validate_corpus(pages)
    s = report.summary()
    print(f"       {s['errors']} errors, {s['warnings']} warnings")
    if report.errors:
        print("       ERRORS found (non-blocking for build):")
        for i in report.errors[:10]:
            print(f"         [{i.code}] {i.path}: {i.message}")

    # Stage 3a: Turtle
    print("[3/9] Generating Turtle...")
    g = build_graph(pages, public_only=True)
    ttl_path = output_dir / "data" / "ontology.ttl"
    ttl_path.parent.mkdir(parents=True, exist_ok=True)
    g.serialize(destination=str(ttl_path), format="turtle")
    print(f"       {len(g)} triples → {ttl_path}")

    # Stage 3b: WebVOWL
    print("[4/9] Generating WebVOWL JSON...")
    vowl = build_webvowl(pages)
    vowl_path = output_dir / "data" / "ontology.json"
    with open(vowl_path, "w") as f:
        json.dump(vowl, f)
    print(f"       {len(vowl['class'])} classes → {vowl_path}")

    # Stage 4: Reasoning closure (EL-profile transitive subClassOf)
    print("[5/9] Computing reasoning closure...")
    closure = compute_closure(pages)
    inferred_path = output_dir / "data" / "ontology-inferred.ttl"
    inferred_count = emit_inferred_ttl(pages, closure, inferred_path)
    print(f"       {len(closure.ancestors)} classes, "
          f"{inferred_count} inferred triples → {inferred_path}")

    # Stage 5: Page API (closure-enriched)
    print("[6/9] Generating Page API...")
    api_dir = output_dir / "api" / "pages"
    count = build_page_api(pages, api_dir, closure=closure)
    print(f"       {count} page files → {api_dir}")

    # Stage 6: Search Index
    print("[7/9] Generating Search Index...")
    idx = build_search_index(pages)
    idx_path = output_dir / "api" / "search-index.json"
    with open(idx_path, "w") as f:
        json.dump(idx, f)
    print(f"       {len(idx)} entries → {idx_path}")

    # Stage 7: Graph tiers (NGG1) — ADR-NG-001 §2
    print("[8/9] Emitting graph tiers (NGG1)...")
    tiers = emit_graph_tiers(pages, output_dir)
    print(f"       {tiers['nodes']} nodes, {tiers['edges']} edges → {tiers['graph_dir']}")

    # Stage 8: Scaffold index (compact one-file class index)
    print("[9/9] Emitting scaffold index...")
    backlinks = build_backlink_index(pages)
    scaffold_path = output_dir / "data" / "scaffold-index.json"
    scaffold = emit_scaffold_index(pages, closure, backlinks, scaffold_path)
    print(f"       {scaffold['counts']['classes']} classes → {scaffold_path}")

    # Write context
    context_dir = output_dir / "api" / "schema"
    context_dir.mkdir(parents=True, exist_ok=True)
    context = {
        "@context": {
            "vc": "https://narrativegoldmine.com/ns/v1#",
            "owl": "http://www.w3.org/2002/07/owl#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "prov": "http://www.w3.org/ns/prov#",
        }
    }
    with open(context_dir / "context.jsonld", "w") as f:
        json.dump(context, f, indent=2)

    # Write validation report
    report_data = report.summary()
    report_data["issues"] = [
        {"path": i.path, "severity": i.severity, "code": i.code, "message": i.message}
        for i in report.issues
    ]
    with open(output_dir / "api" / "validation-report.json", "w") as f:
        json.dump(report_data, f, indent=2)

    elapsed = time.time() - t0
    print(f"\nPipeline complete in {elapsed:.1f}s")
    print(f"  Output: {output_dir}")
    print(f"  Classes: {len(vowl['class'])}")
    print(f"  Triples: {len(g)}")
    print(f"  Pages: {count}")
    print(f"  Search: {len(idx)}")

    return report


def main():
    pages_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("mainKnowledgeGraph/pages")
    output_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("www")

    report = build(pages_dir, output_dir)
    sys.exit(1 if report.errors else 0)


if __name__ == "__main__":
    main()
