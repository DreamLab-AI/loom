# Remediation of REVIEW-2026-09-21-external

Response to `REVIEW-2026-09-21-external.md` (first external review, of v7, 31 pp),
applied 21 September 2026 in paper-v8 (25 pp). Every reviewer item is dispositioned
below under the reviewer's own headings as **fixed** (manuscript edited), **verified**
(recomputed from released artefacts, the review's claim confirmed or resolved),
**deferred-as-owed-experiment** (the review is right and the answer needs a run we have
not made; the manuscript now names the owed experiment rather than claiming past it),
or **disagreed-with-reason**.

Verification passes that back this ledger: `notes/R1-factcheck.md` (line-anchored
fact-check of the review against v7), `notes/R2-priorwork.md` (prior-work
differentiation), `notes/R3-data.md` (recomputation from released rows).

## Headline: the contribution was made smaller, and two sections left the paper

The reviewer's summary judgement was that the empirical work is worth publishing and
the unifying argument is not yet sound. v8 accepts that. Three structural moves carry
most of the remediation:

- **Exposure accounting is the contribution; the ceiling is its scalar.** The paper no
  longer offers a test for whether reasoning occurs. §`sec:ceiling` now carries the
  reviewer's own A-requires-B-has-part-C counterexample and concedes the narrow
  statement: *this recall score does not establish reasoning beyond answer-name
  exposure.*
- **The routing extension left the paper.** It is now a separate 5 pp companion note
  (`docs/research/companion-routing/`), which states plainly that its quantity is
  ranker-relative and is not the exposure scalar.
- **The architecture section (v7 §18, D1–D7) was cut.** What survives is §`sec:roadmap`
  (two measured axes, two specified and unmeasured) and §`sec:companion`, which defers
  the architectural consequences and names the evidence they would need.

---

## "The largest conceptual problem is that answer-name exposure does not distinguish delivery from reasoning."

**Fixed.** The reviewer's three-relation counterexample is reproduced in the manuscript
and its conclusion accepted: zero gain is compatible with successful reasoning, negative
gain with reasoning that sometimes fails, positive gain with memorisation or matcher
asymmetry.

| v7 overreach the review named | v8 disposition |
|---|---|
| Figure 2 attributes uplift to exposure rather than reasoning | **Fixed.** `fig:ceiling`'s caption and the surrounding text now frame the panel as two compared *paths*, not as an attribution of cause. |
| §9 calls the ceiling a detector of serving-versus-synthesis regimes | **Fixed.** "Detector" survives in the manuscript exactly once, as a negation: the ceiling is "a delivery-recall reference, never as a reasoning detector" (§`sec:ceiling-prior`). |
| §18 says the instrument shows reasoning is not occurring | **Fixed** by deleting §18. No sentence of the form "reasoning is/was not occurring" remains; `grep` for *rules out*, *absence of reasoning*, *does not reason* returns nothing. |
| D7's broader claim about promises of reasoning over private data | **Fixed** by deleting §18. §`sec:companion` defers the architectural consequences outright. |

**On the word "ceiling".** **Disagreed-with-reason, with mitigation.** The reviewer
preferred *copy baseline* or *input-exposure reference*. We kept the term for continuity
with v2–v7 and the arXiv record, and instead pinned the interpretation at the point of
definition: the term is glossed in the subtitle of the paper as "an input-exposure
control", and §`sec:ceiling` now states that it "is a copy baseline or input-exposure
reference and *not* an upper bound", that "every later use should be read as 'the recall
a copy already achieves'", and that a model can exceed it and a perfect answer may sit
on it. Renaming mid-series would cost more than it buys; controlling the reading
throughout was the reviewer's stated fallback and is what we did.

## "The routing extension contains a formal change of metric that the paper presents as continuity."

**Fixed, by separation.** The reviewer's repair — "give the two quantities different
names and withdraw the claim that routing validates the same formal instrument" — is
adopted in its stronger form: the routing study is no longer in the paper.

- §`sec:accounting` states the general form of the objection before the routing case
  arises: a task with no natural no-op extractor needs a baseline *constructed* from a
  ranker, "and a constructed baseline is a different instrument from the no-op copy
  defined here".
- §`sec:companion` states it for the routing note specifically: the quantity "is
  ranker-relative and is a different instrument from the exposure scalar defined here…
  its sensitivity is a property of the baseline rather than evidence about the no-op
  ceiling."
- §`sec:limits` carries the portable rule: "Across papers, the counts of
  §`sec:accounting` are the figures to compare."
- The v7 claim that the scalar's seven-point movement was evidence about the *exposure*
  scalar's stability is withdrawn. The reviewer's reading — baseline sensitivity, not
  instability of the original scalar — is the one the companion note now states.

**Oracle threshold tuning.** **Fixed in the companion note**; it reports the tuning as
favourable within the chosen ranker family on this dataset and does not present the
tuned comparison as a deployment result. **Deferred-as-owed-experiment:** a held-out or
repeated-tuning resampling estimate.

**"Cannot introduce supervision".** **Fixed.** The companion note declares the
exclusion-clause segmentation as a choice made after examining results, rather than as a
procedure incapable of introducing benchmark adaptation.

**Arithmetic.** **Fixed and disclosed.** The +11.6 → +4.7 difference is 6.9 points, not
8.1. The companion note states the corrected cascade (11.6 → 9.3 → 4.7), gives it in
count terms (66 → 68 → 72 of 86 against a fixed 76), and discloses in the text that the
original draft misreported the difference as 8.1.

## "The central measure establishes target-name retention, not faithful delivery."

**Fixed (scoping) and deferred-as-owed-experiment (validation).**

- Every use of "delivery" is now scoped at first use: §`sec:intro` states that
  "'delivery' is scored as recall of exposed gold target names and nothing more:
  relational correctness, precision against fabrication and the accuracy of the
  attribution metadata are unmeasured here". "Vetted" appears nowhere in v8;
  "attributably" appears nowhere; "attribution" survives only as the name of an
  explicitly *open* axis (§`sec:roadmap`, Axis 4).
- The five failure modes the reviewer lists — wrong relation, reversed subject/object,
  explicit denial, added falsehood, unusably broad list — are enumerated verbatim in
  §`sec:method` under "What the scorer does not validate, and the audit that would".
- The error-cancellation claim is weakened as the reviewer requires: A2 now says the
  paired difference is "first-order insensitive to *shared* matcher error, but
  cancellation is approximate because error rates may differ between structured
  scaffold text and generated prose".
- **The human audit is accepted as central evidence, not an optional extension.** It is
  named as owed in §`sec:method`, in §`sec:limits` (first bullet) and in the conclusion.
  It has not been run. This is the single largest outstanding debt in the paper.

**Answer sets (T-COMMON any-of vs item-level counting).** **Fixed, with the rate
recomputed.** This is R3 finding 1. Recomputed from the released rows: 45 of 510
questions are `gold_type=="any"`, of which 17 are genuinely multi-alternative; 166 of
the 735 pooled `n10` items come from "any" questions, and **52 of those are compliant
omissions** — the model recovered a different accepted alternative and the paper's own
scorer credits the question in full. The published "one exposed item in fourteen"
(735/10,600 = 0.069) becomes **one in 15.5** (683/10,600 = 0.064) once they are excluded;
two models account for 24 of the 52. §`sec:tenmodel` now reports both figures and states
that the item-level rate is an **upper bound on an error rate rather than an error
rate**. §`sec:method` adds the reviewer's three-way distinction (required components,
interchangeable acceptable answers, optional additions) as outstanding work. Headline
recall is unaffected: the any-collapse already applies at question level.

