# Semantic audit of the lexical title matcher

> **The adjudicator in this audit is a large language model, not a human.**  
> Every verdict below was produced by `openai/gpt-4.1` at temperature 0 via the `openrouter` backend on 2026-09-21. No human has annotated any unit. `human-audit-sample.csv` and `ANNOTATION-GUIDE.md` in this directory prepare that pass; until it is done and re-summarised, nothing here is human-validated.

A focused, stratified audit of when the paper's lexical title matcher agrees with semantic answer correctness, and how it fails. THE ADJUDICATOR IS A LARGE LANGUAGE MODEL, NOT A HUMAN. No human has annotated any unit in this release; human-audit-sample.csv is prepared for that pass and is blind to both the matcher category and the model judge's verdict.

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

## 3. Headline rates (95% Wilson intervals)

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

## 4. Model-judged corrected estimate

Matcher credit rate (context utilisation) is n11/(n11+n10) = 9865/10600 = 0.9307. The model judge's corrected relational-correctness rate reweights each matcher stratum by the judged probability that the answer actually asserts the gold edge: (n11 * P(correct_relation | n11) + n10 * P(correct_relation | n10)) / (n11 + n10). Both strata are sampled, so this is an estimate; the interval propagates the two Wilson intervals through the same weights. The adjudicator is a model, not a human.

| quantity | as judged | quote-verified |
|---|---|---|
| matcher credit rate (context utilisation), released sweep | 0.9307 | 0.9307 |
| P(correct_relation given credited), judged | 0.9708 | 0.9708 |
| P(correct_relation given omitted), judged | 0.0250 | 0.0250 |
| **corrected relational-correctness rate of exposed items** | **0.9052** [0.8763, 0.9224] | **0.9052** [0.8763, 0.9224] |
| overstatement by the lexical matcher | 2.54 pp | 2.54 pp |

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
| gold title runs words together, so no spaced answer can match it | 58 | 12 | 0 | 0 |
| gold title is a single word | 1862 | 128 | 0 | 80 |
| *denominator* | 9,865 | 735 | 3 | 757 |

Gold titles that run words together (a defect in the frozen question set, not in any model — each is unmatchable for every model at once): `DeFi`, `EdDSA`, `GraphQL`, `MultiAccessEdgeComputing`, `OpenXR`, `SustainabilityReporting`, `WebRTC`

**Cross-check against the paper's existing optional-answer sensitivity analysis.** The paper's existing sensitivity analysis: an n10 item on an `any`-type question whose answer nonetheless satisfies the question via a different accepted alternative, so the any-collapse scorer already counts the question as fully correct. Counted here over the whole frame, to be compared with the judge's sampled `n10_compliant_omission_rate`. Over the whole frame that rule flags 52/735 = 0.0707 of omissions as compliant (166 n10 items come from `any`-type questions at all). The judge, on the stratified sample, puts it at 0.042 [0.018, 0.094] (5/120) as judged and 0.017 [0.005, 0.059] (2/120) quote-verified — that is, the judge is *stricter* than the mechanical any-collapse rule, because it requires the substituted item to be genuinely asserted rather than merely matched. The deterministic figure should therefore be read as the upper end of the compliant-omission correction.

How the judge scored the sampled units carrying each mechanism:

| mechanism | units in sample | judge verdicts | n11 `correct_relation` |
|---|---:|---|---|
| gold title is a substring of the question's own subject name | 7 | absent 1, correct_relation 4, name_only 2 | 0.571 [0.251, 0.842] (4/7) |
| credit came from the loose >=0.8 word-overlap path, not a substring hit | 3 | absent 2, name_only 1 | - |
| gold title runs words together, so no spaced answer can match it | 7 | correct_relation 7 | 1.000 [0.510, 1.000] (4/4) |
| gold title is a single word | 76 | absent 19, correct_relation 45, name_only 9, negated 3 | 0.955 [0.849, 0.987] (42/44) |

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

## Reproduce

```
python3 tools/paper/semantic_audit.py sample
python3 tools/paper/semantic_audit.py judge
python3 tools/paper/semantic_audit.py summarise
```

Judge model `openai/gpt-4.1` via `openrouter` at `https://openrouter.ai/api/v1` (key from `OPENROUTER_API_KEY`), temperature 0, rubric sha256[:16] `2895842241850808`, seed 42. 423/423 units returned a parseable verdict (0 failures, 2 repair re-asks). Total cost of the audit: **$0.9296** over 425 judge calls (396,229 prompt + 17,139 completion tokens). gpt-4.1 list price $2.00/M input, $8.00/M output. `cost_estimate_usd` covers the uncached calls of the last run only; `cost_estimate_usd_total` sums every call ever written to judge-cache.jsonl, which is what the audit actually cost to produce. Every call is cached in `judge-cache.jsonl` keyed by sha256(judge_model || rubric_hash || prompt), so re-running `judge` is free and returns identical verdicts.

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
