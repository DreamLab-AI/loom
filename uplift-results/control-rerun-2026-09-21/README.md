# Control rerun, five arms, common retry policy — 2026-09-21

Regenerates the control cohort the paper reports in §`sec:controls` as a rerun "under a
common retry policy, with a fifth fluent-noise arm", whose rows were never written to
disk (`main.tex` L400; `docs/research/paper-v8/notes/R3-data.md` §3; `R5-numbers.md`
§1.3). This directory is that missing evidence, regenerated from scratch.

## What was run

| | |
|---|---|
| harness | `tools/paper/control_rerun.py` (imports `control_harness.py`, does not fork it) |
| questions | 57 frozen: 24 `uplift-results/general/arcane-questions.json` + 33 `thin-questions.json` |
| arms | `true`, `shuffled`, `masked`, `irrelevant`, `fluent_noise` |
| cells | 5 × 57 = 285 planned; 5 skipped (`arc_sov09`, no scaffold) ⇒ **280 attempted** |
| endpoint | loom façade `http://192.168.2.132:8084/v1/chat/completions`, `loom_options.scaffold=false` |
| model | `qwen3.8-27b-heretic-q8_0` (façade model alias `qwen3.8-27B`) |
| scaffolds | `http://192.168.2.132:8084/loom/scaffold` (LLM-free retrieval), fetched once per question |
| budgets | `max_tokens=4096`, single retry at `8192` if the answer came back empty |
| temperature | 0.0 |
| concurrency | 3 |
| judge | `openai/gpt-4.1` via `https://openrouter.ai/api/v1`, temperature 0, `judge_v2.RUBRIC` verbatim |
| started | 2026-09-21 14:47 UTC |

## Commands

```bash
# 1. generate the rows (resumable: re-running skips cells already non-empty)
python3 tools/paper/control_rerun.py --sets arcane,thin --concurrency 3 \
    --out uplift-results/control-rerun-2026-09-21

# 2. judge them (OPENROUTER_API_KEY must be set; OPENAI_API_KEY 401s in this estate)
python3 tools/paper/judge_openrouter.py \
    --rows uplift-results/control-rerun-2026-09-21/rows.jsonl \
           uplift-results/paper-v2/live-results.jsonl \
    --sets arcane,thin --model openai/gpt-4.1 \
    --out uplift-results/control-rerun-2026-09-21/judged.json

# 3. contrasts, Holm over the family, common-intersection sensitivity
python3 tools/paper/analyze_controls_v2.py \
    --judged uplift-results/control-rerun-2026-09-21/judged.json \
    --rows uplift-results/control-rerun-2026-09-21/rows.jsonl \
           uplift-results/paper-v2/live-results.jsonl \
    --label "rerun-4096-8192-5arm-2026-09-21" \
    --out-json uplift-results/control-rerun-2026-09-21/analysis.json

# 4. render the prose table from the JSON (never transcribed by hand)
python3 tools/paper/render_controls_md.py \
    --analysis uplift-results/control-rerun-2026-09-21/analysis.json \
    --title "Negative controls — five-arm rerun, common retry policy" \
    --out uplift-results/control-rerun-2026-09-21/ANALYSIS.md
```

Steps 2–4 are wrapped in `finish.sh`, which also writes the per-arm placebo-exposure
summary (`exposure.json` / `EXPOSURE.md`).

The `loom` and `raw` arms are **not** regenerated: they are the 2026-08-17
`uplift-results/paper-v2/live-results.jsonl` rows, re-judged by the same judge in the
same pass so every contrast is judge-controlled. That reuse is stated here because the
paper never said whether the rerun's raw arm was fresh or reused (R5 §1.4 flags exactly
this as unconfirmable) — for this cohort it is **reused, and those rows were produced
under the live harness's own retry policy, not the 4096/8192 policy applied to the five
scaffold arms**. Any contrast against `raw` in this cohort therefore mixes retry
policies, and that is a limitation of this rerun, not a hidden assumption.

## Files

| file | contents |
|---|---|
| `rows.jsonl` | one row per (question, arm): the exact `system_message` and `user_message` sent, the `injected_block` and its char/token size, the `donor` pairing for `irrelevant` (with both seed-IRI sets and a verified `iri_disjoint` flag), `fluent_noise_slugs`, the full `attempts` ledger (budget, finish_reason, prompt/completion/total tokens, latency, errors), the final completion, `final_budget`, and the per-row `exposure` measurement |
| `manifest-skipped.json` | cells that were never attempted, and why |
| `judged.json` | per-row judge score with judge model, base URL, temperature, rubric sha256[:16], candidate sha256[:16], timestamp |
| `analysis.json` | accounting, per-scope contrasts with bootstrap CIs / exact signed-rank / Holm, and the common-intersection repeat; every contrast lists the question ids behind it |
| `ANALYSIS.md` | generated from `analysis.json` |
| `exposure.json`, `EXPOSURE.md` | per-arm summary of the measured placebo exposure (`tools/paper/exposure_summary.py`) |
| `preamble.md` | the hand-written commentary `ANALYSIS.md` is built around |
| `finish.sh` | steps 2–4 above in one idempotent script |
| `run.log` | harness stderr, including the running s/cell and ETA |

## The fluent-noise arm

`irrelevant` is a donor scaffold: well-formed, on-corpus, wrong entities — but still a
coherent answer to *some* question, so a model could in principle key on that coherence.
`fluent_noise` removes the coherence while holding everything else fixed. Sections are
drawn at random from the same 8,146-class scaffold index, restricted to classes that are
**seed-disjoint** from the recipient — disjoint from its seed slugs *and* from their
1-hop relation targets and named ancestors (`_forbidden_slugs`) — rendered by the very
same `ontology_scaffold_v1._section_for` renderer the live path uses, and packed
greedily best-fit up to the recipient's own true-block token count. Best-fit rather than
fill-then-clamp: a candidate is kept only if it still fits, so the block lands at
0.95–0.99 of the true block's length instead of losing a whole trailing section to
`_clamp`. Selection is seeded on the question id (`fluent-noise::<set>::<id>`), so the
arm regenerates identically.

## Placebo exposure — measured, not asserted

Every row records how much of the question's gold target set its **injected block**
actually exposes, using the paper's own matcher (`decompose_exposure.gold_hit`,
byte-identical to the bench scorer). Gold targets for a control question are the titles
of its *true* scaffold's seed classes plus the question's declared `topic` — the
entities a correct answer has to name. Each row carries both its own arm's `exposure`
and the `true_block_exposure` of the same question, so the placebo arms can be scored
against the ceiling they are meant to fall short of.

This matters because the placebo arms are **not** at zero exposure. Sampled
fluent-noise blocks score 0.0–0.4 gold exposure, because generic class titles
("Smart Contracts", "Model", "Routing") recur across an 8,146-class corpus and the
matcher is lexical. Measuring it turns a claim into a covariate.

## Prompt placement (carried over deliberately)

Both the live path and the controls inject as a **system message**. The control harness
omits the `SYSTEM_PREAMBLE` authority sentence the live serving path prepends; this
rerun keeps that omission, unchanged, so the two cohorts stay comparable. Each row
records `system_preamble_included: false` and the exact system message sent, so the
difference is visible per row rather than buried in the harness.
