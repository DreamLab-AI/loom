# General-knowledge harness experiment — bare vs harness

600 gradings, 5 models. Quality = judge 0-5 vs frontier gold.
Question: does the confidence-gated ontology harness help lower-tier models on GENERAL questions, or make them jagged?

## Per-model quality (0-5): bare -> harness (Δ), by category

| model | off_domain b→h (Δ) | in_domain_general b→h (Δ) | adjacent b→h (Δ) |
|---|---|---|---|
| claude-haiku-4.5 | 5.00→5.00 (+0.00) | 5.00→4.95 (-0.05) | 5.00→5.00 (+0.00) |
| deepseek-chat | 5.00→5.00 (+0.00) | 5.00→5.00 (+0.00) | 5.00→5.00 (+0.00) |
| gemini-2.5-flash-lite | 4.85→4.35 (-0.50) | 4.95→4.85 (-0.10) | 4.95→5.00 (+0.05) |
| gpt-4.1-mini | 4.90→4.95 (+0.05) | 5.00→5.00 (+0.00) | 5.00→5.00 (+0.00) |
| mistral-small-24b | 4.95→4.95 (+0.00) | 5.00→4.85 (-0.15) | 4.95→4.95 (+0.00) |

## Averaged across models, per category

| category | bare | harness | Δ | n(gradings) |
|---|---|---|---|---|
| off_domain | 4.94 | 4.85 | -0.09 | 100 |
| in_domain_general | 4.99 | 4.93 | -0.06 | 100 |
| adjacent | 4.98 | 4.99 | +0.01 | 100 |

## Paired harness−bare delta with 95% bootstrap CI (the isolated assertion)

Paired per (model,question); bootstrap over pairs. CI excludes 0 ⇒ real effect.

| subset | mean Δ | 95% CI | n pairs | reading |
|---|---|---|---|---|
| all questions | -0.047 | [-0.107, 0.0] | 300 | null / no effect |
| off_domain | -0.09 | [-0.25, 0.02] | 100 | null / no effect |
| in_domain_general | -0.06 | [-0.14, 0.0] | 100 | null / no effect |
| adjacent | 0.01 | [-0.02, 0.05] | 100 | null / no effect |
| off_domain gate-engaged (worst case) | -0.225 | [-0.625, 0.05] | 40 | null / no effect |

## Jaggedness check — off-domain questions

| off-domain subset | bare | harness | Δ | n |
|---|---|---|---|---|
| gate ENGAGED (mis-fired) | 4.95 | 4.72 | -0.23 | 40 |
| gate skipped (correct) | 4.93 | 4.93 | +0.00 | 60 |

*If harness ≈ bare on mis-fired off-domain questions, models shrug off irrelevant injected context (robust, non-jagged). If harness < bare, the over-firing gate measurably degrades general answers.*

## pgfplots — bare vs harness by category (x=0 off,1 in-domain,2 adjacent)
```
% bare
(0,4.94) (1,4.99) (2,4.98) 
% harness
(0,4.85) (1,4.93) (2,4.99) 
```