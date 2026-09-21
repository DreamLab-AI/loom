#!/usr/bin/env python3
"""Per-arm summary of the measured placebo exposure recorded by control_rerun.py.

The reviewer's ask was not "assert the placebo arms carry no gold content" but
"measure it". Each row of `rows.jsonl` carries the gold-target exposure of the
block that was actually injected, computed with the paper's own lexical matcher
(`decompose_exposure.gold_hit`). This aggregates those per-row numbers per arm,
so the control table can be read against a measured exposure covariate rather
than an assumption.
"""
from __future__ import annotations
import argparse, json, statistics
from collections import defaultdict
from pathlib import Path


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rows", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--md", required=True, type=Path)
    args = ap.parse_args(argv)

    per = defaultdict(list)
    tok = defaultdict(list)
    rows = 0
    for line in open(args.rows):
        r = json.loads(line)
        e = r.get("exposure") or {}
        if e.get("exposure_fraction") is None:
            continue
        rows += 1
        per[r["arm"]].append(e["exposure_fraction"])
        tok[r["arm"]].append(r.get("injected_block_est_tokens") or 0)

    out = {"rows": rows, "per_arm": {}}
    for arm in sorted(per):
        v = per[arm]
        out["per_arm"][arm] = {
            "n": len(v),
            "mean_exposure_fraction": round(statistics.mean(v), 4),
            "median": round(statistics.median(v), 4),
            "min": round(min(v), 4), "max": round(max(v), 4),
            "n_zero_exposure": sum(1 for x in v if x == 0),
            "n_full_exposure": sum(1 for x in v if x == 1),
            "mean_injected_est_tokens": round(statistics.mean(tok[arm]), 1),
        }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    json.dump(out, open(args.out, "w"), indent=1)

    L = ["# Measured placebo exposure of the injected block, per arm", "",
         "Gold targets per question = the titles of its **true** scaffold's seed classes "
         "plus the question's declared `topic`. Exposure is the fraction of those titles "
         "the injected block matches under `decompose_exposure.gold_hit` (the bench "
         "scorer, byte-identical). `true` is the ceiling each placebo arm is meant to "
         "fall short of; the token column shows the arms are length-matched.", "",
         "| arm | n | mean exposure | median | min | max | zero-exposure rows | "
         "full-exposure rows | mean injected tokens |",
         "|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for arm, s in out["per_arm"].items():
        L.append(f"| {arm} | {s['n']} | {s['mean_exposure_fraction']:.4f} | "
                 f"{s['median']:.4f} | {s['min']:.4f} | {s['max']:.4f} | "
                 f"{s['n_zero_exposure']} | {s['n_full_exposure']} | "
                 f"{s['mean_injected_est_tokens']:.1f} |")
    L += ["", "Non-zero exposure in a placebo arm is expected and is the point of "
              "measuring it: the matcher is lexical, and generic class titles recur "
              "across an 8,146-class corpus, so a block about unrelated entities can "
              "still contain a gold token by coincidence.", ""]
    args.md.write_text("\n".join(L) + "\n")
    print(f"wrote {args.out} and {args.md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
