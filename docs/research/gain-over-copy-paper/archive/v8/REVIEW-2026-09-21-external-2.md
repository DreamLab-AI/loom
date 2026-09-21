# External review 2 of paper v8 (26 pp), received 2026-09-21

Verbatim, same reviewer, after reading the v8 revision against v7.

---

**This is substantially better. The central argument is more defensible, the contribution is clearer, and several corrections involve actual reanalysis rather than softer wording. But I still would not submit it yet: the newly disclosed gaps in the experimental records are now the biggest obstacle.**

I compared the 26-page revision with the previous version. My judgement has shifted from "the unifying argument is not sound" to **"a worthwhile empirical paper is emerging, but the evidence and remaining contradictions need resolving."**

**The most important improvements are substantive.**

| Previous problem | What this version does | My assessment |
| --- | --- | --- |
| Exposure was treated as evidence that reasoning did not occur | Explicitly explains why zero gain can coexist with successful reasoning | The main conceptual correction is right, although older claims survive elsewhere. |
| Routing replaced the metric with a ranker while retaining its identity | Moves routing to a companion note and distinguishes the instruments | This resolves the principal formal objection. |
| Suppression on omitted targets became a claim about all out-of-corpus knowledge | Precisely identifies the tested population and policy | Much stronger and appropriately bounded. |
| All omitted alternatives were treated as failures | Identifies 52 omissions that were compliant answers and recalculates | A useful new analysis. |
| Novelty was overstated relative to existing diagnostics | Acknowledges the nearest precedents and claims an inexpensive implementation and reporting discipline | More credible positioning. |
| The copy ceiling received credit for rejecting harmful page edits | Identifies the before/after judge as the instrument that actually rejected them | Correct and important. |
| Architecture prescriptions exceeded the evidence | Defers much of the broader architectural argument | Improves focus considerably. |
| Large denominators implied independent observations | Clarifies the repeated 1,136 target instances in the abstract | Properly fixed. |

The optional-answer correction is particularly helpful. Removing 52 compliant omissions changes the reported omission rate from **6.93% to 6.44%**, or from one in 14.4 to one in 15.5 exposed targets. It shows that the objection was real, but does not overturn the broad observed pattern. That is exactly the kind of sensitivity analysis the paper needed.

The revised suppression section is also much better. It separates the observed loss of recall from possible mechanisms, and proposes a targeted experiment to distinguish them.

**The largest remaining issue is experimental provenance, and the new disclosure makes it more serious than it appeared previously.**

Page 2 now says the control rerun's completions, judge outputs and generating script were never written to disk. Page 21 says the control rerun, composition study and paraphrase stress-set have **no artefact on disk at all**.

That is different from having completed experiments whose files have not yet been uploaded.

These studies support several of the paper's most interesting claims:

* the corrected content-specificity result;
* the collapse from 0.96 to 0.34 under paraphrasing;
* the two-premise versus single-premise model differences.

Yet the manuscript presents precise estimates, bootstrap intervals and significance decisions that cannot presently be traced back to the observations.

**The next question is therefore where those exact numbers came from and what computational record survives.** Aggregate logs or saved analysis outputs may exist even if individual answers were lost. If they do, identify and preserve them. If they do not, these studies need to be rerun before they carry the weight assigned to them.

The current language also mixes three distinct situations:

| Situation | Appropriate treatment |
| --- | --- |
| Results retained but not publicly released | Release the existing records. |
| Inputs and code retained, derived outputs missing | Recompute and verify the reported values. |
| Questions, code or experimental observations not retained | Recover the original record or repeat the experiment; do not describe this merely as pending release. |

For example, a deleted harness may be recoverable from Git history if it was committed. A lost generated question set is harder: regenerating it may produce a different benchmark and therefore a new experiment.

The frank disclosure improves transparency. **It does not substitute for an auditable record.** I would prioritise this over adding another experiment or polishing another paragraph.

**Several contradictory claims remain because the revision adds correct qualifications without removing the old statements.**

These are the ones I would fix first:

| Location | Remaining problem | Required change |
| --- | --- | --- |
| §5.3, p. 6 | Still says g>0 would evidence reasoning over structure | Describe recovery beyond the copy baseline; the sign does not identify reasoning. |
| §5.3 and Figure 2 | Still equate approximately zero gain with faithful delivery | Zero net gain can conceal offsetting omissions and recoveries, as your own accounting explains. |
| §5.5, pp. 6–7 | Says none of the four preceding instruments is both judge-free and deterministic | This conflicts with the immediately preceding discussion of the graded lexical reader–context diagnostic. Delete the blanket distinction. |
| Introduction, p. 1 | Still says the scalar can move under choices the counts are immune to, pointing to §18 | §18 now correctly says that the moving ranker baseline is a different instrument. The old argument no longer follows. |
| §§7 and 11.1 | Says generation must be evaluated as gain above the ceiling rather than against a bare model | Report the exposure reference alongside appropriate quality and system baselines. Positive copy gain is not a prerequisite for useful generation. |
| Figure 4, p. 17 | Still labels performance as "guessing + composition", "what guessing achieves" and "composes over structure" | Replace these with descriptions of the experimental conditions and observed differences. |

