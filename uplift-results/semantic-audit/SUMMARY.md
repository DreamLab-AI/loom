# Semantic audit of the lexical title matcher

> **The adjudicator in this audit is a large language model, not a human.**  
> Every verdict below was produced by `openai/gpt-4.1` at temperature 0 via the `openrouter` backend on 2026-09-21. No human has annotated any unit. `human-audit-sample.csv` and `ANNOTATION-GUIDE.md` in this directory prepare that pass; until it is done and re-summarised, nothing here is human-validated.

A focused, stratified audit of when the paper's lexical title matcher agrees with semantic answer correctness, and how it fails. Protocol v1 (sections 3, 4, 9) is the earlier release, unchanged. Protocol v2 (sections 3A, 3B, 4A) adds the symmetric quote gate, the `unresolved` category, three-way sensitivity bounds and a stratified cluster bootstrap; section 12 audits the bare-arm recoveries behind the suppression result. THE ADJUDICATOR IS A LARGE LANGUAGE MODEL, NOT A HUMAN. No human has annotated any unit in this release; human-audit-sample.csv is prepared for that pass and is blind to both the matcher category and the model judge's verdict.

**Judge-family caveat.** The judge is GPT-4.1, cross-family to the local Qwen model the Loom serves and to eight of the ten sweep models. It is the SAME family as gpt-4.1-mini, one of the ten, so that model's per-model row carries a self-preference risk the others do not; the per-model breakdown is reported so a reader can check it.

## 1. Sampling frame

Unit = (question, model, gold item, arm = scaffold). The frame is every such unit in the released ten-model sweep: **11,360** units (510 questions x 10 models x their gold items).

| matcher category | meaning | frame | sampled |
|---|---|---:|---:|
| n11 | exposed & credited | 9,865 | 240 |
| n10 | exposed & omitted | 735 | 120 |
| n01 | unexposed & credited | 3 | 3 |
| n00 | unexposed & not credited | 757 | 60 |
| **total** | | **11,360** | **423** |

Exposure is model-independent (regenerated from the frozen question set and app/data/scaffold-index.json), so all ten models share the same 1060 exposed and 76 unexposed gold items; the 760 n00 units are 76 distinct items repeated across ten models and are therefore not independent.

Gate: the regenerated 2x2 matches `uplift-results/paper-v2/decomposition.json` exactly ({'n11': 9865, 'n10': 735, 'n01': 3, 'n00': 757}).

Sampling is stratified: within each matcher category the quota is allocated over (template x gold_type) cells proportionally to cell size (largest remainder), then over models within each cell proportionally to their counts, then simple random sampling without replacement inside each bucket. Seed 42. All 3 n01 units are a census, not a sample.

Sampled units by (template / gold_type):

| category | T-COMMON / any | T-REL / all | T-TAX / all |
|---|---|---|---|
| n11 | 12 | 180 | 48 |
| n10 | 27 | 83 | 10 |
| n01 | 0 | 3 | 0 |
| n00 | 0 | 60 | 0 |

Sampled units by model: claude-haiku-4.5 41, deepseek-chat 42, gemini-2.5-flash-lite 47, gemini-3.5-flash-lite 39, gemini-3.7-flash-t0 38, glm-4.6 41, gpt-4.1-mini 45, llama-3.3-70b 41, mistral-small-24b 45, qwen-2.5-72b 44

## 2. Confusion: matcher category vs model-judge verdict

| matcher category | correct_relation | name_only | wrong_relation | reversed | negated | absent | n |
|---|---|---|---|---|---|---|---|
| n11 (exposed & credited) | 233 | 4 | 1 | 0 | 0 | 2 | 240 |
| n10 (exposed & omitted) | 3 | 24 | 2 | 0 | 5 | 86 | 120 |
| n01 (unexposed & credited) | 0 | 1 | 0 | 0 | 0 | 2 | 3 |
| n00 (unexposed & not credited) | 0 | 6 | 0 | 0 | 2 | 52 | 60 |

`required_or_alternative`, by category:

| matcher category | required | alternative | n/a |
|---|---|---|---|
| n11 | 236 | 4 | 0 |
| n10 | 93 | 5 | 22 |
| n01 | 3 | 0 | 0 |
| n00 | 60 | 0 | 0 |

## 3. Headline rates, protocol v1 (95% Wilson intervals)

Two readings of every rate. *As judged* takes the judge at its word. *Quote-verified* demotes to `absent` those verdicts in the **uncredited** strata (n10, n00) whose cited span cannot be found in the answer: there the quote is the judge's entire evidence for contradicting the matcher, so an absent span means an unsupported claim. Verdicts in the credited strata are left alone, because the matcher has independently established that the title occurs — see section 9. Where the two columns differ, the quote-verified one is the defensible figure.

