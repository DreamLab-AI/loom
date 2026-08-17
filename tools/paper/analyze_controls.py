#!/usr/bin/env python3
"""Control-arm contrast analysis for paper-v2 §7.3 (the attribution evidence).

Paired, per-set and pooled-over-arcane+thin contrasts on the 0-5 judge scale:
  each control arm − raw          (how much of the effect each variant retains)
  true − irrelevant               (the content-specific component)
  true − shuffled / true − masked (structure and entity-name components)
with 10k-resample bootstrap CIs and Wilcoxon signed-rank per contrast.

Mechanism check: empty-completion rate per arm at the 1536 budget (a system
message may change reasoning length and thus token exhaustion).

Merges results into uplift-results/paper-v2/analysis.json under "controls".
"""
from __future__ import annotations
import json, sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from analyze import bootstrap_ci, wilcoxon_signed_rank, cliffs_delta  # noqa: E402

DIR = Path("uplift-results/paper-v2")
ARMS = ("true", "shuffled", "masked", "irrelevant")


def main():
    judged = json.load(open(DIR / "judged.json"))
    by = defaultdict(dict)
    for x in judged:
        by[(x["set"], x["id"])][x["arm"]] = x["score"]

    contrasts = [(a, "raw") for a in ARMS] + [
        ("true", "irrelevant"), ("true", "shuffled"), ("true", "masked"),
        ("loom", "true"),
    ]
    out = {}
    for scope in ("arcane", "thin", "pooled"):
        keys = [k for k in by if scope == "pooled" and k[0] in ("arcane", "thin")
                or k[0] == scope]
        scope_out = {}
        for hi, lo in contrasts:
            pairs = [(by[k][hi], by[k][lo]) for k in keys
                     if hi in by[k] and lo in by[k]]
            if len(pairs) < 5:
                continue
            diffs = [a - b for a, b in pairs]
            ci_lo, ci_hi = bootstrap_ci(diffs)
            w, z, p = wilcoxon_signed_rank(diffs)
            scope_out[f"{hi}-{lo}"] = {
                "n": len(diffs),
                "mean_diff": round(sum(diffs) / len(diffs), 4),
                "boot95": [round(ci_lo, 4), round(ci_hi, 4)],
                "wilcoxon_p": round(p, 6),
                "cliffs_delta": round(cliffs_delta(diffs), 4),
                "wins": sum(1 for d in diffs if d > 0),
                "losses": sum(1 for d in diffs if d < 0),
                "ties": sum(1 for d in diffs if d == 0),
            }
        out[scope] = scope_out

    # mechanism: empty-rate per arm at the 1536 budget
    empties = defaultdict(lambda: [0, 0])  # arm -> [empty, total]
    for fname in ("control-results.jsonl", "live-results.jsonl"):
        for line in open(DIR / fname):
            r = json.loads(line)
            if "error" in r or "skipped" in r or r["set"] == "general":
                continue
            if r.get("retry_budget"):
                continue  # 4096 retries excluded: mechanism concerns the 1536 budget
            t = empties[r["arm"]]
            t[1] += 1
            if not (r.get("content") or "").strip():
                t[0] += 1
    out["empty_rate_1536"] = {
        arm: {"empty": e, "total": n, "rate": round(e / n, 3) if n else None}
        for arm, (e, n) in sorted(empties.items())
    }

    analysis = json.load(open(DIR / "analysis.json"))
    analysis["controls"] = out
    json.dump(analysis, open(DIR / "analysis.json", "w"), indent=1)
    print(json.dumps(out, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
