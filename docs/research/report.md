# Ontology Uplift Benchmark Report

> **Local models, `temp=0`, 37-question set.** The first **cloud** model (Gemini 3.7 Flash,
> `temp=1.0`, 510-question set) is reported separately in
> [`report-gemini-3.7-flash.md`](report-gemini-3.7-flash.md) — different temperature and set
> size mean its absolute recall is not cell-for-cell comparable here, but its paired uplift
> (+0.583) lands the grounded ceiling at the same ~0.94 as both models below. Three models,
> one ceiling.

Score files: scores-muse-glimmer-raw.jsonl, scores-muse-glimmer-scaffold.jsonl, scores-muse-glimmer-scaffold-prose.jsonl, scores-muse-glimmer-tools.jsonl, scores-gemma-raw.jsonl, scores-gemma-scaffold.jsonl, scores-gemma-scaffold-prose.jsonl, scores-gemma-tools.jsonl

## Summary (model x mode)

| model | mode | n | errors | mean recall | recall (engaged only) | extra recall (T-TAX ancestors) | judge 0-5 | mean injected tok | mean latency ms |
|---|---|---|---|---|---|---|---|---|---|
| gemma | raw | 37 | 0 | 0.146 | - | 0.063 | - | 0 | 31466 |
| gemma | scaffold | 37 | 0 | 0.939 | 0.939 | 0.930 | - | 1398 | 5108 |
| gemma | scaffold-prose | 37 | 0 | 0.939 | 0.939 | 0.930 | - | 1373 | 4832 |
| gemma | tools | 37 | 0 | 0.973 | 0.973 | 0.897 | - | 0 | 7613 |
| muse-glimmer | raw | 37 | 0 | 0.268 | - | 0.166 | - | 0 | 34720 |
| muse-glimmer | scaffold | 37 | 0 | 0.939 | 0.939 | 1.000 | - | 1398 | 9783 |
| muse-glimmer | scaffold-prose | 37 | 0 | 0.946 | 0.946 | 1.000 | - | 1373 | 10085 |
| muse-glimmer | tools | 37 | 0 | 0.649 | 0.649 | 0.267 | - | 0 | 14971 |

## Mean recall by domain

| domain | gemma (raw) | gemma (scaffold) | gemma (scaffold-prose) | gemma (tools) | muse-glimmer (raw) | muse-glimmer (scaffold) | muse-glimmer (scaffold-prose) | muse-glimmer (tools) |
|---|---|---|---|---|---|---|---|---|
| ai | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 |
| artificial-intelligence | 0.500 | 0.833 | 0.833 | 1.000 | 0.500 | 0.833 | 0.917 | 1.000 |
| blockchain | 0.222 | 1.000 | 1.000 | 1.000 | 0.111 | 1.000 | 1.000 | 1.000 |
| data | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| distributed-collaboration | 0.000 | 0.917 | 0.917 | 1.000 | 0.000 | 0.917 | 0.917 | 0.333 |
| distributed-systems | 0.083 | 0.917 | 0.917 | 1.000 | 0.083 | 0.917 | 0.917 | 0.667 |
| finance | 0.333 | 1.000 | 1.000 | 1.000 | 0.333 | 1.000 | 1.000 | 1.000 |
| governance | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 0.000 |
| infrastructure | 0.000 | 0.917 | 0.917 | 0.667 | 0.000 | 0.917 | 0.917 | 0.333 |
| machine-learning | 0.000 | 0.917 | 0.917 | 1.000 | 0.444 | 0.917 | 0.917 | 0.667 |
| metaverse | 0.333 | 1.000 | 1.000 | 1.000 | 0.333 | 1.000 | 1.000 | 0.667 |
| robotics | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.667 |
| security | 0.000 | 1.000 | 1.000 | 1.000 | 0.333 | 1.000 | 1.000 | 0.667 |
| spatial-computing | 0.000 | 0.750 | 0.750 | 1.000 | 0.167 | 0.750 | 0.750 | 0.667 |
| standards | 0.000 | 1.000 | 1.000 | 1.000 | 0.000 | 1.000 | 1.000 | 0.000 |

## Mean recall by template

| template | gemma (raw) | gemma (scaffold) | gemma (scaffold-prose) | gemma (tools) | muse-glimmer (raw) | muse-glimmer (scaffold) | muse-glimmer (scaffold-prose) | muse-glimmer (tools) |
|---|---|---|---|---|---|---|---|---|
| T-REL | 0.110 | 0.898 | 0.898 | 0.955 | 0.133 | 0.898 | 0.909 | 0.909 |
| T-TAX | 0.200 | 1.000 | 1.000 | 1.000 | 0.467 | 1.000 | 1.000 | 0.267 |

## Paired uplift (per model, vs raw, intersection of question ids)

- PAIRED UPLIFT gemma (scaffold - raw): delta=+0.793 recall, 95% CI [+0.680, +0.894] (bootstrap 10000 resamples, seed 42), n=37, excluded_not_engaged=0, excluded_errors=0
- PAIRED UPLIFT gemma (scaffold-prose - raw): delta=+0.793 recall, 95% CI [+0.680, +0.894] (bootstrap 10000 resamples, seed 42), n=37, excluded_not_engaged=0, excluded_errors=0
- PAIRED UPLIFT gemma (tools - raw): delta=+0.827 recall, 95% CI [+0.707, +0.932] (bootstrap 10000 resamples, seed 42), n=37, excluded_not_engaged=0, excluded_errors=0
- PAIRED UPLIFT muse-glimmer (scaffold - raw): delta=+0.671 recall, 95% CI [+0.527, +0.804] (bootstrap 10000 resamples, seed 42), n=37, excluded_not_engaged=0, excluded_errors=0
- PAIRED UPLIFT muse-glimmer (scaffold-prose - raw): delta=+0.678 recall, 95% CI [+0.534, +0.809] (bootstrap 10000 resamples, seed 42), n=37, excluded_not_engaged=0, excluded_errors=0
- PAIRED UPLIFT muse-glimmer (tools - raw): delta=+0.381 recall, 95% CI [+0.162, +0.581] (bootstrap 10000 resamples, seed 42), n=37, excluded_not_engaged=0, excluded_errors=0

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
