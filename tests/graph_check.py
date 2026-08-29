#!/usr/bin/env python3
"""Dream-cycle graph evaluator: assert the facade's graph engine is loaded.

Reads the /health JSON from stdin (piped from curl) so the dream.config.json
evaluator entrypoint needs no inline quoting — inline `python3 -c "…"` loses
its inner quotes crossing the annexe ssh `bash -lc` boundary (witnessed
2026-08-28, SyntaxError every night it ran).
"""
import json
import sys

health = json.load(sys.stdin)
graph = health.get("graph", {})
print(
    "engine:", graph.get("engine"),
    "triples:", graph.get("triples"),
    "available:", graph.get("available"),
    "loaded:", graph.get("loaded_files"),
)
sys.exit(0 if graph.get("available") and (graph.get("triples") or 0) > 0 else 1)