| quantity | as judged | quote-verified |
|---|---|---|
| **Precision of the matcher's credit**: of credited (n11) items, share the judge calls `correct_relation` | 0.971 [0.941, 0.986] (233/240) | 0.971 [0.941, 0.986] (233/240) |
| of credited items, share that are `name_only` | 0.017 [0.006, 0.042] (4/240) | 0.017 [0.006, 0.042] (4/240) |
| of credited items, share `wrong_relation` / `reversed` / `negated` | 0.004 [0.001, 0.023] (1/240) | 0.004 [0.001, 0.023] (1/240) |
| of credited items, share the judge cannot find at all | 0.008 [0.002, 0.030] (2/240) | 0.008 [0.002, 0.030] (2/240) |
| **Compliant omissions**: of omitted (n10) items, share where the answer supplied an accepted alternative | 0.042 [0.018, 0.094] (5/120) | 0.017 [0.005, 0.059] (2/120) |
| of omitted items, share the judge says the answer *does* assert the edge (outright matcher false negative) | 0.025 [0.009, 0.071] (3/120) | 0.025 [0.009, 0.071] (3/120) |
| **Matcher false negatives**: of omitted items, share where the judge finds the item named at all (`correct_relation` or `name_only`) | 0.225 [0.160, 0.308] (27/120) | 0.108 [0.064, 0.177] (13/120) |
| of omitted items, share the answer explicitly *denies* the edge | 0.042 [0.018, 0.094] (5/120) | 0.042 [0.018, 0.094] (5/120) |
| of omitted items, share that are true omissions (absent and not substituted) | 0.708 [0.622, 0.782] (85/120) | 0.825 [0.747, 0.883] (99/120) |
| **n00 sanity**: of unexposed & uncredited items, share the judge calls `absent` | 0.867 [0.758, 0.931] (52/60) | 0.933 [0.841, 0.974] (56/60) |
| of unexposed & uncredited items, share the judge calls `correct_relation` | 0.000 [0.000, 0.060] (0/60) | 0.000 [0.000, 0.060] (0/60) |
| of unexposed & uncredited items, share where the judge finds the item referred to at all | 0.133 [0.069, 0.242] (8/60) | 0.067 [0.026, 0.159] (4/60) |

n01 (unexposed & credited; census of 3 units): {'absent': 2, 'name_only': 1}

## 3A. Symmetric quote gate (protocol v2)

One policy for every failed supporting quote in every stratum: re-ask the judge for a verbatim supporting span with the answer supplied again; if the re-asked span verifies, keep the verdict with the repaired evidence; if it still fails, the unit is `unresolved` - neither the original verdict nor `absent`. Acceptance is verbatim (whitespace-normalised) OR matcher-normalised containment, so the gate is never stricter than protocol v1.

