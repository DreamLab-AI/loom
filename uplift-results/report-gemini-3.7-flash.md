# Ontology Uplift Benchmark Report

Score files: scores-gemini-3.7-flash-raw.jsonl, scores-gemini-3.7-flash-scaffold.jsonl

## Summary (model x mode)

| model | mode | n | errors | mean recall | recall (engaged only) | extra recall (T-TAX ancestors) | judge 0-5 | mean injected tok | mean latency ms |
|---|---|---|---|---|---|---|---|---|---|
| gemini-3.7-flash | raw | 510 | 0 | 0.359 | - | 0.208 | - | 0 | 2368 |
| gemini-3.7-flash | scaffold | 510 | 0 | 0.942 | 0.942 | 1.000 | - | 1343 | 1214 |

## Mean recall by domain

| domain | gemini-3.7-flash (raw) | gemini-3.7-flash (scaffold) |
|---|---|---|
| ai | 0.393 | 0.889 |
| artificial-intelligence | 0.267 | 0.938 |
| blockchain | 0.343 | 0.963 |
| data | 0.296 | 0.908 |
| distributed-collaboration | 0.224 | 0.948 |
| distributed-systems | 0.475 | 0.946 |
| finance | 0.432 | 0.957 |
| governance | 0.523 | 0.948 |
| infrastructure | 0.245 | 0.964 |
| machine-learning | 0.482 | 0.950 |
| metaverse | 0.353 | 0.942 |
| robotics | 0.420 | 0.912 |
| security | 0.376 | 0.916 |
| spatial-computing | 0.227 | 0.969 |
| standards | 0.396 | 0.977 |

## Mean recall by template

| template | gemini-3.7-flash (raw) | gemini-3.7-flash (scaffold) |
|---|---|---|
| T-COMMON | 0.333 | 1.000 |
| T-REL | 0.339 | 0.898 |
| T-TAX | 0.397 | 0.997 |

## Paired uplift (per model, vs raw, intersection of question ids)

- PAIRED UPLIFT gemini-3.7-flash (scaffold - raw): delta=+0.583 recall, 95% CI [+0.546, +0.618] (bootstrap 10000 resamples, seed 42), n=510, excluded_not_engaged=0, excluded_errors=0

## Honest notes (read before quoting numbers)

1. **The scaffold contains gold-adjacent facts BY DESIGN.** Questions and gold
   are both derived from the knowledge graph, and scaffold mode injects a
   budget-clamped extract of that same graph. Scaffolded scores therefore
   measure *grounded-answer capability and retrieval quality* (can the model
   find, trust and restate the injected facts); raw scores measure *parametric
   knowledge*. The paired delta is "uplift available from grounding", not
   "model quality".
2. **Substring scoring undercounts paraphrases.** A gold item only counts as a
   hit when its title (or >=80% of its length-4+ words) appears in the answer.
   Treat absolute recall numbers as lower bounds; the paired deltas — same
   questions, same scorer, same model — are the signal.
3. Questions where the scaffold never engaged are excluded from the paired
   delta: both arms saw the identical prompt, so they measure nothing about
   uplift. They are counted separately above.