## "The novelty is narrower than the manuscript's rhetoric suggests."

**Fixed.** §`sec:ceiling-prior` ("Relation to prior instruments") was rewritten to concede
precedent on each component rather than claim a new construct; see `notes/R2-priorwork.md`.

| Prior work | v8 concession |
|---|---|
| RAGChecker | "already performs the exposure/delivery decomposition"; ours is "the same move restricted to a deterministic surface matcher", traded for zero judge cost. We "claim no originality for the decomposition". |
| Recall Is Not Enough / reader–context diagnostic | Named "the closest comparator overall" and "the nearest neighbour to this instrument", with its graded lexical variant and intervention validation acknowledged. |
| Sufficient Context | Named as asking "a strictly stronger question"; its own ablation (93% vs 81% for the string-in-context baseline) is quoted, and our ceiling identified as "exactly that weaker string-in-context baseline, deliberately, to buy determinism". |
| MuSiQue | Our single-premise controls are "an evaluation-time application of MuSiQue's shortcut-resistance filters to a served pipeline rather than a new construction", restated again in §`sec:composition`'s "Lineage and scope". |

The reviewer's assessment table is adopted as the paper's own framing: the contribution
is "an inexpensive, deterministic, judge-free diagnostic and a reporting convention…
It is not priority over any one component, nor a general boundary between copying and
reasoning." **The trade-off is now measured rhetorically and named explicitly**:
"removing semantic judgement removes semantic discrimination". Measuring that trade-off
quantitatively is the owed human audit above.

