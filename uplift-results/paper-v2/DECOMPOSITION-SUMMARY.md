# Exposure/Recovery Decomposition — 2×2 contingency per model

**Sanity gate.** Mean per-question copy ceiling = `0.9645` (pooled over items `0.9331`); target 0.964 → PASS. Exposure regeneration: byte-identical to stored n_gold_exposed. Per-model recall and gain reproduce the stored summaries to 3 dp (no errors).

Scaffold text regenerated with the git-recovered v1 engine (`ontology_scaffold_v1.py`, blob c7b8fb1), index `app/data/scaffold-index.json`, budget 1500 tok, max_seeds 4, hops 1, prose off, confidence-injection off — the exact sweep configuration. Every scaffold row's recomputed exposed-item count equals the stored `n_gold_exposed`, so the flags are byte-exact, not approximate.

## Per-model pooled 2×2 (scaffold arm, over individual gold items)

`n11`=exposed&recovered, `n10`=exposed&omitted, `n01`=unexposed&recovered, `n00`=unexposed&omitted. Utilisation = n11/(n11+n10); unexposed-recovery = n01/(n01+n00). Recall/ceiling/gain use the paper's question-level scorer (T-COMMON any-collapse); gain 95% CI is a seeded 10k percentile bootstrap over the 510 questions.

| model | n11 | n10 | n01 | n00 | utilisation | unexposed-recov | recall | ceiling gap (gain) | gain 95% CI |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| gemini-3.7-flash | 1012 | 48 | 0 | 76 | 0.9547 | 0.0000 | 0.942 | -0.022 | [-0.029, -0.016] |
| gemini-3.5-flash-lite | 1004 | 56 | 0 | 76 | 0.9472 | 0.0000 | 0.938 | -0.026 | [-0.036, -0.018] |
| claude-haiku-4.5 | 1003 | 57 | 1 | 75 | 0.9462 | 0.0132 | 0.934 | -0.031 | [-0.042, -0.020] |
| glm-4.6 | 1002 | 58 | 0 | 76 | 0.9453 | 0.0000 | 0.928 | -0.036 | [-0.049, -0.025] |
| deepseek-chat | 987 | 73 | 0 | 76 | 0.9311 | 0.0000 | 0.924 | -0.040 | [-0.053, -0.029] |
| llama-3.3-70b | 990 | 70 | 0 | 76 | 0.9340 | 0.0000 | 0.916 | -0.049 | [-0.065, -0.034] |
| qwen-2.5-72b | 979 | 81 | 1 | 75 | 0.9236 | 0.0132 | 0.914 | -0.050 | [-0.066, -0.036] |
| mistral-small-24b | 972 | 88 | 1 | 75 | 0.9170 | 0.0132 | 0.906 | -0.058 | [-0.076, -0.042] |
| gpt-4.1-mini | 962 | 98 | 0 | 76 | 0.9075 | 0.0000 | 0.905 | -0.060 | [-0.077, -0.044] |
| gemini-2.5-flash-lite | 954 | 106 | 0 | 76 | 0.9000 | 0.0000 | 0.897 | -0.067 | [-0.086, -0.050] |
| **pooled** | 9865 | 735 | 3 | 757 | 0.9307 | 0.0039 | — | — | — |

Raw-arm recall (no scaffold, every gold item unexposed) is recovery alone:

| model | raw recall | scaffold recall |
|---|---:|---:|
| gemini-3.7-flash | 0.375 | 0.942 |
| gemini-3.5-flash-lite | 0.323 | 0.938 |
| claude-haiku-4.5 | 0.151 | 0.934 |
| glm-4.6 | 0.243 | 0.928 |
| deepseek-chat | 0.346 | 0.924 |
| llama-3.3-70b | 0.227 | 0.916 |
| qwen-2.5-72b | 0.309 | 0.914 |
| mistral-small-24b | 0.285 | 0.906 |
| gpt-4.1-mini | 0.162 | 0.905 |
| gemini-2.5-flash-lite | 0.227 | 0.897 |

## Study 2 — matched-pairs rank-biserial vs Cliff's delta

