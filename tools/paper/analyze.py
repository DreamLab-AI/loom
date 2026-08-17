#!/usr/bin/env python3
"""Paired statistical analysis for the paper-v2 live-ecosystem experiment.

Input: judged scores per (set, id, arm) — the same question answered by the same
model through two serving paths (loom scaffold vs raw). The paired design means
every comparison is within-question; between-question variance cancels.

Per question set (arcane / thin / general) and pooled:
  - mean score per arm, paired mean difference (loom - raw)
  - 95% bootstrap CI on the paired mean difference (BCa-free percentile,
    10,000 resamples, seeded)
  - Wilcoxon signed-rank test (exact for n<=25 via scipy fallback to normal
    approximation; pure-python implementation here, no scipy dependency)
  - Cliff's delta on the paired differences (dominance effect size)
  - Holm-Bonferroni correction across the reported set-level tests

Also summarises the ecosystem telemetry captured by the loom arm: engagement
rate (fusion_path != NoMatch), injected-token distribution, latency deltas.

Usage:
  python3 tools/paper/analyze.py \
      --scores uplift-results/paper-v2/judged.json \
      --live uplift-results/paper-v2/live-results.jsonl \
      --out uplift-results/paper-v2/analysis.json
"""
from __future__ import annotations
import argparse, json, math, random, sys
from collections import defaultdict
from pathlib import Path


def bootstrap_ci(diffs, n=10_000, alpha=0.05, seed=7):
    rng = random.Random(seed)
    k = len(diffs)
    means = sorted(sum(rng.choice(diffs) for _ in range(k)) / k for _ in range(n))
    lo = means[int((alpha / 2) * n)]
    hi = means[int((1 - alpha / 2) * n) - 1]
    return lo, hi


def wilcoxon_signed_rank(diffs):
    """Two-sided Wilcoxon signed-rank, normal approximation with tie/zero
    handling (Pratt zeros dropped). Returns (W, z, p)."""
    d = [x for x in diffs if x != 0]
    n = len(d)
    if n == 0:
        return 0.0, 0.0, 1.0
    ranked = sorted((abs(x), i) for i, x in enumerate(d))
    ranks = [0.0] * n
    i = 0
    while i < n:
        j = i
        while j + 1 < n and ranked[j + 1][0] == ranked[i][0]:
            j += 1
        avg = (i + j) / 2 + 1
        for k in range(i, j + 1):
            ranks[ranked[k][1]] = avg
        i = j + 1
    w_plus = sum(r for r, x in zip(ranks, d) if x > 0)
    mu = n * (n + 1) / 4
    sigma = math.sqrt(n * (n + 1) * (2 * n + 1) / 24)
    if sigma == 0:
        return w_plus, 0.0, 1.0
    z = (w_plus - mu) / sigma
    p = 2 * (1 - 0.5 * (1 + math.erf(abs(z) / math.sqrt(2))))
    return w_plus, z, p


def cliffs_delta(diffs):
    """Dominance of positive over negative paired differences."""
    pos = sum(1 for x in diffs if x > 0)
    neg = sum(1 for x in diffs if x < 0)
    n = len(diffs)
    return (pos - neg) / n if n else 0.0


def holm(pvals):
    """Holm-Bonferroni adjusted p-values, order-preserving."""
    order = sorted(range(len(pvals)), key=lambda i: pvals[i])
    m = len(pvals)
    adj = [0.0] * m
    running = 0.0
    for rank, i in enumerate(order):
        running = max(running, (m - rank) * pvals[i])
        adj[i] = min(1.0, running)
    return adj


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--scores", type=Path, required=True,
                    help="judged.json: list of {set,id,arm,score}")
    ap.add_argument("--live", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args(argv)

    scores = json.load(open(args.scores))
    by_q = defaultdict(dict)  # (set,id) -> {arm: score}
    for s in scores:
        by_q[(s["set"], s["id"])][s["arm"]] = s["score"]

    sets = sorted({k[0] for k in by_q})
    report, pvals, labels = {}, [], []
    for name in sets + ["pooled"]:
        keys = [k for k in by_q if (name == "pooled" or k[0] == name)
                and "loom" in by_q[k] and "raw" in by_q[k]]
        diffs = [by_q[k]["loom"] - by_q[k]["raw"] for k in keys]
        if not diffs:
            continue
        lo, hi = bootstrap_ci(diffs)
        w, z, p = wilcoxon_signed_rank(diffs)
        report[name] = {
            "n_pairs": len(diffs),
            "mean_loom": round(sum(by_q[k]["loom"] for k in keys) / len(keys), 4),
            "mean_raw": round(sum(by_q[k]["raw"] for k in keys) / len(keys), 4),
            "paired_mean_diff": round(sum(diffs) / len(diffs), 4),
            "boot95_lo": round(lo, 4), "boot95_hi": round(hi, 4),
            "wilcoxon_W": w, "wilcoxon_z": round(z, 3), "p_raw": round(p, 6),
            "cliffs_delta": round(cliffs_delta(diffs), 4),
            "wins": sum(1 for d in diffs if d > 0),
            "losses": sum(1 for d in diffs if d < 0),
            "ties": sum(1 for d in diffs if d == 0),
        }
        if name != "pooled":
            pvals.append(p)
            labels.append(name)
    for lbl, adj in zip(labels, holm(pvals)):
        report[lbl]["p_holm"] = round(adj, 6)

    # ecosystem telemetry from the live run
    tel = defaultdict(lambda: {"engaged": 0, "total": 0, "injected": [],
                               "lat_loom": [], "lat_raw": []})
    for line in open(args.live):
        r = json.loads(line)
        if "error" in r:
            continue
        t = tel[r["set"]]
        if r["arm"] == "loom":
            t["total"] += 1
            lm = r.get("loom") or {}
            if lm.get("fusion_path") and lm["fusion_path"] != "NoMatch":
                t["engaged"] += 1
                t["injected"].append(lm.get("injected_tokens") or 0)
            t["lat_loom"].append(r["latency_s"])
        else:
            t["lat_raw"].append(r["latency_s"])
    telemetry = {}
    for name, t in tel.items():
        inj = sorted(t["injected"])
        telemetry[name] = {
            "engagement_rate": round(t["engaged"] / t["total"], 3) if t["total"] else None,
            "injected_tokens_median": inj[len(inj) // 2] if inj else 0,
            "latency_median_loom_s": round(sorted(t["lat_loom"])[len(t["lat_loom"]) // 2], 1) if t["lat_loom"] else None,
            "latency_median_raw_s": round(sorted(t["lat_raw"])[len(t["lat_raw"]) // 2], 1) if t["lat_raw"] else None,
        }

    out = {"paired": report, "telemetry": telemetry}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    json.dump(out, open(args.out, "w"), indent=1)
    print(json.dumps(out, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