## "The control experiments are valuable, but do not yet establish why the system helps."

The corrected interpretation stands and was strengthened. Item by item:

1. **Seed disjointness ≠ answer disjointness.** **Fixed (wording) + deferred (measurement).**
   The v7 description of the placebo as a correctly specified floor is withdrawn in both
   §`sec:ceiling-controls` and §`sec:controls`: "seed disjointness is a *construction
   guarantee*, not answer disjointness". **The requested measurement is not merely
   unmeasured but unrecoverable** (R3 finding 3): `control_harness.py` builds each arm's
   block in memory and persists only the model's answer, so neither the donor pairing nor
   the injected bytes survive the run; and the arcane/thin cohorts carry free-text
   reference answers rather than the {slug, title} gold items the matcher needs. The
   manuscript states this and names the two cheap fixes (persist the injected system text
   per row; itemise the free-text gold).
2. **Message placement.** **Verified — and the review's premise was right about v7 being
   wrong, but wrong about which way.** This is R3 finding 2. v7 claimed the live node
   injects into the last user message while controls use a system message. Checked
   against the code: **both** the served node and the control harness merge the block into
   a *system* message. v7's claim was factually incorrect and is corrected. The real
   envelope difference, now stated plainly in §`sec:controls`, is the **wrapper text**:
   the live path prepends an authority instruction, the controls fetch the bare block from
   the LLM-free `/loom/scaffold` endpoint and serve the same content bytes under a weaker
   instruction. The paper now states that the loom−true contrast confounds serving path
   with that missing sentence and that **no contrast isolates content from instruction
   strength**.
3. **"Uniform budget" is inaccurate.** **Fixed.** The phrase appears nowhere in v8. The
   rerun is described throughout as "a common retry policy", and §`sec:controls` and
   Table `tab:controls`'s caption both add the cost consequence the reviewer asked for:
   "a common retry policy, not a uniform realised budget: per-question final budgets vary
   within arms, so cost per arm is not matched".
4. **One clearly defined common cohort.** **Deferred-as-owed-experiment, and named as
   owed.** The reviewer is right that varying pairwise-complete sets prevent
   reconciliation. §`sec:controls` now states that the table "cannot be reconciled row by
   row", and names the analysis owed in the reviewer's own terms: "repeat every contrast
   on the single set of question IDs complete in all six arms, and report the exact IDs
   and n for each row". It has not been run, and cannot be until the rows are released.
5. **Causal controls and headline sweep are different studies.** **Fixed.**
   §`sec:controls` states it as a scope condition: "They are different studies; neither
   inherits the other's interpretation, and no causal reading established or left open
   here transfers to the sweep."

**On the reviewer's prioritisation** ("one tightly controlled experiment over further
heterogeneous arms"): **agreed and recorded.** No new arms were added in v8.

## "The suppression result deserves a more prominent, narrower treatment."

**Fixed, both halves.**

- **More prominent.** §`sec:suppression` is now promoted and opens by claiming its
  independence: "This is the one result in the paper that needs no ceiling to state."
  It appears in the abstract and as one of three stand-alone results in the conclusion.
