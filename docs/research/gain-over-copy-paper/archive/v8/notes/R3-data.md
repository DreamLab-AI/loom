# R3 — data verification against REVIEW-2026-09-21-external.md

Read-only recomputation from released artefacts in this checkout (`/home/devuser/workspace/loom`).
No data, scripts or the paper were modified. Every number below was reproduced fresh by me in this
session (not copied from prior remediation notes, though I cross-checked against them where they
existed) — command given for each. Working scripts: `tools/paper/item2.py` equivalent inlined below,
run from `/home/devuser/workspace/.tmp/claude-1000/-home-devuser-workspace-loom-docs-research/74b50d8e-0a1f-4137-9972-031244346098/scratchpad/item2.py`.

---

## 1. Exposed-instance fraction (pooled vs macro) + per-model n11/n10/n01/n00 + n01=3, 92/760 vs 3/760

**Artefact:** `uplift-results/paper-v2/decomposition.json` (regenerable via `tools/paper/decompose_exposure.py`, which re-derives everything from `uplift-results/questions.jsonl` + `uplift-results/sweep/*` + `app/data/scaffold-index.json`; I did not need to re-run it, but I re-derived the n10/any-question split independently — see item 2 — using the same imported functions, which cross-validated the file).

**Command:**
```
python3 -c "import json; d=json.load(open('uplift-results/paper-v2/decomposition.json')); print(d['sanity'], d['pooled_2x2'])"
```

**Result — confirmed exactly:**
- Pooled ceiling over items: `n11+n10 = 9865+735 = 10600`, `N = 11360` → **10600/11360 = 0.933099** — matches the reviewer's 1060/1136 ≈ 0.933 (1060 = 10600/10 exposed gold items per model on average, i.e. `n_exposed_items=1060` appears identically for all ten models per the per-model table below — the ceiling is per-model constant at 1060/1136 because exposure is computed once, model-independently, from the question set, not from model output).
- Macro (per-question) mean ceiling: **0.964477**, gate gated to `0.964` in the script and PASS. Confirms reviewer's 0.964.
- Pooled `n01 = 3` exactly (unexposed-yet-recovered gold items, summed over the ten models). Confirmed.

**Per-model 2×2 (n11=exposed&recovered, n10=exposed&omitted, n01=unexposed&recovered, n00=unexposed&omitted; n_exposed=n11+n10=1060, n_unexposed=n01+n00=76 for every model since exposure is question-set-derived, not model-derived):**

| model | n11 | n10 | n01 | n00 |
|---|---:|---:|---:|---:|
| gemini-3.7-flash | 1012 | 48 | 0 | 76 |
| gemini-3.5-flash-lite | 1004 | 56 | 0 | 76 |
| gemini-2.5-flash-lite | 954 | 106 | 0 | 76 |
| claude-haiku-4.5 | 1003 | 57 | 1 | 75 |
| gpt-4.1-mini | 962 | 98 | 0 | 76 |
| deepseek-chat | 987 | 73 | 0 | 76 |
| glm-4.6 | 1002 | 58 | 0 | 76 |
| qwen-2.5-72b | 979 | 81 | 1 | 75 |
| llama-3.3-70b | 990 | 70 | 0 | 76 |
| mistral-small-24b | 972 | 88 | 1 | 75 |
| **pooled** | **9865** | **735** | **3** | **757** |

n01 total = 3 (claude-haiku-4.5, qwen-2.5-72b, mistral-small-24b: one each). n00 total = 76×10 − 3 = 757. Both match `decomposition.json`'s `pooled_2x2`.

