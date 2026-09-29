# Negative controls — five-arm rerun under the common retry policy, 2026-09-21

Generated `2026-09-29T09:32:06.533868+00:00` by `tools/paper/analyze_controls_v2.py` from `uplift-results/control-rerun-2026-09-21/judged.json`.

- Judge: `openai/gpt-4.1` via `https://openrouter.ai/api/v1`, temperature 0, rubric sha256[:16] `7231654abc776c3e` (byte-identical to `judge_v2.RUBRIC`).
- Completion rows: `uplift-results/control-rerun-2026-09-21/rows.jsonl`, `uplift-results/paper-v2/live-results.jsonl`
- Question sets: arcane, thin; arms: fluent_noise, irrelevant, loom, masked, raw, shuffled, true.
- Contrast family: 11 contrasts, Holm-Bonferroni across the whole family at the pooled scope.

## Per-arm accounting: planned → attempted → completed → graded

| arm | planned | attempted | completed (non-empty) | empty | empty rate | graded |
|---|---:|---:|---:|---:|---:|---:|
| fluent_noise | 57 | 56 | 55 | 1 | 1.8% | 55 |
| irrelevant | 57 | 56 | 54 | 2 | 3.6% | 54 |
| loom | 57 | 57 | 56 | 1 | 1.8% | 56 |
| masked | 57 | 56 | 54 | 2 | 3.6% | 54 |
| raw | 57 | 57 | 55 | 2 | 3.5% | 55 |
| shuffled | 57 | 56 | 53 | 3 | 5.4% | 53 |
| true | 57 | 56 | 53 | 3 | 5.4% | 53 |

`included` is not a per-arm quantity: every contrast is paired, so each one is computed on its own pairwise-complete question set. Those n values are the `n` column of the contrast tables below, and the exact question ids behind each contrast are listed per contrast in `analysis.json`.

Mean judge score per arm: fluent_noise 4.0909, irrelevant 4.1296, loom 4.1071, masked 4.3148, raw 3.5091, shuffled 4.3396, true 4.3585.

## Contrasts — pooled (complete-case, pairwise)

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 54 | +0.5741 | [+0.2222, +0.9259] | 0.0018 | 0.0105 | **yes** | 24/7/23 |
| true − raw | 52 | +0.8077 | [+0.4423, +1.2115] | 0.0002 | 0.0015 | **yes** | 25/5/22 |
| shuffled − raw | 52 | +0.7500 | [+0.3654, +1.1538] | 0.0003 | 0.0027 | **yes** | 23/6/23 |
| masked − raw | 53 | +0.7547 | [+0.3208, +1.2075] | 0.0006 | 0.0040 | **yes** | 26/6/21 |
| irrelevant − raw | 52 | +0.6346 | [+0.3846, +0.9231] | 0.0000 | 0.0004 | **yes** | 24/4/24 |
| fluent-noise − raw | 53 | +0.6038 | [+0.2264, +0.9623] | 0.0002 | 0.0015 | **yes** | 26/5/22 |
| true − shuffled | 52 | +0.0192 | [-0.2115, +0.2500] | 0.8868 | 1.0000 | no | 7/7/38 |
| true − masked | 53 | +0.0000 | [-0.2264, +0.2264] | 1.0000 | 1.0000 | no | 6/7/40 |
| true − irrelevant | 53 | +0.1509 | [-0.1321, +0.4528] | 0.3982 | 1.0000 | no | 12/8/33 |
| true − fluent-noise | 53 | +0.1698 | [-0.0755, +0.4528] | 0.2518 | 1.0000 | no | 10/4/39 |
| loom − true | 53 | -0.2075 | [-0.4717, +0.0566] | 0.1992 | 0.9962 | no | 5/15/33 |

## Contrasts — arcane (complete-case, pairwise)

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 24 | +0.9167 | [+0.2917, +1.5417] | 0.0138 | 0.1104 | no | 12/2/10 |
| true − raw | 23 | +1.2174 | [+0.5652, +1.9130] | 0.0017 | 0.0188 | **yes** | 12/1/10 |
| shuffled − raw | 23 | +1.1304 | [+0.3913, +1.8696] | 0.0101 | 0.0906 | no | 12/3/8 |
| masked − raw | 23 | +1.1304 | [+0.3043, +1.9565] | 0.0144 | 0.1104 | no | 12/2/9 |
| irrelevant − raw | 23 | +0.6087 | [+0.1304, +1.1304] | 0.0312 | 0.1875 | no | 8/2/13 |
| fluent-noise − raw | 23 | +0.6087 | [-0.1739, +1.3043] | 0.0684 | 0.2734 | no | 11/3/9 |
| true − shuffled | 23 | +0.0870 | [-0.1739, +0.4348] | 1.0000 | 1.0000 | no | 3/3/17 |
| true − masked | 23 | +0.0870 | [-0.1739, +0.4783] | 1.0000 | 1.0000 | no | 1/2/20 |
| true − irrelevant | 23 | +0.6087 | [+0.1739, +1.1304] | 0.0361 | 0.1875 | no | 9/2/12 |
| true − fluent-noise | 23 | +0.6087 | [+0.2174, +1.1304] | 0.0078 | 0.0781 | no | 8/0/15 |
| loom − true | 23 | -0.2609 | [-0.6522, +0.1304] | 0.3281 | 0.9844 | no | 2/5/16 |