Figure 4 is especially important because it visually reinstates the interpretation that the revised prose explicitly rejects.

I would label it simply:

* horizontal axis: **Recall with answer-bearing premise only**;
* vertical axis: **Recall with both premises**;
* diagonal: **Equal recall**;
* positive region: **Adding route information helps**;
* negative region: **Adding route information hurts**.

The caption's "composition-specific component" should likewise become **paired effect of adding the route-bearing block**.

These are not cosmetic edits. Readers often remember figures more strongly than qualifications in the surrounding text.

**Two mathematical and measurement statements still need tightening.**

First, §5.2 says:

> "Every quantity this paper reports is a function of them."

That is false for the four pooled counts. They determine the pooled exposure and recovery summaries, but not the macro-averaged question scores, question-level uncertainty estimates or judged outcomes. Even different allocations of the same counts across questions can produce different macro scores.

Specify that the four counts determine the listed **micro-averaged exposure/recovery metrics**. Question-level records remain necessary.

Second, §§9–10 now describe the omission rate as an **upper bound on the error rate**. That is too broad. Counting optional alternatives can exaggerate omission, but the matcher can also credit relationally wrong answers and miss other kinds of error.

The safe statement is that the raw omission count includes some compliant omissions. The adjusted figure remains a target-name omission rate, not a bound on semantic error.

Similarly, "first-order insensitive to shared matcher error" still sounds like a mathematical guarantee that has not been established. Applying the same matcher consistently is sensible; error cancellation additionally requires assumptions about errors in structured context and generated prose. I would state the possibility of cancellation without claiming an order of robustness.

**The control section is more honest, but its cohort still needs reconstruction.**

The combination of:

* zero attrition;
* approximately 50–55 observations per arm;
* an initial 57-question cohort;
* different pairwise-complete sets;

is not adequately explained by saying that pairwise-complete sets differ. That describes the consequence, not the reason.

If every intended question was completed in every arm, why are the sets different? Were arms launched on different subsets? Were grading failures excluded? Were existing raw answers reused? Was eligibility applied differently?

A compact accounting table should show, for each arm:

**planned → attempted → completed → graded → included.**

Without the records, this cannot be repaired by prose alone.

There is also a useful correction to my earlier review: **the revised manuscript says both live and control paths place the context in system messages.** If that reflects an audit of the evaluated implementation, the specific placement criticism no longer applies. The newly identified authority-wrapper difference is the relevant confound.

However, the statement that *no contrast below isolates content from instruction strength* may now be too broad. If all control arms share the same weaker wrapper, true-versus-fluent-noise can still compare content within that wrapper. The live-versus-true comparison is the one directly confounded by the wrapper difference. Separate those cases.

**The paper now identifies the semantic-validation gap correctly, but identifying it has not closed it.**

The new §9 passage is good: it explicitly says the matcher cannot validate relations, direction, negation or precision, and calls for a human audit.

That still leaves a methods paper asking readers to adopt an instrument whose relationship to meaningful answer correctness has not been established.

I would not ask you to solve general semantic evaluation. A focused audit could establish something useful:

* sample across templates and exposure/recovery categories;
* distinguish correct relational answers from name-only matches;
* separate required targets from acceptable alternatives;
* report where lexical scoring and human assessment disagree.

This would let the paper say when the inexpensive proxy is useful and how it fails. That is a stronger contribution than repeatedly acknowledging that it gives up semantic discrimination.

The promotion-gate discussion also needs this discipline. A block with ceiling 1 is **name-complete under the matcher**, not necessarily answer-complete. The later term-stuffing caveat is correct, but the stronger label should be removed where it first appears.

**The revision is clearer, but it still reads partly like a response to a reviewer embedded inside the paper.**

You have shortened it from 31 to 26 pages and moved the system context earlier, both helpful changes. The abstract's broken `emph` commands are also fixed.

However, multiple sections follow this pattern:

1. make the original claim;
2. explain why the claim cannot be interpreted that way;
3. describe the experiment that would establish it.

That leaves the reader managing the revision history.

The next editorial pass should **replace disputed statements**, then consolidate limitations. Keep the detailed change history in a response document or repository notes.

The abstract could also lose the parenthetical explanation that the signed-rank test is not a test of the mean. That distinction belongs next to the statistical analysis; it currently interrupts an already crowded abstract.

**My recommendation is now quite specific.**

1. **Resolve the evidential record.** Recover, rerun or remove the studies whose underlying observations are unavailable. Pin retained experiments to one release.
2. **Do one consistency pass across prose, equations and figures.** Eliminate the surviving reasoning claims, the obsolete scalar-instability argument and the unsupported error-bound language.
3. **Complete the targeted semantic audit.** Together with the optional-answer sensitivity analysis, this would substantially strengthen the instrument.
4. **Choose the smallest paper supported by the resulting record.** If only the sweep, production pairs and their analyses are recoverable, a focused paper built around those is preferable to retaining several major but unverifiable studies.

I would now be much more receptive to the paper as an empirical systems/evaluation contribution. **The main conceptual repair is largely achieved. Publication readiness now depends on making the evidence as inspectable as the paper argues evaluation should be.**
