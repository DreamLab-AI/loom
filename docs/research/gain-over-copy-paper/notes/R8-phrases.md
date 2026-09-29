# R8: line-anchored misreading-phrase sweep

Scope: `gain-over-copy-paper.tex`, `assets/fig-ceiling.tex`, `assets/fig-ecosystem.tex`,
`assets/fig-lifecycle-source.html`, `docs/research/README.md`, top-level `README.md` (research
section, lines ~127–149 and the summary blurbs at lines 22, 32, 409). Cross-checked against
`BRIEF-2026-09-29-reframe.md` (§§3, 4, 8, 12), which independently names most of these same
misreadings and forbidden phrases — several hits below are literal instances of language the
brief already lists for removal.

All quotes are verbatim from the file at the stated line. Line numbers are `cat -n` line numbers
in the current working tree.

---

## A. Copy-not-reason / "no reasoning occurring" / uplift-is-delivery-not-reasoning

**2 hits**

1. **`README.md:141`** — *"The reading: the model adds **faithful delivery of the exposed facts,
   not reasoning over the injected structure** — exactly the product for private-knowledge
   grounding, where the answer is trustworthy because the curated source is."*
   This is close to verbatim the phrase the brief lists for removal ("delivery, not reasoning").
   It flatly asserts the negative (no reasoning) that the paper's own conclusion (tex:851)
   explicitly refuses to assert ("does not establish reasoning, and equally does not establish
   its absence"). Proposed replacement: *"The reading: on this metric the model's score is fully
   explained by restating names the context already showed it; the metric does not test whether
   reasoning occurred, and a full account of the paper (linked below) shows the diagnostic cuts
   both ways."*

2. **`gain-over-copy-paper.tex:179`** — *"A near-ceiling result shows the measured recall is
   achievable by delivery alone: the metric cannot credit recovery beyond exposure, and no
   reasoning is needed to explain the score."*
   Read alone (outside the paragraph's own next two sentences, which hedge it), "no reasoning is
   needed to explain the score" reads as "no reasoning occurred." Proposed replacement: *"...the
   metric cannot credit recovery beyond exposure, so a reasoning-free explanation of the score is
   always available; that does not mean it is the only one (§4.5 below)."*

Supporting (not separately counted): `gain-over-copy-paper.tex:62` ("Delivery of an authoritative,
human-curated source is the product") reinforces the A-reading when quoted outside its paragraph;
the paper's own guard against A is strong and repeated (tex:181, tex:851), so the risk is confined
to passages read in isolation from those guards — which is exactly how `README.md:141` and
`tex:179` circulate.

---

## B. Copying-is-better / retrieval-without-an-LLM-is-better

**2 hits**

1. **`gain-over-copy-paper.tex:240`** — *"positive gain over copy is not a prerequisite for
   useful generation, and a component that restates a curated source faithfully is doing its job
   at a gain of zero."* Read against §K/§L below, this normalises "the model is doing its job by
   copying," inviting B. Proposed replacement: *"...and a component that restates a curated
   source at a gain of zero has not been shown to be doing anything worse than one with positive
   gain — the scalar does not rank them on answer quality."*

2. **`README.md:145–147`** — *"**Static structured scaffold is the product.** ... `POST
   /loom/scaffold` is this, and it works with no model at all. **Prose adds nothing over
   structure** (+0.007 Muse / +0.000 Gemma)."* This is the strongest B-shaped passage in the
   docs: it states, without qualification, that the retrieval-only endpoint "is the product" and
   that generation ("prose") adds nothing — drawn from the 37-question local bench, not the paper
   under discussion (see §N). Proposed replacement: *"On the 37-question local bench, prepending
   generated prose to the structured scaffold added no measured recall over the structure alone
   (+0.007 Muse / +0.000 Gemma); this bears on prose-vs-structure in the prompt, not on whether an
   LLM call is needed at all — `/loom/scaffold` returns text, not an answer to the user's
   question."*

---

## C. Copy ceiling as hard upper bound / low value diagnoses need for synthesis

**2 hits** (the paper's own formal definition, tex:137, explicitly rules this out — the risk is in later restatements that drop the qualifier)

1. **`gain-over-copy-paper.tex:610`** — *"Where the ceiling is low, GraphRAG-style machinery
   (community summaries, multi-hop traversal, GNN soft-prompts) has room to earn its keep,
   provided the low ceiling is not simply retrieval or vocabulary failure."* The "provided"
   clause is doing all the load-bearing work but sits at the end of a long sentence; a reader
   quoting the first half gets exactly the forbidden reading. Proposed replacement: *"A low
   ceiling is uninformative on its own: §5 shows the same corpus's ceiling collapsing from 0.964
   to 0.328 purely from question phrasing, with no change in what synthesis could recover. Ruling
   out retrieval and vocabulary failure is a precondition for reading a low ceiling as evidence
   that composition is required, not a caveat on that reading."*

2. **`gain-over-copy-paper.tex:240`** — *"A ceiling near $0.5$ or below says the answer strings
   are largely missing, which may mean the task genuinely requires composition, or equally that
   retrieval failed..."* — same structure, composition-need listed first, alternatives after.
   Proposed replacement: reorder so retrieval/vocabulary/gold failure are listed before
   composition, since §5 (paraphrase) shows that is the dominant cause on this corpus.

---

## D. Baseline settles buy-a-model-vs-build-an-ontology / value concentrates at curation

**2 hits**

1. **`README.md:137`** — *"That is the whole bet of the swappable façade: **the scaffold carries
   the recall, not the model behind the door.**"* States as settled exactly the question the
   paper's own Table (`tab:bar`, tex:798–802) marks "Unmeasured": whether graph-structured
   delivery beats a flat-text or direct-query baseline at equal budget. Proposed replacement:
   *"...the scaffold, not the specific model, explains most of the measured recall difference in
   this bench; whether an ontology-structured scaffold beats a simpler retrieval baseline at
   equal budget is not tested here (open question, §research README)."*

2. **`gain-over-copy-paper.tex:612`** — *"it is natural to hypothesise that investment should
   favour upstream curation, whose cost is amortised rather than repaid per query. We state that
   as a hypothesis only..."* The hedge is present and correct here; flagged only because it is the
   nearest thing in the paper to the "value concentrates at curation" claim and is liable to be
   quoted without its own next sentence. No rewrite needed beyond ensuring any pull-quote in
   secondary material (README, blurbs) keeps "We state that as a hypothesis only" attached.

---

## E. 11,360-independent-trials / 0.964-vs-0.933 mixed / "0/760" semantic zero / exhaustive unexposed-recovery claim

**2 hits** (the paper is otherwise unusually careful here — see tex:62, tex:314, tex:381, tex:410,
tex:841, all of which correctly state "1,136 instances under ten models, not 11,360 independent
facts" and keep 0.964/0.933 apart)

1. **`README.md:141`** — cites *"the copy ceiling is 0.964"* with no companion mention of the
   item-pooled 0.933 anywhere in the top-level README, and no mention that 11,360/1,136 are
   dependent observations (that caveat exists only in `docs/research/README.md:24–25`, a
   different file). A reader who only opens the top-level README gets the macro figure with none
   of the guards the paper insists on. Proposed replacement: add the one-line caveat used in
   `docs/research/README.md:24–25` wherever 0.964 appears standalone.

2. **`gain-over-copy-paper.tex:482`** — *"Under injection the \emph{same} items are recovered at
   $\mathbf{0.4\%}$ (3 of 760), and seven of the ten models drop to \emph{exactly zero}."*
   "Seven of the ten models drop to exactly zero" is easy to misread as "the pooled result is
   zero," when the pooled result is 3/760, not 0/760 — exactly the semantic-zero conflation the
   brief names (4.6). Proposed replacement: *"...and on a per-model basis seven of the ten
   recover none of their 76 unexposed items; pooled across all ten, the total is 3 of 760, not
   zero."*

---

## F. "423 answers" / 0.905-vs-0.931-as-two-judges / relational accuracy read as whole-answer accuracy

**3 hits**

1. **`gain-over-copy-paper.tex:54`** (abstract) — *"Correcting the exposed-item rate for judged
   relational correctness gives an estimated 0.905 against 0.931 under this judge."* The phrase
   "under this judge" grammatically attaches to both numbers, implying both are judge outputs
   (i.e. two judge scores), when 0.931 is the deterministic lexical matcher's context-utilisation
   figure (Table 5, pooled) and only 0.905 is judge-corrected. `docs/research/README.md:29–30`
   already disambiguates this correctly ("estimated 0.905 against the matcher's 0.931"). Proposed
   replacement (abstract): *"...gives an estimated 0.905 against the lexical matcher's own 0.931."*

2. **`gain-over-copy-paper.tex:54`** (abstract) — *"A stratified audit of 423 of those
   observations, under a cross-family judge and a symmetric quote gate, puts the precision of the
   matcher's credit at 0.971..."* "Precision of the matcher's credit" is correctly scoped to
   relation-level assertion, but a skim reads 0.971 as "97% of the audited answers are fully
   correct." Proposed replacement: *"...puts the precision of the matcher's credit — whether a
   credited item actually names the target **in the relation the question asked for** — at
   0.971..."*

3. **`gain-over-copy-paper.tex:788`** (Table `tab:bar`, row 1) — *"stratified relational audit of
   423 observations under a cross-family judge"* — correctly says "observations," not "answers,"
   but the (question, model, gold-item)-triple definition given at tex:319 is not restated here,
   so a reader who enters at the table sees a bare count with no unit label. Proposed
   replacement: *"...stratified relational audit of 423 item-level observations (not 423
   answers; several observations can share one answer, §sec:audit)..."*

---

## G. Human-judged / human validation implied for model judgements; unqualified "independent adjudication"

**1 hit**

1. **`gain-over-copy-paper.tex:54`** (abstract) — *"A stratified audit of 423 of those
   observations, under a cross-family judge and a symmetric quote gate..."* — "judge" appears
   with no explicit "model, not a human" qualifier at first mention; that qualifier exists only
   in the body (tex:321, in bold: "The adjudicator is a model, not a human"), three pages later.
   A reader stopping at the abstract, or an abstract-only indexer, has no signal against reading
   "judge" as human. Proposed replacement: *"...under a cross-family **model** judge (not a human
   rater; §sec:audit) and a symmetric quote gate..."*

No hit found for an unqualified "independent adjudication" — the paper never uses that phrase; it
uses "cross-family" throughout, which is accurate but (per brief §9) should have "independent"
defined explicitly wherever used to mean "different model family from the candidate," not
"methodologically independent of this paper's authors."

---

## H. Pretraining contamination / memorisation asserted from public availability; "private"/"unseen" corpus

**4 hits** — this is the category with the clearest literal violations of the brief's own §4.10
and §8 forbidden-phrase list.

1. **`gain-over-copy-paper.tex:62`** — *"...so the non-trivial raw scores ($0.151$--$0.375$ across
   models) **reflect partial pretraining exposure** rather than a genuinely unseen domain..."*
   This is the exact phrase the brief (§4.10) requires removed: *"Remove 'reflect partial
   pretraining exposure'; the study did not establish memorisation or contamination."* The paper
   asserts a causal mechanism (pretraining exposure) for the raw scores that it never measures.
   Proposed replacement: *"...so the non-trivial raw scores ($0.151$--$0.375$ across models) are
   not evidence that the domain is unseen by these models; the study does not test what produces
   them, whether prior exposure to the public corpus, related public knowledge, or partial
   in-context inference from the question alone."*

2. **`README.md:22`** — *"...every answer is grounded in your curated, reasoner-checked **private
   corpus**..."* Unqualified "private" in the top-level pitch, directly upstream of the paper's
   own correction (tex:62) that "private" denotes deployment pattern, not secrecy, and that the
   corpus is public on GitHub. Proposed replacement: *"...your curated, reasoner-checked corpus,
   served from a LAN-confined deployment with no cloud egress (the corpus itself is published,
   not secret — see the paper's exposure-accounting note on what 'private' means here)."*

3. **`README.md:141`** — cites the paper's old title, *"An Input-Exposure Control for Ontology
   Grounded Generation over **Private Corpora**"* — this is a stale title (the current tex title,
   line 44–45, reads "...over Curated Corpora", already corrected away from "Private"). Keeping
   the old title in the top-level README re-introduces the exact "private corpus" framing the
   current paper retracted. Proposed replacement: update the citation to the current title,
   "...over Curated Corpora."

4. **`README.md:32`** — *"**What it is *for*: making swappable models performant against large,
   important, private customer datasets**..."* Lower severity than (2)/(3) since "private customer
   datasets" describes a target deployment scenario rather than this corpus, but sits in the same
   paragraph as (2) and compounds the private/unseen framing. No rewrite required if (2) is fixed
   and the two are read together, but flagged for completeness.

---

## I. Suppression result generalised beyond scaffold-omitted targets under the tested wrapper

**1 hit** (the tex itself is carefully scoped — tex:482's "Exactly what the tested items are"
paragraph and tex:853's conclusion both attach "under this authority instruction and retrieval
policy" — the drop happens in the summary)

1. **`docs/research/README.md:31–32`** — *"...on gold targets the corpus contains but the
   retrieved scaffold did not expose, recovery falls from 0.121 bare to 0.004 grounded."* This
   sentence omits the qualifier present in the paper's own conclusion (tex:853: "under this
   authority instruction and retrieval policy") and in the dedicated caveat paragraph (tex:482).
   Without it, the sentence reads as a general property of "grounding" rather than of this
   specific wrapper/gate/matcher combination. Proposed replacement: *"...recovery falls from 0.121
   bare to 0.004 grounded, under the authority-instruction wrapper and confidence-gated retrieval
   policy tested here (not shown to generalise to other prompting or gating designs)."*

---

## J. 0.87 floor presented as calibrated/validated

**3 hits** — no location in the tex, either figure, or either README ever states how 0.87 was
derived; it is cited only as a bare threshold, which the brief (§4.12) explicitly flags: *"0.87
is a recorded implementation acceptance floor; measured 0.816; no calibration study."*

1. **`assets/fig-ecosystem.tex:35`** — *"HNSW semantic fallback\\recall 0.816 $<$ 0.87 floor
   $\cdot$ \textbf{default-off}"* — presented on the diagram as a plain inequality test, with no
   annotation that 0.87 is an uncalibrated implementation choice rather than a validated
   acceptance criterion.

2. **`assets/fig-ecosystem.tex:58`** (caption) — *"...its document-embedding recall (0.816) is
   below the 0.87 design floor, a red gate we report rather than hide..."* "Design floor" reads as
   an engineered, validated threshold; nothing in the caption or the main text (tex:837, the only
   other mention) states its provenance.

3. **`README.md:409`** — *"Recall gate RED: `rgb-protocol 0.816`, below `0.87` design floor."*
   Same issue, repeated in the capability table with no caveat.

Proposed replacement (all three): append *"(0.87 is a recorded implementation acceptance
threshold, not a value derived from a calibration study — see limitations)"* at first use in each
document, and add the missing provenance/caveat to `gain-over-copy-paper.tex:837` itself, since
that is the paper's only other mention and currently states the floor with no derivation either.

---

## K. Controls-show-no-benefit / plain-text-equivalent / graph-value-is-only-text-delivery

**1 hit** (the paper's explicit conclusion at tex:526, "whether the scaffold's specific content
matters beyond a well-formed on-corpus block is not established at this power," correctly states
non-establishment rather than equivalence — the risk is one adjacent sentence)

1. **`gain-over-copy-paper.tex:587`** — *"The graded facts are therefore not shown to carry the
   effect beyond what a well-formed, on-corpus block supplies, which is the shape of the noise
   confound~\textcite{cuconasu2024poweofnoise} warn of."* "Not shown to carry the effect beyond
   [a generic block]" is one clause away from "the graph's value is only text delivery" — the
   exact forbidden phrase in the brief (§8). Proposed replacement: *"The graded facts do not
   establish that the scaffold's specific content carries the effect beyond what a well-formed,
   on-corpus block supplies; this is non-establishment under an underpowered cohort (§sec:controls),
   not evidence that content is interchangeable, and matches the shape of the noise confound
   ~\textcite{cuconasu2024poweofnoise} warn readers to control for."*

---

## L. "Compression tax"/omission-mechanism asserted; "attributable"/"faithful"/"hallucination-free"/"verified irrelevant" without a measured outcome

**3 hits.** No literal occurrence of "compression tax," "hallucination-free," or "verified
irrelevant" was found in any of the swept files (confirmed by grep) — those specific phrases are
clean. The hits below are the "faithful"/"attributable"/"verifiably" family the brief also names.

1. **`gain-over-copy-paper.tex:240`** — *"...a component that restates a curated source
   **faithfully** is doing its job at a gain of zero."* "Faithfully" asserts the very property
   (precision against fabrication) that tex:62 and tex:831 both say is explicitly unmeasured.
   Proposed replacement: *"...a component that restates a curated source at a gain of zero has
   not thereby been shown to fabricate anything, but this metric does not check that either — it
   is doing the one job it measures."*

2. **`README.md:22`** — *"Loom makes any LLM answer from it, **verifiably**."* No verification
   procedure is described anywhere in the linked evidence; the paper's own Table `tab:bar` (row 4,
   tex:803–807) states attribution accuracy is "Unmeasured" and precision against fabrication is
   "not scored." "Verifiably" is a stronger, unqualified claim contradicted by the paper it links
   to. Proposed replacement: *"Loom makes any LLM answer from it, **with every answer tagged to
   the corpus generation it was served from**"* (traceable, not verified — matches tex:415's own
   phrasing: "that provenance attests traceable generation, not human authorship").

3. **`README.md:32`** — *"...answering accurately and **attributably** on an in-domain corpus..."*
   Same issue: "attributably" asserts the unmeasured property from Table `tab:bar` row 4.
   Proposed replacement: *"...answering **with source-generation metadata attached** on an
   in-domain corpus (attribution accuracy itself is not yet measured, §research README)."*

---

## M. OWL closure/reasoner presented as validating factual truth; build-pipeline logic conflated with model reasoning

**1 hit**

1. **`README.md:28`** — *"Three verbs describe what it exists to give an operator — **ground** an
   LLM's answers in **checked formal semantics** rather than parametric guesses..."* combined
   with line 30's *"The reasoner (Whelk EL++) checks it at build time; the model restates it at
   query time."* Whelk checks logical *consistency* of the ontology (no contradictions) at build
   time — it does not check, and cannot check, the *factual* correctness of curated prose, which
   `README.md:415` itself admits is "AI-generated synthetic content produced under human
   direction... not an authoritative encyclopaedia." "Checked formal semantics" juxtaposed with
   "ground an LLM's answers" invites reading answer-level factual grounding where only
   build-time logical consistency was checked. Proposed replacement: *"...ground an LLM's answers
   in a corpus whose ontology structure is checked for logical consistency at build time (not for
   factual accuracy of its content, which remains human-authored/AI-drafted and unverified per
   answer) rather than parametric guesses."*

---

## N. Sweep/production/podcast studies pooled as one; Qwen-2.5-72B sweep row conflated with the local 27B deployment

**4 hits.** The brief itself names this exact risk (§9: "No pooling across the sweep, production
and podcast studies. Keep Qwen-2.5-72B sweep row distinct from the local 27B alias.") The tex body
keeps the three studies apart by section (§Method labels "Study 1"/"Study 2"/§casestudy
separately, and the sweep table (tex:399) and casestudy table (tex:648–649) never share a row) —
the violations are in the summarising documents and one internal tex sentence.

1. **`gain-over-copy-paper.tex:97`** — *"...delegation to the local model through an
   OpenAI-compatible fa\c{c}ade, currently Qwen3.8-27B on two workstation GPUs. **This is the
   subject of every measurement below.**"* The ten-model sweep (Study 1, §sec:method) measures
   nine *other* API models plus Qwen-2.5-72B against an *offline* scaffold harness — none of that
   study runs through the live Loom node or Qwen3.8-27B at all. As written, this sentence claims
   Qwen3.8-27B (the served node's model) is "the subject" of measurements that in fact concern a
   different, offline harness and different models. Proposed replacement: *"...currently
   Qwen3.8-27B on two workstation GPUs. **The served node itself, and its Qwen3.8-27B backend, are
   the subject of Study 2 and the podcast case study below (§sec:live, §sec:casestudy); the
   ten-model sweep of §sec:tenmodel instead measures the offline scaffold engine against nine
   other API models.**"*

2. **`README.md:129–139`** — the three-model uplift table (Gemma-4-31B, Muse-Glimmer-30B, Gemini
   3.7 Flash) mixes a 37-question local bench (Gemma, Muse) with a 510-question run (Gemini 3.7
   Flash) in one table, with the distinction relegated to a dagger footnote most readers of a
   scannable table will not read before drawing the "~0.94 across three models" conclusion in the
   prose above the table (line 129: *"static ontology scaffolding is a **decisive,
   model-agnostic win**"*). Proposed replacement: split into two tables (37-question local bench;
   510-question single-model run), each headed with its own $n$ and protocol, and move the
   "model-agnostic" claim to scope explicitly to the 37-question bench only.

3. **`README.md:141`** — *"...across ten models from **five providers** the gain over copy is
   uniformly negative..."* The paper's own count (`gain-over-copy-paper.tex:298`) is *"ten models
   from **eight** developer families (Google, Anthropic, OpenAI, Zhipu, DeepSeek, Meta, Alibaba,
   Mistral)."* This is a plain numeric discrepancy (5 vs 8), and it sits in the same sentence as
   the Study-1 findings quoted from a study the reader has not been told is distinct from the
   table just above it (finding 2). Proposed replacement: correct "five providers" to "eight
   developer families," matching the paper.

4. **`README.md:146`** — *"**Prose adds nothing over structure** (+0.007 Muse / +0.000 Gemma).
   Loom ships prose off the default path — it costs budget for no recall."* This is drawn from the
   37-question local bench (finding 2 above), not from the ten-model sweep or the copy-ceiling
   paper just cited two paragraphs earlier (line 141); presented under the shared heading "Three
   findings shaped Loom's defaults" with no per-finding attribution to which study it comes from.
   Proposed replacement: label each of the three numbered findings (lines 145–147) with the study
   it is drawn from (37-question local bench vs. the copy-ceiling paper).

---

## O. OG-RAG or other prior systems treated as refuted

**0 hits.** No occurrence of "OG-RAG" was found in any swept file. The Related Work section
(§sec:related, tex:75–89) and §sec:ceiling-prior (tex:195–196) are explicit and consistent that
the paper claims *narrower* ground than each of RAGChecker, the reader–context diagnostic and
Sufficient Context, not refutation: *"Against the three closest instruments we claim narrower
ground than a new construct"* (tex:89); *"restricting it to a deterministic surface matcher buys
zero judge cost and exact reproducibility, and costs semantic discrimination"* (tex:196). Works
marked "withdrawn" (`copyasdecode2026`, footnote at tex:60; `answerpresence2026`, tex:85) are
reported as withdrawn by their own authors, not refuted by this paper, and the paper is careful to
say it adopts only their method, not their claims. This category is clean; no rewrite proposed.

---

## Hit counts by category

| Category | Hits |
|---|---|
| A — copy-not-reason | 2 |
| B — copying-is-better | 2 |
| C — ceiling-as-hard-bound | 2 |
| D — buy-model-vs-build-ontology | 2 |
| E — 11,360/0.964/0.933/zero conflation | 2 |
| F — 423-answers / two-judges / relational-as-whole-answer | 3 |
| G — human-judged implied | 1 |
| H — pretraining contamination / private corpus | 4 |
| I — suppression over-generalised | 1 |
| J — 0.87 floor as calibrated | 3 |
| K — controls-show-no-benefit | 1 |
| L — faithful/attributable/verifiably unmeasured | 3 |
| M — reasoner validates truth | 1 |
| N — studies pooled / model rows conflated | 4 |
| O — prior systems refuted | 0 |
| **Total** | **31** |

Concentration: `README.md` (top-level) carries roughly half of all hits (lines 22, 28, 32, 137,
141, 145–147, 409) — it is written as a standalone pitch and reproduces several claims the paper
itself now hedges or retracts (stale title at 141, "private corpus" at 22, "verifiably" at 22,
pooled tables at 129–139). `docs/research/README.md` is markedly cleaner (1 hit, line 31–32) —
it already carries most of the guards the top-level README lacks. Within the paper itself, hits
cluster in three places that get quoted out of context: the abstract (tex:54, 3 separate hits
across F/G), §sec:analysis "Consequence 1/2" (tex:610, 612), and the README-facing "Reading the
number" paragraph (tex:240, 3 hits across B/C/L).

---

## Acceptance-test verdict (brief §12)

The eight things title + abstract + every `\caption` + conclusion must convey when read *alone*,
against what is actually there:

1. **Grounding improved recall substantially.** ✅ Conveyed — abstract ("unaided recall averages
   0.26 and grounded recall 0.92"), conclusion ("A large raw-to-grounded uplift...").
2. **Copying scored higher on a limited recall measure without proving better answers.**
   ⚠️ Partial — abstract and `fig:gain`'s caption both state gain is negative, but neither states
   the "without proving better answers" half explicitly; it has to be inferred from A2/A3-style
   phrasing elsewhere in the body (tex:154, "A recall reference point, not an endorsement").
3. **The control does not establish presence or absence of reasoning.** ⚠️ Partial — the
   conclusion states this explicitly and well (tex:851: "does not establish reasoning, and
   equally does not establish its absence"), but the abstract does not, and none of the ten
   `\caption`s state it in the brief's required wording ("The copy comparison tests what a recall
   score establishes; it does not test whether reasoning occurred" appears nowhere in the tex).
   `fig:gain`'s caption comes closest ("the scale says nothing about the precision or relational
   correctness of what each model returns") but frames it as a precision gap, not a reasoning gap.
4. **A separate production study found a model-judged quality benefit.** ✅ Conveyed — abstract
   ("A paired production study lifts judged quality by +0.27 pooled"), conclusion, and `tab:live`'s
   caption all state it, with "judged" consistently attached.
5. **Graph-vs-flat-text is untested by a matched comparison.** ❌ Missing from title, abstract,
   every caption and the conclusion. It exists only inside a table *row* (`tab:bar`, row 3,
   tex:798–802: "Does it beat competitive retrieval baselines? ... Unmeasured") and in the
   companion-work section (tex:827) — neither is a caption, abstract sentence or conclusion
   sentence, so a reader who reads only the required elements never encounters this.
6. **Public/synthetic provenance proves neither training exposure nor novelty.** ❌ Missing, and
   actively contradicted in the body: tex:62 currently *asserts* training exposure ("reflect
   partial pretraining exposure") rather than declining to establish it (§H above). Not present in
   abstract, any caption, or the conclusion.
7. **Useful operational failures alongside positives.** ⚠️ Partial — the conclusion states the
   paraphrase collapse (0.964→0.328) and the suppression finding (0.121→0.004) as failures
   alongside the positives, but omits the single most dramatic operational failure in the paper:
   the write-path page-integration result ("every arm degrades judged page quality," tex:684, and
   the resulting "integration phase was disabled," tex:695) is absent from title, abstract, every
   caption and the conclusion, though it drives an entire section (§sec:pagejudge) and an
   architectural change (§sec:lifecycle).
8. **Different experiments use different outcomes.** ❌ Missing as an explicit statement anywhere
   in title/abstract/captions/conclusion. The abstract moves from sweep numbers to audit numbers
   to paraphrase numbers to the production study to the controls in sequence, correctly keeping
   each study's own numbers within its own clause, but never states the meta-point that these are
   different instruments on different cohorts that should not be pooled or compared cell-for-cell
   — a statement the brief (§9) requires and the tex body only makes locally, per-study (e.g.
   tex:532, "These controls are... a different cohort. They are different studies; neither
   inherits the other's interpretation" — true of §sec:controls alone, not restated as a global
   reading rule).

**Summary: 2 of 8 fully met (1, 4); 3 of 8 partially met (2, 3, 7); 3 of 8 missing entirely from
the required elements (5, 6, 8).** Item 6 is not just missing but currently contradicted by
tex:62's "pretraining exposure" sentence (§H, hit 1) — that is the single highest-priority fix,
since it is both a missing acceptance-test item and an active violation of the brief's own §4.10.

### Verbatim required elements, for reference

**Title** (`gain-over-copy-paper.tex:44–45`): *"The Copy Ceiling: An Input-Exposure Control for
Ontology-Grounded Generation over Curated Corpora"*

**Abstract** (`gain-over-copy-paper.tex:54`): full text quoted under §F above, hits 1–2.

**`\caption`s**, in document order: `tab:pipeline` (112–114); `fig:ceiling` (`assets/fig-ceiling.tex:45–57`);
`fig:ecosystem` (`assets/fig-ecosystem.tex:48–59`); `tab:audit` (336–345); `tab:sweep` (384–387);
`tab:decomp` (408–413); `fig:gain` (468–477); `tab:live` (496–500); `tab:controls` (534–545);
`tab:casestudy` (636–641); `fig:lifecycle` (740–746); `tab:bar` (778–780). None of these twelve
captions individually states acceptance-test items 5, 6 or 8; item 3 is stated only partially, in
`fig:gain`'s caption.

**Conclusion** (`gain-over-copy-paper.tex:848–857`): full text quoted/paraphrased under the
acceptance-test items above.
