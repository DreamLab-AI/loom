# External review of paper v7 (31 pp), received 2026-09-21

Verbatim, first external reviewer. Disposition: `REMEDIATION-2026-09-21.md` (paper-v8).

---

**There is a worthwhile paper here, with useful experiments and unusually candid reporting. But I would not submit this version unchanged.** Its strongest contribution is a practical evaluation discipline for a deployed system. Its weakest part is the attempt to turn that discipline into a general test for whether reasoning occurs, and then into a prescription for enterprise architecture.

If I were reviewing it as a research-methods paper, I would currently recommend **major revision, or rejection with encouragement to resubmit**, depending on the venue. That judgement comes from several substantive conceptual problems, not simply from wanting more experiments.

I read all 31 pages, inspected the presentation, checked several closest research precedents and consulted the public repository documentation. I have not rerun the experiments, so I distinguish defects visible in the manuscript from results that require the released data to verify.

**What I think is valuable is the empirical investigation underneath the headline.** Several findings deserve attention:

| Finding | My assessment |
| --- | --- |
| Large raw-to-grounded recall gains coexist with answer names already being present in context | A useful warning about what an evaluation actually establishes. |
| Models omit some exposed targets and recover almost none of the unexposed ones | A useful operational diagnostic, subject to the scorer and answer-set issues below. |
| Vocabulary changes collapse lexical retrieval, while the fallback almost never activates | One of the strongest engineering findings in the paper. It identifies a concrete failure mechanism. |
| Grounding reduces recovery of targets omitted from the scaffold | Potentially the most interesting empirical result, if its scope and mechanism are handled carefully. |
| Correcting attrition and adding fluent noise weakens the content-specificity claim | Scientifically valuable. Publishing this correction increases confidence in the research process. |
| Better ontology-term resolution does not produce better edited pages | A valuable demonstration that improving an intermediate metric can damage the end product. |
| Two-premise and single-premise conditions produce different model behaviours | Promising, although the interpretation needs stronger controls. |

The negative findings give the paper substance. The problem is that later sections repeatedly make claims stronger than those findings support.

**The largest conceptual problem is that answer-name exposure does not distinguish delivery from reasoning.**

Your matcher answers:

> Does the text contain the name of a correct answer?

That is different from:

> Does the text explicitly supply the answer to this question, without requiring reasoning?

Consider a context containing:

* A requires B.
* B has part C.
* D has part E.

The question asks for a part of something A requires. Correctly choosing C can require following the two relations, despite C already appearing in the context. Your exposure ceiling is 1. A perfect answer scores 1. Gain over copy is zero.

Consequently, **zero gain is compatible with successful reasoning, and negative gain is compatible with reasoning that sometimes fails.** Positive gain is also compatible with memorisation, guessing or matcher errors, as you acknowledge.

Section 11 effectively demonstrates this limitation within your own paper: `b-only` and `both` expose the answer name, yet the route information changes performance. That is a reason to treat exposure accounting and reasoning evaluation as separate instruments.

The careful passage in §3.3 gets this right: the measure constrains what the evaluation can attribute, rather than establishing what the model did internally. But later claims abandon that distinction:

* Figure 2 attributes uplift to exposure rather than reasoning.
* §9 calls the ceiling a detector of serving versus synthesis regimes.
* §18 says the instrument shows reasoning is not occurring.
* D7 makes a broader claim about promises of reasoning over private data.

Those statements do not follow.

The defensible conclusion is: **this recall score does not establish reasoning beyond answer-name exposure.** That is already useful. You do not need the stronger claim that reasoning was absent.

There is a related problem with calling this a "ceiling". You explicitly explain that it is not an upper bound, but then repeatedly use the word to support architectural conclusions. I would prefer *copy baseline* or *input-exposure reference*. If you retain the title, the interpretation must remain tightly controlled throughout.

**The routing extension contains a formal change of metric that the paper presents as continuity.**