- **Narrower.** The "roughly thirty times worse at anything outside the corpus" claim is
  **deleted**. A dedicated paragraph, "Exactly what the tested items are", restates the
  reviewer's scope verbatim: gold targets *from this corpus*, absent from *the particular
  retrieved scaffold*, under *this* authority instruction, retrieval policy and matcher —
  and states that "'Absent from the retrieved scaffold' is not 'outside the corpus', and
  neither is 'arbitrary out-of-domain knowledge'". The reviewer's point that obedience may
  be desirable is adopted: "the result is a cost to price rather than a defect to fix".
- **Verified.** 92/760 raw vs 3/760 grounded, seven models at zero, recomputed from the
  released sweep rows (R3 finding 1).
- **Deferred-as-owed-experiment, specified in full.** The reviewer's four-condition
  follow-up (authority instruction / incompleteness warning / labelled-supplement
  permission / neutral wrapper) and outcome-type scoring (omission, contradiction,
  abstention) are written into §`sec:suppression` as "The experiment that would explain
  it", and the paper commits: "It is the single experiment we would run next."

## "The composition study is promising, but 'composer' and 'anti-composer' overstate what the comparison identifies."

**Fixed.** The labels "composer" and "anti-composer" appear nowhere in v8.

- §`sec:composition` now says Δ "measures the effect of *adding route information under
  this context construction*, and not composition ability as such", and that a high
  `b-only` score "can reflect useful semantic inference, elimination among the candidates
  on offer, or broad enumeration, **none of which is well described as guessing**".
- Adding A's block is acknowledged to change "distractor competition and answer-selection
  pressure as well as supplying a missing premise".
- **The headroom caveat is generalised as the reviewer asked**: the deltas are read "as
  configuration-specific behaviour rather than as a capability ranking, since a model with
  a weak single-premise baseline has more room to improve", not only for GPT-5.6.
- **Multiplicity.** **Fixed.** The groups are labelled "three exploratory,
  configuration-specific behavioural groups, not a capability ladder and not model
  properties", with the non-adjustment across twelve models stated and the expectation of
  spurious boundary assignments made explicit.
- **Deferred-as-owed-experiment.** All four of the reviewer's decisive controls (swap
  edges so the answer changes; reverse relation direction at matched names and length;
  consistent renaming to unfamiliar identifiers; bounded answer set scored with precision)
  are written into the manuscript as named and unrun, with the reviewer's own standard
  quoted: "A model that follows a changed relation to the changed answer is evidence of
  composition in a way that a recall rise on adding a relevant block is not."

## "The architecture recommendations exceed both the evidence and the metric's operational capabilities."

**Fixed by deletion.** v7 §18 and D1–D7 are cut. Each specific objection is nonetheless
answered rather than merely removed:

1. **"Every served path reports its ceiling" needs an instrument.** Removed with §18. The
   ceiling is presented only as an offline benchmark measurement against a known gold set;
   §`sec:runnable` requires "a list of questions, each with a gold target set", which
   forecloses the live-request reading.
2. **D1 penalises faithful extraction.** **Fixed, and the direction reversed in the text.**
   §`sec:analysis` now states: "Note the direction: the gate is answer-completeness of the
   source, not positive gain over it. Requiring an extraction to *exceed* what its own
   source exposes would penalise faithful extraction, whose objective is precisely to
   recover what the source already says."
3. **Copy deficit is not an optimisation objective.** Removed with §18. No cross-system
   preference ordering on gain is claimed anywhere in v8; §`sec:analysis` calls the ceiling
   "a prioritisation heuristic that tells you where to look first, not a classifier".
4. **Value-concentrates-upstream needs economic evidence.** **Fixed.** Now stated as a
   hypothesis and disclaimed in the same breath: "We state that as a hypothesis only: this
   paper measures neither curation effort and maintenance against query volume, nor a
   quality-matched alternative serving path, so the economic allocation is outside its
   evidence."
5. **No matched test isolates the ontology's contribution.** **Deferred-as-owed-experiment
   and named.** §`sec:companion` states that the deferred architectural consequences "would
   need evidence this paper does not have: a matched comparison of the ontology scaffold
   against a direct graph query and flat-text retrieval at equal budgets". The reviewer's
   point that a direct graph-query baseline is especially important for graph-generated
   questions is accepted; it has not been run.

