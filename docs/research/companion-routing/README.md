# Companion note: gain over a judge-free ranker

`main.tex` is a standalone five-page note measuring a typed-decision seam (the agentbox
skill router, 115 exposed rubrics plus a decline option, 86 labelled turns) against the
strongest judge-free ranker over the same options.

## Why it is separate

It was §"A Second Domain: Typed Skill Routing" of paper v7 and was split out on
2026-09-21 following the first external review. The review's finding: under the paper's
exposure definition every option in this task is exposed, so the exposure ceiling is 1 and
gain over exposure is accuracy minus 1 (for the 4B engine, 0.884 − 1 = −0.116). The +0.047
the section reported is something else: gain over an oracle-tuned, judge-free ranker. The
note names that quantity distinctly (**gain over ranker**, `g_rank`), states both numbers
side by side, and withdraws the paper's claim that the routing study validated the same
formal instrument. The seven-point movement under the two repairs is **baseline
sensitivity in `g_rank`**, not instability in the exposure scalar.

Other corrections incorporated here, not merely relocated:

- the arithmetic: 11.6 → 4.7 is **6.9** points, not 8.1;
- oracle threshold tuning is favourable within the ranker family on this corpus and is not
  a general lower bound or a deployment estimate — a held-out split or repeated-tuning
  resample is needed first;
- exclusion-clause stripping is a **declared** task-aware preprocessing choice; the claim
  that it "cannot introduce supervision" is withdrawn.

Retained in full: the two self-inflicted defects, the three-engine table with exact
McNemar, the power statement (0.50 at n=86; ~157 turns for 80 %), the population
breakdown, the label audit, the cut analyses, the leakage figure (37.1 % vs 2.7 %), the
self-authored single-seed corpus and the non-deterministic cloud judge.

## Pointers

- Decision record: `agentbox/docs/adr/ADR-2095-measure-typed-decision-seams-against-a-copy-ceiling.md`
- Harness notes (operational): `agentbox/docs/reference/copy-ceiling-harness-notes.md`
- Rig: `agentbox/crates/system-one/system-one-eval` (corpus at `tests/system-one/routing-cases.json`)
- Scope: `agentbox/docs/proposals/sovereign-system-one.md`
- Parent paper: `../paper-v8/`

## Build

```
latexmk -pdf main.tex
```

Exit 0, five two-column pages, no undefined references.
