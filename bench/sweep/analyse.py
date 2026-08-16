#!/usr/bin/env python3
"""Analyse the cross-provider uplift sweep into pre-print tables + plot data.

Consumes uplift-results/sweep/scores-<label>-{raw,scaffold}.jsonl (produced by
run-one-model.sh) and emits, for every model that has BOTH arms scored:

  - raw recall, scaffold recall, paired uplift + naive & domain-clustered 95% CI
  - copy ceiling (mean gold-exposed recall on the scaffold arm) and the signed
    GAIN OVER COPY (scaffold recall - copy ceiling) -- the model-discriminating
    faithfulness signal
  - engagement, truncation and retry health

Outputs: a JSON blob, a markdown table (stdout + file), and pgfplots coordinate
blocks for the figures. Robust to partial sweeps (skips models missing an arm).

Usage: PYTHONPATH=app python3 bench/sweep/analyse.py [--outdir uplift-results/sweep]
"""
from __future__ import annotations
import argparse, glob, json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import bench_ontology_uplift as bu  # bootstrap_ci, bootstrap_ci_clustered  # noqa


def _read(path):
    return [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()]


def _mean(xs):
    xs = [x for x in xs if x is not None]
    return sum(xs) / len(xs) if xs else None


def analyse_model(label, raw_path, sc_path, resamples=10000, seed=42):
    raw = {r["id"]: r for r in _read(raw_path)}
    sc = {r["id"]: r for r in _read(sc_path)}
    ids = sorted(set(raw) & set(sc))
    rr = [raw[i]["recall"] for i in ids if raw[i].get("recall") is not None]
    sr = [sc[i]["recall"] for i in ids if sc[i].get("recall") is not None]
    # copy ceiling on the scaffold arm
    exp = [sc[i]["n_gold_exposed"] / sc[i]["n_gold"] for i in ids
           if isinstance(sc[i].get("n_gold_exposed"), int) and (sc[i].get("n_gold") or 0) > 0]
    deltas, clusters = [], {}
    n_ne = n_err = 0
    for i in ids:
        a, b = raw[i], sc[i]
        if a.get("recall") is None or b.get("recall") is None:
            n_err += 1; continue
        if not b.get("scaffold_engaged"):
            n_ne += 1; continue
        d = b["recall"] - a["recall"]
        deltas.append(d)
        clusters.setdefault(b.get("domain") or "", []).append(d)
    d_mean = _mean(deltas)
    lo, hi = bu.bootstrap_ci(deltas, resamples=resamples, seed=seed) if deltas else (None, None)
    clo, chi = bu.bootstrap_ci_clustered(list(clusters.values()), resamples=resamples, seed=seed) if deltas else (None, None)
    sc_recall = _mean(sr); copy = _mean(exp)
    trunc = sum(1 for i in ids if sc[i].get("finish_reason") == "length"
                or raw[i].get("finish_reason") == "length")
    retried = sum(1 for i in ids if isinstance(sc[i].get("attempts"), int) and sc[i]["attempts"] > 1)
    return {
        "model": label, "n": len(ids),
        "raw_recall": _mean(rr), "scaffold_recall": sc_recall,
        "uplift": d_mean, "ci": [lo, hi], "clustered_ci": [clo, chi],
        "copy_ceiling": copy,
        "gain_over_copy": (None if sc_recall is None or copy is None else round(sc_recall - copy, 4)),
        "n_not_engaged": n_ne, "n_errors": n_err,
        "n_truncated": trunc, "n_retried": retried,
        "provider_model": (sc[ids[0]].get("model") if ids else label),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--outdir", default="uplift-results/sweep")
    ap.add_argument("--resamples", type=int, default=10000)
    args = ap.parse_args()
    labels = sorted({os.path.basename(p).replace("scores-", "").rsplit("-", 1)[0]
                     for p in glob.glob(os.path.join(args.outdir, "scores-*-scaffold.jsonl"))})
    rows = []
    for lab in labels:
        rp = os.path.join(args.outdir, f"scores-{lab}-raw.jsonl")
        sp = os.path.join(args.outdir, f"scores-{lab}-scaffold.jsonl")
        if os.path.exists(rp) and os.path.exists(sp):
            try:
                rows.append(analyse_model(lab, rp, sp, resamples=args.resamples))
            except Exception as e:
                print(f"skip {lab}: {e}", file=sys.stderr)
    rows.sort(key=lambda r: (r["gain_over_copy"] if r["gain_over_copy"] is not None else -9))
    with open(os.path.join(args.outdir, "sweep-analysis.json"), "w") as fh:
        json.dump(rows, fh, indent=2)

    def f(x, n=3):
        return "-" if x is None else f"{x:.{n}f}"
    md = ["# Cross-provider uplift sweep — analysis", "",
          f"{len(rows)} models, 510-question set (seed 42), uniform temp=0 / max_tokens=2048.",
          "Sorted by gain-over-copy (recall beyond what a no-op extractor of the injected context would get).", "",
          "| model | n | raw | scaffold | uplift [naive CI] | domain-clustered CI | copy ceiling | gain over copy | trunc | retried |",
          "|---|---|---|---|---|---|---|---|---|---|"]
    for r in rows:
        md.append(
            f"| {r['model']} | {r['n']} | {f(r['raw_recall'])} | {f(r['scaffold_recall'])} "
            f"| {f(r['uplift'])} [{f(r['ci'][0])},{f(r['ci'][1])}] "
            f"| [{f(r['clustered_ci'][0])},{f(r['clustered_ci'][1])}] "
            f"| {f(r['copy_ceiling'])} | **{f(r['gain_over_copy'])}** "
            f"| {r['n_truncated']} | {r['n_retried']} |")
    md += ["", "## pgfplots — raw vs scaffold vs copy-ceiling (per model)", "```"]
    md.append("% raw"); md.append("".join(f"({r['model']},{f(r['raw_recall'])}) " for r in rows))
    md.append("% scaffold"); md.append("".join(f"({r['model']},{f(r['scaffold_recall'])}) " for r in rows))
    md.append("% copy ceiling"); md.append("".join(f"({r['model']},{f(r['copy_ceiling'])}) " for r in rows))
    md.append("% gain over copy (signed)"); md.append("".join(f"({r['model']},{f(r['gain_over_copy'])}) " for r in rows))
    md.append("```")
    text = "\n".join(md)
    open(os.path.join(args.outdir, "sweep-analysis.md"), "w").write(text)
    print(text)


if __name__ == "__main__":
    raise SystemExit(main())