The review notes Cliff's delta is an independent-samples statistic; the judged design is matched pairs (same question, loom vs raw arm), so the rank-biserial correlation from the Wilcoxon signed ranks r = (T+ − T−)/(T+ + T−) (zero-differences excluded) is the correct paired effect size and replaces it.

| set | n pairs | n≠0 | wins | losses | ties | rank-biserial r | Cliff's δ (old) |
|---|---:|---:|---:|---:|---:|---:|---:|
| arcane | 24 | 13 | 10 | 3 | 11 | +0.670 | +0.292 |
| general | 60 | 2 | 2 | 0 | 58 | +1.000 | +0.033 |
| thin | 30 | 17 | 12 | 5 | 13 | +0.431 | +0.233 |
| pooled | 114 | 32 | 24 | 8 | 82 | +0.589 | +0.140 |

## Interpretation

1. **n01 ≈ 0 — the item-level claim survives, now MEASURED not inferred.** Pooled across all ten models only **3 of 11,360 gold items** are unexposed-yet-recovered (n01), 0.026% of gold slots; the unexposed-recovery rate is 0.39% and no single model exceeds n01 = 1. The review is algebraically right that a negative g = (n01 − n10)/|G| does not by itself prove n01 = 0 — but the direct 2×2 shows n01 is empirically negligible. "Models essentially do not recover gold beyond what the scaffold exposed" therefore holds at item level as a measurement, and the paper can state it as such rather than leaning on the sign of g.
2. **The negative gain is almost pure n10.** With n01 ≈ 0, g = (n01 − n10)/|G| ≈ −n10/|G| = -0.0644 pooled, dominated by the **735 exposed gold items that answers omitted**. The shortfall under the copy ceiling is imperfect *copying of exposed gold* (plus lexical-match undercount of paraphrase), not the presence of beyond-exposure reasoning being outrun. This reframes the negative g honestly: it is a copy-fidelity deficit, not evidence either way about reasoning — and the separate n01 measurement settles the reasoning question directly.
3. **Context utilisation is high but imperfect:** pooled 93.1% of exposed gold items surface in the answer (per model 90.0–95.5%). The gap (1 − utilisation = 6.9%) is the true ceiling on the copy story: even with the fact in front of it a model drops ~1 in 14 exposed items, lexical-match undercount included.
4. **Ordering — utilisation is not merely shifted recall.** Ranking by context utilisation does NOT exactly reproduce the ranking by headline recall: the two agree everywhere except a single adjacent transposition (deepseek-chat and llama-3.3-70b swap — llama utilises exposed context slightly better, 0.934 vs 0.931, yet scores lower recall, 0.916 vs 0.924). Utilisation pools over exposed items whereas headline recall averages per-question ratios with the T-COMMON any-collapse, and because the copy ceiling c_i varies per item the two are genuinely different measures. In practice the divergence is small (top-four and bottom-four are stable), so 'who uses the context best' and 'who scores highest' nearly — but not exactly — coincide.
5. **Study 2 effect sizes.** Cliff's delta is an independent-samples dominance statistic; the judged design is matched pairs, so the Wilcoxon rank-biserial r = (T+ − T−)/(T+ + T−) (zeros excluded) is the correct effect size. It is markedly larger than the reported Cliff's delta (pooled r = +0.589 vs δ = +0.140) because the judged pairs are heavily tied (82 of 114 pairs), and the mis-applied delta divided the win−loss margin by the full pair count including those ties, deflating it; the rank-biserial conditions on the non-tied pairs. Direction is unchanged — loom is favoured on every set — and strengthened. Caveat: the 'general' set's r = +1.000 rests on just 2 non-tied pairs (58 ties), so treat that set's effect size as directional only.
6. **Matcher caveat.** Recovery and exposure share one lexical matcher, so n01 would miss a gold fact the model rephrased beyond the ≥80%-word threshold; it is a lower bound on genuine paraphrase recovery. That bound is ≈ 0 here, so even generous slack cannot turn the item-level conclusion around — the support for 'no beyond-exposure recovery' is robust to matcher choice.

