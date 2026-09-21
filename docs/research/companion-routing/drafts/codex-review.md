**Verdict: the strongest defensible claim is that a cheap paired control exposes benchmark construction effects, complementary errors and rubric-policy inconsistencies. This dataset does not establish superiority over the best thresholded BM25 baseline available in the supplied scores.**

The most consequential error is computational: **BM25 achieves 68/86 = 79.1% at threshold 9.65**, exceeding the claimed oracle optimum of 66/86. The judge’s gain therefore falls to **8/86 = 9.3 points; two-sided exact paired p = 0.1153**.

There is also a stronger descriptive finding buried in the provenance: **six of those eight net wins come from nine previously authored prompts**, where the judge scores 9/9 and BM25 3/9. That is worth foregrounding as a hypothesis about sensitivity to prompt construction—but those nine prompts cover only two closely related skills.

## 1. Statistics

### The paired difference is appropriate; the inference is fragile

Keep \(d_i=J_i-B_i\). It is the correct estimand for the paired accuracy difference. Binary outcomes do not invalidate a mean difference or automatically invalidate its normal interval.

However, with only 22 discordant pairs, I would report a **two-sided exact McNemar test**, equivalent to testing whether judge wins among discordant pairs follow \(\mathrm{Binomial}(22,0.5)\).

| Comparison | Judge-only / baseline-only | Gain | Exact two-sided p |
|---|---:|---:|---:|
| Original labels, reported BM25 rule | 15 / 7 | +9.30 points | 0.13380 |
| Corrected labels, reported BM25 rule | 16 / 6 | +11.63 points | **0.05248** |
| Corrected labels, genuinely maximised BM25 threshold | 14 / 6 | **+9.30 points** | **0.11532** |
| Corrected labels, reported embedding rule | 21 / 3 | +20.93 points | 0.000277 |

For the reported 16–6 comparison:

- Normal interval reproduces: **[+0.01163, +0.22092]**.
- Uncorrected asymptotic McNemar: \(p=0.03301\).
- Continuity-corrected McNemar: \(p=0.05501\).
- Exact McNemar: \(p=0.05248\).
- Ordinary paired percentile bootstrap: **[+0.01163, +0.22093]**.

The bootstrap interval and exact test need not agree: they are not inversions of the same procedure. Do not select whichever crosses the desired significance boundary.

For the corrected 68/86 BM25 baseline, the paired percentile bootstrap interval is **[−0.01163, +0.19767]**. This is the comparison I would report, explicitly as exploratory.

**“Exact” does not repair the sampling design.** These tests assume independent sampled pairs and fixed procedures. The corpus is purposive, related prompts are clustered, and baseline tuning and adjudication used the evaluation data. Thus these are conditional reference calculations, not calibrated evidence about production traffic.

The house-standard cluster bootstrap would require defensible skill-family or prompt-source clusters. Neither the four confusion classes nor the three provenance categories provide enough independent clusters. A turn bootstrap should be labelled as such; inventing clusters after seeing the outcomes would not restore confirmatory inference.

### The “POWER” line is not power analysis

“Significance would hold from about \(n=70\)” is an algebraic extrapolation holding the observed effect and variance fixed. It does not calculate a probability of detecting an effect, account for clustering, or establish a required sample size. Delete it.

### Subgroups: no lexical superiority claim survives Holm

Using the **run-file tags underlying the published table**, the reported BM25 rule gives:

| Population | Judge-only / BM25-only | Exact p | Holm p, seven rows |
|---|---:|---:|---:|
| All | 16 / 6 | 0.0525 | 0.3149 |
| Skill | 12 / 6 | 0.2379 | 0.7137 |
| `none` | 4 / 0 | 0.1250 | 0.6250 |
| Late | 6 / 0 | 0.0313 | 0.2188 |
| Early | 10 / 6 | 0.4545 | 0.9090 |
| Boundary | 4 / 0 | 0.1250 | 0.6250 |
| Near-neighbour | 7 / 5 | 0.7744 | 0.9090 |

If “all” is a separately specified primary comparison, correct the six secondary rows instead; the conclusion remains unchanged.

For cosine, the aggregate and `none` comparison remain significant even correcting across fourteen comparisons. The reported early-group margins also force a sufficiently small exact p-value to survive that correction. However, the supplied files lack cosine’s per-item predictions, preventing a complete independent paired audit.

