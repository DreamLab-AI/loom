#!/usr/bin/env python3
"""Control-arm contrast analysis, cohort-agnostic (paper-v9 remediation).

`analyze_controls.py` hard-codes the paper-v2 directory, the four-arm tuple and
merges into that directory's `analysis.json`. This version takes its cohort on
the command line so the same code analyses both the original 1536-token
four-arm cohort and the five-arm common-retry-policy rerun, and adds the three
things the reviewer asked for:

  1. the per-arm accounting chain planned -> attempted -> completed -> graded
     -> included (with `included` reported per contrast, since each paired
     contrast uses its own pairwise-complete question set);
  2. Holm-Bonferroni adjustment over the whole declared contrast family, so the
     table's significance marks are family-wise corrected rather than per-row;
  3. a common-intersection sensitivity analysis - every contrast repeated on the
     single question set that is graded in *every* arm, which removes the
     "different contrasts rest on different questions" objection outright.

Statistics are the paper's own: percentile bootstrap (10,000 resamples, seed 7)
from `analyze.bootstrap_ci`, the EXACT conditional signed-rank test from
`analyze.exact_signed_rank` (valid at any n, which matters here: several
contrasts have very few non-zero pairs), and `analyze.holm`.
"""
from __future__ import annotations
import argparse, json, sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from analyze import bootstrap_ci, exact_signed_rank, holm, sign_dominance  # noqa: E402

# Full declared family, in table order. Arms absent from the cohort are dropped
# (the four-arm cohort has no fluent_noise), and the family size used by Holm is
# the number of contrasts actually computed for that cohort.
SCAFFOLD_ARMS = ("true", "shuffled", "masked", "irrelevant", "fluent_noise")


def build_family(arms: list) -> list:
    fam = [(a, "raw") for a in ("loom",) + SCAFFOLD_ARMS if a in arms]
    fam += [("true", b) for b in ("shuffled", "masked", "irrelevant", "fluent_noise")
            if b in arms and "true" in arms]
    if "loom" in arms and "true" in arms:
        fam.append(("loom", "true"))
    return fam


def contrast_stats(pairs: list) -> dict:
    diffs = [a - b for a, b in pairs]
    lo, hi = bootstrap_ci(diffs)
    w, p = exact_signed_rank(diffs)
    return {"n": len(diffs),
            "mean_diff": round(sum(diffs) / len(diffs), 4),
            "boot95": [round(lo, 4), round(hi, 4)],
            "exact_signed_rank_p": round(p, 6),
            "w_plus": w,
            "sign_dominance": round(sign_dominance(diffs), 4),
            "wins": sum(1 for d in diffs if d > 0),
            "losses": sum(1 for d in diffs if d < 0),
            "ties": sum(1 for d in diffs if d == 0)}


def run_family(by: dict, keys: list, family: list, restrict: set | None = None) -> dict:
    out, ps, names = {}, [], []
    for hi_arm, lo_arm in family:
        ks = [k for k in keys if hi_arm in by[k] and lo_arm in by[k]
              and (restrict is None or k in restrict)]
        if len(ks) < 5:
            out[f"{hi_arm}-{lo_arm}"] = {"n": len(ks), "skipped": "n<5"}
            continue
        st = contrast_stats([(by[k][hi_arm], by[k][lo_arm]) for k in ks])
        st["question_ids"] = sorted(k[1] for k in ks)
        out[f"{hi_arm}-{lo_arm}"] = st
        ps.append(st["exact_signed_rank_p"])
        names.append(f"{hi_arm}-{lo_arm}")
    for name, adj in zip(names, holm(ps)):
        out[name]["holm_p"] = round(adj, 6)
        out[name]["holm_sig_0.05"] = adj < 0.05
    out["_family_size"] = len(ps)
    return out


def accounting(rows_files: list, judged: list, keep_sets: set, planned_per_arm: dict) -> dict:
    attempted, completed = defaultdict(int), defaultdict(int)
    seen = set()
    for path in rows_files:
        for line in open(path):
            r = json.loads(line)
            if r.get("set") not in keep_sets:
                continue
            k = (r["set"], r["id"], r["arm"])
            if k in seen:
                continue
            seen.add(k)
            if "skipped" in r:
                continue
            attempted[r["arm"]] += 1
            if "error" not in r and (r.get("content") or "").strip():
                completed[r["arm"]] += 1
    graded = defaultdict(int)
    for j in judged:
        if j.get("score") is not None:
            graded[j["arm"]] += 1
    arms = sorted(set(attempted) | set(graded))
    return {a: {"planned": planned_per_arm.get(a),
                "attempted": attempted[a],
                "completed_non_empty": completed[a],
                "empty": attempted[a] - completed[a],
                "empty_rate": round((attempted[a] - completed[a]) / attempted[a], 4)
                if attempted[a] else None,
                "graded": graded[a]} for a in arms}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--judged", required=True, type=Path)
    ap.add_argument("--rows", nargs="+", required=True)
    ap.add_argument("--sets", default="arcane,thin")
    ap.add_argument("--planned-per-arm", type=int, default=57)
    ap.add_argument("--out-json", required=True, type=Path)
    ap.add_argument("--label", default="cohort")
    args = ap.parse_args(argv)
    keep_sets = set(args.sets.split(","))

    judged = [j for j in json.load(open(args.judged))
              if j.get("score") is not None and j["set"] in keep_sets]
    by = defaultdict(dict)
    for j in judged:
        by[(j["set"], j["id"])][j["arm"]] = j["score"]
    arms = sorted({j["arm"] for j in judged})
    family = build_family(arms)

    result = {"label": args.label,
              "generated_at": datetime.now(timezone.utc).isoformat(),
              "judged_file": str(args.judged),
              "rows_files": list(args.rows),
              "sets": sorted(keep_sets),
              "arms": arms,
              "family": [f"{a}-{b}" for a, b in family],
              "judge": {k: judged[0].get(k) for k in
                        ("judge_model", "judge_base", "judge_temperature",
                         "rubric_sha256_16") if k in judged[0]}}

    result["accounting"] = accounting(args.rows, judged, keep_sets,
                                      {a: args.planned_per_arm for a in arms})
    result["mean_score_per_arm"] = {
        a: round(sum(j["score"] for j in judged if j["arm"] == a)
                 / max(1, sum(1 for j in judged if j["arm"] == a)), 4) for a in arms}

    # common intersection: questions graded in EVERY arm of the cohort
    inter = {k for k, v in by.items() if all(a in v for a in arms)}
    result["common_intersection"] = {"n_questions": len(inter),
                                     "question_ids": sorted(k[1] for k in inter)}

    for scope in ("arcane", "thin", "pooled"):
        keys = [k for k in by if (scope == "pooled" and k[0] in keep_sets) or k[0] == scope]
        if not keys:
            continue
        result.setdefault("contrasts", {})[scope] = run_family(by, keys, family)
        result.setdefault("contrasts_common_intersection", {})[scope] = run_family(
            by, keys, family, restrict=inter)

    args.out_json.parent.mkdir(parents=True, exist_ok=True)
    json.dump(result, open(args.out_json, "w"), indent=1)
    print(f"wrote {args.out_json}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
