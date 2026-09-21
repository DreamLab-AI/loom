#!/usr/bin/env python3
"""Render ANALYSIS.md from an `analyze_controls_v2.py` analysis.json.

Keeps the prose table and the JSON in lockstep: the markdown is generated, not
transcribed, so the paper's table can never drift from the artefact again (the
drift that motivated this whole remediation).
"""
from __future__ import annotations
import argparse, json
from pathlib import Path

LABEL = {"loom-raw": "loom − raw", "true-raw": "true − raw",
         "shuffled-raw": "shuffled − raw", "masked-raw": "masked − raw",
         "irrelevant-raw": "irrelevant − raw", "fluent_noise-raw": "fluent-noise − raw",
         "true-shuffled": "true − shuffled", "true-masked": "true − masked",
         "true-irrelevant": "true − irrelevant",
         "true-fluent_noise": "true − fluent-noise", "loom-true": "loom − true"}


def ctable(fam: list, block: dict) -> str:
    out = ["| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |",
           "|---|---:|---:|---|---:|---:|:--:|---|"]
    for k in fam:
        v = block.get(k, {})
        if "mean_diff" not in v:
            out.append(f"| {LABEL.get(k,k)} | {v.get('n','–')} | — | — | — | — | — | *{v.get('skipped','absent')}* |")
            continue
        out.append(
            f"| {LABEL.get(k,k)} | {v['n']} | {v['mean_diff']:+.4f} | "
            f"[{v['boot95'][0]:+.4f}, {v['boot95'][1]:+.4f}] | {v['exact_signed_rank_p']:.4f} | "
            f"{v.get('holm_p', float('nan')):.4f} | {'**yes**' if v.get('holm_sig_0.05') else 'no'} | "
            f"{v['wins']}/{v['losses']}/{v['ties']} |")
    return "\n".join(out)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--analysis", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--title", default=None)
    ap.add_argument("--preamble", type=Path, default=None,
                    help="markdown file prepended after the header")
    args = ap.parse_args(argv)
    d = json.load(open(args.analysis))
    fam = d["family"]
    j = d.get("judge", {})

    L = [f"# {args.title or d['label']}", "",
         f"Generated `{d['generated_at']}` by `tools/paper/analyze_controls_v2.py` "
         f"from `{d['judged_file']}`.", "",
         f"- Judge: `{j.get('judge_model','?')}` via `{j.get('judge_base','?')}`, "
         f"temperature {j.get('judge_temperature','?')}, rubric sha256[:16] "
         f"`{j.get('rubric_sha256_16','?')}` (byte-identical to `judge_v2.RUBRIC`).",
         f"- Completion rows: " + ", ".join(f"`{p}`" for p in d["rows_files"]),
         f"- Question sets: {', '.join(d['sets'])}; arms: {', '.join(d['arms'])}.",
         f"- Contrast family: {len(fam)} contrasts, Holm-Bonferroni across the "
         f"whole family at the pooled scope.", ""]
    if args.preamble and args.preamble.exists():
        L += [args.preamble.read_text().rstrip(), ""]

    L += ["## Per-arm accounting: planned → attempted → completed → graded", "",
          "| arm | planned | attempted | completed (non-empty) | empty | empty rate | graded |",
          "|---|---:|---:|---:|---:|---:|---:|"]
    for arm, a in sorted(d["accounting"].items()):
        L.append(f"| {arm} | {a['planned']} | {a['attempted']} | {a['completed_non_empty']} | "
                 f"{a['empty']} | {a['empty_rate']:.1%} | {a['graded']} |"
                 if a["empty_rate"] is not None else
                 f"| {arm} | {a['planned']} | {a['attempted']} | {a['completed_non_empty']} | "
                 f"{a['empty']} | — | {a['graded']} |")
    L += ["",
          "`included` is not a per-arm quantity: every contrast is paired, so each one is "
          "computed on its own pairwise-complete question set. Those n values are the `n` "
          "column of the contrast tables below, and the exact question ids behind each "
          "contrast are listed per contrast in `analysis.json`.", "",
          "Mean judge score per arm: " +
          ", ".join(f"{a} {v}" for a, v in sorted(d["mean_score_per_arm"].items())) + ".", ""]

    for scope in ("pooled", "arcane", "thin"):
        if scope not in d.get("contrasts", {}):
            continue
        L += [f"## Contrasts — {scope} (complete-case, pairwise)", "",
              ctable(fam, d["contrasts"][scope]), ""]

    ci = d["common_intersection"]
    L += ["## Common-intersection sensitivity analysis", "",
          f"The single question set graded in **every** arm ({', '.join(d['arms'])}): "
          f"**n = {ci['n_questions']}**. Repeating the whole family on just those "
          f"questions removes the objection that different rows of the table rest on "
          f"different questions.", ""]
    for scope in ("pooled",):
        if scope in d.get("contrasts_common_intersection", {}):
            L += [ctable(fam, d["contrasts_common_intersection"][scope]), ""]
    L += ["Question ids in the intersection: " +
          ", ".join(f"`{q}`" for q in ci["question_ids"]), ""]

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(L) + "\n")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
