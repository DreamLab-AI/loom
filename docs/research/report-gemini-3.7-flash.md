# Ontology Uplift — Gemini 3.7 Flash (first cloud model)

**The first cloud model benched against the Loom scaffold, and the strongest
external validation of the model-agnostic-uplift thesis so far.** Gemini 3.7
Flash arrives at the *same* grounded recall (~0.94) as the two local models —
from a higher raw baseline — and, like them, gets **faster** when grounded.

## Provenance (read before quoting)

| Field | Value |
|---|---|
| Model | `gemini-3.7-flash` (released 2026-08-13), Google OpenAI-compat endpoint `…/v1beta/openai/` |
| Auth | `GOOGLE_API_KEY` (AI Studio) as Bearer |
| Corpus generation | mirrored deployed generation — **8,146 classes / 282,492 triples**, `generatedAt 2026-08-15T13:22:49Z` (byte-matches the live façade at `192.168.2.132:8084`) |
| Question set | seed 42, **510 questions** across 15 domains (T-REL 285, T-TAX 180, T-COMMON 45) |
| Run config | `--reasoning-effort low --temp 1.0 --max-tokens 2048 --timeout 120 --retries 3 --sleep 0.4` |
| Errors | **0 / 1020** calls (raw + scaffold) |
| Date | 2026-08-16 |

**Why this config, and why it differs from the local-model runs.** Gemini 3.x
cannot disable thinking (floor `low`, default `medium`) and thinking tokens are
drawn from the output budget — at the harness default `--max-tokens 400` the
model truncated to `finish_reason: length` (verified live), so we raised the
ceiling to 2048 and set `reasoning_effort=low`. Google also flags `temperature<1.0`
as off-label for Gemini 3.x (looping / degradation), so this run uses `temp=1.0`,
whereas the local Gemma/Muse runs used `temp=0`. **These numbers are therefore
NOT directly comparable, cell-for-cell, to the local-model report** — the paired
uplift is a *within-model* delta (same temperature both arms) and stays valid;
the absolute raw recall is not on the same footing as the temp=0 runs. The set is
also 510 questions vs the local report's 37 (the deployed generation has 15
domains with ≥50 eligible classes).

## Summary (model x mode)

| model | mode | n | errors | mean recall | recall (engaged only) | extra recall (T-TAX ancestors) | mean injected tok | mean latency ms |
|---|---|---|---|---|---|---|---|---|
| gemini-3.7-flash | raw | 510 | 0 | 0.359 | - | 0.208 | 0 | 2368 |
| gemini-3.7-flash | scaffold | 510 | 0 | **0.942** | 0.942 | 1.000 | 1343 | **1214** |

Grounding nearly **halves** latency (2.37 s → 1.21 s): the model stops doing
heavy open-ended recall and restates supplied facts — the same signature the
local models show, now on a cloud endpoint where the drop is network-dominated.

## Paired uplift (headline)

```
PAIRED UPLIFT gemini-3.7-flash (scaffold − raw):
  delta = +0.583 recall, 95% CI [+0.546, +0.618]
  (bootstrap 10000 resamples, seed 42), n = 510,
  excluded_not_engaged = 0, excluded_errors = 0
```

The CI excludes zero by a wide margin at n=510. The scaffold engaged on **every**
question (0 not-engaged), so the delta is computed over the full set.

## Mean recall by domain

| domain | raw | scaffold |
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

The lift concentrates where the model is weakest raw — spatial-computing
0.227→0.969, distributed-collaboration 0.224→0.948, infrastructure 0.245→0.964 —
and adds least where it is already strong (governance 0.523→0.948). That
weak-gets-the-most-lift pattern is the fingerprint of real grounding, not gold
leakage.

## Mean recall by template

| template | raw | scaffold |
|---|---|---|
| T-COMMON (shared ancestor of a pair) | 0.333 | 1.000 |
| T-REL (requires/uses/parts/…) | 0.339 | 0.898 |
| T-TAX (parent + ancestors) | 0.397 | 0.997 |

## Cross-model convergence (the thesis)

Three models, three very different raw baselines, one grounded ceiling:

| Model | Raw | + Loom scaffold | Paired uplift | Runs |
|---|---|---|---|---|
| Gemma-4-31B (local, temp 0) | 0.146 | 0.939 | +0.793 | 37 q |
| Muse-Glimmer-30B (local, temp 0) | 0.268 | 0.939 | +0.671 | 37 q |
| **Gemini 3.7 Flash (cloud, temp 1.0)** | 0.359 | **0.942** | **+0.583** | 510 q |

The stronger the model's parametric knowledge, the smaller the *uplift* it needs
— but all three land at ~0.94 grounded. Grounding closes the gap regardless of
where the model starts. This is exactly what the façade design bets on: the model
behind the door is swappable because the scaffold, not the model, carries the
recall.

## Honest notes

The three standing honesty notes apply verbatim (scaffold contains gold-adjacent
facts *by design*; substring scoring undercounts paraphrases → absolute recall is
a lower bound, the paired delta is the signal; not-engaged questions are excluded
— here there were none). See [`report.md`](report.md) and the
[bench protocol](../../bench/UPLIFT-BENCH-PROTOCOL.md) cloud-model axis.
Reproduce with `bench/run-gemini.sh`.