This is more serious than loose terminology.

In §3, the ceiling is the fraction of gold items exposed in the input. In §12, every correct option is exposed. Under the original definition:

c_exposure = 1.

For the 4B router, accuracy is 76/86 ≈ 0.884. Applying the original definition gives:

g_exposure = 0.884 − 1 = −0.116.

Instead, §12 reports:

g_BM25 = 0.884 − 0.837 = +0.047.

That second number is an improvement over an oracle-tuned ranker. It is a legitimate comparison, but **it is not the scalar defined by the exposure table**.

This also undermines the paper's repeated argument that the scalar changed while the counts remained fixed. The counts stayed fixed because the shown options and model answers stayed fixed. The reported scalar changed because you altered a different procedure: the ranker used as a baseline.

You have demonstrated baseline sensitivity, not instability of the original exposure scalar.

I would repair this by giving the two quantities different names and withdrawing the claim that routing validates the same formal instrument. The routing study could remain an illustration of a broader principle—evaluate against strong simple baselines—or become a separate paper.

Two further qualifications matter:

* Oracle threshold tuning gives a favourable baseline **within the chosen ranker family on this dataset**. It does not make the measured advantage a general lower bound on semantic reasoning or deployment performance.
* Removing exclusion clauses uses less text, but that alone does not establish that the procedure "cannot introduce supervision". Choosing a task-aware preprocessing rule after examining results can introduce benchmark adaptation without adding any text.

**The central measure establishes target-name retention, not faithful delivery.**

You acknowledge the absence of precision and attribution measurements. Nevertheless, "faithful delivery", "vetted facts" and "attributably" carry much of the argument.

An answer can mention every gold entity while:

* assigning the wrong relation;
* reversing subject and object;
* explicitly denying the correct relationship;
* adding substantial false information;
* giving an unusably broad list.

Your matcher can reward these answers. The embedding re-score addresses some lexical variation, but does not establish relational correctness, entailment or factual precision. Similarity between a title and a span is still not a test that the answer asserts the right fact.

Using the same matcher on context and output does not guarantee error cancellation. Structured scaffolds and generated answers differ in length, wording and syntax, so their matching errors can be systematically different.

For a paper whose contribution is a measurement instrument, **validation of the instrument against independently assessed meaning is central evidence**, rather than an optional future extension. A stratified human audit would be more valuable here than another model sweep.

There is also a subtle issue with your answer sets. T-COMMON accepts any one of several valid targets, whereas the item-level decomposition counts each target separately. A model can therefore give a completely acceptable answer while accumulating "exposed but omitted" items.

The invariant showing that copy text receives the same question-level score does not resolve that issue. You need to distinguish:

* required answer components;
* interchangeable acceptable answers;
* optional additional information.

Until that is done, "one exposed item in fourteen is dropped" should not be read as an error rate. Some omissions may represent correct compliance with the question.

**The novelty is narrower than the manuscript's rhetoric suggests.**

You cite the relevant neighbours, which is good. The remaining problem is the strength of the differentiation.

