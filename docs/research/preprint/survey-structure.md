# Structural & Rhetorical Survey for "Private Knowledge, Faithfully Served"

**Purpose.** Guidance for the v2 rewrite. Twelve closely-comparable arXiv papers, dissected for
section skeleton, how the Introduction narrates requirements→design→contribution, where each places
its "differentiating factor" sentence, how it defends a *singular* contribution statistically, and
one structural move worth stealing. Then: a recommended skeleton for our paper, three ranked
framings for our singular contribution, and a verdict on our current structure.

Our paper in one line: *we serve a curated, human-vetted ontology into a swappable model's context;
we introduce a per-item **copy ceiling** (the recall a no-op extractor of the injected context would
score) and show that the large grounding uplift is, to measurement, faithful extraction rather than
reasoning over structure; and we show a confidence gate prevents out-of-domain regression.*

The paper is therefore **primarily a measurement-methodology paper** (the copy ceiling), secondarily
a **rigorously-measured negative result** (uplift ≈ copy), wrapped around a system (the Loom
scaffold). That identity dictates the structural choices below.

---

## Part 1 — The comparable papers

Grouped into the four clusters from the brief. ★ = section skeleton verified from the paper's own
HTML; ○ = contribution/structure established from abstract + secondary sources.

### Cluster A — KG-RAG / GraphRAG / ontology-grounded generation

**A1. From Local to Global: A GraphRAG Approach to Query-Focused Summarization** — arXiv **2404.16130**
(Edge et al., Microsoft, 2024). ★
- *Contribution:* graph-index + community-summary pipeline enabling *global* sensemaking over private
  corpora; beats vector-RAG on comprehensiveness/diversity.
- *Skeleton:* 1 Introduction · 2 Background (2.1 RAG systems, 2.2 KGs+LLMs, 2.3 Adaptive benchmarking,
  2.4 RAG evaluation criteria) · 3 Methods (3.1 workflow as a chain of `X → Y` transforms, 3.2
  question generation, 3.3 **criteria for evaluating global sensemaking**) · 4 Evaluation · Discussion
  · Conclusion.
- *Intro narration:* problem (RAG fails on global questions) → why prior QFS doesn't scale → "to
  combine the strengths… we propose GraphRAG" → single differentiating sentence: *"The GraphRAG
  method and its ability to perform global sensemaking… form the main contribution of this work."*
  Placed as its own standalone sentence mid-intro, not buried in a list.
- *Statistical defence:* head-to-head LLM-as-judge with win-rates; no CIs, but validated with a
  claim-based statistic.
- **Steal (high value):** the **"control criterion" Directness** — they deliberately add a fourth
  judging criterion *they expect no method to win*, purely as a sanity reference against which the
  real criteria are read. This is the exact rhetorical role of our **copy ceiling**: a reference the
  reader uses to calibrate the headline number. Frame the copy ceiling explicitly as a *control*, the
  way GraphRAG frames Directness. Also steal: motivation sentence "answer questions over **private
  and/or previously unseen** document collections" — near-identical to our setting; cite for the
  private-corpus requirement.

**A2. Graph Retrieval-Augmented Generation: A Survey** — arXiv **2408.08921** (Peng et al., 2024). ○
Canonical GraphRAG survey; cite as the field-defining reference and to locate our schema-level
injection as under-covered.

**A3. Retrieval-Augmented Generation with Graphs (GraphRAG)** — arXiv **2501.00309** (2025). ○
Broad taxonomy of graph-structured RAG; use to support the claim that published KG-RAG injects
*instance* triples, not *schema-level* class/relation/taxonomy content.

**A4. A Survey of GraphRAG for Customized/Domain Applications** — arXiv **2501.13958** (2025). ○
Domain-specialisation survey; supports the "curated domain corpus" framing.

**A5. Towards Practical GraphRAG: Efficient KG Construction… in Enterprise Environments** —
arXiv **2507.03226** (2025). ○
- *Contribution:* cost-efficient GraphRAG deployment for enterprise, reducing LLM-extraction and
  traversal cost. Closest paper to our *enterprise/on-prem/amortised-cost* requirement.
- *Steal:* its Introduction leads with **deployment constraints** (cost, adoption barriers) before
  method — the requirements-first arc our brief wants. Cite for "curation cost amortised once."

