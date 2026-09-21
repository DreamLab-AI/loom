# Crossover analysis — judged quality vs web-findability

1540 gradings, 7 models + web arm. Quality = judge 0-5 vs independent frontier gold.

## Quality by findability bin, per grounding arm (avg across models)

| findability bin | raw | ontology | web | n(questions) |
|---|---|---|---|---|
| 0.0-0.4 (private) | 1.76 | 4.20 | - | 16 |
| 0.4-0.6 | 3.03 | 4.41 | - | 16 |
| 0.6-0.8 | 3.04 | 4.00 | - | 35 |
| 0.8-1.0 (public) | 3.42 | 4.20 | - | 43 |

## Per-model quality (0-5), by stratum

| model | pub raw | pub onto | Δpub | priv raw | priv onto | Δpriv | fam-match |
|---|---|---|---|---|---|---|---|
| claude-haiku-4.5 | 1.68 | 4.46 | 2.78 | 1.21 | 4.64 | 3.43 |  |
| deepseek-chat | 3.98 | 4.17 | 0.19 | 1.79 | 4.14 | 2.36 |  |
| gemini-2.5-flash-lite | 3.20 | 4.00 | 0.80 | 1.43 | 3.93 | 2.50 |  |
| gemini-3.5-flash-lite | 3.39 | 4.12 | 0.74 | 1.50 | 4.29 | 2.79 |  |
| gemini-3.7-flash-t0 | 3.77 | 4.09 | 0.32 | 2.00 | 4.36 | 2.36 |  |
| gpt-4.1-mini | 2.97 | 4.09 | 1.12 | 1.50 | 4.14 | 2.64 | YES |
| mistral-small-24b | 3.46 | 4.15 | 0.69 | 1.71 | 4.14 | 2.43 |  |

## pgfplots — crossover (x=findability, y=quality)
```
% raw (parametric)
(0.2,1.76) (0.5,3.03) (0.7,3.04) (0.9,3.42) 
% ontology-grounded
(0.2,4.20) (0.5,4.41) (0.7,4.00) (0.9,4.20) 
% web-grounded

```