## "The statistical reporting shows care, but several uncertainty claims remain narrower than readers may realise."

| Gap | Disposition |
|---|---|
| Repeated observations (11,360 ≠ 11,360 independent facts) | **Fixed.** Every occurrence of the denominator now carries its structure: abstract, §`sec:intro`, §`sec:ceiling`, §`sec:rescore`, §`sec:tenmodel`, §`sec:limits`, conclusion. **Verified** in R3: 510 questions × 1,136 gold items × 10 models = 11,360, matching `decomposition.json` exactly. |
| Question and graph dependence | **Fixed.** §`sec:limits`: "fifteen domains and one corpus do not establish transfer beyond it." The domain-clustered bootstrap is presented as addressing dependence *within* this corpus only. |
| Mean vs rank estimands | **Fixed, and moved adjacent to the headline.** The abstract itself now says "exact signed-rank p=0.0023, a rank estimand, not a test of the mean"; §`sec:method` states the two are "complementary rather than interchangeable". |
| Ceiling-limited general questions | **Fixed.** §`sec:limits` and §`sec:ood` both state the 58/60 ties at means 5.0/4.95, the weak sensitivity, and that "the equivalence claim is confined to that instrument and that question set". |
| Oracle tuning in routing | **Deferred-as-owed-experiment**, in the companion note (above). |
| Model rankings under truncation | **Fixed.** §`sec:tenmodel`: "The 174 truncated GLM-4.6 rows are a substantial qualification on any reading of this table as a model ranking: the configuration is a valid fixed operating point, not a clean comparison of capability." **Verified** in R3 against `sweep-analysis.json` (174 truncated, 5 retried; Gemini 2.5 Flash-Lite 16). |
| Observed-effect power calculations | **Fixed.** §`sec:limits`: "Power figures quoted anywhere in this paper are computed against observed effects and are therefore post hoc; the next study should fix the smallest practically meaningful effect in advance and power for it." |

## "There are specific internal errors and contradictions that should be corrected before any further experiments."

| Location (v7) | Disposition in v8 |
|---|---|
| Abstract: literal `emphexposure`, `emphcopy`, `emphgain` in the rendered PDF | **Fixed and re-checked.** `pdftotext main.pdf` grepped for `emph[a-z]`, `textbf[a-z]`, `texttt[a-z]`, stray `\ref{`, `\cite`, `TODO` and `??` — zero hits. |
| §12: "+11.6 to +4.7 is 8.1 points" | **Fixed** in the companion note (6.9; cascade 11.6 → 9.3 → 4.7; the original misreport disclosed in the text). |
| §3 vs §§7, 9.1, 19: absolute recall called a lower bound | **Fixed.** The string "lower bound" appears **nowhere** in v8. A2 and §`sec:method` both say absolute recall "is not a bound in either direction". |
| §18 p. 23: promotion gate vs §10.1 judge ratings | **Fixed, and the attribution corrected further.** The contradiction is gone with §18. In addition, this review pass found that the surviving sentence credited the *copy-ceiling* gate with a catch the *judged* gate actually made; §`sec:pagejudge` now names which instrument did the work and states that the ceiling gate "did not, and could not". |
| §18 D1: reject-on-non-exceedance | **Fixed** (see architecture item 2). |
| §18 D2: "thirty times worse" | **Fixed.** The claim is deleted; "thirty" survives only in "thirty model×threshold combinations" (§`sec:rescore`). |
| §11.1 and §19: 0.964 conflated with 1060/1136 ≈ 0.933 | **Fixed in both directions.** §`sec:limits` states the two are "not interchangeable" and gives both. This review pass found one surviving conflation in §`sec:indomain` ("below the fraction of gold already exposed") and corrected it to "below the question-averaged exposed fraction", with the item-pooled 0.933 named in the same sentence. |
| §§3.3 and 11: stale references to the multihop design "in §19" | **Fixed.** All cross-references resolve to `\label{sec:composition}`. A full `\ref`/`\label` audit finds **no dangling references**, and `latexmk` reports no undefined references or citations. |