**A6. KAPING — Knowledge-Augmented language model Prompting for zero-shot KGQA** (Baek et al., 2023;
already in our refs as `baek2023kaping`). ○ Injects KG *triples* into the prompt zero-shot — the
instance-level foil to our schema-level injection.

*(Also in-family and already cited: LightRAG `guo2024lightrag`; the unifying roadmap
`pan2024unifying`. G-Retriever, arXiv 2402.07630, textual-graph QA, is worth adding as a KG-grounded
generation comparator.)*

### Cluster B — Baseline / control that *reframes* prior results (null-result methodology)

**B1. Stronger Baselines for Retrieval-Augmented Generation with Long-Context LMs** —
arXiv **2506.03989** (Laitenberger et al., 2025). ★ **The single best structural template for us.**
- *Contribution:* a *simple* baseline (DOS RAG) matches/beats elaborate multi-stage pipelines →
  reframes the value of pipeline complexity. A pure "the baseline was the story" paper.
- *Skeleton:* 1 Introduction · 2 Experimental Setup (2.1 Benchmarks, 2.2 the complex methods, 2.3
  **Baselines** — the control defined here, prominently) · 3 Results · 4 Analysis ("**Why is DOS RAG
  effective?**" — four numbered mechanisms, each tied to an ablation) · 5 Related Work · 6 Conclusion
  (restates the four mechanisms) · Limitations.
- *Intro narration:* poses a *question* ("do complex pipelines still offer measurable benefits over
  simpler approaches?") → controlled evaluation under **matched token budgets** → differentiating
  sentence is a *recommendation*: *"We recommend establishing DOS RAG as a simple yet strong baseline
  for future RAG evaluations."* The contribution is a normative methodological recommendation, not a
  system. Mirror this: *"grounding evaluations that inject graph-derived gold should report a copy
  ceiling as standard"* — that is already our Conclusion's last line; **promote it to the Intro.**
- *Statistical defence:* mean ± SD over **five runs** per point; controlled token-budget matching so
  the comparison is apples-to-apples. Note: they solve the single-sample problem we have as a
  limitation — *five runs per condition*. Worth emulating even partially.
- **Steal:** the *matched-budget controlled comparison* framing and the "Analysis: why does the
  simple thing win" section that pre-empts the reader's "surely the complex method must help"
  objection. Our analogue: "Analysis: why does recall track the copy ceiling" (exposure, faithful
  extraction, no reasoning-over-structure).

**B2. Retrieval Augmented Generation or Long-Context LLMs? A Comprehensive Study and Hybrid
Approach** — arXiv **2407.16833** (Li et al., Google, EMNLP-Industry 2024). ★
- *Contribution:* systematic RAG-vs-long-context comparison overturning prior "RAG wins"
  (Xu et al.); then Self-Route (a confidence/self-reflection router). **Directly parallels our
  confidence-gated harness.**
- *Skeleton:* 1 Introduction · 2 Related Work · 3 Benchmarking RAG vs LC (3.1 datasets/metrics, 3.2
  models/retrievers, 3.3 results) · 4 Self-Route (4.1 motivation, 4.2 method, 4.3 results) · 5
  Analysis (failure patterns; cost/perf trade-off) · Conclusion.
- *Intro narration:* prior work said X → "Different from findings in previous work… we find that LC
  consistently outperforms RAG" — the differentiating sentence is an *explicit contradiction of a
  named prior result*, placed early. Then a second contribution (Self-Route) motivated by an
  *observation in the data* (63% of predictions identical).
- *Statistical defence:* per-dataset scores across 3 models + a **prediction-agreement distribution**
  (the 63%-identical histogram) to justify routing. Also explicitly flags **data leakage**: models
  emit gold words absent from context, mitigated by "answer based only on the provided passage." (We
  should cite this as prior acknowledgement of the exact exposure problem the copy ceiling
  formalises.)
- **Steal:** the two-part structure "measurement that reframes → mechanism the measurement reveals →
  a routing method that exploits it." Their **Self-Route = confidence-gated routing** is the nearest
  published cousin of our gate; cite it as the design precedent for gated injection and contrast
  (they route RAG-vs-LC; we gate inject-vs-abstain).

**B3. Long Context vs. RAG for LLMs: An Evaluation and Revisits** — arXiv **2501.01880** (2024). ○
Revisits the same question; notably **filters out questions answerable without external context** —
a methodological control kin to ours (isolating where grounding is actually load-bearing). Cite in
Method to justify our raw-arm as the "is the domain genuinely private" control.

**B4. In Defense of RAG in the Era of Long-Context LLMs (OP-RAG)** — arXiv **2409.01666** (2024). ○
Order-preserving RAG; a rebuttal-shaped paper. Cite to show the sub-field argues by installing better
controls/baselines — the genre our paper belongs to.

### Cluster A′ — The methodological ancestors: input-only baselines & shortcut learning

**C1. How Much Reading Does Reading Comprehension Require?** — arXiv **1808.04926**
(Kaushik & Lipton, 2018). ★ **The intellectual root of the copy ceiling.** (Already cited as
`kaushik2018reading` — elevate its prominence.)
- *Contribution:* question-only / passage-only baselines reveal that much apparent RC competence is
  shortcut exploitation of what the input already exposes.
- *Skeleton:* 1 Introduction · 2 Datasets (+ "Generating Corrupt Data" — how they null out an input
  channel) · 3 Models · 4 Experimental Results (per-dataset) · 5 Discussion — **entirely normative**:
  "Provide rigorous RC baselines", "Test that full context is essential", "Caution with cloze-style
  datasets", "A note on publishing incentives."
- *Statistical defence:* Δ(min) = full − max(Q-only, P-only), reported per task; the *signed gap* to
  the input-only baseline is exactly our *gain over copy*. Our copy ceiling is the RAG-era,
  continuous, per-item generalisation of their discrete input-only baseline — **say this explicitly
  and cite it as the lineage.**
- **Steal:** the Discussion-as-normative-recommendations format. Our Discussion should end with
  "report a copy ceiling", "test with negative-control scaffolds", "don't read uplift as reasoning."

**C2. Annotation Artifacts in Natural Language Inference Data** — arXiv **1803.02324**
(Gururangan et al., 2018). ○ Hypothesis-only baseline predicts NLI labels without the premise. Cite
alongside C1/C3 as the input-only-baseline tradition.

**C3. Hypothesis Only Baselines in Natural Language Inference** — arXiv **1805.01042**
(Poliak et al., 2018). ○ Same tradition, across ten NLI datasets — a *multi-dataset* input-only
sweep, structurally like our ten-model copy-ceiling sweep.

**C4. Probing Neural Network Comprehension of Natural Language Arguments** — arXiv **1907.07355**
(Niven & Kao, 2019). ○ Apparent "argument understanding" is spurious-cue exploitation; a clean
"the gain wasn't what you thought" paper to cite in Related Work.

### Cluster C — New metric / measurement instrument as the primary contribution

**D1. FActScore** — arXiv **2305.14251** (Min et al., 2023). ★ **Best template for "metric is the
contribution."**
- *Skeleton:* 1 Introduction (ends with a numbered contributions list) · 2 Related Work (three
  labelled sub-paragraphs: factual precision, fact verification, model-based eval) · 3 **FActScore:**
  the metric's own section (3.1 **Definition** — formal, with an explicit *assumptions* list; 3.2
  studied LMs; 3.3 data; 3.4 results) · 4 the automatic estimator (validated to <2% error) · 5
  large-scale application (13 LMs) · Limitations.
- *Intro narration:* "evaluating X is non-trivial because (1)… (2)…" → "we introduce FActScore, a new
  evaluation that…" — differentiating sentence is the *definitional* "we introduce" placed in the
  first third, then reinforced by a numbered contributions list.
- *Statistical defence:* human-annotation agreement rates; estimator error rate <2%; a **ranking-
  consistency** check (does the metric rank models the same as ground truth). Crucially §3.1 states
  the metric's **assumptions** as an explicit bulleted list — pre-empting validity objections.
- **Steal:** (i) the dedicated metric section with a formal Definition and an explicit
  **assumptions list**; our copy ceiling deserves the same — define $c_i$ formally *and* list its
  assumptions (deliberate exposure, lexical matcher, per-item). (ii) The move "our metric is costly →
  here is an automatic estimator that approximates it" maps onto "human curation is costly → it is
  amortised once."

**D2. ALCE — Enabling LLMs to Generate Text with Citations** — arXiv **2305.14627**
(Gao et al., 2023). ★
- *Skeleton:* 1 Introduction · 2 Task Setup & Datasets · 3 **Automatic Evaluation** (3.1 fluency,
  3.2 correctness, 3.3 citation quality, **3.4 "ALCE is Robust to Shortcut Cases"**) · 4 Modeling ·
  5 Experiments · 6 Human agreement · Analysis.
- *Intro narration:* prior work relies on human eval / commercial engines → "We present ALCE, the
  **first** reproducible benchmark for…" — differentiating sentence claims *primacy* explicitly, then
  a bulleted list of findings.
- *Statistical defence:* automatic metrics validated by **correlation with human judgement**; a
  dedicated **§3.4 shortcut-robustness** subsection proving two degenerate strategies (copy top-1
  passage; first two sentences) score high on one axis but fail another — i.e. they *stress-test the
  metric against gaming.*
- **Steal (high value):** the **§3.4 shortcut-robustness subsection** is the direct model for a
  "What the copy ceiling does and does not say / negative controls" subsection. ALCE's degenerate
  "copy the passage" shortcut is *literally the behaviour our copy ceiling measures* — cite ALCE and
  note we make that shortcut the metric's reference point rather than something to detect and reject.

**D3. RAGAS** — arXiv **2309.15217** (Es et al., 2023; already cited `es2023ragas`). ○ Reference-free
faithfulness/context-relevance/answer-relevance metrics. Cite as the faithfulness-metric baseline our
copy ceiling complements (RAGAS scores faithfulness to retrieved context; we score fidelity against a
computed copy ceiling of that context).

**D4. Evaluating Correctness and Faithfulness of Instruction-Following Models for QA** —
arXiv **2307.16877** (Adlakha et al., 2024; already cited `adlakha2024evaluating`). ★
- *Contribution:* separates **correctness** (satisfies information need) from **faithfulness**
  (grounded in provided knowledge); shows EM/F1 mislead; proposes **recall** (for correctness) and
  **K-Precision** (token-overlap for faithfulness).
- *Skeleton:* 1 Introduction (Figure-1 worked example; ends with bulleted contributions) · 2 Related
  Work (four labelled sub-paras) · 3 Experimental Setup · 4 Correctness metrics · 5 Faithfulness
  metrics · Results · Conclusion.
- *Steal:* our two-axis framing (in-domain fidelity vs out-of-domain non-regression) is the
  deployment analogue of their correctness-vs-faithfulness split — cite as precedent that a *single
  number is inadequate and two orthogonal axes are required*. Their **recall** metric is our lexical
  scorer; their **K-Precision** (fraction of response tokens in the knowledge) is a near-cousin of the
  copy ceiling's exposure count — a strong citation for our scorer's validity.

**D5. Evaluating Verifiability in Generative Search Engines** — arXiv **2304.09848** (Liu et al.,
2023). ○ Human citation-recall/precision protocol; cite for the attribution axis we specify as future
work.

### Cluster D — Contamination / leakage quantification

**E1. Time Travel in LLMs: Tracing Data Contamination** — arXiv **2308.08493** (Golchin & Surdeanu,
2024; already cited `golchin2024timetravel`). ○ Detects contamination by testing whether a model can
*reproduce* held-out inputs. Our copy ceiling inverts this: instead of auditing leakage post-hoc, we
make the deliberate exposure the metric's reference. Keep as lineage.

**E2. NLP Evaluation in Trouble: measure data contamination per benchmark** — arXiv **2310.18018**
(Sainz et al., 2023; already cited `sainz2023contamination`). ○ Position paper; cite for "how much of
a score is visible in the input" — the question the copy ceiling answers quantitatively.

---

## Part 2 — Cross-cutting structural findings

1. **Where the differentiating sentence goes.** In every metric/baseline paper it sits in the
   **first third of the Introduction**, as a standalone "we introduce X, the first/new…" sentence,
   *immediately after* a "prior work does Y but…" turn, and is then reinforced by a numbered
   contributions list. GraphRAG even isolates it as its own paragraph. **Our Intro currently states
   the copy ceiling only after two paragraphs of setup and folds primacy into the contributions
   list — move a single sharp "we introduce the copy ceiling, the first per-item input-exposure
   control for graph-grounded generation" sentence up.**

2. **A metric-contribution paper gives the metric its own numbered section** (FActScore §3, ALCE §3),
   with (a) a formal definition, (b) an explicit assumptions list, and (c) a shortcut/robustness
   subsection. **Our copy ceiling is currently a `\paragraph` inside Method — this is the biggest
   structural mismatch with the comparanda.**

3. **A baseline-reframing paper defines the control in "Experimental Setup / Baselines"** and pays it
   off in an "Analysis: why does the simple thing win" section (Stronger Baselines §4;
   Kaushik & Lipton §5). The reframing claim lives in the Intro; the *mechanism* lives in Analysis.

4. **Evaluation criteria / desiderata are stated *before* results** (GraphRAG §3.3; Adlakha §1). The
   "what a good system must satisfy" frame is set up front so the reader knows how to read the
   numbers. **Our "Multivariate Bar" does this job but sits *after* Results — it is currently a
   payoff where it should (partly) be a setup.**

5. **Statistical defence of a singular claim** in the strongest comparanda uses: multiple runs per
   condition (Stronger Baselines: 5), correlation-with-human validation (FActScore, ALCE, Adlakha),
   ranking-consistency checks (FActScore), agreement/overlap distributions (RAG-or-LC's 63% histogram),
   and signed-gap-to-baseline (Kaushik & Lipton's Δ(min)). **Bootstrap CIs are rarer than domain norms
   assume — our domain-clustered block bootstrap + ITT is already *more* rigorous than most; the gaps
   are (a) single sample per arm and (b) no negative-control scaffold, both of which the comparanda
   would flag.**

---

## Part 3 — Recommended skeleton for our paper

Narrative arc the brief asks for — *requirements → design constraints → system → isolating
experiments → singular contribution* — realised as:

| # | Section | Role in the arc | Modelled on |
|---|---------|-----------------|-------------|
| 1 | **Introduction** | Requirements gap → the copy-ceiling contribution sentence (first third) → contributions list | FActScore, ALCE, B2 |
| 2 | **The Private-Corpus Setting** *(new, ~½ col)* | The deployment **requirements**: LAN-only/on-prem, model-swappable, human-vetted & auditable, curated-once-amortised. States *why* the bar is what it is. | A5 (enterprise GraphRAG), A1 motivation |
| 3 | **Related Work** | Position against the four comparanda; make the differentiating factor explicit (schema-level injection; per-item copy ceiling; conjunctive bar) | FActScore/Adlakha labelled sub-paras |
| 4 | **The Ontology Scaffold** (system) | **Design → system**: given §2's requirements, the design is a static schema-level extract behind a swappable model | your current §3 |
| 5 | **The Copy Ceiling** *(promote to its own section)* | The **measurement-methodology contribution**: 5.1 formal definition $c_i$ + assumptions list; 5.2 gain-over-copy; 5.3 **what it does/doesn't say + negative controls** (the "robustness" subsection) | FActScore §3, ALCE §3.4, Kaushik&Lipton |
| 6 | **Method** | The **isolating experiments**: graph-as-oracle Q/gold, deterministic config, lexical scorer, statistics, OOD arm | your current §4 |
| 7 | **Results** | 7.1 in-domain fidelity; 7.2 ten-model gain-over-copy (the model-discriminating axis); 7.3 OOD gate non-regression | your current §5 |
| 8 | **Analysis: uplift is exposure, not reasoning** *(rename/refocus current Discussion opener)* | The **mechanism** the control reveals — mirrors Stronger Baselines §4 | B1 §4 |
| 9 | **The Multivariate Bar — scorecard** *(shrink; move the *framing* to §1/§2, keep the *reconciliation* here)* | Which axes met, which specified (web-search baseline, attribution) | GraphRAG criteria, Adlakha two-axis |
| 10 | **Limitations & Threats to Validity** | keep as-is (already strong) | Stronger Baselines, FActScore |
| 11 | **Conclusion** | Lead with the normative recommendation ("report a copy ceiling as standard") | B1, Kaushik&Lipton |

**Net changes vs current structure:** *add* §2 requirements; *promote* the copy ceiling from a Method
paragraph to §5 with a definition+assumptions+robustness structure; *split* "The Multivariate Bar" —
its framing rises to §1/§2, its scorecard shrinks to §9; *rename* the Discussion opener to a
Stronger-Baselines-style "Analysis" that names the mechanism. Everything else is kept.

---

## Part 4 — Three candidate framings for the SINGULAR contribution (ranked)

### #1 (recommended) — The copy ceiling as a measurement instrument
**Claim (one sentence):** *"We introduce the **copy ceiling** — a per-item, graph-derived
input-exposure baseline — and the signed **gain over copy**, the first control that separates faithful
delivery of injected facts from reasoning over ontological structure in graph-grounded generation."*

- **Defensible against (nobody already did it):**
  - Input-only baselines exist for **RC** (Kaushik & Lipton 1808.04926) and **NLI** (Gururangan
    1803.02324, Poliak 1805.01042) but are **discrete and task-classification**; none is a *per-item
    continuous* ceiling for **RAG / KG-grounded generation**.
  - KG-RAG papers (GraphRAG 2404.16130, KAPING, LightRAG, G-Retriever 2402.07630) report grounding
    uplift **without any input-exposure control** — their uplift is un-controlled for what the
    injected context already contains, especially for **schema-level** injection.
  - Faithfulness metrics (RAGAS 2309.15217, FActScore 2305.14251, Adlakha's K-Precision 2307.16877)
    score against a **knowledge source**, not against a **computed copy-of-the-exact-injected-text
    ceiling**; ALCE (2305.14627 §3.4) treats "copy the passage" as a *shortcut to reject*, not as the
    metric's *reference point*.
  - Contamination work (Golchin 2308.08493, Sainz 2310.18018) audits leakage **post-hoc**; we make
    deliberate exposure the metric's reference *a priori*.
- **Statistical evidence it needs:** (a) the ceiling is computed correctly (done); (b) **gain over
  copy actually discriminates models** — currently the spread (−0.067…−0.022) is small, so defend the
  *ordering* with a paired, domain-clustered bootstrap on the gain itself and a rank-correlation of
  gain vs model recency/capability (borrow FActScore's ranking-consistency check); (c) **negative
  controls** — irrelevant / shuffled-target / label-masked scaffolds — to show gain-over-copy *moves*
  when structure is destroyed (this is the single most important missing experiment); (d) a
  multiple-comparison note across the ten models.

### #2 — A rigorously-measured negative result (the reframing)
**Claim:** *"The large recall uplift from ontology-scaffold grounding is, to within measurement,
exactly what a copy of the injected context scores; raw uplift is therefore not evidence of reasoning
over structure."*

- **Defensible against:** the entire KG-RAG cluster reports uplift as the headline; this is the
  genre-move of Stronger Baselines (2506.03989), RAG-or-LC (2407.16833), Niven & Kao (1907.07355) —
  install the control, overturn the naive reading. Novel *for schema-level ontology injection*.
- **Evidence it needs:** an **equivalence test (TOST)** with a pre-registered margin (not just "the CI
  includes a small negative"); **replicates per arm** to kill the single-sample limitation (Stronger
  Baselines uses 5); the same negative controls as #1. Weaker as a *standalone* headline because "no
  effect beyond copying" invites "so the system is pointless" — must be paired with #1's reframe
  (faithful delivery *is* the product).
- **Verdict:** #1 and #2 are two faces of one coin — the metric (#1) is what licenses the negative
  result (#2). **Lead with #1 (the instrument); state #2 as what the instrument reveals.**

### #3 — A conjunctive multivariate evaluation protocol
**Claim:** *"A deployable private-knowledge system must clear in-domain delivery fidelity **and**
out-of-domain non-regression **simultaneously**, on two different instruments; we define the bar and
establish both axes."*

- **Defensible against:** single-axis RAG-eval papers (RAGAS, ALCE, FActScore); the *conjunction held
  together* is a genuinely novel framing and echoes Adlakha's correctness-vs-faithfulness split at
  deployment scale.
- **Evidence it needs:** to be the *headline* it needs the **web-search baseline arm actually run** —
  otherwise two of four axes are only "specified" and reviewers read it as incomplete.
- **Verdict:** strongest as the **organising frame** (→ §1/§2), weakest as the singular contribution.
  Use it to structure the paper, not to headline it.

---

## Part 5 — Verdict on the current structure

Current headings: Intro / Related Work / The Ontology Scaffold / Method / Results (in-domain; ten
models; OOD gate) / **The Multivariate Bar** / Discussion / Limitations / Conclusion.

**Keep:** Intro, Related Work, The Ontology Scaffold, Method, Results, Limitations, Conclusion — the
bones are sound and Related Work + Limitations are already stronger than most comparanda.

**The one change that matters most — promote the copy ceiling to its own section.** Every
metric-as-contribution comparator (FActScore §3, ALCE §3) gives the metric a numbered section with a
formal definition, an explicit assumptions list, and a shortcut-robustness subsection. Ours is a
`\paragraph` in Method. This under-sells the paper's actual primary contribution. Add **§5 The Copy
Ceiling** = definition ($c_i$) + assumptions + gain-over-copy + a "what it does/doesn't say + negative
controls" subsection lifted in spirit from ALCE §3.4 and Kaushik & Lipton's Δ(min).

**"The Multivariate Bar" — split it.** The comparanda state evaluation desiderata *before* results
(GraphRAG §3.3, Adlakha §1), not after. So:
- *Move the framing up:* a compact statement of the four-axis bar becomes the close of the
  Introduction and the substance of the new **§2 Private-Corpus Setting** (requirements).
- *Keep a shrunk scorecard late:* the "which axes are met / which are specified" reconciliation stays
  as a short §9 (or folds into Discussion).
- **Do not make "The Multivariate Bar" a full contribution-framing section immediately before
  Results** — that is framing #3, and as Part 4 argues it is the weakest headline; used as a pre-Results
  *setup* it is fine, used as the paper's central claim it over-promises the two unmeasured axes.

**Where does the control-baseline definition go — Method or its own section?** The evidence splits by
paper *identity*: baseline-reframing papers (Stronger Baselines, RAG-or-LC, Kaushik & Lipton) put the
control in **Experimental Setup / Baselines**; metric-first papers (FActScore, ALCE) give it its
**own section**. **Because our primary contribution is the copy ceiling *as an instrument*, follow the
metric-first camp: its own section (§5), not a Method paragraph.** Keep the *computation details*
(the exposure count, the matcher) in Method/§5.1; keep the *framing* ("this is a control, like
GraphRAG's Directness") in the Intro.

**Add the requirements narrative.** No current section carries the deployment requirements (LAN-only,
model-swap, auditability, amortised curation) — the Intro jumps straight to results. §2 fills this and
is what makes the arc *requirements → design → system → experiments → contribution* legible. Cite the
enterprise/private-corpus comparanda (2507.03226, 2404.16130) here.

**Rename the Discussion opener** to a Stronger-Baselines-style **"Analysis: uplift is exposure, not
reasoning"** that names the mechanism — this is where the negative result (framing #2) is paid off,
and it pre-empts the reviewer's "surely grounding does more than copy."

**Two statistical additions the comparanda would demand:** (1) **negative-control scaffolds**
(irrelevant / shuffled / label-masked) to show gain-over-copy responds to structure — currently
flagged as future work, but it is the experiment that turns #1 from "plausible" to "confirmed"; (2)
**replicates per arm** (à la Stronger Baselines' five runs) or an explicit equivalence-margin (TOST)
to retire the single-sample limitation.

---

## Appendix — arXiv ID quick reference

| ID | Short name | Cluster | Verified skeleton |
|----|-----------|---------|-------------------|
| 2404.16130 | GraphRAG (Local→Global) | A | ★ |
| 2408.08921 | Graph-RAG survey | A | ○ |
| 2501.00309 | RAG with Graphs | A | ○ |
| 2501.13958 | GraphRAG domain survey | A | ○ |
| 2507.03226 | Practical/enterprise GraphRAG | A | ○ |
| 2402.07630 | G-Retriever | A | ○ |
| 2506.03989 | Stronger Baselines for RAG | B | ★ |
| 2407.16833 | RAG or Long-Context (Self-Route) | B | ★ |
| 2501.01880 | Long Context vs RAG revisits | B | ○ |
| 2409.01666 | In Defense of RAG (OP-RAG) | B | ○ |
| 1808.04926 | Kaushik & Lipton (input-only) | A′ | ★ |
| 1803.02324 | Gururangan (annotation artifacts) | A′ | ○ |
| 1805.01042 | Poliak (hypothesis-only) | A′ | ○ |
| 1907.07355 | Niven & Kao (spurious cues) | A′ | ○ |
| 2305.14251 | FActScore | C | ★ |
| 2305.14627 | ALCE | C | ★ |
| 2309.15217 | RAGAS | C | ○ |
| 2307.16877 | Adlakha (correctness vs faithfulness) | C | ★ |
| 2304.09848 | Evaluating Verifiability | C | ○ |
| 2308.08493 | Time Travel (contamination) | D | ○ |
| 2310.18018 | Sainz (contamination position) | D | ○ |