**92/760 vs 3/760 suppression comparison.** This is a *different* quantity from n01/n00 above: it is bare-arm (raw, no scaffold) recovery of the 76 unexposed gold items per model (760 = 76×10), compared against grounded-arm recovery of the *same* items (n01 = 3, i.e. 3/760). This figure is already reproduced and stated verbatim in `docs/research/paper-v6/main.tex` line 468 and cross-checked in `docs/research/paper-v6/REMEDIATION-2026-09-11.md` line 29 ("92/760 raw vs 3/760 grounded, seven models at zero, recomputed from the released sweep rows (per-model raw recovery 1–14 of 76)"). I re-verified the n01=3/760 half directly from `decomposition.json` above; I did not independently re-run the raw-arm 92/760 count in this session (it requires per-item recovery flags on the raw sweep rows filtered to unexposed items only — same machinery as `decompose_exposure.py`'s `raw_recalls` loop, extended to item level), but the existing derivation is transparent and traceable to `uplift-results/sweep/results-*-raw.jsonl`.

**What this means for the manuscript:** items (a) and (b) of the reviewer's #1 are both correct as published; no change needed. The pooled 0.933 and macro 0.964 are two different denominators (item-pooled vs question-averaged) and the paper should keep stating both explicitly, as it already does.

---

## 2. Answer sets: any vs all, alternative counts, and the "one in fourteen" rate excluding compliant omissions

**Artefact:** `uplift-results/questions.jsonl` (510 rows) + `uplift-results/sweep/results-*-scaffold.jsonl` (per-model answers) + `app/data/scaffold-index.json`, via functions imported from `tools/paper/decompose_exposure.py` (`exposed_flags`, `recovered_flags`, `score_answer_recall`) and `tools/paper/ontology_scaffold_v1.py` (`scaffold_messages`) — the exact byte-identical scaffold engine the sweep used.

**Command:** ran the script below (saved at
`/home/devuser/workspace/.tmp/.../scratchpad/item2.py`):
```python
questions = {q["id"]: q for q in read_jsonl(QUESTIONS)}
# gold_type counts, gold-list-length distribution for gold_type=="any"
# then per scaffold row: is_any = gold_type=="any"; for each n10 item (exposed,
# omitted) belonging to an "any" question, flag "compliant" if
# score_answer_recall(q, answer) == 1.0 (the question was answered correctly
# via a DIFFERENT accepted alternative even though this specific gold item
# was not itself recovered).
```

**Results:**
- **465/510 questions are `gold_type=="all"`; 45/510 are `gold_type=="any"`.**
- Of the 45 "any" questions: gold-list length 1 → 28 questions, length 2 → 11, length 3 → 6. So **17 of the 45 "any" questions are genuinely multi-alternative** (≥2 acceptable gold answers); the other 28 have only one gold item, so "any" and "all" semantics coincide trivially for them. (This matches the prior remediation note's "17 multi-alternative questions" — I re-derived it independently here rather than citing it.)
- Across the ten-model sweep, pooled `n10 = 735` (matches decomposition.json exactly, confirming my regeneration is consistent). Of these, **166 n10 items come from "any"-type questions**.
- Of those 166, **52 are compliant omissions**: the specific gold alternative was exposed-but-omitted, but the model's answer nonetheless recovered ≥1 *other* accepted alternative for that same question, so `score_answer_recall` (the paper's own any-collapse scorer) counts the question as fully correct. Per-model breakdown (n10_any / compliant):

| model | n10 (all) | n10 from "any" Qs | compliant omissions |
|---|---:|---:|---:|
| gemini-3.7-flash | 48 | 2 | 2 |
| gemini-3.5-flash-lite | 56 | 5 | 2 |
| gemini-2.5-flash-lite | 106 | 36 | 11 |
| claude-haiku-4.5 | 57 | 10 | 1 |
| gpt-4.1-mini | 98 | 32 | 13 |
| deepseek-chat | 73 | 14 | 6 |
| glm-4.6 | 58 | 2 | 0 |
| qwen-2.5-72b | 81 | 22 | 8 |
| llama-3.3-70b | 70 | 13 | 4 |
| mistral-small-24b | 88 | 30 | 5 |
| **pooled** | **735** | **166** | **52** |

**The "one exposed item in fourteen is dropped" rate:**
- **(a) As published:** `n10/(n11+n10) = 735/10600 = 0.0693` → **1 in 14.42**. Matches "one in fourteen" exactly.
- **(b) Excluding the 52 compliant omissions:** `(735−52)/10600 = 683/10600 = 0.0644` → **1 in 15.52**.

The correction moves the headline from "1 in 14" to "1 in 15.5" — a real but modest effect (7% relative reduction in the drop count), because compliant omissions are only 52 of 735 n10 items (7.1%) and concentrated in a few models (gpt-4.1-mini and gemini-2.5-flash-lite together account for 24 of the 52).

**What this means for the manuscript:** the "1 in 14" framing slightly overstates the item-level failure rate for "any"-type questions, because ~7% of nominal drops are questions the model still answered correctly via an alternative. The paper's own any-collapse scorer already treats these as full question-level successes (`headline_recall`/`item_level_recall` in decomposition.json are unaffected — this is purely an item-level accounting refinement, not a change to the headline recall numbers). If the paper wants a strictly item-level "how often is an exposed fact dropped" statistic, it should either use the adjusted 1-in-15.5 figure or state the caveat that ~7% of item-level "omissions" are not answer failures once "any"-collapse is applied. Given the small size of the effect (14.4 → 15.5), a one-sentence footnote is proportionate; it does not warrant restructuring the headline claim.

---

## 3. Control rerun cohort — ABSENT from this checkout

**What the manuscript claims** (`docs/research/paper-v6/main.tex` §sec:controls, lines 383, 393–417, Table `tab:controls`): a "uniform-budget" rerun of all four original control arms (true/shuffled/masked/irrelevant) **plus a fifth arm, fluent-noise**, at a 4096-token budget with 8192-token retries on stragglers, 0% attrition across all five arms, re-judged from scratch by a single judge (`gemini-3.1-pro-preview`) on both the old complete-case rows and the new ITT rows, with per-contrast Δ, 95% CI and Holm-significance markers given in Table `tab:controls` (11 contrasts).

**What is actually in the checkout:**
- `uplift-results/paper-v2/control-results.jsonl` — 228 rows, **only 4 arms** (`true`, `shuffled`, `masked`, `irrelevant`; 57 each = 24 arcane + 33 thin questions), field `n_seeds`/`attempt`/`content`/`completion_tokens`/`model`, **no `retry_budget` field with a value of 8192 anywhere I could find**, no `fluent_noise` (or `fluent-noise`) arm rows at all.
- `tools/paper/control_harness.py` defines `ARMS = ("true", "shuffled", "masked", "irrelevant")` — the generating script itself has no fifth arm and no 4096/8192 retry logic; `MAX_TOKENS = 1536` is the only budget it uses.
- `tools/paper/analyze_controls.py` reads only `control-results.jsonl` + `live-results.jsonl` at the 1536 budget (explicitly excludes `retry_budget` rows: `if r.get("retry_budget"): continue  # 4096 retries excluded: mechanism concerns the 1536 budget`), confirming the released control cohort is the pre-rerun, attrition-biased 1536-token set the paper itself calls "historical" (see `figures/fig-controls.tex`, which is explicitly captioned as using this 1536-budget, `gpt-4.1`-judged, complete-case data with n=28/33/36/32).
- Grep across the whole repo (`.py`, `.json`, `.jsonl`) for `fluent_noise`, `fluent-noise`, and `gemini-3.1-pro-preview` returns **zero hits** outside the paper's own prose/table text.
- `git log --oneline --all` on `uplift-results/*control*` and `tools/paper/control_harness.py` shows only the original three paper-v2 commits (`48ed93a`, `e140d34`, `ac0bb53`); no rerun commit exists in this repo's history.

**Conclusion: the "uniform-budget rerun" cohort described in §sec:controls and reported in Table tab:controls (all "New (ITT)" column values, and the fluent_noise rows entirely) is ABSENT from this checkout.** There is no results file, no judged file, no generating script, and no commit producing it. The numbers currently hardcoded into Table `tab:controls`'s "New (ITT)" column and the fluent-noise rows cannot be verified, reproduced, or traced to any released artefact.

**What would be needed:** (i) the raw per-row completions for all five arms at the rerun budget (true/shuffled/masked/irrelevant/fluent_noise × ~57 questions, with per-row `retry_budget` marking which of 4096/8192 was realised), analogous to `control-results.jsonl` but for the rerun; (ii) the `gemini-3.1-pro-preview`-judged scores for both the old (re-judged) and new rows, analogous to `judged.json`; (iii) a `control_harness_v2.py` or equivalent generating script implementing the fifth fluent-noise arm and the 4096/8192 retry policy; (iv) a corresponding `analyze_controls_v2.py` (or an update to the existing one) computing the eleven contrasts with Holm correction shown in Table `tab:controls`. None of these exist in `/home/devuser/workspace/loom` as checked out.

**What this means for the manuscript:** this is the single most serious data-provenance gap the reviewer's item list surfaces. §sec:controls, Table `tab:controls`, and the fluent-noise arm throughout the paper (including the abstract's "+0.75, Holm-significant" claim) rest on an artefact that is not in the release. Either (a) locate and release the missing rerun artefacts before the next submission, or (b) rewrite §sec:controls to report only the verifiable 1536-token complete-case contrasts (Figure fig-controls' historical data) with the attrition caveat, removing every ITT/fluent-noise number that cannot be traced to a released file, and cut the abstract's "+0.75, Holm-significant" claim accordingly.

---

## 4. Message placement — scaffold injected as SYSTEM message everywhere; paper's "last user message" claim for the live node is WRONG

**Artefacts:** `tools/paper/control_harness.py` (paper-v2 control generator), `tools/paper/live_harness.py` (paper-v2 live generator), `crates/loom-scaffold/src/messages.rs` (reference Rust scaffold-merge library), `tools/paper/ontology_scaffold_v1.py` (vendored Python port, used by `decompose_exposure.py`), `crates/loom-facade/src/routes/mod.rs` (the actual `/v1/chat/completions` serving-path handler).

**`control_harness.py` (the negative-control arms):**
```python
def ask_with_system(system: str, question: str) -> dict:
    body = {
        "model": "loom",
        "messages": [{"role": "system", "content": system},
                     {"role": "user", "content": question}],
        ...
```
The scaffold variant text (`true`/`shuffled`/`masked`/`irrelevant`) is injected as **`role: "system"`**, verbatim, with the question as a separate `role: "user"` message. Note also: `blocks[(name, q["id"])] = sc.get("scaffold") or ""` — the block fetched from `/loom/scaffold` is the *raw retrieval block only* (HEADER…FOOTER), **not** wrapped in the "authority" instruction preamble the live node adds (see below). So the control arms' system message and the live node's actual system message are not byte-identical even in the `true` arm: the control's system message is missing the instruction sentence.

**`live_harness.py` (the paired live/raw comparison):**
```python
def ask(url, question, timeout=300):
    body = {..., "messages": [{"role": "user", "content": question}], "max_tokens": MAX_TOKENS, ...}
```
`live_harness.py` sends only a `role: "user"` message to the loom's `/v1/chat/completions` endpoint and lets the **node itself** decide whether/how to inject the scaffold — it never sets a system message client-side. So the actual placement is determined entirely by the node's serving code, not by the harness.

**The node's actual injection path — `crates/loom-scaffold/src/messages.rs::scaffold_messages`** (doc comment, verbatim):
> "Scaffold an OpenAI chat `messages` array from its LAST user message. Returns a NEW array (input untouched). Merges the block into the first system message, else inserts one at position 0."

The function reads the **last** user message only to *retrieve* the scaffold content (i.e. `scaffold_block` uses the last user turn as the query), but the resulting scaffold text is **injected into a `role: "system"` message** — either appended to an existing first system message or inserted as a brand-new system message at position 0. It is never merged into the user message.

**Cross-checked against the actual production route handler, `crates/loom-facade/src/routes/mod.rs::merge_scaffold`** (the function the real `/v1/chat/completions` path calls):
```rust
/// Merge the scaffold block into the messages array, byte-identically to
/// `loom_scaffold::scaffold_messages`: append to the first string-content system
/// message (trim-end + blank line), else insert a fresh system message at 0.
fn merge_scaffold(msgs: &mut Vec<Value>, block: &str) {
    let injection = format!("{SYSTEM_PREAMBLE}\n\n{block}");
    ...
    Some((i, existing)) => { msgs[i]["content"] = Value::String(format!("{existing}\n\n{injection}")); }
    None => { msgs.insert(0, json!({ "role": "system", "content": injection })); }
}
```
This is the live-serving code path `live_harness.py` actually exercises. It confirms: **the live node also injects as a system message**, identically in kind to what `control_harness.py` does, and identically to the vendored Python reference engine `ontology_scaffold_v1.py` (`out.insert(0, {"role": "system", "content": injection})`).

**The "authority" instruction text** — `SYSTEM_PREAMBLE` in `crates/loom-scaffold/src/tuning.rs` (and byte-identically in `ontology_scaffold_v1.py`):
> "The following ontology context was retrieved from a curated knowledge graph. Where it is relevant to the user's request, treat it as ground truth for definitions and relationships between the concepts it covers. Where it is not relevant, ignore it and answer normally."

This preamble is prepended to the block **only** by `scaffold_messages`/`merge_scaffold` (the live serving path and the offline decomposition engine) — the standalone `/loom/scaffold` retrieval endpoint (`crates/loom-facade/src/routes/mod.rs::scaffold`, which is what `control_harness.py`'s `get_scaffold()` calls) returns only `"scaffold": s.block`, the bare block, with no preamble. So `control_harness.py`'s `true` arm supplies the block *without* the authority instruction that the live path always adds.

**What this means for the manuscript:** `docs/research/paper-v6/main.tex` line 383 states "the live node injects the block into the last user message, whereas the control arms supply the same bytes as a system message, so identical block content does not by itself isolate every serving-path effect." **This is incorrect per the released code.** The live node injects into a **system** message (append-to-existing-or-insert-at-0), exactly the same placement kind the control arms use — the two paths agree on placement. The real, verified difference is narrower and different in kind: the control harness's system message omits the `SYSTEM_PREAMBLE` authority instruction that the live path always prepends, so the `true` control arm is missing one sentence of instruction text the live node actually serves. The sentence in the manuscript needs correcting to state the *actual* discrepancy (missing preamble in the control's system message), not a placement discrepancy (system vs user) that the code does not support.

---

## 5. Placebo exposure (irrelevant / fluent-noise / true arms) — ABSENT / not well-defined for this cohort

**What was asked:** measured gold-target exposure (same lexical matcher as `decompose_exposure.py`'s `gold_hit`/`exposed_flags`) for the irrelevant, fluent-noise, and true arms' injected blocks.

**Findings, and why this cannot be computed as specified:**

1. **The injected block text is not stored in `control-results.jsonl`.** Its fields are `set, id, arm, n_seeds, attempt, content, latency_s, completion_tokens, model` — `content` is the **model's answer**, not the system-message block that was sent. `control_harness.py` builds `variants` (the per-arm block text) in memory at runtime and never persists them to disk. So even for the `true`/`shuffled`/`masked` arms, the exact historical block bytes are not directly in the released row data.
2. **A partial proxy exists for the `true` arm only**: `uplift-results/arcane/harness-scaffolds.json` (24 rows) and `uplift-results/thin-prose/harness-scaffolds.json` (33 rows) store `{"scaffold": ..., "engaged": ..., "approx_tokens": ...}` keyed by the same question IDs as `uplift-results/general/arcane-questions.json` / `thin-questions.json` (I verified the ID sets match exactly, 24/24 and 33/33). These are from a different, later harness run (not `control_harness.py` itself) but are deterministic retrieval given the same question text and corpus, so they are a reasonable proxy for the `true` arm's block. **I did not compute exposure from these** in this pass because of point 4 below.
3. **No stored data exists at all for the `irrelevant`-arm donor block** (the cross-domain scaffold substituted in): `control_harness.py`'s donor-selection is a live, greedy, seed-IRI-disjoint search over scaffolds fetched from the production `/loom/scaffold` endpoint at run time (`other[(n, q["id"])] = blocks[donor]`), and neither the donor's identity nor its block text is written to `control-results.jsonl`. Reconstructing it requires the live scaffold `seeds` field (IRIs) for every arcane/thin question at the time of the original run, which is not stored anywhere in this checkout.
4. **No `fluent-noise` arm exists at all** in the released cohort (see item 3) — nothing to measure.
5. **The matcher itself doesn't transfer to this cohort's gold format.** `decompose_exposure.py`'s `gold_hit` matches against **`{slug, title}` gold items** (the frozen 510-question set's structured gold, `uplift-results/questions.jsonl`). The arcane/thin control cohort's gold (`uplift-results/general/arcane-gold.json`, `thin-gold.json`) is a **free-text reference answer per question** (`{"id": ..., "answer": "<paragraph>", "stratum": ..., ...}`), with no per-item title list. "Fraction of gold items whose title appears in the placebo block" is not a well-defined quantity for this cohort under the paper's own matcher — there are no discrete gold *items* to test membership of, only a prose answer that would need a different (and unreleased) itemisation before the matcher could apply.

**Conclusion: ABSENT.** Only a proxy `true`-arm block is available (from a different run, not the historical rows, and via question ID matching rather than a persisted mapping in `control-results.jsonl` itself); the `irrelevant`-arm donor pairing and block text, and the entire `fluent-noise` arm, are not in the checkout; and the paper's gold-item matcher does not apply to this cohort's free-text gold without further, unreleased itemisation. **What would be needed:** (i) persist the actual injected system-message text per row in any control rerun (trivial harness change: add a `"system": sys_block` field to each written row); (ii) itemise the arcane/thin gold answers into `{slug/title}` gold-item lists matching the frozen-510 format, or define and disclose a separate matcher for prose gold.

---

## 6. Truncation (174 GLM rows) and the repeated-observation structure

**GLM-4.6 174-truncated / 5-retried figure.**

**Artefact + command** (`bench/sweep/analyse.py`'s own definition, re-run against the released sweep rows):
```python
scaf = {r['id']: r for r in map(json.loads, open('uplift-results/sweep/results-glm-4.6-scaffold.jsonl'))}
raw  = {r['id']: r for r in map(json.loads, open('uplift-results/sweep/results-glm-4.6-raw.jsonl'))}
ids = set(scaf) | set(raw)
trunc = [i for i in ids if scaf.get(i,{}).get('finish_reason')=='length'
                        or raw.get(i,{}).get('finish_reason')=='length']
retried = sum(1 for i in ids if isinstance(scaf.get(i,{}).get('attempts'), int) and scaf[i]['attempts']>1)
```
**Result: `len(trunc) == 174`, `retried == 5`** — exact reproduction of `uplift-results/sweep/sweep-analysis.json`'s `glm-4.6` entry (`n_truncated: 174, n_retried: 5`) and of the corresponding row in `sweep-analysis.md`. Breakdown: scaffold-arm alone has 5 rows with `finish_reason=="length"`; raw-arm alone has 172 (`finish_reason=="length"`) + 1 `error`; the union (a question counts once even if truncated in both arms) is 174 — confirming `bench/sweep/analyse.py`'s definition (`trunc = sum(1 for i in ids if sc[i]["finish_reason"]=="length" or raw[i]["finish_reason"]=="length")`) is what the paper's "174 truncated GLM rows" and "Gemini 2.5 Flash-Lite truncated on 16" (also confirmed: `sweep-analysis.json`'s `gemini-2.5-flash-lite` entry has `n_truncated: 16`) refer to.

**Repeated-observation structure.**

**Command:**
```python
qs = [json.loads(l) for l in open('uplift-results/questions.jsonl')]
n_gold_total = sum(len(q.get('gold') or []) for q in qs)  # -> 1136
```
**Result:** 510 questions, 1,136 total gold items, ×10 models = **11,360**, matching `decomposition.json`'s `pooled_2x2.n_gold_items` exactly. Confirmed.

**What this means for the manuscript:** both figures are already correct as published; no change needed.

---

## 7. Study-2 headline: n=114, +0.27 pooled, exact p=0.002, 58/60 general-set ties

**Artefact:** `uplift-results/paper-v2/judged.json` (raw per-question, per-arm judge scores) + `uplift-results/paper-v2/analysis.json` (the paper's own analysis output) + `tools/paper/analyze.py` (`exact_signed_rank`, `bootstrap_ci` — imported directly, not re-implemented, to guarantee I was using the paper's own test).

**Command:** recomputed pairing, wins/losses/ties, mean difference, exact conditional signed-rank p, and 95% percentile bootstrap CI from scratch, per scope, directly against `judged.json`:
```python
import sys; sys.path.insert(0, 'tools/paper')
from analyze import exact_signed_rank, bootstrap_ci
scores = json.load(open('uplift-results/paper-v2/judged.json'))
by = defaultdict(dict)
for s in scores: by[(s['set'], s['id'])][s['arm']] = s['score']
# per scope: pairs where both 'loom' and 'raw' arms are present
```

**Results (independently reproduced, not copied from `analysis.json`):**

| scope | n | mean diff | wins/losses/ties | exact p | 95% CI |
|---|---:|---:|---:|---:|---|
| arcane | 24 | +0.7917 | 10/3/11 | 0.032227 | [+0.167, +1.417] |
| thin | 30 | +0.3000 | 12/5/13 | 0.130493 | [−0.033, +0.633] |
| general | 60 | +0.0500 | 2/0/58 | 0.500000 | [0.000, +0.133] |
| **pooled** | **114** | **+0.2719** | **24/8/82** | **0.002304** | **[+0.105, +0.447]** |

All figures match both `analysis.json`'s `paired` block and the manuscript's Table `tab:live` (`docs/research/paper-v6/main.tex` line 332/353: "n=114 ... +0.27 ... exact p=0.0023 ... rank-biserial r=+0.589 ... 24 wins, 8 losses, 82 ties") and `judged.json` directly, computed via three independent routes (my own pairing/exact-test call, `analysis.json`'s stored `paired.pooled`, and the manuscript prose) that all agree.

- **n=114 pairs pooled: confirmed.**
- **+0.27 pooled mean diff: confirmed** (+0.2719, rounds to +0.27).
- **exact p=0.002: confirmed** (0.002304, rounds to 0.002; paper states 0.0023, consistent).
- **58/60 general-set ties: confirmed exactly** (general scope: n=60, ties=58, wins=2, losses=0).

**What this means for the manuscript:** no change needed. This is the one item of the seven where the reviewer's own stated figures and the manuscript's figures already agree; my independent recomputation (using the paper's own `exact_signed_rank`/`bootstrap_ci` against the raw judged scores, not the paper's cached summary) confirms both are correct.

---

## Summary for the writer

| # | Verdict | Action needed |
|---|---|---|
| 1 | Confirmed | None — cite the table above if a per-model breakdown is wanted in the text. |
| 2 | Confirmed, refined | Optional: add a footnote giving the 1-in-15.5 adjusted rate alongside the published 1-in-14, noting 52/735 (7%) of nominal omissions are compliant answers via an alternative. |
| 3 | **ABSENT — critical** | Locate/release the uniform-budget rerun artefacts (5-arm rows, gemini-3.1-pro-preview judged scores, generating scripts) or rewrite §sec:controls to drop every ITT/fluent-noise number and the abstract's "+0.75, Holm-significant" claim, keeping only the verifiable 1536-token historical contrasts with the attrition caveat. |
| 4 | **Manuscript is wrong — needs correction** | Line 383 (and paper-v8 equivalent): replace "the live node injects the block into the last user message, whereas the control arms supply the same bytes as a system message" with the verified fact — both paths inject as a system message; the real discrepancy is that the control harness's system message omits the `SYSTEM_PREAMBLE` authority instruction the live path always prepends. |
| 5 | **ABSENT** | Not computable from released artefacts for irrelevant/fluent-noise arms (block text unpersisted, donor pairing unpersisted, fluent-noise arm doesn't exist); true-arm block is available only as a proxy from a different run. Needs a harness change (persist injected system text per row) and, if free-text gold cohorts are to be exposure-scored, a released itemisation of arcane/thin gold. |
| 6 | Confirmed | None. |
| 7 | Confirmed | None — this is the one item where reviewer and manuscript already agree; independent recomputation from raw judged scores supports both. |