- failed citations adjudicated: **29**, in every stratum, under one policy
- repaired (the re-asked span verifies): **11** (6 character-for-character, 5 only under the matcher's own normalisation)
- unresolved (the judge conceded no supporting span exists, or supplied one that still does not verify): **18**
- of the unresolved, the judge explicitly conceded on 18
- by matcher category: {'n00': {'unresolved': 4}, 'n10': {'unresolved': 14}, 'n11': {'repaired': 11}}
- per-unit adjudication log, with the original quote, the re-ask prompt and response, the repaired quote, both verification results and the decision: `quote-gate-v2.jsonl`

**Finding.** Protocol v1 kept the 11 failures in credited strata and demoted the 18 in uncredited strata, and the reviewer rightly called that asymmetric. Applying ONE policy to all 29 reproduces the same split from evidence rather than from assumption: every one of the 11 credited-stratum failures was repaired by a span that verifies, and on every one of the 18 uncredited-stratum failures the judge conceded that no supporting span exists. The v1 asymmetry was the right call; it is now earned.

Acceptance is deliberately no stricter than protocol v1 (a repaired span counts if it verifies character-for-character under whitespace normalisation **or** under the matcher's own `normalise`), so v2 can never demote a unit v1 would have kept. `unresolved` is a third outcome: neither the original verdict nor `absent`.

### 3B. Every headline rate, both protocols side by side

The two v1 columns are section 3 unchanged. The three v2 columns are the sensitivity bounds: *against the matcher* and *for the matcher* are the worse and the better of the two readings of an unresolved unit (verdict stands / verdict withdrawn) **for that particular statistic**, and *excluded* drops the unresolved units from numerator and denominator. Where the three v2 columns agree, no unresolved unit can touch the rate.

| quantity | v1 as judged | v1 quote-verified | v2 against matcher | v2 for matcher | v2 excluded |
|---|---|---|---|---|---|
| **Precision of the matcher's credit**: of credited (n11) items, share the judge calls `correct_relation` | 0.971 [0.941, 0.986] (233/240) | 0.971 [0.941, 0.986] (233/240) | 0.971 [0.941, 0.986] (233/240) | 0.971 [0.941, 0.986] (233/240) | 0.971 [0.941, 0.986] (233/240) |
| of credited items, share that are `name_only` | 0.017 [0.006, 0.042] (4/240) | 0.017 [0.006, 0.042] (4/240) | 0.017 [0.006, 0.042] (4/240) | 0.017 [0.006, 0.042] (4/240) | 0.017 [0.006, 0.042] (4/240) |
| of credited items, share `wrong_relation` / `reversed` / `negated` | 0.004 [0.001, 0.023] (1/240) | 0.004 [0.001, 0.023] (1/240) | 0.004 [0.001, 0.023] (1/240) | 0.004 [0.001, 0.023] (1/240) | 0.004 [0.001, 0.023] (1/240) |
| of credited items, share the judge cannot find at all | 0.008 [0.002, 0.030] (2/240) | 0.008 [0.002, 0.030] (2/240) | 0.008 [0.002, 0.030] (2/240) | 0.008 [0.002, 0.030] (2/240) | 0.008 [0.002, 0.030] (2/240) |
| **Compliant omissions**: of omitted (n10) items, share where the answer supplied an accepted alternative | 0.042 [0.018, 0.094] (5/120) | 0.017 [0.005, 0.059] (2/120) | 0.042 [0.018, 0.094] (5/120) | 0.017 [0.005, 0.059] (2/120) | 0.019 [0.005, 0.066] (2/106) |
| of omitted items, share the judge says the answer *does* assert the edge (outright matcher false negative) | 0.025 [0.009, 0.071] (3/120) | 0.025 [0.009, 0.071] (3/120) | 0.025 [0.009, 0.071] (3/120) | 0.025 [0.009, 0.071] (3/120) | 0.028 [0.010, 0.080] (3/106) |
| **Matcher false negatives**: of omitted items, share where the judge finds the item named at all (`correct_relation` or `name_only`) | 0.225 [0.160, 0.308] (27/120) | 0.108 [0.064, 0.177] (13/120) | 0.225 [0.160, 0.308] (27/120) | 0.108 [0.064, 0.177] (13/120) | 0.123 [0.073, 0.199] (13/106) |
| of omitted items, share the answer explicitly *denies* the edge | 0.042 [0.018, 0.094] (5/120) | 0.042 [0.018, 0.094] (5/120) | 0.042 [0.018, 0.094] (5/120) | 0.042 [0.018, 0.094] (5/120) | 0.047 [0.020, 0.106] (5/106) |
| of omitted items, share that are true omissions (absent and not substituted) | 0.708 [0.622, 0.782] (85/120) | 0.825 [0.747, 0.883] (99/120) | 0.708 [0.622, 0.782] (85/120) | 0.825 [0.747, 0.883] (99/120) | 0.802 [0.716, 0.867] (85/106) |
| **n00 sanity**: of unexposed & uncredited items, share the judge calls `absent` | 0.867 [0.758, 0.931] (52/60) | 0.933 [0.841, 0.974] (56/60) | 0.867 [0.758, 0.931] (52/60) | 0.933 [0.841, 0.974] (56/60) | 0.929 [0.830, 0.972] (52/56) |
| of unexposed & uncredited items, share the judge calls `correct_relation` | 0.000 [0.000, 0.060] (0/60) | 0.000 [0.000, 0.060] (0/60) | 0.000 [0.000, 0.060] (0/60) | 0.000 [0.000, 0.060] (0/60) | 0.000 [0.000, 0.064] (0/56) |
| of unexposed & uncredited items, share where the judge finds the item referred to at all | 0.133 [0.069, 0.242] (8/60) | 0.067 [0.026, 0.159] (4/60) | 0.133 [0.069, 0.242] (8/60) | 0.067 [0.026, 0.159] (4/60) | 0.071 [0.028, 0.170] (4/56) |

## 4. Model-judged corrected estimate (protocol v1)

Superseded by section 4A, which handles the design's uncertainty properly; kept here unchanged so the two protocols can be compared.

Matcher credit rate (context utilisation) is n11/(n11+n10) = 9865/10600 = 0.9307. The model judge's corrected relational-correctness rate reweights each matcher stratum by the judged probability that the answer actually asserts the gold edge: (n11 * P(correct_relation | n11) + n10 * P(correct_relation | n10)) / (n11 + n10). Both strata are sampled, so this is an estimate; the interval propagates the two Wilson intervals through the same weights. The adjudicator is a model, not a human.

| quantity | as judged | quote-verified |
|---|---|---|
| matcher credit rate (context utilisation), released sweep | 0.9307 | 0.9307 |
| P(correct_relation given credited), judged | 0.9708 | 0.9708 |
| P(correct_relation given omitted), judged | 0.0250 | 0.0250 |
| **corrected relational-correctness rate of exposed items** | **0.9052** [0.8763, 0.9224] | **0.9052** [0.8763, 0.9224] |
| overstatement by the lexical matcher | 2.54 pp | 2.54 pp |

## 4A. Corrected exposed-item rate, with the design's uncertainty

Estimator. The frame holds N11 = 9,865 exposed-and-credited and N10 = 735 exposed-and-omitted gold-item slots, N11 + N10 = 10,600 exposed slots. Each stratum is sampled at its own rate (240 of 9,865 and 120 of 735), so the corrected relational-correctness rate of exposed items is the frame-weighted combination of the two judged stratum rates:

    R = (9865 * p11 + 735 * p10) / 10600

where p11 = P(the answer asserts the gold edge | the matcher credited it) and p10 = P(the answer asserts the gold edge | the matcher scored it omitted), both estimated on the stratified sample. Interval: a stratified cluster bootstrap. Within each stratum the QUESTION is resampled with replacement (not the unit), because one question contributes several gold items and a gold item's target is fixed by its question; both stratum rates are recomputed on each draw and recombined through the same weights, so the interval carries the sampling weights, the uncertainty in BOTH rates, and the within-question clustering. 10,000 draws, seed 42, percentile method. A model-clustered bootstrap is reported beside it as a robustness bound. R is an ESTIMATE under this adjudicator (openai/gpt-4.1, temperature 0) and this protocol, on this sample. It is not a measurement of truth, it is not human-validated, and a different judge or rubric would move it.

| reading of `unresolved` | p11 | p10 | corrected rate | naive propagated Wilson | question-cluster bootstrap 95% | model-cluster bootstrap 95% |
|---|---|---|---|---|---|---|
| unresolved_verdict_stands | 0.9708 (233/240) | 0.0250 (3/120) | **0.9052** | [0.8763, 0.9224] | [0.8840, 0.9231] | [0.8856, 0.9235] |
| unresolved_withdrawn | 0.9708 (233/240) | 0.0250 (3/120) | **0.9052** | [0.8763, 0.9224] | [0.8840, 0.9231] | [0.8856, 0.9235] |
| unresolved_excluded | 0.9708 (233/240) | 0.0283 (3/106) | **0.9054** | [0.8764, 0.9230] | [0.8844, 0.9232] | [0.8858, 0.9236] |

The question-cluster bootstrap resamples 184 distinct questions in the n11 stratum and 59 in the n10 stratum, 10,000 draws, seed 42. The model-clustered column resamples the ten models instead, as a robustness bound on the other crossed factor; it is 0.0379 wide against the question-clustered 0.0391, so clustering on either factor gives an interval of much the same width and neither dominates. The naive propagated-Wilson column is 0.0461 wide: it is not a valid interval for this design (it adds the two Wilson bounds rather than resampling the joint distribution, and it ignores clustering entirely), and it is reported only so the two can be compared.

Across the three readings of `unresolved` the corrected rate moves only between 0.9052 and 0.9054, so the headline correction does not depend on how the unresolved units are counted. Matcher credit rate (context utilisation) on the released sweep is 0.9307; the corrected rate is 0.9052, an overstatement of 2.54 pp by the lexical matcher. **This is an estimate under this adjudicator and this protocol, on this sample; it is not human-validated and a different judge or rubric would move it.**

## 5. Per template

| stratum | n | n11 `correct_relation` | n10 acceptable alternative |
|---|---:|---|---|
| T-COMMON | 39 | 1.000 [0.757, 1.000] (12/12) | 0.185 [0.082, 0.367] (5/27) |
| T-REL | 326 | 0.978 [0.944, 0.991] (176/180) | 0.000 [0.000, 0.044] (0/83) |
| T-TAX | 58 | 0.938 [0.832, 0.979] (45/48) | 0.000 [0.000, 0.278] (0/10) |

## 6. Per gold_type

| stratum | n | n11 `correct_relation` | n10 acceptable alternative |
|---|---:|---|---|
| all | 384 | 0.969 [0.938, 0.985] (221/228) | 0.000 [0.000, 0.040] (0/93) |
| any | 39 | 1.000 [0.757, 1.000] (12/12) | 0.185 [0.082, 0.367] (5/27) |

## 7. Per model

| stratum | n | n11 `correct_relation` | n10 acceptable alternative |
|---|---:|---|---|
| claude-haiku-4.5 | 41 | 0.917 [0.742, 0.977] (22/24) | 0.000 [0.000, 0.278] (0/10) |
| deepseek-chat | 42 | 0.958 [0.798, 0.993] (23/24) | 0.083 [0.015, 0.354] (1/12) |
| gemini-2.5-flash-lite | 47 | 0.958 [0.798, 0.993] (23/24) | 0.000 [0.000, 0.184] (0/17) |
| gemini-3.5-flash-lite | 39 | 1.000 [0.862, 1.000] (24/24) | 0.000 [0.000, 0.299] (0/9) |
| gemini-3.7-flash | 38 | 1.000 [0.867, 1.000] (25/25) | 0.000 [0.000, 0.354] (0/7) |
| glm-4.6 | 41 | 1.000 [0.867, 1.000] (25/25) | 0.000 [0.000, 0.278] (0/10) |
| gpt-4.1-mini | 45 | 1.000 [0.857, 1.000] (23/23) | 0.125 [0.035, 0.360] (2/16) |
| llama-3.3-70b | 41 | 1.000 [0.862, 1.000] (24/24) | 0.091 [0.016, 0.377] (1/11) |
| mistral-small-24b | 45 | 0.917 [0.742, 0.977] (22/24) | 0.071 [0.013, 0.315] (1/14) |
| qwen-2.5-72b | 44 | 0.957 [0.790, 0.992] (22/23) | 0.000 [0.000, 0.215] (0/14) |

## 8. Why the matcher errs: deterministic mechanism census

Deterministic, judge-free flags over all 11,360 frame units, by matcher category. These identify structural failure modes of the matcher exhaustively; the judged sample estimates how often each one actually changes the verdict.

| mechanism | n11 | n10 | n01 | n00 |
|---|---:|---:|---:|---:|
| gold title is a substring of the question's own subject name | 208 | 2 | 0 | 0 |
| credit came from the loose >=0.8 word-overlap path, not a substring hit | 28 | 0 | 3 | 0 |
| gold title runs words together, so `normalise` cannot match a spaced rendering of it (section 8A counts what this actually costs) | 58 | 12 | 0 | 0 |
| gold title is a single word | 1862 | 128 | 0 | 80 |
| *denominator* | 9,865 | 735 | 3 | 757 |

Gold titles that run words together: `DeFi`, `EdDSA`, `GraphQL`, `MultiAccessEdgeComputing`, `OpenXR`, `SustainabilityReporting`, `WebRTC`. This flag marks a title `normalise` cannot match in spaced form; it is NOT a count of lost credits, and section 8A resolves what it costs unit by unit.

**Cross-check against the paper's existing optional-answer sensitivity analysis.** The paper's existing sensitivity analysis: an n10 item on an `any`-type question whose answer nonetheless satisfies the question via a different accepted alternative, so the any-collapse scorer already counts the question as fully correct. Counted here over the whole frame, to be compared with the judge's sampled `n10_compliant_omission_rate`. Over the whole frame that rule flags 52/735 = 0.0707 of omissions as compliant (166 n10 items come from `any`-type questions at all). The judge, on the stratified sample, puts it at 0.042 [0.018, 0.094] (5/120) as judged and 0.017 [0.005, 0.059] (2/120) quote-verified — that is, the judge is *stricter* than the mechanical any-collapse rule, because it requires the substituted item to be genuinely asserted rather than merely matched. The deterministic figure should therefore be read as the upper end of the compliant-omission correction.

How the judge scored the sampled units carrying each mechanism:

| mechanism | units in sample | judge verdicts | n11 `correct_relation` |
|---|---:|---|---|
| gold title is a substring of the question's own subject name | 7 | absent 1, correct_relation 4, name_only 2 | 0.571 [0.251, 0.842] (4/7) |
| credit came from the loose >=0.8 word-overlap path, not a substring hit | 3 | absent 2, name_only 1 | - |
| gold title runs words together, so `normalise` cannot match a spaced rendering of it (section 8A counts what this actually costs) | 7 | correct_relation 7 | 1.000 [0.510, 1.000] (4/4) |
| gold title is a single word | 76 | absent 19, correct_relation 45, name_only 9, negated 3 | 0.955 [0.849, 0.987] (42/44) |

## 8A. The run-together (camelCase) gold titles, precisely

Earlier drafts said a run-together gold title means "no spaced answer can match it", which is true of the normalisation and misleading about the outcome. Frame-wide census of all 70 units carrying one of the 7 run-together gold titles:

| gold title | spaced form | frame units | credited | of which verbatim substring | of which word-bag only | omitted | omitted with spaced form in the answer | omitted, not mentioned at all |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| `DeFi` | De Fi | 10 | 10 | 10 | 0 | 0 | 0 | 0 |
| `EdDSA` | Ed DSA | 10 | 9 | 9 | 0 | 1 | 0 | 1 |
| `GraphQL` | Graph QL | 10 | 9 | 9 | 0 | 1 | 0 | 1 |
| `MultiAccessEdgeComputing` | Multi Access Edge Computing | 10 | 9 | 9 | 0 | 1 | 1 | 0 |
| `OpenXR` | Open XR | 10 | 10 | 10 | 0 | 0 | 0 | 0 |
| `SustainabilityReporting` | Sustainability Reporting | 10 | 1 | 1 | 0 | 9 | 9 | 0 |
| `WebRTC` | Web RTC | 10 | 10 | 10 | 0 | 0 | 0 | 0 |
| **total** | | **70** | **58** | **58** | **0** | **12** | **10** | **2** |

**Mechanism.** 58 of the 70 frame units carrying a run-together gold title ARE credited, and every one of them by a verbatim substring hit (58 of 58; 0 via the loose word-bag path): the model reproduced the run-together form the scaffold put in front of it, so `normalise`'s refusal to split camel case never bit. The mechanism bites only on the 12 omitted units, and there on the 10 where the answer does contain the SPACED form of the title and the matcher cannot see it; the remaining 2 do not mention the item under any spelling and are genuine omissions. The defect therefore costs 10 units of 11,360, not 70, and those come from 2 of the 7 run-together titles `MultiAccessEdgeComputing`, `SustainabilityReporting`. The remaining titles are conventional unspaced proper nouns (`DeFi`, `WebRTC`, `OpenXR`, `EdDSA`, `GraphQL` are written that way in the field), which is why models reproduce them exactly and the matcher never misses them.

Omissions in which the spaced form is present in the answer and the matcher cannot see it (10):

- `claude-haiku-4.5|q0110|1` (claude-haiku-4.5, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `deepseek-chat|q0110|1` (deepseek-chat, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `gemini-2.5-flash-lite|q0110|1` (gemini-2.5-flash-lite, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `gemini-3.5-flash-lite|q0110|1` (gemini-3.5-flash-lite, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `glm-4.6|q0110|1` (glm-4.6, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `gpt-4.1-mini|q0110|1` (gpt-4.1-mini, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `llama-3.3-70b|q0110|1` (llama-3.3-70b, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `mistral-small-24b|q0110|1` (mistral-small-24b, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `qwen-2.5-72b|q0110|1` (qwen-2.5-72b, q0110) — gold `SustainabilityReporting`, the answer says "Sustainability Reporting"
- `qwen-2.5-72b|q0478|1` (qwen-2.5-72b, q0478) — gold `MultiAccessEdgeComputing`, the answer says "Multi Access Edge Computing"

Omissions where the item is not mentioned under any spelling, so the matcher is right (2): `deepseek-chat|q0420|1` (EdDSA), `gemini-2.5-flash-lite|q0160|2` (GraphQL)

## 9. Judge reliability

Every non-`absent` verdict must cite a verbatim span of the answer. This check normalises both (lowercase, strip punctuation, collapse whitespace — the matcher's own `normalise`) and asks whether the cited span, or every fragment of an elided span, actually occurs in the answer. A failure means one of two things. In a CREDITED stratum (n11/n01) the matcher has already established that the title occurs, and inspection shows these are abridged list reconstructions — the judge quotes a list but drops the gloss between entries; the verdict stands and only the citation is loose (`abridged_citation`). In an UNCREDITED stratum (n10/n00) the matcher says the title does not occur, so the quote is the whole of the judge's evidence for the contrary claim; if that span is not in the answer the claim is unsupported (`unsupported_citation`) and the conservative reading demotes it to `absent`.

- verdicts required to cite a span: 281
- span verified verbatim in the answer: 0.897 [0.856, 0.927] (252/281)
- citations that do not verify: 29 ({'abridged_citation': 11, 'unsupported_citation': 18}), by matcher category {'n00': 4, 'n10': 14, 'n11': 11}
- of those, **18 unsupported** (uncredited stratum: the judge's only evidence is a span that is not in the answer) — demoted to `absent` in the quote-verified column of sections 3 and 4
- and 11 merely abridged (credited stratum: the matcher independently confirms the title is present) — verdict kept

Unsupported citations, in full:

  - `claude-haiku-4.5|q0387|0` (n00) claimed `name_only` citing "Odometry", which does not occur in the answer
  - `gemini-2.5-flash-lite|q0165|2` (n00) claimed `name_only` citing "Innovation", which does not occur in the answer
  - `gemini-3.7-flash-t0|q0431|3` (n00) claimed `negated` citing "Trust Management" is omitted from the list explicitly presented as what Revocation Registry enables", which does not occur in the answer
  - `qwen-2.5-72b|q0436|3` (n00) claimed `name_only` citing "Phishing Resistant Authentication", which does not occur in the answer
  - `deepseek-chat|q0252|1` (n10) claimed `name_only` citing "Platform and Environment", which does not occur in the answer
  - `gemini-2.5-flash-lite|q0321|1` (n10) claimed `name_only` citing "Mathematical Science", which does not occur in the answer
  - `gemini-2.5-flash-lite|q0508|1` (n10) claimed `name_only` citing "Infrastructure", which does not occur in the answer
  - `gemini-3.5-flash-lite|q0348|1` (n10) claimed `name_only` citing "AI Technique", which does not occur in the answer
  - `gpt-4.1-mini|q0097|2` (n10) claimed `name_only` citing "Distributed Data Structure", which does not occur in the answer
  - `gpt-4.1-mini|q0129|2` (n10) claimed `name_only` citing "Controlled Vocabulary", which does not occur in the answer
  - `gpt-4.1-mini|q0254|2` (n10) claimed `name_only` citing "", which does not occur in the answer
  - `gpt-4.1-mini|q0509|0` (n10) claimed `name_only` citing "Legal and Regulatory", which does not occur in the answer
  - `llama-3.3-70b|q0216|1` (n10) claimed `name_only` citing ""Infrastructure"", which does not occur in the answer
  - `mistral-small-24b|q0097|2` (n10) claimed `name_only` citing ""Distributed Data Structure"", which does not occur in the answer
  - `mistral-small-24b|q0253|1` (n10) claimed `name_only` citing "Platform and Environment", which does not occur in the answer
  - `mistral-small-24b|q0348|1` (n10) claimed `name_only` citing "AI Technique", which does not occur in the answer
  - `mistral-small-24b|q0509|0` (n10) claimed `name_only` citing "", which does not occur in the answer
  - `qwen-2.5-72b|q0215|0` (n10) claimed `name_only` citing "Infrastructure", which does not occur in the answer

## 10. Notable disagreements

Cases where the matcher and the model judge part company, highest judge confidence first (18 in total; up to 8 shown).

- **claude-haiku-4.5|q0016|4** (n11, T-REL, claude-haiku-4.5) — judge: `absent` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does Customer Experience Management enable?  
  Gold edge: `Customer Experience Management --enables--> Customer Experience`  
  Judge quote: ""
- **claude-haiku-4.5|q0110|1** (n10, T-REL, claude-haiku-4.5) — judge: `correct_relation` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does Climate Modelling enable?  
  Gold edge: `Climate Modelling --enables--> SustainabilityReporting`  
  Judge quote: "Climate Modelling enables the following:

1. **Carbon Footprint Measurement** ...
2. **Sustainability Reporting**"
- **claude-haiku-4.5|q0244|2** (n00, T-REL, claude-haiku-4.5) — judge: `name_only` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does Sustainable Finance enable?  
  Gold edge: `Sustainable Finance --enables--> Voluntary Carbon Market`  
  Judge quote: "These three capabilities represent how sustainable finance mechanisms channel capital toward environmentally and socially beneficial activities."
- **claude-haiku-4.5|q0387|0** (n00, T-REL, claude-haiku-4.5) — judge: `name_only` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does rb 0012 wheeled mobile robot use?  
  Gold edge: `rb 0012 wheeled mobile robot --uses--> Odometry`  
  Judge quote: "Odometry"
- **deepseek-chat|q0024|4** (n00, T-REL, deepseek-chat) — judge: `name_only` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does Mapping use?  
  Gold edge: `Mapping --uses--> Point Cloud`  
  Judge quote: "the graph lists **SLAM**, **Localisation**, and **Lidar** as related technologies it "uses" in its broader relations."
- **deepseek-chat|q0088|1** (n11, T-TAX, deepseek-chat) — judge: `wrong_relation` (confidence 1.00)  
  Q: In the DreamLab knowledge graph, what is the immediate parent concept of Financial Reporting? Also name up to three broader ancestor concepts.  
  Gold edge: `Financial Reporting --subClassOf--> Governance and Regulation`  
  Judge quote: "Governance and Regulation (ancestor, as Financial Reporting is part of Blockchain, which is part of Governance and Regulation)"
- **gemini-2.5-flash-lite|q0110|1** (n10, T-REL, gemini-2.5-flash-lite) — judge: `correct_relation` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does Climate Modelling enable?  
  Gold edge: `Climate Modelling --enables--> SustainabilityReporting`  
  Judge quote: "Sustainability Reporting"
- **gemini-2.5-flash-lite|q0125|3** (n11, T-REL, gemini-2.5-flash-lite) — judge: `absent` (confidence 1.00)  
  Q: According to the DreamLab knowledge graph, what does Linked Data use?  
  Gold edge: `Linked Data --uses--> OWL`  
  Judge quote: ""

## 11. Human pass

**Not performed.** `human-audit-sample.csv` (120 units, blind, blank verdict columns) and `ANNOTATION-GUIDE.md` are in this directory. After annotating, re-run `summarise --human human-audit-sample-filled.csv` to add agreement and kappa against the model judge.

## 12. The bare-arm lexical recoveries behind the suppression result

The 92 raw-arm lexical recoveries, judged, plus the relational version of the suppression comparison.

The paper's suppression result is lexical: on the 760 unexposed gold-item slots the BARE model lexically recovers 92 and the GROUNDED model 3. That compares name hits. This block asks the harder question of both arms - does the answer assert the correct relation, or does it only name the target? - and restates the comparison on relational recoveries.

Enumerated from the released sweep rows with the paper's own matcher and the regenerated exposure vector: **92 of 760** unexposed gold-item slots are lexically credited by the bare arm (count verified against the paper's 92/760: True). Every one was judged under the same rubric and the same symmetric quote gate; 3 of the 92 citations needed adjudication, outcomes {'repaired': 2, 'unresolved': 1}.

| reading of `unresolved` | n judged | relationally correct | name only | wrong / reversed / negated | absent |
|---|---:|---|---|---|---|
| unresolved_verdict_stands | 92 | 0.772 [0.676, 0.846] (71/92) | 0.163 [0.101, 0.252] (15/92) | 0.000 [0.000, 0.040] (0/92) | 0.065 [0.030, 0.135] (6/92) |
| unresolved_withdrawn | 92 | 0.772 [0.676, 0.846] (71/92) | 0.152 [0.093, 0.239] (14/92) | 0.000 [0.000, 0.040] (0/92) | 0.076 [0.037, 0.149] (7/92) |
| unresolved_excluded | 91 | 0.780 [0.685, 0.853] (71/91) | 0.154 [0.094, 0.242] (14/91) | 0.000 [0.000, 0.041] (0/91) | 0.066 [0.031, 0.137] (6/91) |

Sensitivity bounds on relational correctness: against the matcher 0.772 [0.676, 0.846] (71/92), for the matcher 0.780 [0.685, 0.853] (71/91), unresolved excluded 0.780 [0.685, 0.853] (71/91).

Per model:

| model | raw lexical recoveries | relationally correct | verdicts after the gate |
|---|---:|---:|---|
| claude-haiku-4.5 | 3 | 2 | {'correct_relation': 2, 'name_only': 1} |
| deepseek-chat | 13 | 9 | {'absent': 1, 'correct_relation': 9, 'name_only': 3} |
| gemini-2.5-flash-lite | 6 | 5 | {'absent': 1, 'correct_relation': 5} |
| gemini-3.5-flash-lite | 10 | 7 | {'absent': 1, 'correct_relation': 7, 'name_only': 2} |
| gemini-3.7-flash | 14 | 8 | {'absent': 1, 'correct_relation': 8, 'name_only': 5} |
| glm-4.6 | 12 | 8 | {'absent': 2, 'correct_relation': 8, 'name_only': 1, 'unresolved': 1} |
| gpt-4.1-mini | 1 | 1 | {'correct_relation': 1} |
| llama-3.3-70b | 9 | 8 | {'correct_relation': 8, 'name_only': 1} |
| mistral-small-24b | 10 | 10 | {'correct_relation': 10} |
| qwen-2.5-72b | 14 | 13 | {'correct_relation': 13, 'name_only': 1} |

**The grounded arm's side of the comparison.** The grounded arm's unexposed-yet-credited census is 3 units in the whole released sweep. Under the v2 gate 0 of them assert the gold relation, so precision on that census is zero: the three lexical credits are two items the judge cannot find at all and one named without the relation.

| unit | model | question | gold item | judge verdict | gate | after gate |
|---|---|---|---|---|---|---|
| `claude-haiku-4.5|q0055|3` | claude-haiku-4.5 | q0055 | Nearest Neighbor Search | absent | ok | absent |
| `mistral-small-24b|q0055|3` | mistral-small-24b | q0055 | Nearest Neighbor Search | absent | ok | absent |
| `qwen-2.5-72b|q0055|3` | qwen-2.5-72b | q0055 | Nearest Neighbor Search | name_only | ok | name_only |

### 12A. Suppression, restated relationally

Recoveries of gold items the deterministic scaffold does not expose, per 760 unexposed item-slots, bare arm vs grounded arm. The lexical row is the paper's published comparison; the relational row requires the answer to assert the gold edge, not merely to name the target.

| comparison | bare (raw) arm | grounded (scaffold) arm |
|---|---|---|
| lexical recovery of unexposed gold, per 760 slots (the paper's published row) | 0.121 [0.100, 0.146] (92/760) | 0.004 [0.001, 0.011] (3/760) |
| **relational** recovery (the answer asserts the gold edge) | 0.093 [0.075, 0.116] (71/760) | 0.000 [0.000, 0.005] (0/760) |

Of the 92 bare-arm lexical recoveries, 71 assert the correct relation, so the suppression comparison restated relationally is 71/760 bare versus 0/760 grounded. Relational adjudication costs the bare arm 21 of its 92 credits and the grounded arm all 3 of its 3; the direction and the near-total suppression survive, and the grounded side goes to exactly zero.

## Reproduce

```
python3 tools/paper/semantic_audit.py sample      # frame + stratified sample
python3 tools/paper/semantic_audit.py judge       # verdicts (protocol v1)
python3 tools/paper/semantic_audit.py quotegate   # symmetric quote gate (v2)
python3 tools/paper/semantic_audit.py rawhits     # the bare-arm recoveries
python3 tools/paper/semantic_audit.py summarise   # every table above
```

`quotegate` writes the per-unit adjudication log `quote-gate-v2.jsonl`; `rawhits` writes `raw-hits-sample.jsonl` and `raw-hits-audit.jsonl`. `summarise` folds both in when they are present and reports protocol v1 and protocol v2 side by side, so the earlier release's numbers stay visible and unchanged.

Judge model `openai/gpt-4.1` via `openrouter` at `https://openrouter.ai/api/v1` (key from `OPENROUTER_API_KEY`), temperature 0, rubric sha256[:16] `2895842241850808`, seed 42. 423/423 units returned a parseable verdict (0 failures, 2 repair re-asks). Total cost of the audit: **$1.2241** over 549 judge calls (522,142 prompt + 22,473 completion tokens). gpt-4.1 list price $2.00/M input, $8.00/M output. `cost_estimate_usd` covers the uncached calls of the last run only; `cost_estimate_usd_total` sums every call ever written to judge-cache.jsonl, which is what the audit actually cost to produce. Every call is cached in `judge-cache.jsonl` keyed by sha256(judge_model || rubric_hash || prompt), so re-running `judge` is free and returns identical verdicts.

Repair policy: A reply whose field 1 is not one of the six rubric labels, or which is not valid JSON, gets exactly one re-ask with an appended formatting correction (the rubric and the item are unchanged); malformed JSON whose fields are nonetheless unambiguous is salvaged by mechanical field extraction. Both paths are cached under their own key and flagged (`repair_pass`, `parse_repair`) on the verdict.

## Rubric given to the judge

```
You are auditing whether a model's free-text answer actually ASSERTS a specific fact from a knowledge graph, or merely mentions a name.

You are given: a QUESTION about the DreamLab knowledge graph; the GOLD EDGE, which is the single graph fact being audited (subject, relation, target); the GOLD ITEM, which is that edge's target; the FULL GOLD SET for the question and whether the question requires ALL of them or ANY ONE of them; and the model's ANSWER.

Decide what the ANSWER asserts about the GOLD ITEM, with respect to the relation the QUESTION asks for. Judge the answer's claims only; do not reward or punish style, length, hedging or extra correct material.

Field 1 - "asserted", exactly one of:
  "correct_relation" - the answer states that the GOLD ITEM stands in the relation the question asks for, with the correct direction and no negation. Paraphrase, synonym and inflection count: "relies on X", "X is a prerequisite" and "depends on X" are the same assertion. A bare list offered as the answer to the question counts as asserting the relation for every item in it.
  "name_only" - the GOLD ITEM's name (or an unambiguous paraphrase of it) appears in the answer, but the answer does not assert that it stands in the asked-for relation: e.g. it appears in a caveat, a restatement of the question, an aside, a different relation's list, an example of something else, or a list the answer explicitly declines to endorse.
  "wrong_relation" - the answer places the GOLD ITEM in a DIFFERENT relation to the subject than the one asked for (e.g. the question asks what the subject enables and the answer says the subject depends on it, or calls it a sibling/synonym/part).
  "reversed" - the answer asserts the asked-for relation but with subject and target swapped (e.g. asked "what does A enable?", the answer says the GOLD ITEM enables A).
  "negated" - the answer explicitly denies the relation (e.g. "A does not depend on the GOLD ITEM", "the GOLD ITEM is not among them").
  "absent" - the GOLD ITEM is not referred to at all, under any name.

Choose the first label that fits in this order of precedence: negated, reversed, wrong_relation, correct_relation, name_only, absent.

Field 2 - "required_or_alternative", exactly one of:
  "required" - a complete answer to the QUESTION must include the GOLD ITEM (the question requires ALL of the gold set, or the gold set has only this member).
  "alternative" - the question accepts ANY ONE of several gold items, and the ANSWER satisfies the question by supplying a DIFFERENT member of the gold set instead. Use this only when the answer really does supply an accepted alternative.
  "n/a" - the question accepts any one of several gold items, but the answer supplies none of them (so nothing was substituted).

Field 3 - "confidence", a number between 0 and 1: how sure you are of field 1.

Field 4 - "quote": the shortest verbatim span of the ANSWER your field-1 label rests on, copied exactly. Use the empty string "" if and only if field 1 is "absent".

Respond with ONLY a JSON object and nothing else:
{"asserted": "...", "required_or_alternative": "...", "confidence": 0.0, "quote": "..."}
```
