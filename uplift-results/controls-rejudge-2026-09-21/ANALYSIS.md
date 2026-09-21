# Negative controls — original 1536-token four-arm cohort, re-judged 2026-09-21

Generated `2026-09-21T14:38:18.482640+00:00` by `tools/paper/analyze_controls_v2.py` from `uplift-results/controls-rejudge-2026-09-21/judged.json`.

- Judge: `openai/gpt-4.1` via `https://openrouter.ai/api/v1`, temperature 0, rubric sha256[:16] `7231654abc776c3e` (byte-identical to `judge_v2.RUBRIC`).
- Completion rows: `uplift-results/paper-v2/control-results.jsonl`, `uplift-results/paper-v2/live-results.jsonl`
- Question sets: arcane, thin; arms: irrelevant, loom, masked, raw, shuffled, true.
- Contrast family: 9 contrasts, Holm-Bonferroni across the whole family at the pooled scope.

## What this cohort is

The original four-arm negative-control cohort of §`sec:controls`: 57 frozen questions
(24 `arcane` + 33 `thin`), each answered by the local `qwen3.8-27B` through four
scaffold variants (`true` / `shuffled` / `masked` / `irrelevant`) at a **1536-token**
budget, generated 2026-08-17 by `tools/paper/control_harness.py` into
`uplift-results/paper-v2/control-results.jsonl`. The `loom` and `raw` arms come from
the same day's `live-results.jsonl`. Nothing was re-generated here: this is a
**re-judge** of the archived completions.

## Two record corrections

**1. The judge scores were archived after all.** `docs/research/paper-v8/notes/R5-numbers.md`
§1.3 records the control-arm judge outputs as unarchived. They are not:
`uplift-results/paper-v2/judged.json` (360 rows, committed 2026-08-17 in `48ed93a`)
holds the original `gpt-4.1` scores for every arm including the four control arms.
Re-judging those same completions today with `openai/gpt-4.1` at temperature 0
(OpenRouter) reproduces them almost exactly:

| | value |
|---|---|
| rows compared | 240 (arcane+thin, all six arms) |
| exact score agreement | **227 / 240 (94.6%)** |
| agreement within ±1 | 240 / 240 (100%) |
| mean score change (new − archived) | +0.0125 |
| arms with zero mean drift | `raw`, `shuffled`, `true` |

The contrast table below is therefore reproducible from released artefacts, and
`analysis-archived-2026-08-17.json` in this directory carries the same analysis run
on the archived scores for side-by-side comparison. Where they differ the shift is
≤0.09 on the 0–5 scale, except `loom − true`, which moves from −0.0625 to +0.0625 —
i.e. a *sign flip on a quantity whose bootstrap CI comfortably spans zero in both
runs, and which is therefore not a finding in either direction*.

paper-v4's published complete-case figures are reproduced by both runs
(v4: true−raw +0.59, true−irrelevant +0.35, irrelevant−raw +0.04, loom−true −0.06;
archived re-analysis here: +0.5938, +0.3478, +0.0357, −0.0625). This settles the
drift flagged in R5 §1.2: **v4's numbers are the `gpt-4.1` ones and they are exactly
what these artefacts produce.** The v5–v8 "old (complete-case)" column
(true−raw +0.78 etc.) matches neither, and is the one column still unbacked by any
artefact in this repository.

**2. The prose's "56" is right and the file's "57 rows per arm" is also right.**
Question `arc_sov09` never reached the model: `/loom/scaffold` declined to inject for
it, so `control_harness.run_one` wrote a placeholder row `{"skipped": "no-scaffold"}`
for all four arms instead of a completion. The file therefore has 57 rows per arm but
only **56 attempts** per arm. Recomputing attrition against 56 reproduces the paper's
published rates *exactly*:

| arm | empty | attempted | rate | paper v6 L206 |
|---|---:|---:|---:|---|
| irrelevant | 28 | 56 | 50.0% | 28/56 = 50.0% ✓ |
| true | 24 | 56 | 42.9% | 24/56 = 42.9% ✓ |
| masked | 23 | 56 | 41.1% | 23/56 = 41.1% ✓ |
| shuffled | 20 | 56 | 35.7% | 20/56 = 35.7% ✓ |