**Multiplicity correction is unnecessary for a clearly descriptive table.** It becomes necessary when treating selected rows as discoveries. Also, claims that gains *differ between groups* require interaction tests; significance within one group and absence elsewhere is not evidence of heterogeneity.

### The late-discriminator analysis is not reproducible as specified

There are **21 tag disagreements** between `routing-cases.json` and `openjev-run-v2.json`.

| Tag source | Late n | Judge correct | BM25 correct | Gain |
|---|---:|---:|---:|---:|
| Run file / published table | 32 | 31 | 25 | +18.75 points |
| Labelled corpus | 41 | 37 | 29 | +19.51 points |

Under corpus tags, early gain is **+4.44 points**, rather than +7.41. Resolve the tag provenance before making any late-clause interpretation.

The prose also reports boundary **+16.7** and early **+3.7**, whereas its own table reports **+33.3** and **+7.4**. These are substantive stale-number errors.

## 2. Adjudication

### Turn 14: correction justified; disclosure incomplete

The `browser` rubric explicitly directs browser tasks to `browser-automation`. Screenshot-and-fill is such a task. Correcting the gold is defensible independently of the judge’s prediction.

The section commendably reports the old result, new result and significance sensitivity. But it should also disclose:

- The audit selected **only baseline-correct/judge-wrong rows**.
- The item rationale cites agreement by two judges, including another judge at 0.98. That corroboration is not independent textual adjudication.
- The actual adjudicators, blinding, timing and rubric version are unspecified.

Retain the corrected label. A known-wrong label is not preferable merely because its result is “conservative.” Present the old labels as a sensitivity analysis.

### The other six disputed turns

| Turn | Gold → judge | Assessment |
|---|---|---|
| 18 | `perplexity-research` → `web-researcher` | Genuine overlap. Synthesis and government/academic sources directly support the gold; trusted-domain research supports the alternative. No forced correction. |
| 25 | `ontology-augment` → `none` | Clear judge error: asking what the KG says is an explicit trigger. |
| 27 | `ontology-enrich` → `ontology-augment` | Keep gold. Validating and filling an existing dataset fits enrichment. Augment can propose governed enrichment, so overlap exists, but it is not the better label here. |
| 28 | `design-audit` → `ui-ux-pro-max-skill` | **Clear judge error under the very criterion used for turn 14.** The chosen rubric explicitly redirects existing-UI critique to `design-audit`. |
| 33 | `codebase-video` → `open-montage` | Keep gold. Repository video with narration and captions directly matches `codebase-video`. The general video rubric overlaps but does not require relabelling. |
| 65 | `context7` → `none` | Clear judge error: current, version-specific library documentation is its stated purpose. |

**I find no obligatory additional correction among these six.** But “four genuinely ambiguous and two plain errors” is too generous to the judge: turn 28 has an explicit boundary violation.

### The more damaging asymmetry lies outside those six

**Turn 15 warrants re-adjudication.** It asks for console errors from an existing Chrome session; both systems choose the current gold, `chrome-cdp`. Yet `browser-automation` says it is the entry point for **all browser work**, explicitly including reading console traffic. The supplied `chrome-cdp` rubric does not contain the rationale’s claimed distinction about already-open tabs versus launching a fresh browser.

If entry-point policy determines turn 14, explain why it does not determine turn 15. Correcting turn 15 against both systems would lower both accuracies without changing their gain, but it demonstrates why an audit restricted to judge losses is insufficient.

Other issues deserving a symmetric review include:

- Turn 8 already describes BM25’s `codebase-video` choice as a near-miss on a genuine hand-off.
- Turn 2 is explicitly acknowledged as a dual-skill request.
- Turn 52’s rubric says “NEVER auto-routed; only ever invoked explicitly.” Clarify whether requesting the procedure without naming the skill satisfies that condition.

Freeze a policy for entry-point versus downstream skills and single versus acceptable-set labels, then apply it to **all 86 turns**, blinded to system identity.

## 3. The oracle-tuned ceiling

### The restricted “floor” argument is valid

For fixed labels and a fixed ranker family,

\[
A_J-\max_t A_B(t)\leq A_J-A_B(t_0).
\]

Thus an actual in-sample optimum minimises the observed judge advantage over that family of threshold rules.

It is **not** a lower confidence bound, a production-performance guarantee, or a floor relative to all judge-free procedures.

### The reported threshold is not optimal

The supplied BM25 scores show:

- Turn 78, gold `none`: score **9.178100**.
- Turn 81, gold `none`: score **9.571490**.
- The next score is **9.742038**, on a correctly classified skill turn.

