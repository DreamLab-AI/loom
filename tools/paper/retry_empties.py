#!/usr/bin/env python3
"""Re-run token-exhausted pairs at a larger completion budget.

42/234 main-run completions exhausted max_tokens=1536 entirely in reasoning and
emitted no content (completion_tokens == 1536, empty message). An empty answer
is a budget artefact, not a knowledge outcome, so affected QUESTION PAIRS are
re-run — BOTH arms, so each pair stays internally consistent — at 4096. The
protocol deviation is documented in the paper's Method; the per-arm empty rate
at 1536 is reported as an observation.

Writes replacements in place: rows for affected (set,id) pairs are removed from
live-results.jsonl and re-appended from the fresh 4096 runs; stale judgements
for those pairs are purged from judged.json (re-judged by the next judge pass).
"""
from __future__ import annotations
import json, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import live_harness as lh  # noqa: E402  (reuse ask/run_one machinery)

DIR = Path("uplift-results/paper-v2")
LIVE = DIR / "live-results.jsonl"
JUDGED = DIR / "judged.json"
BIG = 4096


def main():
    rows = [json.loads(l) for l in open(LIVE)]
    affected = set()
    for r in rows:
        if "error" in r:
            continue
        if not (r.get("content") or "").strip():
            affected.add((r["set"], r["id"]))
    print(f"{len(affected)} affected pairs", file=sys.stderr)
    if not affected:
        return 0

    # purge affected rows + judgements
    keep = [r for r in rows if (r["set"], r["id"]) not in affected]
    qmeta = {}
    for name, path in lh.SETS.items():
        for q in json.load(open(path)):
            qmeta[(name, q["id"])] = q
    if JUDGED.exists():
        judged = json.load(open(JUDGED))
        judged = [j for j in judged if (j["set"], j["id"]) not in affected]
        json.dump(judged, open(JUDGED, "w"), indent=1)

    lh.MAX_TOKENS = BIG
    fresh = []
    for i, (setname, qid) in enumerate(sorted(affected)):
        q = qmeta[(setname, qid)]
        for arm, url in (("loom", lh.LOOM), ("raw", lh.RAW)):
            r = lh.run_one(setname, q, arm, url)
            r["retry_budget"] = BIG
            fresh.append(r)
        print(f"  {i+1}/{len(affected)} pairs re-run", file=sys.stderr)

    with open(LIVE, "w") as f:
        for r in keep + fresh:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    empties = sum(1 for r in fresh if "error" not in r and not (r.get("content") or "").strip())
    print(f"done: {len(fresh)} calls re-run at {BIG}; {empties} still empty", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