## Contrasts — thin (complete-case, pairwise)

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 30 | +0.3000 | [-0.0333, +0.6333] | 0.1305 | 0.7830 | no | 12/5/13 |
| true − raw | 29 | +0.4828 | [+0.0690, +0.8966] | 0.0528 | 0.3696 | no | 13/4/12 |
| shuffled − raw | 29 | +0.4483 | [+0.1379, +0.7931] | 0.0220 | 0.1978 | no | 11/3/15 |
| masked − raw | 30 | +0.4667 | [+0.0333, +0.9000] | 0.0308 | 0.2462 | no | 14/4/12 |
| irrelevant − raw | 29 | +0.6552 | [+0.3448, +0.9655] | 0.0007 | 0.0077 | **yes** | 16/2/11 |
| fluent-noise − raw | 30 | +0.6000 | [+0.3000, +0.9333] | 0.0014 | 0.0140 | **yes** | 15/2/13 |
| true − shuffled | 29 | -0.0345 | [-0.3793, +0.2759] | 0.9922 | 1.0000 | no | 4/4/21 |
| true − masked | 30 | -0.0667 | [-0.3667, +0.2000] | 1.0000 | 1.0000 | no | 5/5/20 |
| true − irrelevant | 30 | -0.2000 | [-0.5000, +0.0667] | 0.2852 | 1.0000 | no | 3/6/21 |
| true − fluent-noise | 30 | -0.1667 | [-0.4667, +0.0667] | 0.3750 | 1.0000 | no | 2/4/24 |
| loom − true | 30 | -0.1667 | [-0.5333, +0.2333] | 0.4087 | 1.0000 | no | 3/10/17 |

## Common-intersection sensitivity analysis

The single question set graded in **every** arm (fluent_noise, irrelevant, loom, masked, raw, shuffled, true): **n = 51**. Repeating the whole family on just those questions removes the objection that different rows of the table rest on different questions.

| contrast | n | Δ (0–5 scale) | 95% bootstrap CI | exact p | Holm p | Holm sig. | W/L/T |
|---|---:|---:|---|---:|---:|:--:|---|
| loom − raw | 51 | +0.6078 | [+0.2549, +0.9608] | 0.0018 | 0.0105 | **yes** | 24/7/20 |
| true − raw | 51 | +0.7843 | [+0.4118, +1.1961] | 0.0003 | 0.0027 | **yes** | 24/5/22 |
| shuffled − raw | 51 | +0.7647 | [+0.3725, +1.1765] | 0.0003 | 0.0027 | **yes** | 23/6/22 |
| masked − raw | 51 | +0.7843 | [+0.3529, +1.2353] | 0.0005 | 0.0035 | **yes** | 25/5/21 |
| irrelevant − raw | 51 | +0.6078 | [+0.3333, +0.8824] | 0.0001 | 0.0007 | **yes** | 23/4/24 |
| fluent-noise − raw | 51 | +0.6275 | [+0.2353, +1.0000] | 0.0001 | 0.0011 | **yes** | 25/4/22 |
| true − shuffled | 51 | +0.0196 | [-0.2157, +0.2549] | 0.8868 | 1.0000 | no | 7/7/37 |
| true − masked | 51 | +0.0000 | [-0.2157, +0.2353] | 1.0000 | 1.0000 | no | 5/6/40 |
| true − irrelevant | 51 | +0.1765 | [-0.0980, +0.4902] | 0.3159 | 1.0000 | no | 12/7/32 |
| true − fluent-noise | 51 | +0.1569 | [-0.0980, +0.4510] | 0.3225 | 1.0000 | no | 9/4/38 |
| loom − true | 51 | -0.1765 | [-0.4510, +0.0980] | 0.2776 | 1.0000 | no | 5/14/32 |

Question ids in the intersection: `arc_rgb01`, `arc_rgb02`, `arc_rgb03`, `arc_rgb04`, `arc_rgb05`, `arc_rgb06`, `arc_rgb07`, `arc_rgb08`, `arc_rgb09`, `arc_rgb10`, `arc_rgb11`, `arc_rgb12`, `arc_sov01`, `arc_sov02`, `arc_sov03`, `arc_sov04`, `arc_sov05`, `arc_sov06`, `arc_sov07`, `arc_sov08`, `arc_sov10`, `arc_sov11`, `arc_sov12`, `thin_prov01`, `thin_prov02`, `thin_prov03`, `thin_prov04`, `thin_prov05`, `thin_prov07`, `thin_prov08`, `thin_prov09`, `thin_prov10`, `thin_prov11`, `thin_rgb_01`, `thin_rgb_06`, `thin_rgb_07`, `thin_rgb_08`, `thin_rgb_09`, `thin_rgb_10`, `thin_rgb_11`, `thin_sove01`, `thin_sove02`, `thin_sove03`, `thin_sove04`, `thin_sove05`, `thin_sove06`, `thin_sove07`, `thin_sove08`, `thin_sove09`, `thin_sove10`, `thin_sove11`

