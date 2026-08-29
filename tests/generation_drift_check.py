#!/usr/bin/env python3
"""Generation drift check for the loom model-facade.

Reads a /health JSON payload on stdin and compares the served top-level
``generation`` (source: ScaffoldIndex) against ``semantic.generation``
(source: MirrorManifest). Emits a DRIFT line for each inconsistency.

Warn-only by default (exit 0) so nightly evaluators degrade gracefully;
pass ``--strict`` to exit 1 on drift.

Usage:
    curl -s --max-time 10 http://127.0.0.1:8084/health \
        | python3 tests/generation_drift_check.py
"""
import json
import sys
from datetime import datetime


def _ts(value):
    if not isinstance(value, str):
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def main() -> int:
    strict = "--strict" in sys.argv[1:]
    try:
        health = json.load(sys.stdin)
    except Exception as exc:
        print(f"DRIFT-CHECK ERROR: unreadable /health payload: {exc}")
        return 1 if strict else 0

    served = health.get("generation") or {}
    semantic = (health.get("semantic") or {}).get("generation") or {}
    index_classes = health.get("index_classes")

    drift = []

    sc = served.get("class_count")
    mc = semantic.get("class_count")
    if sc is not None and mc is not None and sc != mc:
        drift.append(f"class_count served={sc} promoted={mc} (delta={mc - sc:+d})")

    sg = _ts(served.get("generated_at"))
    mg = _ts(semantic.get("generated_at"))
    if sg and mg and sg < mg:
        drift.append(
            f"served generation {served.get('generated_at')} is older than "
            f"promoted mirror {semantic.get('generated_at')}"
        )

    if (served.get("verified_single_generation") is False
            and semantic.get("verified_single_generation") is True):
        drift.append("served index is unverified while a verified mirror generation exists")

    if index_classes is not None and mc is not None and index_classes != mc:
        drift.append(f"index_classes={index_classes} != promoted class_count={mc}")

    if drift:
        for entry in drift:
            print(f"DRIFT: {entry}")
        print(f"DRIFT-CHECK: {len(drift)} finding(s) — facade may be serving a stale generation")
        return 1 if strict else 0

    print("DRIFT-CHECK: OK — served generation matches promoted mirror generation")
    return 0


if __name__ == "__main__":
    sys.exit(main())