## "An adversarial reviewer's strongest objection would be this…"

The objection was played back against v8 sentence by sentence in this pass. It is now
substantially rebuttable, and the paper concedes the part that is not:

- *"…treats that result as evidence about reasoning"* — **conceded and withdrawn.** The
  paper's own statement is now "this recall score does not establish reasoning beyond
  answer-name exposure", and the conclusion adds that it "equally does not establish its
  absence".
- *"…evidence about fidelity"* — **partially conceded, and named as the owed validation.**
  Delivery is scoped to recall of exposed names everywhere; the human audit is owed.
- *"…evidence about architecture"* — **conceded and withdrawn** by deleting §18.
- *"When it encounters a task where exposure is uninformative, it replaces the reference
  with a ranker but preserves the name"* — **conceded and repaired**: the ranker-based
  quantity is in a different document, under a different name, declared a different
  instrument in three places.

## Reviewer's priority table

| Priority | Change | Disposition |
|---|---|---|
| Essential | Remove claims that the ceiling detects the absence of reasoning | **Fixed.** No such claim survives; the counterexample is in the paper. |
| Essential | Separate exposure accounting from the routing ranker baseline | **Fixed** by splitting the routing study into a companion note and declaring the quantities distinct. |
| Essential | Audit required vs alternative gold targets; validate semantic correctness on a human-reviewed sample | **Half fixed, half deferred-as-owed-experiment.** The gold-target audit was run and its result published (52 compliant omissions; 1-in-14 → 1-in-15.5). The human semantic validation has **not** been run and is named as owed in three places. |
| High | One matched content-intervention experiment with measured placebo exposure | **Deferred-as-owed-experiment.** Blocked on harness changes the paper names: the current rig cannot recover what any placebo exposed. |
| High | Compare against direct graph answers, constrained extraction and flat-text retrieval | **Deferred-as-owed-experiment**, named in §`sec:companion` as a precondition of the deferred architectural claims. |
| High | Replicate on independently authored questions and another corpus | **Deferred-as-owed-experiment.** §`sec:limits` states that fifteen domains and one corpus do not establish transfer. The routing companion is a second *domain* but is self-authored and single-seed, which it says of itself. |
| High | Release exact prompts, checkpoints, retries, row data and table-generation scripts together | **Partially fixed; the remainder disclosed.** See the next section. |
| Editorial | Shorten; move superseded results and architecture speculation out | **Fixed in part; declined in part.** 31 pp → 26 pp. §18 cut, routing split out, the historical control figure removed. The reviewer's further suggestion to move the *system narrative* out was **declined by the author** — see the decision below. |

## Evidence release: what is now tracked, what is absent

The reviewer's closing point — "Pin this version to a commit and publish a
table-to-artifact manifest" — is the item with the most movement and the most residue.

**Now under version control, for the first time** (commits `ce137f5` and `0acd8da`,
21 September 2026; 278 tracked files under `uplift-results/`):

- the frozen 510-question set, and the full ten-model sweep (results, scores, summaries,
  per-provider logs);
- the production-node paired study rows, both judge passes (`gpt-4.1` and the
  `claude-opus-4-6` third-family re-judge), the merged analysis and the exposure
  decomposition;
- the historical four-arm control cohort at the 1536-token budget;
- the study directories behind the judged arms (`general/`, `arcane/`, `arcane-prose/`,
  `thin-prose/`, `oracle/`, `quality/`);
- the routing-seam evidence (`uplift-results/routing/`: corpus, per-run reports,
  embedding caches, analysis scripts, manifest) and the rig snapshot
  (`tools/routing-eval/`).

**Deliberately excluded:** `uplift-results/ingest/` (a 52 MB corpus SQL dump and concept
records; not per-row evidence), and `app/data/` (the 7.9 MB scaffold index and the
ontology TTLs).

**Absent, and disclosed as reported-but-not-released.** The team's decision was to keep
these numbers with an unmissable disclosure rather than cut them. The disclosure now
appears in **four** places: the abstract, the §`sec:intro` release statement, §`sec:controls`
("Cohort, per-arm n, and what is released"), and a dedicated §`sec:limits` bullet,
"Release completeness".

