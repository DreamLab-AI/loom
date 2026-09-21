# General-knowledge harness experiment — bare vs harness

570 gradings, 5 models. Quality = judge 0-5 vs frontier gold.
Question: does the confidence-gated ontology harness help lower-tier models on GENERAL questions, or make them jagged?

## Per-model quality (0-5): bare -> harness (Δ), by category

| model | arcane_documented b→h (Δ) | arcane_thin b→h (Δ) |
|---|---|---|
| claude-haiku-4.5 | 4.50→4.62 (+0.12) | 4.07→3.95 (-0.12) |
| deepseek-chat | 4.75→4.88 (+0.12) | 4.22→4.27 (+0.05) |
| gemini-2.5-flash-lite | 4.00→3.75 (-0.25) | 3.49→2.98 (-0.51) |
| gpt-4.1-mini | 4.56→4.62 (+0.06) | 3.93→3.95 (+0.02) |
| mistral-small-24b | 3.44→3.25 (-0.19) | 3.00→3.00 (+0.00) |

## Averaged across models, per category

| category | bare | harness | Δ | n(gradings) |
|---|---|---|---|---|
| arcane_documented | 4.25 | 4.22 | -0.03 | 80 |
| arcane_thin | 3.74 | 3.63 | -0.11 | 205 |

## Paired harness−bare delta with 95% bootstrap CI (the isolated assertion)

Paired per (model,question); bootstrap over pairs. CI excludes 0 ⇒ real effect.

| subset | mean Δ | 95% CI | n pairs | reading |
|---|---|---|---|---|
| all questions | -0.088 | [-0.218, 0.039] | 285 | null / no effect |
| arcane_documented | -0.025 | [-0.237, 0.188] | 80 | null / no effect |
| arcane_thin | -0.112 | [-0.273, 0.044] | 205 | null / no effect |
| off_domain gate-engaged (worst case) | - | [None, None] | 0 | - |

## Jaggedness check — off-domain questions

| off-domain subset | bare | harness | Δ | n |
|---|---|---|---|---|
| gate ENGAGED (mis-fired) | - | - | +- | 0 |
| gate skipped (correct) | - | - | +- | 0 |

*If harness ≈ bare on mis-fired off-domain questions, models shrug off irrelevant injected context (robust, non-jagged). If harness < bare, the over-firing gate measurably degrades general answers.*

## pgfplots — bare vs harness by category (x=0 off,1 in-domain,2 adjacent)
```
% bare
(0,4.25) (1,3.74) 
% harness
(0,4.22) (1,3.63) 
```