So: **planned 57 per arm, attempted 56, one question structurally excluded.** The
paper should say so rather than leaving an unexplained 56. The `loom`/`raw` arms were
attempted 57 times each (the live harness does not gate on scaffold engagement) —
another reason the per-contrast n values differ between rows of the table.

## What changes when the family is corrected for

The paper marks `true − raw` with a Holm dagger. Holm-Bonferroni over the **whole
nine-contrast family** of this cohort (not per row) leaves only `loom − raw`
significant at 0.05 (Holm p = 0.0124); `true − raw` goes to Holm p = 0.318. On the
common-intersection set (n = 20 questions graded in all six arms) **nothing in the
family survives Holm**, though the point estimates keep their sign and rough
magnitude. The honest reading of this cohort is that its 36–50% attrition leaves it
underpowered for everything except the loom-vs-raw comparison — which is precisely
why the rerun under a common retry policy was worth doing.

## Per-arm accounting: planned → attempted → completed → graded

| arm | planned | attempted | completed (non-empty) | empty | empty rate | graded |
|---|---:|---:|---:|---:|---:|---:|
| irrelevant | 57 | 56 | 28 | 28 | 50.0% | 28 |
| loom | 57 | 57 | 56 | 1 | 1.8% | 56 |
| masked | 57 | 56 | 33 | 23 | 41.1% | 33 |
| raw | 57 | 57 | 55 | 2 | 3.5% | 55 |
| shuffled | 57 | 56 | 36 | 20 | 35.7% | 36 |
| true | 57 | 56 | 32 | 24 | 42.9% | 32 |

`included` is not a per-arm quantity: every contrast is paired, so each one is computed on its own pairwise-complete question set. Those n values are the `n` column of the contrast tables below, and the exact question ids behind each contrast are listed per contrast in `analysis.json`.

Mean judge score per arm: irrelevant 3.6429, loom 4.1071, masked 4.1212, raw 3.4909, shuffled 3.9167, true 4.1875.

## Contrasts — pooled (complete-case, pairwise)

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 54 | +0.5926 | [+0.2407, +0.9444] | 0.0014 | 0.0124 | **yes** | 23/6/25 |
| true − raw | 32 | +0.5938 | [+0.1250, +1.1250] | 0.0398 | 0.3184 | no | 11/4/17 |
| shuffled − raw | 36 | +0.2500 | [-0.3611, +0.8889] | 0.4493 | 1.0000 | no | 11/8/17 |
| masked − raw | 33 | +0.5152 | [+0.0303, +1.0303] | 0.0596 | 0.4170 | no | 12/4/17 |
| irrelevant − raw | 28 | -0.0357 | [-0.6071, +0.4643] | 0.9176 | 1.0000 | no | 7/7/14 |
| true − shuffled | 30 | +0.3000 | [-0.1000, +0.7333] | 0.2407 | 1.0000 | no | 8/4/18 |
| true − masked | 27 | +0.1111 | [-0.2222, +0.4815] | 0.6719 | 1.0000 | no | 5/4/18 |
| true − irrelevant | 23 | +0.3913 | [+0.0000, +0.8696] | 0.1094 | 0.6562 | no | 5/2/16 |
| loom − true | 32 | +0.0625 | [-0.2812, +0.3750] | 0.6892 | 1.0000 | no | 8/5/19 |

