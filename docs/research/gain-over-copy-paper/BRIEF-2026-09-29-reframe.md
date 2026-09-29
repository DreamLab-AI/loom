# Authoring brief: reframe the paper after the press interpretation review (2026-09-29)

Editorial guidance for the v10 revision. Not a source of evidence; not to be cited.
Author decisions taken on 2026-09-29: continuity title kept ("The Copy Ceiling: ..."); the
five-arm control rerun (uplift-results/control-rerun-2026-09-21/) is judged, analysed and
reported as a completed study; the human annotation sample stays "prepared, not performed";
this revision is tag paper-v10; the 150-word summary is not produced; the press draft is
context only and is never referenced.

## 1. Editorial task
Reframe so a technically interested reader does not conclude that language models cannot
reason, that knowledge graphs are unnecessary, or that a context dump beats generated prose.
Change title emphasis, abstract, opening, result labels, captions and conclusion so the main
claims are accurate read alone. Organising proposition: we built a useful grounding system,
challenged its successful-looking evaluation, and found that answer exposure, answer quality
and corpus improvement need separate measurements; a cheap copy baseline helps interpret
recall uplift; the deployment experiments show benefits and failures recall alone cannot show.
State the contribution as an empirical systems evaluation with a reusable diagnostic and
reporting procedure.

## 2. Corpus context (author clarified)
Developed to support VisionFlow and the author's research; almost entirely LLM-derived under
the author's direction; state actual review procedures, never imply per-assertion human
verification. Published at narrativegoldmine.com; ported from Logseq to Obsidian (visionGraph).
~7M words today; earlier public descriptions said ~4M. Keep the evaluated snapshot distinct
from the present corpus: report snapshot id/date and measured word count where established
(see notes/R6-corpus.md); otherwise omit the word count and keep page/class/triple counts.
Public availability does not establish memorisation; synthetic authorship does not establish
novelty or reliability.

## 3. Two misinterpretations to prevent first
A. "Below copy means not reasoning." Include early, in plain language: context says Project A
uses Module B and Module B requires Component C; answering which component the project needs
may require following both relations, yet Component C already appears in the context, so a
reasoned answer and a verbatim copy score the same. Negative gain does not prove absence of
reasoning; positive gain does not prove graph deduction. Required sentence: "The copy
comparison tests what a recall score establishes; it does not test whether reasoning occurred."
B. "Inconclusive controls show graphs and plain text are equivalent." The controls failed to
establish content specificity (missing completions, unequal sets, limited power); no matched
comparison against strong flat-text retrieval was run. Do not convert non-establishment into
equivalence, absence of benefit, or an investment recommendation. Keep the author's response:
grounding gave a large recall improvement and a separate model-judged quality improvement;
graphs may contribute via retrieval, organisation, versioning and governance as well as
inference; the study does not isolate each contribution's value or cost.

## 4. Required corrections
4.1 Copying scores higher on target-name recall, not "more accurate"; the copy is not evaluated
for concision, usability or whole-answer precision.
4.2 No "compression tax" as mechanism; selection, reformulation, alternatives, optional gold,
genuine omission and truncation may all contribute; not isolated.
4.3 Copy ceiling = recall a copy already achieves; not an upper bound; prefer "copy baseline"
in explanatory prose; define the limitation at first use; never read a low value as a need
for synthesis.
4.4 The baseline does not price user benefit, curation, reasoning, governance or model
substitution; a diagnostic once questions, gold and candidate contexts exist.
4.5 11,360 item-model observations = 1,136 instances x 10 models, dependent; 760 unexposed;
macro 0.964 vs item-pooled 0.933, never mixed.
4.6 Three unexposed positives all failed adjudication; 60 of 757 negatives sampled, none
hidden-correct, upper bound ~6% for that stratum; not a 0/760 semantic interval.
4.7 423 sampled item-model observations, not answers; 233/240 is the exposed-and-credited
stratum; label strata.
4.8 0.931 = lexical item-pooled exposed recovery; ~0.905 = frame-weighted model-judged
relational estimate; not two judges, not whole-answer accuracy.
4.9 Production quality was model-judged (second family as robustness); human sample not
annotated; never "human-judged".
4.10 Remove "reflect partial pretraining exposure"; the study did not establish memorisation
or contamination.
4.11 Suppression: targets in corpus but absent from the retrieved scaffold; 92/760 -> 3/760
lexical; 71 of 92 bare validated, 0 of 3 grounded; negatives not exhaustively judged;
policy-specific; causes not isolated.
4.12 0.87 is a recorded implementation acceptance floor; measured 0.816; no calibration
study; distinguish index acceptance from the absence-keyed per-query trigger (default off).
4.13 No refutation of OG-RAG or other systems.
4.14 Reasoner/CI checks are consistency and entailment, not factual validation; keep
build-pipeline logic distinct from model reasoning.