Consequently, any threshold

\[
9.571490 < t \leq 9.742038
\]

under a “decline if score \(<t\)” rule fixes both errors without losing a correct answer. At \(t=9.65\):

- BM25: **68/86 = 79.07%**.
- Declines: **19**, of which **14** are correct.
- Judge gain: **9.30 points**.
- Discordant pairs: **14–6**.

Exhaustively checking every score boundary confirms that **68 is the maximum** for this scalar-threshold family on the saved scores. This is a repair of the stated analysis, not a new modelling choice. The remaining caveat is whether the earlier mined scores used precisely the same exposure and ranker implementation; that must be verified.

### A stronger judge-free procedure

A reasonable next control is **policy-aware lexical routing**: honour explicit rubric redirects and exclusions rather than treating their words as positive evidence for the excluded skill.

Two concrete sensitivities illustrate its potential:

- Optimised BM25 plus the `browser` entry-point redirect scores **69/86 = 80.23%** here.
- Additionally respecting the signature-image exclusion in `pdf-signing` can recover turn 46, producing **70/86 = 81.40%**.

The latter is an estimated rule-based sensitivity, not a validated general algorithm. These changes are motivated by observed errors; they need a fixed parser/rule specification and held-out evaluation before becoming performance claims.

Other credible controls include positive-versus-exclusion clause indexing, an explicit `none` rubric, and lexical/embedding fusion with calibrated rejection. Full rankings and embedding scores are absent, so assigning those methods a numerical score would be speculation.

Finally, **cosine under a trained embedding is judge-free, but not model-free or free of learned semantic judgement**. The wording should distinguish these properties.

## 4. What the finding actually is

I would lead with:

> “On this diagnostic corpus, comparison with a cheap lexical router identifies complementary failures, a labelling inconsistency, and substantial sensitivity to prompt provenance. Aggregate superiority over an optimally thresholded lexical baseline is not established.”

Then show the overlooked provenance decomposition:

| Provenance | n | Judge | Optimised BM25 | Net wins |
|---|---:|---:|---:|---:|
| Previously authored, different task | 9 | 9 | 3 | **+6** |
| Directory-derived | 63 | 53 | 51 | +2 |
| Log-shaped, newly authored | 14 | 14 | 14 | 0 |
| Total | 86 | 76 | 68 | +8 |

This is more informative than saying author-written phrasing merely “flatters a lexical ranker.” The records offer a concrete pattern: most of the remaining advantage occurs in prompts not newly constructed from the directory.

But provenance is confounded with skill family and task style. The nine earlier prompts target only `diagrams-as-code` and `explainer`; they are not an independent routing benchmark. Their nominal exact p-value is 0.03125, but this is a post-hoc, clustered comparison.

The complementarity is also descriptive and useful:

- Both correct: 60.
- Judge only: 16.
- BM25 only: 6.
- Both wrong: 4.
- Oracle union: **82/86 = 95.35%**.

However, **predictions disagree on 24/86 = 27.91%**, not 25.6%. The reported 22/86 measures disagreement in *correctness*. Two additional rows contain different wrong predictions. The union assumes an oracle selector; it does not establish an achievable cascade accuracy.

### The conceptual transfer is overstated

In the original instrument, scoring a verbatim copy reproduces the ceiling by construction. Here BM25 does not copy the answer; it solves a selection problem imperfectly.

Showing all answer options is standard classification, not evidence that their correct association with the question has been exposed. A complete answer vocabulary does not make classification “delivery.”

Call this an **exposure-matched routing baseline**, or explicitly say it is an analogy extending the control philosophy. Do not claim that the original copy identity transferred unchanged.

Nor do these data establish that independent scoring or uncompressed rubrics *caused* the gains. Model size, architecture, training, context handling and rejection mechanism all changed together. Truncation and option-count ablations are needed.

## 5. Threats a hostile reviewer would raise

**The hardest critique is that the benchmark encodes the authors’ routing policy and construction process, without independently sampled tasks or independently adjudicated acceptable answers.**

The supplied evidence makes that critique concrete: inconsistent tags, overlapping skills, inconsistent entry-point policy, and gains concentrated in nine related earlier prompts. More precise p-values cannot answer it.

New measurement is needed:

- Independent or prospectively collected prompts, preserving necessary conversational context.
- Blinded, symmetric adjudication with explicit precedence and multi-label rules.
- Frozen development/test separation for thresholds, cascades and rubric changes.
- Skill-family/source clustering and replication across engines and runs.