## Contrasts — arcane (complete-case, pairwise)

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 24 | +0.8333 | [+0.2083, +1.4583] | 0.0269 | 0.2417 | no | 10/2/12 |
| true − raw | 19 | +0.9474 | [+0.3158, +1.6842] | 0.0312 | 0.2500 | no | 7/1/11 |
| shuffled − raw | 20 | +0.6500 | [-0.2500, +1.6000] | 0.2183 | 1.0000 | no | 8/4/8 |
| masked − raw | 21 | +0.7619 | [+0.1429, +1.4762] | 0.0527 | 0.3691 | no | 8/2/11 |
| irrelevant − raw | 16 | +0.3125 | [-0.1250, +0.8125] | 0.3750 | 1.0000 | no | 4/2/10 |
| true − shuffled | 18 | +0.2778 | [-0.1667, +0.7222] | 0.3750 | 1.0000 | no | 4/1/13 |
| true − masked | 19 | +0.2632 | [-0.1053, +0.6842] | 0.3750 | 1.0000 | no | 4/2/13 |
| true − irrelevant | 14 | +0.5000 | [-0.0714, +1.1429] | 0.2500 | 1.0000 | no | 3/1/10 |
| loom − true | 19 | +0.0000 | [-0.5263, +0.4211] | 1.0000 | 1.0000 | no | 5/3/11 |

## Contrasts — thin (complete-case, pairwise)

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 30 | +0.4000 | [+0.0667, +0.7333] | 0.0472 | 0.4245 | no | 13/4/13 |
| true − raw | 13 | +0.0769 | [-0.4615, +0.6154] | 0.9844 | 1.0000 | no | 4/3/6 |
| shuffled − raw | 16 | -0.2500 | [-0.9375, +0.4375] | 0.5156 | 1.0000 | no | 3/4/9 |
| masked − raw | 12 | +0.0833 | [-0.6667, +0.7500] | 0.7812 | 1.0000 | no | 4/2/6 |
| irrelevant − raw | 12 | -0.5000 | [-1.5000, +0.3333] | 0.5391 | 1.0000 | no | 3/5/4 |
| true − shuffled | 12 | +0.3333 | [-0.4167, +1.1667] | 0.5781 | 1.0000 | no | 4/3/5 |
| true − masked | 8 | -0.2500 | [-0.8750, +0.2500] | 0.7500 | 1.0000 | no | 1/2/5 |
| true − irrelevant | 9 | +0.2222 | [-0.2222, +0.7778] | 0.7500 | 1.0000 | no | 2/1/6 |
| loom − true | 13 | +0.1538 | [-0.2308, +0.6154] | 0.7500 | 1.0000 | no | 3/2/8 |

## Common-intersection sensitivity analysis

The single question set graded in **every** arm (irrelevant, loom, masked, raw, shuffled, true): **n = 20**. Repeating the whole family on just those questions removes the objection that different rows of the table rest on different questions.

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 20 | +0.8000 | [+0.1000, +1.4500] | 0.0479 | 0.4307 | no | 10/2/8 |
| true − raw | 20 | +0.6000 | [+0.1000, +1.2000] | 0.0625 | 0.5000 | no | 7/2/11 |
| shuffled − raw | 20 | +0.3000 | [-0.4000, +1.0000] | 0.4365 | 1.0000 | no | 7/4/9 |
| masked − raw | 20 | +0.4500 | [-0.1000, +1.0500] | 0.2246 | 1.0000 | no | 7/3/10 |
| irrelevant − raw | 20 | +0.2000 | [-0.1500, +0.5500] | 0.3438 | 1.0000 | no | 5/3/12 |
| true − shuffled | 20 | +0.3000 | [-0.1500, +0.8500] | 0.3125 | 1.0000 | no | 4/2/14 |
| true − masked | 20 | +0.1500 | [-0.1000, +0.4500] | 0.5312 | 1.0000 | no | 4/2/14 |
| true − irrelevant | 20 | +0.4000 | [+0.0000, +0.9000] | 0.1875 | 1.0000 | no | 4/1/15 |
| loom − true | 20 | +0.2000 | [-0.2500, +0.5500] | 0.2812 | 1.0000 | no | 6/1/13 |

Question ids in the intersection: `arc_rgb01`, `arc_rgb02`, `arc_rgb05`, `arc_rgb07`, `arc_rgb08`, `arc_rgb09`, `arc_sov01`, `arc_sov02`, `arc_sov04`, `arc_sov05`, `arc_sov06`, `arc_sov07`, `arc_sov08`, `arc_sov11`, `thin_prov07`, `thin_prov09`, `thin_rgb_08`, `thin_rgb_11`, `thin_sove02`, `thin_sove03`

