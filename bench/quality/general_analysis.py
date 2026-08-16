#!/usr/bin/env python3
"""Headline analysis for the general-knowledge harness experiment.

Does a confidence-gated ontology harness help LOWER-TIER models on GENERAL
questions, or make them jagged? Consumes judged bare/harness rows (judge.py
--modes bare,harness against the general gold) + the per-question gate
engagement, and reports:

  - per-model bare vs harness quality (0-5), by category
    (off_domain / in_domain_general / adjacent)
  - the harness delta per category -> uplift where it should help,
    non-regression where it should stay out of the way
  - JAGGEDNESS check: on OFF-DOMAIN questions where the gate mis-fired
    (engaged), does harness degrade vs bare?
  - pgfplots data.

Usage: python3 bench/quality/general_analysis.py \
    --judged 'uplift-results/general/judged-*.json' \
    --scaffolds uplift-results/general/harness-scaffolds.json \
    --out uplift-results/general/analysis.md
"""
from __future__ import annotations
import argparse, glob, json, random, statistics


def mean(xs):
    xs = [x for x in xs if isinstance(x, (int, float))]
    return statistics.mean(xs) if xs else None


def paired_deltas(rows, subset):
    """Per-(model,question) harness-minus-bare deltas over a row subset."""
    by = {}
    for r in rows:
        if subset(r):
            by.setdefault((r["model"], r["id"]), {})[r["mode"]] = r["score"]
    return [v["harness"] - v["bare"] for v in by.values()
            if "harness" in v and "bare" in v]


def bootstrap_ci(deltas, resamples=10000, seed=42):
    if len(deltas) < 2:
        return (None, None)
    rng = random.Random(seed)
    n = len(deltas)
    means = sorted(sum(deltas[rng.randrange(n)] for _ in range(n)) / n
                   for _ in range(resamples))
    return (round(means[int(0.025 * resamples)], 3),
            round(means[int(0.975 * resamples)], 3))


def delta_stat(rows, subset):
    d = paired_deltas(rows, subset)
    if not d:
        return "-", (None, None), 0
    m = round(statistics.mean(d), 3)
    return m, bootstrap_ci(d), len(d)


def f(x, n=2):
    return "-" if x is None else f"{x:.{n}f}"


