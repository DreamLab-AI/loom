# General-knowledge harness experiment — bare vs harness

240 gradings, 5 models. Quality = judge 0-5 vs frontier gold.
Question: does the confidence-gated ontology harness help lower-tier models on GENERAL questions, or make them jagged?

## Per-model quality (0-5): bare -> harness (Δ), by category

| model | off_domain b→h (Δ) | in_domain_general b→h (Δ) | adjacent b→h (Δ) |
|---|---|---|---|
| claude-haiku-4.5 | -→- (+-) | -→- (+-) | -→- (+-) |
| deepseek-chat | -→- (+-) | -→- (+-) | -→- (+-) |
| gemini-2.5-flash-lite | -→- (+-) | -→- (+-) | -→- (+-) |
| gpt-4.1-mini | -→- (+-) | -→- (+-) | -→- (+-) |
| mistral-small-24b | -→- (+-) | -→- (+-) | -→- (+-) |

## Averaged across models, per category

| category | bare | harness | Δ | n(gradings) |
|---|---|---|---|---|
| off_domain | - | - | +- | 0 |
| in_domain_general | - | - | +- | 0 |
| adjacent | - | - | +- | 0 |

## Paired harness−bare delta with 95% bootstrap CI (the isolated assertion)

Paired per (model,question); bootstrap over pairs. CI excludes 0 ⇒ real effect.

| subset | mean Δ | 95% CI | n pairs | reading |
|---|---|---|---|---|
| all questions | -0.092 | [-0.333, 0.15] | 120 | null / no effect |
| off_domain | - | [None, None] | 0 | - |
| in_domain_general | - | [None, None] | 0 | - |
| adjacent | - | [None, None] | 0 | - |
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
(0,-) (1,-) (2,-) 
% harness
(0,-) (1,-) (2,-) 
```