Other threats:

- **Production prevalence:** 16/86 `none` is unlike roughly 65% in production. Simple reweighting would suggest judge 95.0% versus optimised BM25 83.9%, but only under unchanged class-conditional difficulty—an unsupported assumption. Do not present that as a forecast.
- **Small negative sample:** 16/16 `none` recall has a two-sided exact 95% lower bound of only **79.4%**.
- **Coverage:** only **62 of 115 skills** appear as positive gold labels.
- **Repeated threshold passes:** separate passes cannot isolate threshold effects from run variation unless deterministic scores are established. Prefer replaying one fixed score matrix.
- **Confidence semantics:** confidence below 0.5 accompanies many non-declined choices. It is not the thresholded entailment score. Without its definition as an outcome probability, “under-confident” is not justified; the observed AUC **0.7276** supports rank association.
- **Top-3:** the run supplies `soft_correct`, not ranked predictions. Verify that it literally means gold in the top three, and document handling of `none`.

## 6. Legitimate versus illegitimate re-tuning

**Principled repairs now:**

- Correct the BM25 threshold search within the already declared family.
- Recompute correctness from current labels; do not trust old joined booleans.
- Resolve tag-version discrepancies and stale prose.
- Report exact paired inference, effect uncertainty and transparent multiplicity.
- Audit all labels under one frozen rubric-policy criterion.
- Separate exploratory sensitivity analyses from primary results.

**Legitimate exploratory work requiring future validation:**

- Policy-aware lexical rules, richer rejection features, ranker fusion.
- Acceptable-set scoring applied symmetrically across the corpus.
- Provenance decomposition and family-level clustering.
- Cascade and confidence-gating development.

**Forking-path behaviour to avoid:**

- Switching to one-sided, mid-p or bootstrap inference because exact two-sided p exceeds 0.05.
- Promoting threshold 0.3 because this corpus favours it.
- Choosing whichever late-tag version strengthens the story.
- Crediting ambiguous judge answers while retaining equally defensible baseline answers as wrong.
- Selecting provenance groups, cascades or redirects post hoc and presenting their performance as confirmatory.
- Calling any improved baseline a universal “ceiling.”

## Numbers you computed

The core results can be reproduced read-only with standard Python:

```bash
python -B - <<'PY'
import json, math, statistics

C = json.load(open("routing-cases.json"))["cases"]
J = {x["index"]: x for x in json.load(open("openjev-run-v2.json"))["cases"]}
R = {x["i"]: x for x in json.load(open("mined-rows.json"))}
n = len(C)
gold = [x["expected_skill"] for x in C]
judge = [J[i]["choice"] for i in range(n)]

def exact(b, c):
    return min(1, 2 * sum(math.comb(b+c, k)
                         for k in range(min(b,c)+1)) / 2**(b+c))

def report(pred):
    d = [int(j == y) - int(p == y)
         for j, p, y in zip(judge, pred, gold)]
    b, c = d.count(1), d.count(-1)
    mean = statistics.mean(d)
    se = statistics.stdev(d) / math.sqrt(n)
    return {
        "baseline_correct": sum(p == y for p, y in zip(pred, gold)),
        "judge_only": b, "baseline_only": c,
        "gain": mean, "exact_p": exact(b, c),
        "normal_CI": (mean-1.96*se, mean+1.96*se)
    }

def predict(t):
    return ["none" if R[i]["lex_score"] < t else R[i]["lex_pick"]
            for i in range(n)]

cuts = sorted({R[i]["lex_score"] for i in range(n)})
cuts = [cuts[0]-1] + cuts + [cuts[-1]+1]
best = max(cuts, key=lambda t:
           sum(p == y for p, y in zip(predict(t), gold)))

print("reported", report([R[i]["lex_final"] for i in range(n)]))
print("exhaustive optimum", best, report(predict(best)))
print("threshold 9.65", report(predict(9.65)))

for provenance in sorted({x["provenance"] for x in C}):
    ids = [i for i, x in enumerate(C) if x["provenance"] == provenance]
    pred = predict(9.65)
    print(provenance, len(ids),
          sum(judge[i] == gold[i] for i in ids),
          sum(pred[i] == gold[i] for i in ids))

print("tag mismatches", [
    i for i in range(n)
    if C[i]["late_discriminative"] != J[i]["late_discriminator"]
])
PY
```