CATS = ["off_domain", "in_domain_general", "adjacent"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--judged", required=True)
    ap.add_argument("--scaffolds", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    rows = []
    for p in glob.glob(args.judged):
        rows.extend(json.load(open(p)))
    rows = [r for r in rows if isinstance(r.get("score"), int)]
    scaf = json.load(open(args.scaffolds))
    engaged = {qid: bool(v.get("engaged")) for qid, v in scaf.items()}
    for r in rows:
        r["engaged"] = engaged.get(r["id"], False)
    # stratum field carries the category; derive the category list from the data
    # so arcane_thin / arcane_documented (or any stratification) are picked up.
    global CATS
    CATS = sorted({r["stratum"] for r in rows if r.get("stratum")})
    models = sorted({r["model"] for r in rows})

    out = ["# General-knowledge harness experiment — bare vs harness\n",
           f"{len(rows)} gradings, {len(models)} models. Quality = judge 0-5 vs frontier gold.",
           "Question: does the confidence-gated ontology harness help lower-tier models on "
           "GENERAL questions, or make them jagged?\n"]

    # ---- 1. per-model x category ----
    out.append("## Per-model quality (0-5): bare -> harness (Δ), by category\n")
    hdr = "| model | " + " | ".join(f"{c} b→h (Δ)" for c in CATS) + " |"
    out.append(hdr)
    out.append("|" + "---|" * (len(CATS) + 1))
    for m in models:
        cells = []
        for c in CATS:
            b = mean([r["score"] for r in rows if r["model"] == m and r["mode"] == "bare" and r["stratum"] == c])
            h = mean([r["score"] for r in rows if r["model"] == m and r["mode"] == "harness" and r["stratum"] == c])
            d = None if b is None or h is None else round(h - b, 2)
            cells.append(f"{f(b)}→{f(h)} ({'+' if (d or 0) >= 0 else ''}{f(d)})")
        out.append(f"| {m} | " + " | ".join(cells) + " |")
    out.append("")

    # ---- 2. averaged across models, per category ----
    out.append("## Averaged across models, per category\n")
    out.append("| category | bare | harness | Δ | n(gradings) |")
    out.append("|---|---|---|---|---|")
    coords = {"bare": [], "harness": []}
    for i, c in enumerate(CATS):
        b = mean([r["score"] for r in rows if r["mode"] == "bare" and r["stratum"] == c])
        h = mean([r["score"] for r in rows if r["mode"] == "harness" and r["stratum"] == c])
        n = sum(1 for r in rows if r["mode"] == "bare" and r["stratum"] == c)
        d = None if b is None or h is None else round(h - b, 2)
        out.append(f"| {c} | {f(b)} | {f(h)} | {('+' if (d or 0)>=0 else '')}{f(d)} | {n} |")
        coords["bare"].append((i, b)); coords["harness"].append((i, h))
    out.append("")

    # ---- 2b. STATISTICALLY ISOLATED assertions: paired delta + 95% CI ----
    out.append("## Paired harness−bare delta with 95% bootstrap CI (the isolated assertion)\n")
    out.append("Paired per (model,question); bootstrap over pairs. CI excludes 0 ⇒ real effect.\n")
    out.append("| subset | mean Δ | 95% CI | n pairs | reading |")
    out.append("|---|---|---|---|---|")
    def _row(label, subset):
        m, (lo, hi), n = delta_stat(rows, subset)
        if lo is None:
            reading = "-"
        elif lo > 0:
            reading = "UPLIFT (CI>0)"
        elif hi < 0:
            reading = "DEGRADE (CI<0)"
        else:
            reading = "null / no effect"
        out.append(f"| {label} | {m} | [{lo}, {hi}] | {n} | {reading} |")
    _row("all questions", lambda r: True)
    for c in CATS:
        _row(c, lambda r, c=c: r["stratum"] == c)
    _row("off_domain gate-engaged (worst case)",
         lambda r: r["stratum"] == "off_domain" and r.get("engaged"))
    out.append("")

    # ---- 3. jaggedness: off-domain, gate mis-fired vs correctly-skipped ----
    out.append("## Jaggedness check — off-domain questions\n")
    out.append("| off-domain subset | bare | harness | Δ | n |")
    out.append("|---|---|---|---|---|")
    for label, pred in (("gate ENGAGED (mis-fired)", lambda r: r["engaged"]),
                        ("gate skipped (correct)", lambda r: not r["engaged"])):
        b = mean([r["score"] for r in rows if r["stratum"] == "off_domain" and r["mode"] == "bare" and pred(r)])
        h = mean([r["score"] for r in rows if r["stratum"] == "off_domain" and r["mode"] == "harness" and pred(r)])
        n = sum(1 for r in rows if r["stratum"] == "off_domain" and r["mode"] == "bare" and pred(r))
        d = None if b is None or h is None else round(h - b, 2)
        out.append(f"| {label} | {f(b)} | {f(h)} | {('+' if (d or 0)>=0 else '')}{f(d)} | {n} |")
    out.append("\n*If harness ≈ bare on mis-fired off-domain questions, models shrug off "
               "irrelevant injected context (robust, non-jagged). If harness < bare, the "
               "over-firing gate measurably degrades general answers.*\n")

    # ---- 4. pgfplots ----
    out.append("## pgfplots — bare vs harness by category (x=0 off,1 in-domain,2 adjacent)\n```")
    for arm in ("bare", "harness"):
        out.append(f"% {arm}")
        out.append("".join(f"({x},{f(y)}) " for x, y in coords[arm]))
    out.append("```")

    text = "\n".join(out)
    open(args.out, "w").write(text)
    print(text)


if __name__ == "__main__":
    raise SystemExit(main())