| Absent artefact | Consequence for the reader |
|---|---|
| **Control rerun rows** — the 5-arm common-retry-policy cohort's per-row completions, its `gemini-3.1-pro-preview` judge outputs and its generating script. Never written to disk; `0acd8da`'s own commit message says so. | The entire ITT column of Table `tab:controls`, including the fluent-noise arm, rests on our reporting. Only the historical complete-case column is reproducible. The common-intersection sensitivity analysis the section says it owes cannot be checked until these land. |
| **Routing per-item rows** for the cloud judge (`jev-1.13.0`) and the 421M shared-head encoder. Their scripts called the backends live and never wrote per-item JSON. | Two of the three rows of the companion note's judge table are reported-but-not-released. The 4B engine's row and the repaired lexical/embedding ceilings **are** reproducible from `uplift-results/routing/`. |
| **Semantic re-score results and cache** — `rescore_results.json`, `embed_cache.json` and the imported `embed_lib.py` are all absent; only `rescore.py` and `RESCORE.md` are released. | §`sec:rescore`'s table is re-derivable only by re-running the embedding endpoint. Found in this pass and now disclosed in §`sec:intro` and §`sec:rescore`. |
| **Sweep generator** — `bench/bench_ontology_uplift.py` was deleted from the repository; `bench/sweep/analyse.py` still imports it. | The sweep rows are released as data without their generator. Found in this pass and now disclosed. |
| **Suppression figures** have no artefact file: `tools/paper/parametric_suppression.py` prints to stdout and writes nothing. | The numbers are recomputable from released rows by running the script, but no saved output is pinned. Recorded in `MANIFEST.md`. |

Two further honest notes. There is **no git tag** on this version; v8's release statement
therefore names the two commits directly rather than promising a tag. And v7's release
sentence claimed the semantic re-score's evidence was released; that claim was **false**
and is corrected in v8.

---

## Decisions taken against the review

Recorded here so that a later reader can tell a declined suggestion from an overlooked one.

### The system narrative stays in the paper

**Reviewer's suggestion:** "the system narrative arrives after a substantial series of
results… The write-path study could be a compact case study or a companion paper", and,
more broadly, shorten by moving system material out.

**Disposition: declined by the author**, on the point of the write path and the ecosystem.
The routing study *was* split out (a companion note) and §18's architecture speculation *was*
cut, so the review's substantive editorial items are met. What is declined is the further
step of thinning the system description itself.

**Reason.** The paper's contribution is a measured *production* system, not a metric in the
abstract. The exposure-accounting instrument is corpus-general and travels; the write-path
result does not. It is only checkable against the stack it was measured on, so removing the
pointers would make the one part of the paper that describes an intervention on a live
corpus unreproducible in exchange for roughly a page. Concretely, §`sec:lifecycle` was
restored as its own section, carrying the Event Sourcing / CQRS materialised-view framing
scoped to the assertion layer, the per-assertion fingerprinting that makes promotion
idempotent, and footnotes naming the VisionFlow ingest pipeline and the ontology-bridge
governed propose/approve path. §`sec:ecosystem` now names both components in prose and says
why. §`sec:casestudy` carries the ingest-pipeline footnote at the point the pipeline is first
described. `fig:lifecycle` and `fig:ecosystem` are both back.

**Cost, stated plainly.** The restoration took the manuscript from 24 pp to **26 pp**, two
pages above the post-review low-water mark and one page above the budget the team lead had
provisionally set. The two restored figures account for most of it. §`sec:lifecycle` closes
by saying what it is: "Nothing in this section is a measurement: it is the architecture the
two measured gates were used to choose." That sentence is the guard the review's objection
actually warrants — the section must not read as evidence — rather than deletion.

**Interim state this supersedes.** Between the first remediation pass and this decision the
lifecycle section had been cut and folded into §`sec:pagejudge` as a single paragraph, which
dropped all four provenance items. That fold is reverted. It is recorded here because the
24 pp figure quoted in the first draft of this ledger and in `CHANGES-2026-09-21-v8.md`
refers to that intermediate state.
