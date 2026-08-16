#!/usr/bin/env python3
"""Crossover analysis: judged answer-quality vs web-findability, three arms.

Consumes one or more judged files (bench/quality/judge.py output) + the merged
gold (for findability), and produces the pre-print's central figure data:

  - quality (0-5) as a function of web-findability bin, for each grounding arm
    (raw / ontology-scaffold / web-search), averaged across models -> the
    crossover curve.
  - per-model raw vs scaffold vs web, split public/private stratum.
  - self-preference check: flag models whose judge shared their family.

Usage: python3 bench/quality/crossover.py --judged 'uplift-results/quality/judged-*.json' \
         --gold uplift-results/quality/gold-merged.json --out uplift-results/quality/crossover.md
"""
from __future__ import annotations
import argparse, glob, json, statistics, sys


def mean(xs):
    xs = [x for x in xs if isinstance(x, (int, float))]
    return statistics.mean(xs) if xs else None


def f(x, n=2):
    return "-" if x is None else f"{x:.{n}f}"


BINS = [(0.0, 0.4, "0.0-0.4 (private)"), (0.4, 0.6, "0.4-0.6"),
        (0.6, 0.8, "0.6-0.8"), (0.8, 1.01, "0.8-1.0 (public)")]


def binof(x):
    for lo, hi, name in BINS:
        if x is not None and lo <= x < hi:
            return name
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--judged", required=True, help="glob of judged json files")
    ap.add_argument("--gold", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    rows = []
    for path in glob.glob(args.judged):
        rows.extend(json.load(open(path)))
    rows = [r for r in rows if isinstance(r.get("score"), int)]
    if not rows:
        raise SystemExit("no judged rows")
    gold = {g["id"]: g for g in json.load(open(args.gold))}
    for r in rows:  # backfill findability from gold if missing
        if r.get("findability") is None:
            r["findability"] = gold.get(r["id"], {}).get("web_findability")

    # arm label: raw / scaffold(ontology) / web
    def arm(r):
        return {"raw": "raw", "scaffold": "ontology", "web": "web"}.get(r["mode"], r["mode"])
    models = sorted({r["model"] for r in rows if r["mode"] in ("raw", "scaffold")})

    out = []
    out.append("# Crossover analysis — judged quality vs web-findability\n")
    out.append(f"{len(rows)} gradings, {len(models)} models + web arm. "
               "Quality = judge 0-5 vs independent frontier gold.\n")

    # ---- 1. crossover: quality by findability bin, per arm (avg across models) ----
    out.append("## Quality by findability bin, per grounding arm (avg across models)\n")
    out.append("| findability bin | raw | ontology | web | n(questions) |")
    out.append("|---|---|---|---|---|")
    coords = {"raw": [], "ontology": [], "web": []}
    for lo, hi, name in BINS:
        qids = {r["id"] for r in rows if r.get("findability") is not None and lo <= r["findability"] < hi}
        cells = {}
        for a in ("raw", "ontology", "web"):
            cells[a] = mean([r["score"] for r in rows if arm(r) == a
                             and r.get("findability") is not None and lo <= r["findability"] < hi])
        out.append(f"| {name} | {f(cells['raw'])} | {f(cells['ontology'])} | {f(cells['web'])} | {len(qids)} |")
        xmid = round((lo + min(hi, 1.0)) / 2, 2)
        for a in ("raw", "ontology", "web"):
            if cells[a] is not None:
                coords[a].append((xmid, cells[a]))
    out.append("")

    # ---- 2. per-model, by stratum ----
    out.append("## Per-model quality (0-5), by stratum\n")
    out.append("| model | pub raw | pub onto | Δpub | priv raw | priv onto | Δpriv | fam-match |")
    out.append("|---|---|---|---|---|---|---|---|")
    for m in models:
        def q(mode, stratum):
            return mean([r["score"] for r in rows if r["model"] == m and r["mode"] == mode and r["stratum"] == stratum])
        pr, po = q("raw", "public"), q("scaffold", "public")
        vr, vo = q("raw", "private"), q("scaffold", "private")
        dp = None if pr is None or po is None else round(po - pr, 2)
        dv = None if vr is None or vo is None else round(vo - vr, 2)
        fm = any(r["judge_family_match"] for r in rows if r["model"] == m)
        out.append(f"| {m} | {f(pr)} | {f(po)} | {f(dp)} | {f(vr)} | {f(vo)} | {f(dv)} | {'YES' if fm else ''} |")
    # web arm overall
    wpub = mean([r["score"] for r in rows if r["model"] == "web-perplexity" and r["stratum"] == "public"])
    wpriv = mean([r["score"] for r in rows if r["model"] == "web-perplexity" and r["stratum"] == "private"])
    if wpub is not None or wpriv is not None:
        out.append(f"| web-perplexity (web arm) | {f(wpub)} | — | — | {f(wpriv)} | — | — | |")
    out.append("")

    # ---- 3. pgfplots coordinates for the crossover figure ----
    out.append("## pgfplots — crossover (x=findability, y=quality)\n```")
    for a, style in (("raw", "% raw (parametric)"), ("ontology", "% ontology-grounded"), ("web", "% web-grounded")):
        out.append(style)
        out.append("".join(f"({x},{f(y)}) " for x, y in coords[a]))
    out.append("```")

    text = "\n".join(out)
    open(args.out, "w").write(text)
    print(text)


if __name__ == "__main__":
    raise SystemExit(main())