Additional formulas:

- Exact paired p-value:
  \[
  \min\left(1,\;2\sum_{k=0}^{\min(b,c)}
  {b+c\choose k}2^{-(b+c)}\right).
  \]
- Exact enumeration of the ordinary paired bootstrap: the coefficient of \(z^s\) in
  \[
  \left(\frac b n z+\frac c n z^{-1}
        +1-\frac{b+c}{n}\right)^n
  \]
  gives the bootstrap probability of mean difference \(s/n\). Take its 2.5th and 97.5th percentiles.
- Holm: sort the \(m\) p-values; adjusted \(p_{(k)}\) is
  \[
  \min\{1,\max_{j\leq k}(m-j+1)p_{(j)}\}.
  \]
- Production-mixture sensitivity: \(0.65A_{\text{none}}+0.35A_{\text{skill}}\), explicitly conditional on stable within-class performance.

## Specific edits

1. **Original:** “A no-op extractor has no meaning when the answer is a key rather than a passage, so the copy procedure becomes the best judge-free surface match over the exposed options…”

   **Replace:** “For this choice task we use an exposure-matched routing baseline: a judge-free ranker over the shown rubrics. This extends the control philosophy, but does not preserve the verbatim-copy identity of the original instrument.”

2. **Original:** “The ceiling is thereby flattered, the judge’s gain is a floor…”

   **Replace:** “Optimising the decline threshold on the evaluation corpus minimises the observed judge advantage within this fixed ranker-and-threshold family; it provides neither a population lower bound nor a bound against other judge-free procedures.”

3. **Original:** “The interval excludes zero, but only just, and it holds from about \(n=70\)…”

   **Replace:** “An exhaustive threshold search on the supplied BM25 scores yields 68/86 correct versus 76/86 for the judge: a paired gain of 0.093, with 14 judge-only and six baseline-only successes, exact two-sided \(p=0.115\), and an exploratory turn-bootstrap 95% interval of [−0.012, +0.198].”

4. **Original:** “Each of the three large gains is the predicted consequence of a design choice rather than of general model quality…”

   **Replace:** “These descriptive differences motivate tests of rejection and rubric truncation, but this comparison does not isolate their causal contributions.”

5. **Original:** “The ranker and judge disagree on 25.6% of turns and their union is right on 95.3%…”

   **Replace:** “With the reported BM25 rule, predictions disagree on 24/86 turns (27.9%), correctness differs on 22/86 (25.6%), and an oracle selecting either correct answer would achieve 82/86 (95.3%).”

6. **Original:** “The other six we left standing against the judge; four are genuinely ambiguous … and two are plain judge errors…”

   **Replace:** “We retained the other six labels. Their ambiguity varies: turns 25 and 65 are unwarranted declines, and turn 28 violates an explicit redirect to design-audit; the remaining cases involve overlapping but distinguishable scopes.”

7. **Original:** “A ranker wins by surface, a judge by the rubric’s semantics…”

   **Replace:** “Disagreement can prioritise label review, but neither system’s answer establishes correctness. Review should cover both disagreement directions and rubric-policy inconsistencies among agreements.”

8. **Original:** “On ordinary matching among exposed options, the ranker and the judge are close, which is what total exposure predicts.”

   **Replace:** “Near-neighbour accuracy differs by two turns under the reported baseline. Candidate availability alone predicts neither small gains nor equivalence.”

9. **Original:** “The judge is under-confident but rank-informative…”

   **Replace:** “Reported confidence separates correct from incorrect choices with AUC 0.728 on this corpus. Its calibration interpretation requires establishing what probability the reported field represents.”

The abstract’s “fair ceiling locates” statement should likewise become an exploratory description, and its late-group numbers should wait until the tag discrepancy is resolved.

## Context I still need

- Exact baseline implementation, threshold grid and whether the mined scores share the corrected run’s rubric/tokenisation version.
- Authoritative late-tag definition, tokenizer, assignment procedure and version history.
- Blinded adjudication records and a rule governing entry-point versus downstream skills.
- Full per-option scores, cosine predictions, ranked outputs, the `none` rubric and confidence-field definition.
- Whether prompts, rubrics, checkpoint selection or engine design were revised after examining these cases.
- A justified cluster map and independent production-like evaluation.
- Runtime code confirming why metadata reports **116 hypotheses scored**, while the prose describes 115 pair scores followed by threshold rejection.

**No files were written, moved or deleted; no MCP tools were called.**