## 5. Findings to surface (check every value against artefacts)
5.1 Unaided ~0.265, grounded ~0.92, copy ~0.964, gains -0.067..-0.022: large uplift, small
shortfall on this metric; uplift and added model value are different questions. Avoid
"drops 2-7% of supplied facts"; keep utilisation and gain distinct.
5.2 Production: 114 pairs, +0.27 on 0-5 rubric [+0.11,+0.45], 24/8/82, second family ~+0.342;
+0.79 deep subset does not survive Holm; complete-case; no human judging.
5.3 Podcast: term resolution 0.17 -> 0.55, 346 -> 173 s/episode, fewer assertions (report
counts), no ASR artefact names in the grounded local arm; lexical alignment, not accuracy;
one podcast, ten episodes, one run.
5.4 Paraphrase: exposure 0.964 -> 0.328; absence trigger fires on 2 of 506; "returning
something is not returning enough"; diagnosis of the tested lexical path, not semantic
retrieval in general.
5.5 Omitted-target recovery: keep 12.1% -> 0.4% with 4.11 scope; motivates sufficiency,
supplementation and abstention experiments, not implemented.
5.6 Page integration: every arm negative, means -1.04 to -0.44 on the improvement scale
(separate from the 0-5 scale); integration disabled, landing separated from promotion; an
engineering decision, not proof the redesign works; the before/after judge caught it, not the
copy baseline.
5.7 Controls: served path beats no context; content-specificity contrasts do not survive
correction; 36-50% completion loss, selection bias, unequal sets, wrapper difference,
seed-disjoint placebo not answer-disjoint; non-significance is not no effect.
Plus (author decision): the five-arm rerun with persisted per-row placebo exposure, reported
as completed with its own accounting.

## 6. Instruments table near the results introduction
Columns: Question | Instrument | Positive observation | What remains unresolved. Rows: names
available? (lexical exposure accounting); relations asserted? (stratified model-judged item
audit); deployed path improves answers? (paired reference-based quality judgement);
extraction aligned with corpus? (term resolution, assertion count, latency); integration
improves the page? (blind before/after page judgement). No redundant second scorecard.

## 7. Structure
Abstract: system and attribution problem; copy reference in one sentence; uplift and
below-copy together; attribution is not a reasoning test or quality ranking; production
improvement; one or two failures; reporting recommendation with scope.
Introduction: the development objective (maintained, research-directed knowledge through a
replaceable model interface); the discovery sequence; the fair question and its limits; the
two-edge example; populations and instruments; bounded contribution; closest prior work.
Methods/results: actual pipeline (lexical-only retrieval as tested, upstream closure,
generative delegation); four counts beside the scalar; audit as validation of a limited
proxy; production result under a visible heading with the 0-5 scale; paraphrase, omitted
targets and write path as distinct operational studies; replication/controls/missingness
attached to the claims they qualify.
Discussion: what changes in an engineer's decisions; a generated answer can justify its cost
at lower lexical recall if it improves task success, clarity or time-to-answer, which need
direct measurement; the matched graph-vs-text comparison as the open question; PRD-028 as
proposed future work only.
Conclusion: benefits and attribution limits in one paragraph; concrete deployment lessons;
no generic verdict on graphs or reasoning.

## 8. Wording
Use the framing and value paragraphs from the brief (adapt). Remove: "copy rather than
reason", "delivery, not reasoning", "no reasoning is occurring", "retrieval without an LLM is
better", "the graph's value is only text delivery", "verified irrelevant", "private/unseen
corpus", "human-judged", "no valid unexposed recovery" (exhaustive), "pretraining
contamination", "compression tax", "controls show no benefit"/"plain text is equivalent",
"attributable"/"faithful"/"hallucination-free" without a measured outcome. Rewrite the
proposition, not the adjective.

## 9. Integrity
No evidence from discarded composition results; appendix stays a protocol. No pooling across
the sweep, production and podcast studies. Keep Qwen-2.5-72B sweep row distinct from the local
27B alias. Keep archive, tag, manifest and missing inputs consistent; label reconstructed
contexts. Name what "independent" means for a judge. No equivalence from a wide interval.

## 10. Author queries (do not guess): see notes/R6-corpus.md and AUTHOR-QUERIES.md.
## 11. Outputs: revised manuscript; change log mapping rewrites to misreadings; claims ledger;
unresolved author queries. (Summary D not produced.)
## 12. Acceptance test: title, abstract, captions and conclusion alone must convey: grounding
improved recall substantially; copying scored higher on a limited recall measure without
proving better answers; the control does not establish presence or absence of reasoning; a
separate production study found a model-judged quality benefit; graph-vs-flat-text is
untested by a matched comparison; public/synthetic provenance proves neither training
exposure nor novelty; useful operational failures alongside positives; different experiments
use different outcomes.