| Prior work | What it already covers | Implication for your claim |
| --- | --- | --- |
| [RAGChecker](https://arxiv.org/html/2408.08067v1) | Ground-truth claim coverage in context, context utilisation, self-knowledge and generation diagnostics | The exposure/utilisation decomposition has close precedent. Your main distinction is deterministic surface matching and its practical reporting form. |
| [Recall Is Not Enough](https://arxiv.org/html/2607.00725v2) | Answer presence in the packed context, including a graded lexical variant for free-form answers, plus intervention-based validation | The contrast with a merely binary answer-survival measure is incomplete. This is a particularly close comparator. |
| [Sufficient Context](https://arxiv.org/abs/2411.06037) | Whether the supplied evidence is sufficient to answer, and how models behave when it is not | This makes the distinction between name exposure and answer sufficiency especially important. |
| [MuSiQue](https://arxiv.org/abs/2108.00573) | Construction and filtering of multihop questions to resist disconnected and single-hop shortcuts | Single-premise controls are well motivated, but should be positioned as an application and adaptation of established methodology. |

A cheap, reproducible, judge-free implementation can be a worthwhile contribution. However, the trade-off is that removing semantic judgement also removes semantic discrimination. The paper needs to measure that trade-off.

My assessment is:

* **Novel mathematical quantity:** modest.
* **Useful reporting convention:** plausible.
* **Empirical production audit:** worthwhile.
* **General theory of where reasoning is needed:** unsupported.
* **Validated enterprise architecture:** not established.

A stronger novelty claim would centre on an inexpensive diagnostic, its failure modes, and the operational decisions it changes—not on discovering a general boundary between copying and reasoning.

**The control experiments are valuable, but do not yet establish why the system helps.**

Your corrected interpretation of §8.5 is appropriately cautious. The true-versus-fluent-noise interval, approximately [−0.02, +0.55], is compatible with both a negligible difference and a useful content-specific benefit. It neither establishes specificity nor establishes equivalence.

There are several remaining design issues:

1. **Seed disjointness does not establish answer disjointness.** Different seed classes can share ancestors, relation targets, synonyms and descriptive facts. You recognise this in §3.4, but later describe the placebo as a correctly specified floor. Report measured target exposure and relevant relation overlap for every placebo arm.

2. **The controls change message placement.** The live system injects into a user message while the controls use a system message. That changes instruction priority as well as content.

3. **"Uniform budget" is inaccurate.** A common 4096-token start with arm-dependent 8192-token retries is a common retry policy, not a uniform realised budget. That is acceptable for evaluating a service policy, but it requires different wording and cost reporting.

4. **The rerun needs one clearly defined common cohort.** Table 5 says attrition is eliminated but still describes varying pairwise-complete sets. This may have an innocent explanation, yet it prevents straightforward reconciliation of the contrasts. Show the exact question IDs and n for every row, with a common-intersection sensitivity analysis.

5. **The causal controls and headline recall sweep are different studies.** The ten-model lexical result cannot inherit a causal interpretation established—or left unresolved—on a smaller, differently scored local-model cohort.

I would prioritise one tightly controlled experiment with identical wrappers, fixed questions and explicit exposure measurements over further heterogeneous arms.

**The suppression result deserves a more prominent, narrower treatment.**

The change from 92/760 raw recoveries to 3/760 grounded recoveries is striking. It is more informative than observing that outputs remain below a high copy baseline.

But the manuscript sometimes expands this into a claim that grounding makes a model roughly thirty times worse at anything outside the corpus. That is incorrect.

The tested items are:

* gold targets from this corpus;
* absent from the particular retrieved scaffold;
* assessed under this prompt, retrieval policy and matcher.

"Absent from the retrieved scaffold" is not "outside the corpus", and neither means "arbitrary out-of-domain knowledge".

Moreover, obeying an authoritative supplied source can be desirable behaviour. The question is whether the model mistakes an incomplete context for a complete answer, suppresses a competing answer because of the instructions, becomes distracted, or simply produces shorter lists.

A useful follow-up would compare the same omitted targets under:

* the existing authority instruction;
* an explicit warning that the context may be incomplete;
* permission to supplement it with separately labelled knowledge;
* a neutral context wrapper.

Report whether raw-correct targets become omissions, explicit contradictions or abstentions. That would turn the current behavioural observation into a much more informative account of the failure.

**The composition study is promising, but "composer" and "anti-composer" overstate what the comparison identifies.**

`both − b-only` measures the effect of adding route information under your particular context construction. It does not uniquely identify composition ability.

A high `b-only` score can reflect useful semantic inference, elimination among candidates or broad answer enumeration. Calling all of it "guessing" is too strong. Conversely, adding the A block can change distractor competition and answer-selection behaviour as well as supply a missing premise.

Your headroom caveat for GPT-5.6 is correct and important. It also means the delta should not become a capability ranking: a model with a weak single-premise baseline has more room to improve.

The decisive next controls would be:

* preserve the entities but swap the edges so the correct answer changes;
* preserve answer names and length while reversing relation direction;
* rename entities consistently to unfamiliar identifiers;
* score a bounded answer set with precision as well as recall.

A model that follows the changed relation to the changed answer provides stronger evidence than one whose recall rises when another relevant block is added.

The lack of multiplicity correction also matters when assigning categorical labels across twelve models. At minimum, describe these as exploratory, configuration-specific behavioural groups.

**The architecture recommendations exceed both the evidence and the metric's operational capabilities.**

Section 18 is the weakest part of the manuscript in my view.

First, the copy ceiling requires a gold answer set. On an arbitrary live request, that set is generally unknown. "Every served path reports its ceiling" therefore needs an explanation:

* Is it an offline benchmark measurement?
* Is gold available from an executable graph query?
* Is a model proposing the targets?
* Is the system only reporting which retrieved entities were repeated?

Those are different instruments with different guarantees.

Second, D1 proposes rejecting an extraction whose recall does not exceed what its source exposes. For faithful extraction, recovering information already present in the source is normally the objective. Requiring positive gain can penalise correct extraction and reward unsupported additions.

The relevant distinction is **new to the destination corpus versus supported by the source**. The same claim can be old in the source and valuable new information for the destination.

Third, a copy deficit is unsuitable as a standalone optimisation objective across retrieval systems. For example:

| System | Exposure | Answer recall | Gain over copy |
| --- | ---: | ---: | ---: |
| A | 0.50 | 0.50 | 0.00 |
| B | 1.00 | 0.95 | −0.05 |

A has the better gain but answers substantially less of the question. This is a useful diagnostic difference, not a general preference ordering.

Fourth, the claim that value concentrates upstream at curation requires evidence about curation effort, maintenance, query distribution and alternative serving costs. The paper measures neither that economic allocation nor a quality-matched alternative.

Finally, there is no matched test establishing how much value comes specifically from the ontology. A relation-aware graph lookup, flat-text retrieval and the ontology scaffold should be compared on the same tasks and budgets. Since many questions are graph-generated, a direct graph-query baseline is particularly important.

The case for keeping a governed, reversible corpus is sensible. This paper does not need to claim that the copy ceiling proves that architecture.

**The statistical reporting shows care, but several uncertainty claims remain narrower than readers may realise.**

The paired design, explicit attrition discussion, exact tests and distinction between bootstrap intervals and rank tests are strengths. I would retain them.

The important remaining gaps are:

* **Repeated observations:** 11,360 means 1,136 target instances evaluated across ten models, not 11,360 independent facts. The paper discloses this late; make it explicit wherever the large denominator appears.
* **Question and graph dependence:** domains, classes and shared relation targets create dependence. Domain bootstrapping helps, but fifteen domains and one corpus do not establish broad transfer.
* **Mean versus rank estimands:** the reported exact signed-rank p-value is not a test of the mean difference itself. Keep this distinction adjacent to the headline.
* **Ceiling-limited general questions:** 58/60 ties with near-perfect judge scores provide weak sensitivity to deterioration on harder questions. The equivalence result is confined to that instrument and set.
* **Oracle tuning in routing:** a held-out estimate, or a resampling procedure that repeats tuning, is needed before treating the tuned baseline comparison as a deployment result.
* **Model rankings under truncation:** 174 truncated GLM rows are a substantial qualification. A fixed configuration is a valid operating point, but not a clean comparison of capability.

I would also avoid giving observed-effect power calculations much interpretive weight. For the next study, choose the smallest practically meaningful effect in advance and power for that.

**There are specific internal errors and contradictions that should be corrected before any further experiments.**

| Location | Problem |
| --- | --- |
| Abstract, p. 1 | Literal `emphexposure`, `emphcopy` and `emphgain` appear in the rendered PDF. These are visible typesetting errors. |
| §12, p. 18 | The stated reduction from +11.6 to +4.7 points is about 6.9–7.0 points, not 8.1. |
| §3 versus §§7, 9.1 and 19 | §3 correctly says the matcher can overcount and undercount; later passages call absolute recall a lower bound. It is not generally a lower bound. |
| §18, p. 23 | The text says the promotion gate rejected enrichment that a judge would have accepted, but §10.1 reports that judges rated the enrichment negatively. |
| §18 D1 | Rejecting extraction for failing to exceed source exposure conflicts with the paper's own account of faithful extraction. |
| §18 D2, p. 24 | The "thirty times worse" claim expands a scaffold-omission result to anything outside the corpus. |
| §11.1 and §19 | Some passages conflate the macro ceiling of 0.964 with the fraction of gold target instances exposed. From the counts, that latter fraction is 1060/1136 ≈ 0.933. |
| §§3.3 and 11 | References to the multihop design in §19 are stale; the experiment is in §11. |

The micro/macro distinction is explained elsewhere, so I do not regard the differing numbers themselves as an error. The problem is the prose sometimes treating them as interchangeable.

Reproducibility also needs one authoritative release. The public research index I checked still foregrounds an older paper and older interpretations of the controls. That does not invalidate this manuscript, but it makes independent verification unnecessarily difficult. Pin this version to a commit and publish a table-to-artifact manifest. [Public research index](https://github.com/DreamLab-AI/loom/blob/main/docs/research/README.md)

**An adversarial reviewer's strongest objection would be this:** the paper introduces a surface-overlap reference, demonstrates that generated answers omit some overlapping strings, and then treats that result as evidence about reasoning, fidelity and architecture. When it encounters a task where exposure is uninformative, it replaces the reference with a ranker but preserves the name.

That objection would currently be difficult to rebut.

The answer is to make the contribution smaller and better validated. I would prioritise the work as follows:

| Priority | Change | What it resolves |
| --- | --- | --- |
| Essential | Remove claims that the ceiling detects the absence of reasoning | The main inferential overreach. |
| Essential | Separate exposure accounting from the routing ranker baseline | The formal inconsistency. |
| Essential | Audit required versus alternative gold targets, and validate semantic correctness on a human-reviewed sample | Whether the measured omissions and recoveries mean what the paper says. |
| High | Run one matched content-intervention experiment with measured placebo exposure | Whether relevant evidence, rather than generic context or prompt changes, causes the benefit. |
| High | Compare against direct graph answers, constrained extraction and flat-text retrieval | Whether generation and ontology structure add useful value. |
| High | Replicate on independently authored questions and another corpus | Transfer beyond the benchmark's construction. |
| High | Release exact prompts, checkpoints, retries, row data and table-generation scripts together | Independent reproducibility. |
| Editorial | Shorten the main paper and move superseded results and architecture speculation out | A clearer, more defensible contribution. |

**My preferred paper is already inside this manuscript.** It would centre on exposure accounting, its semantic limitations, the retrieval failure under paraphrasing, and the suppression of omitted answers. The composition study would demonstrate why exposure and reasoning need separate measurements. The write-path study could be a compact case study or a companion paper. I would remove most of §18 and separate the routing extension.

The present manuscript is too long partly because it repeatedly restates and qualifies claims instead of settling on their defensible scope. The abstract is overloaded, the historical control figure competes with its replacement, and the system narrative arrives after a substantial series of results. These are signs that the paper needs restructuring around a smaller claim.

My judgement is that **the work is worth pursuing and contains publishable observations, but the current unifying argument is not yet sound**. The most valuable revision would make every headline as careful as your best limitations paragraphs.
