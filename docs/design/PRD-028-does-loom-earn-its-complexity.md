---
id: PRD-028
title: "Does Loom earn its complexity on genuinely private knowledge? Preregistration specification for a matched-access evaluation against strong flat-text retrieval"
status: proposed requirements and preregistration specification; no new experiments have been run
date: 2026-09-21
version: 1.0
authors: John O'Hare / DreamLab (owner)
linked_prd: [PRD-025 (Ontology Loom & Connector Platform), PRD-026 (Loom Consolidation), PRD-027 (Rust re-engineering)]
linked_adrs: [ADR-135 (loom node boundary + model-swappable façade), agentbox ADR-2095 (measure typed-decision seams against a copy ceiling), agentbox ADR-2023 (Loom façade), agentbox ADR-2084 (one published loom-client)]
evidence_basis: docs/research/gain-over-copy-paper/gain-over-copy-paper.pdf (The Copy Ceiling, v9.2, 2026-09-21) — motivates this study; does not answer its primary question
decision_owned: retain and invest in the structured serving path, simplify it, specialise it for a demonstrated task family, or invest in corpus quality rather than serving complexity
placement: docs/design beside PRD-025..027; the pilot's evidence lands under docs/research/
---

# PRD: Does Loom earn its complexity on genuinely private knowledge?

Version 1.0 • 21 September 2026 • Owner: John O'Hare / DreamLab

Status: proposed requirements and preregistration specification. No new experiments have been run. Numerical thresholds below are proposed product decisions, not established scientific constants. The immediate deliverable is a pilot and a frozen evaluation protocol, followed by a gated full study.

## 1. Decision and purpose

Determine whether Loom's ontology-backed serving path provides a practically valuable improvement over strong, simpler retrieval when answering questions about maintained organisational knowledge that the evaluated models could not reliably know unaided.

The primary decision is whether to retain and invest in the structured serving path. Secondary decisions concern where it helps, whether a local model is adequate, and whether benefits justify corpus preparation and operating costs.

The current Copy Ceiling paper motivates this analysis but does not answer it. It reports large gains over unaided models, high answer exposure, vocabulary-sensitive retrieval and incomplete evidence for content specificity. A new private corpus solves one validity problem; it does not establish architectural advantage on its own.

**Proposed primary question:** At matched information access and context budget, does Loom improve evidence-supported task success by a practically meaningful amount over the strongest flat-text retrieval baseline selected on development data?

**Do first:** inventory an existing unpublished corpus and run a small feasibility study. Do not commission 8,000 new pages before proving that the data, questions and baselines are usable.

## 2. What "out of LLM distribution" means here

Treat these as separate attributes:

| Attribute | Operational meaning | Evidence required |
|---|---|---|
| Private access | Material was not publicly accessible before the evaluation | Source-owner attestation, publication history, access and export records |
| Unseen facts | Target facts are particular to this organisation or are newly created arbitrary facts | Fact provenance and timestamps; frozen-checkpoint synthetic control where applicable |
| Unfamiliar terminology | Queries use aliases, jargon or descriptions absent from canonical titles | Held-out human paraphrases and overlap statistics |
| Domain shift | Documents and workflows differ from the existing Loom ontology | Corpus profile and a distinct subject/workflow family |
| Temporal novelty | The relevant state postdates a documented model checkpoint | Signed snapshot and change timestamps; immutable model identity |

Privateness alone does not prove absence from training. Poor closed-book performance does not prove absence either: it can reflect question difficulty or failed recall. Familiar language can describe genuinely unknown facts, which is desirable for this study. Renaming public entities alone does not establish a realistic private corpus.

Use the claim **"non-public, provenance-documented corpus with measured closed-book performance"** for the natural corpus. Reserve the stronger factual construction claim for information generated after a frozen local checkpoint. Proprietary endpoint training histories may remain unknown. Do not claim that all linguistic patterns are out of distribution.

## 3. Corpus strategy

### 3.1 Primary corpus: naturally occurring private knowledge

Preferred candidate: an unpublished technical or operational corpus from a separate project or organisation, containing source records that already exist for a practical purpose. Plausible DreamLab-adjacent candidates include internal deployment decisions, equipment configurations, project constraints, compatibility records, incident resolutions and versioned operating procedures. These are candidate categories, not identified or accessed datasets.

Choose material whose answers depend on local facts: which revision is deployed, which component depends on which service, what exception applies to a named installation, and which procedure is valid at a specified date. Generic technical definitions are useful distractors but insufficient as the benchmark's main substance.

Prefer a corpus independent of the original Loom authoring process. If DreamLab material is used, disclose shared authorship and domain familiarity, and restrict transfer claims accordingly. A genuinely independent partner corpus is a later replication, not a precondition for beginning the pilot.

### 3.2 Secondary corpus: controlled fictional organisation

Create a distinct synthetic test set using locally generated, seeded assignments of entities, dependencies, capabilities, versions and exceptions after local model checkpoints are frozen. Generate the underlying facts with a deterministic programme. Human authors or local language models may realise them as documents, but each document must be checked against the latent fact ledger. No evaluated model chooses its own test facts or writes final test questions.

Use multiple document styles, realistic repetition and distractors. Randomise arbitrary assignments without destroying meaningful workflows. Avoid a single template that makes fact lookup trivially identifiable.

This track offers stronger control over prior factual exposure and an executable ground truth. It does not substitute for natural document quality or establish enterprise transfer. Report its results separately; never pool it into the primary natural-corpus headline.

### 3.3 Existing public Loom corpus

Retain the existing corpus as a regression reference. Do not rebrand it as private. It can show whether changes help the new corpus while breaking the old one; it cannot establish novelty of facts.

### 3.4 Size and quality requirements

The current manuscript describes approximately 8,138 pages, 8,146 classes and 282,492 closure triples. These are reference measurements, not sufficient criteria for an equivalent corpus. Asserted facts, inferred closure, pages and independent information must remain separate counts.

| Requirement | Pilot | Full study |
|---|---|---|
| Scope | 400–800 source units spanning at least four subdomains | Target 6,000–10,000 substantive source units; justify deviations against the measured Loom profile |
| Information content | Measure tokens, unique facts, entities and duplicate fraction | Aim for 0.5–2 times Loom's deduplicated token volume; measure the Loom reference before accepting this range |
| Structure | Relationships and procedures sufficient for all task families | Compare asserted degree, relation diversity, document length, definition coverage and disconnected regions with Loom |
| Retrieval difficulty | Include realistic nearby distractors | Full index with difficult non-answer documents; no padding with near-duplicates to meet page count |
| Provenance | Every unit has an owner, source, timestamp and stable ID | Same, including version lineage and supersession rules |
| Quality | Source-owner review plus graph/prose consistency checks | All test-supporting evidence independently reviewed; stratified audit of at least 200 additional background units |

Corpus acceptance requires 100% of test questions to have adjudicated answerability and evidence, and no unresolved contradiction in an answer key. Genuine source contradictions may remain if labelled and reflected in the correct answer. Proposed background-quality gate: at least 95% of audited units meet the declared completeness and consistency rubric; remedy failed strata and report the audit interval and repairs. Do not silently delete difficult material.

Do not claim equal quality from matching graph density or passing an ontology consistency checker. Domain reviewers must assess usefulness, factual support, ambiguity and freshness.

## 4. Information parity and corpus construction

Maintain three versioned layers:

1. **Original source collection:** documents as received, with provenance and versions.
2. **Canonical curated collection:** reviewed prose plus explicit fact records, including any facts introduced during curation and their sources.
3. **Serving representations:** Loom blocks and closure; prose/text indexes; lexical, dense and hybrid indexes; graph query interface.

Freeze source and canonical collections before final question authoring. Generate representations without test questions, gold answers or gold entity IDs. No test-driven ontology repair after lock.

Two comparisons answer different questions:

- **Representation/serving comparison — primary:** Loom versus flat-text retrieval over the same canonical knowledge. Make asserted and materialised inferred facts available to the flat baseline as readable, provenance-linked records, with no information exclusive to Loom. Both systems may retrieve and select within the same budget.
- **Whole-product comparison — secondary:** Loom's curated pipeline versus retrieval over original source documents. Attribute any difference to the combined curation and serving package, and count its preparation cost. Do not label it an ontology-only effect.

The primary comparison controls factual access; it does not erase differences in organisation, selection or serialisation. That is the intended treatment. Persist evidence showing which facts each representation contains.

## 5. Questions and gold answers

### 5.1 Sampling and splits

Start with 120 pilot questions on development-only topic groups. Expand to 200 development questions and a provisional 600 sealed test questions. The 120 pilot questions may form part of the 200 development set; they never become test observations. Final test size must pass the power gate in §9.

Split by topic/entity family and task instance before authoring paraphrases. Keep all aliases, versions and paraphrases of a question in the same split. All permitted documents, including those needed for test questions, remain searchable in the evaluation index: this is a query-generalisation test, not an experiment that withholds the answer documents.

Question authors receive the source material and user-workflow brief, not Loom's scaffold, retrieval scores, graph-derived templates or candidate answers. A separate reviewer establishes the answer and minimum sufficient evidence. Prefer real user requests where available and document sampling rules.

Provisional sealed test allocation:

| Task family | Questions | Requirement |
|---|---:|---|
| Specific local facts | 120 | Exact facts with plausible competing entities or values |
| Relations, direction and constraints | 120 | Correct entity plus relationship, polarity, scope and conditions |
| Multi-document dependency questions | 120 | At least two necessary evidence pieces; review single-document shortcuts |
| Procedures, exceptions and version selection | 120 | Correct applicable state, sequence or exception; freeze "as of" time |
| Unanswerable or conflicting requests | 120 | Explicitly distinguish absent evidence, ambiguity, contradiction and false premise |
| **Total** | **600** | Report this designed mix and every stratum separately |

At least 360 of the 480 answerable questions should concern organisation-specific facts rather than generic domain knowledge. Select this property by provenance before model testing, not by discarding questions models answer correctly. Include at least 25% naturally phrased alias/description questions across the test families. Any paired canonical/paraphrase stress set is an additional clustered diagnostic, not extra independent test cases.

### 5.2 Gold record

For every question store: question ID, split, topic cluster, task family, query text, requested date/version, answerability class, required propositions, accepted alternatives, prohibited/conflicting propositions, minimum evidence spans, document/version IDs, source authority rule, evidence-dependency links, allowed answer scope, reviewer IDs and adjudication outcome.

Absence is not automatically falsity. For unanswerable questions, confirm the designated corpus scope and expected action. An answer may appropriately ask for clarification or state that the corpus does not establish a fact.

Graph execution can cross-check answers but cannot be the sole author of the primary questions and gold. Store inferred answers with the supporting premises and inference rule. Preserve disagreements and exclusions with reasons.

### 5.3 Contamination assessment

- Freeze model checkpoint hashes before generating synthetic factual assignments; retain generator seed and creation records privately until the run completes.
- Inventory public counterparts, prior publication, repositories and known previous model interactions for natural sources. Previously uploaded confidential text is not automatically proven training data; record uncertainty.
- Keep private corpus construction, embeddings, reranking and judging local by default. Any cloud comparison is a separate authorised condition with declared data handling; the PRD does not authorise uploading private documents.
- Run closed-book questions in clean, stateless sessions with external tools disabled. Report all results; investigate surprising exact answers rather than deleting them to manufacture novelty.
- Test whether questions reveal answers through names, ordering, identifiers or distractor construction. Fix development design defects before lock; disclose post-lock findings and sensitivity analyses.
- Use canaries only as supplemental leakage indicators, never as proof that a corpus is uncontaminated.

## 6. Experimental arms

Use one frozen local generator for the primary comparison. Choose its checkpoint, quantisation, inference engine and decoding settings on development data, before test access. A second independently trained local family is a replication if resources permit.

| Arm | Behaviour | Role |
|---|---|---|
| A: closed book | Question and common instructions; no corpus | Measures unaided performance, not architectural advantage |
| B: BM25 | Tuned lexical retrieval over canonical facts and prose | Required simple baseline |
| C: hybrid RAG | Tuned lexical+dense retrieval, locally hosted reranker, coherent chunk packing | Required strong baseline |
| D: Loom | Frozen production retrieval, gating and ontology scaffold | Candidate system |
| E: oracle evidence | Human-verified sufficient evidence, same generator and budget | Diagnostic evidence-selection reference; not a deployable competitor |

Choose **B-star**, the better deployable flat-text baseline among B and C, using development success and declared tie-breaking by cost. Freeze the selection. The primary contrast is D minus B-star on the sealed test. Still report B and C separately. Equalise reasonable tuning opportunities and document the explored settings.

Run a direct graph-query comparator on the relation subset. Report an end-to-end question-to-query system with mapping and parsing failures counted. A gold SPARQL query or gold seed entity is an oracle diagnostic and must not be presented as an end-to-end baseline.

Use a common source-grounded answer policy for B–E: answer from the supplied evidence, identify the relevant version, cite sources, state uncertainty, and do not invent missing facts. Keep wrapper and citation instructions constant apart from necessary serialisation. A production-policy comparison may be added separately with its wrapper difference disclosed.

### 6.1 Budget and execution controls

- Default retrieved-evidence cap: 4,096 generator-tokeniser tokens. Tune an answer/reasoning allowance on development data to avoid artificial empty-output failure; freeze and log it.
- Optional 2,048/8,192 context-budget sweep on a preregistered subset. Actual token usage remains an outcome; do not pad short contexts with informative content.
- Count every prompt component, model call, reranker call and retry. Match source snapshot, access scope and generator within a paired comparison.
- Randomise/interleave arm order; control concurrency and report warm/cold cache conditions. Retrieve from the full permitted corpus, not a per-question gold neighbourhood.
- Empty output, timeout, tool failure and malformed response count as failures in the primary service-level endpoint. Report first-attempt and common-retry-policy success separately. No complete-case deletion.
- Do not silently truncate gold evidence in the oracle arm. Label questions that cannot fit the declared budget and keep them visible as a separate budget-limited group.
- Gold keys and test labels remain inaccessible to the serving harness and retrievers.

## 7. Metrics and judging

### 7.1 Primary endpoint: evidence-supported task success

A question succeeds only if the answer supplies all required propositions or accepted alternatives, contains no material false or unsupported assertion, selects the correct version/scope, and cites evidence supporting each material claim. An unanswerable question succeeds when the system performs the pre-labelled appropriate abstention or clarification without inventing an answer. Empty output always fails.

Score success as a binary per-question endpoint. Also report component scores so citation formatting does not obscure whether a failure was factual, evidential or procedural. A dump of the corpus or enumeration of all candidates fails the task-scope rubric even if it contains the correct names.

### 7.2 Secondary outcomes

- Required-proposition recall; material-claim precision; relation/direction/negation correctness.
- Citation support and citation completeness; source/version correctness.
- Correct-answer coverage and selective risk; false abstention on answerable questions; unsupported answering on unanswerable questions. Freeze confidence thresholds on development data.
- Retrieval coverage of minimum evidence, judged context sufficiency, distractor load, latency and index build time.
- The original four exposure counts and gain over copy as lexical diagnostics, not the semantic endpoint or an architecture selection rule.
- End-to-end p50/p95 latency, throughput at declared concurrency, input/output/reasoning tokens, retries, memory, energy where measurable and cost per successful task.
- Breakdowns by task family, private-fact provenance, terminology mismatch, evidence count, corpus coverage and model. Do not treat arbitrary subgroup maxima as confirmatory findings.

Context sufficiency must inspect whether the required relationships and conditions are available, not merely whether answer strings occur. This distinction is motivated by the Sufficient Context study [S1].

### 7.3 Human and model judgement

Two independent, blinded human reviewers assess every primary-model D and B-star test response; a third resolves disagreements. Blind system identity and randomise presentation order. Reviewers can access the question, source evidence, gold rubric and full output. Preserve individual ratings before adjudication and report agreement by component.

Model judges may assist triage and secondary-arm scoring, but report those results separately from human adjudication. For secondary results, validate on a preregistered stratified random human sample, publish sample weights and uncertainty, and audit disagreements in addition to that random sample. A disagreements-only audit cannot estimate overall judge error.

If human review funding is insufficient, reduce scope before testing and power-check the new design, or mark the main results as model-judged estimates. Do not make a sample audit look like a census. The negative examples from the current paper make this a release gate.

## 8. Diagnostic interventions

These explain outcomes; they do not replace the main comparison.

1. **Terminology stress:** held-out natural paraphrases and aliases of fixed tasks. Measure evidence retrieval and semantic success separately.
2. **Evidence removal:** remove one necessary fact from all serving representations for a controlled paired variant. Check calibrated abstention and supplementation behaviour. Avoid changing gold silently.
3. **Matched irrelevant context:** construct distractors verified not to answer the question, with matched budget and common wrapper. Seed disjointness alone is insufficient.
4. **Structure intervention:** use the synthetic track to rewire relations while holding entity inventory approximately fixed; regenerate coherent evidence and recompute gold. Keep all variants of a task in the same statistical cluster.
5. **Single-premise controls:** for multi-document tasks, compare both premises with each premise alone. Remaining shortcuts and relevance cues bound any reasoning claim; follow the connected-evidence motivation of MuSiQue [S2].
6. **Update test, optional second phase:** introduce a small documented revision set after snapshot T0; assess current-version answers at T1, stale-answer rate, update latency, review effort and index rebuild cost for every arm. Timestamp the query and expected authority explicitly.

## 9. Statistics, practical thresholds and interpretation

Preregister the primary contrast, endpoint, task mixture, exclusions, retry rule, sample size and analysis code before opening sealed test results. Keep exploratory analyses labelled.

Primary estimand: the mean paired difference in evidence-supported task success, D minus B-star, on the declared natural-corpus test mixture. Estimate a 95% confidence interval using a paired cluster bootstrap over independent topic/task families, preserving all related variants. Choose the exact cluster construction on development data. Model replication is reported separately, not counted as additional independent questions.

Proposed meaningful advantage: **5 percentage points** absolute success. Classify outcomes as follows:

| Observed evidence | Decision |
|---|---|
| Lower confidence bound exceeds +5 points | Evidence of a practically meaningful quality advantage at the tested operating point |
| Lower bound exceeds zero, point estimate at least +5, but lower bound is at most +5 | Evidence of improvement; practical magnitude remains uncertain |
| Interval includes zero and also includes meaningful improvement | Inconclusive; no claim of equivalence or no value |
| Quality non-inferiority established within a preregistered -2 point margin, with material measured cost/latency reduction | Potential efficiency case; this is a separately preregistered secondary route, not a post-hoc rescue |
| Material disadvantage or unacceptable unsupported-answer behaviour | Do not expand this configuration; diagnose the source |

Proposed efficiency threshold: at least 25% lower total cost per successful task at comparable coverage, with non-inferiority supported and absolute error rates disclosed. Report uncertainty on the cost ratio and a frontier across operating points. These margins must be approved as product-relevant before test lock; if not, retain them explicitly as provisional and avoid categorical success claims.

Use pilot paired discordance and topic clustering to simulate sample sizes for the desired inference, targeting at least 80% power for a declared plausible alternative. Six hundred questions is an initial planning number, not a power guarantee. Demonstrating a lower bound above +5 requires an assumed true effect greater than +5. If the required sample is unaffordable, narrow the claim before running, not after seeing results.

One confirmatory primary contrast needs no family correction. Apply Holm correction to a predefined secondary hypothesis family and report adjusted p-values; label confidence intervals with their actual coverage procedure. Do not turn non-significant subgroup differences into capability rankings. Any gatekeeping between superiority and non-inferiority must be preregistered.

Designed task proportions are not measured deployment frequencies. Report stratum results and a separately labelled deployment-weighted estimate only if genuine query frequencies are available.

## 10. Economics and local-model value

Log source acquisition, cleaning, ontology mapping, validation, annotation, index construction and subsequent update effort separately. Distinguish one-off research costs from recurring product costs. Compare canonical-information arms for serving cost, and raw-source versus curated arms for the whole curation package.

For each arm report: setup cost + update/maintenance cost over a declared horizon + query volume × mean variable cost. Include failures and retries. Use measured local hardware time and documented labour/electricity assumptions; show sensitivity rather than treating owned GPUs as free. Any break-even volume is meaningful only at acceptable matched quality, answer coverage and freshness.

After the primary test, compare at least two frozen local model families at matched quality and budgets. A larger/cloud model comparator is optional and uses authorised data only. "Local matches larger" requires a preregistered non-inferiority design, not merely overlapping confidence intervals.

## 11. Functional requirements and acceptance criteria

| ID | Requirement | Acceptance evidence |
|---|---|---|
| FR-01 | Versioned corpus import and provenance | Stable IDs, content hashes and a complete snapshot manifest |
| FR-02 | Information parity export | Machine-readable fact availability map plus audited prose/graph correspondence |
| FR-03 | Independent question and gold store | Author/reviewer history, evidence spans and sealed test access log |
| FR-04 | Interchangeable arm interface | Same query contract, source scope, output policy and token accounting |
| FR-05 | Durable per-attempt records | Atomic persistence of every request, retrieved context and completion before scoring |
| FR-06 | Failure-aware execution | Resume-safe IDs; no silent duplicates or missing attempts; terminal failure states |
| FR-07 | Blinded semantic adjudication | Stored independent ratings, adjudication and quote/evidence checks |
| FR-08 | Reproducible paired analysis | Frozen scripts reproduce every table from retained rows without model calls |
| FR-09 | Privacy-preserving replay | Offline/local replay for primary private data; no hidden network dependency |
| FR-10 | Release completeness | All regeneration inputs retained in the authorised archive; omissions explicitly declared |

Minimum run record: run/attempt/question/arm IDs; corpus and index hashes; model/checkpoint and inference configuration; system/user prompts; exact ordered retrieved bytes and source IDs; retrieval scores; token budgets and actual usage; timestamps; hardware/concurrency; output; finish/error status; retry relationship; scorer version; human/model judgements; release eligibility. Persist actual contexts rather than reconstructing them from exposure totals later.

Suggested artefacts: corpus-profile, source-manifest, canonical-facts, representation-manifests, question-gold records, preregistration, frozen configurations, per-attempt JSONL, adjudication records, analysis scripts, cost ledger and a findings report. These are requirements for the implementation, not artefacts claimed to exist now.

## 12. Delivery plan and stop/go gates

| Phase | Deliverable | Gate |
|---|---|---|
| 0: candidate inventory | Provenance, permission, volume and quality profile for candidate corpora | A usable non-public corpus exists; source handling is authorised |
| 1: corpus pilot | 400–800 units, paired representations, 120 development questions | Independent gold is achievable; private facts are substantive; representation parity passes |
| 2: baseline pilot | All five arms on development data, failure rates and cost timings | Strong baseline is functioning; no systematic truncation; outcome rubric is reliable |
| 3: scale and lock | Full profile, development tuning, powered sample plan, sealed test and preregistration | Review budget and compute budget approved; primary contrast and margins frozen |
| 4: execution | Retained attempts and blinded adjudication | Completeness reconciled against planned calls; failures remain in denominators |
| 5: analysis | Primary result, diagnostics, economics and claims ledger | Every conclusion maps to an estimand and retained evidence |
| 6: replication/update | Second local family, independent corpus or temporal test | Only after the core result is interpretable; no automatic expansion |

Planning load: 600 test questions × 5 arms × 2 models = 6,000 generation attempts before retries, pilot calls and diagnostics. The minimum first model costs 3,000 test attempts. At an illustrative 4,000 input and 1,000 generated tokens per call, 6,000 attempts represent 24 million input and 6 million generated tokens; actual reasoning budgets can substantially change this estimate. Price only after pilot measurements.

Primary human review comprises 600 × 2 primary arms = 1,200 responses per primary model. At five minutes per response and two independent reviewers, this is about 200 reviewer-hours before adjudication or gold authoring. Replace that timing assumption with pilot measurements. This review workload may be more constraining than GPU capacity.

Stop or redesign if the corpus is largely public material with renamed titles, gold is generated solely from Loom's graph, baselines lack equivalent facts, the oracle fails because questions are ill-defined, or evidence cannot be retained. A strong flat baseline matching Loom is an informative result; it is not a reason to change the endpoint.

## 13. What the next paper may claim

- A measured advantage over simpler retrieval at a specified information and resource budget, if supported.
- A located advantage on a predefined task population, with appropriate uncertainty and multiplicity control.
- A quality–cost or update-governance advantage if independently measured.
- Improved diagnosis of retrieval versus generation failures even if architectural superiority is absent.

It may not infer general ontology superiority from beating closed book, claim uncontaminated training from low unaided scores, claim reasoning absence from negative gain over copy, or claim enterprise generalisation from a single synthetic organisation.

The product outcome is a decision: keep Loom's current structure, simplify it, specialise it for a demonstrated task family, or invest in corpus quality rather than serving complexity.

## 14. Assumptions and unresolved inputs

This PRD does not assume an eligible private corpus already exists. The candidate corpus, source-owner permissions, empirical Loom token profile, local checkpoint identities, labour budget, deployment query distribution and acceptable business error costs remain to be supplied. Defaults above permit pilot planning without pretending those decisions have been made.

Scope excludes building a new corpus or running the benchmark in this task. A corpus build should be authorised only after the inventory establishes which existing material can be reused and what its quality gaps are.

## 15. Research basis

These sources inform the design; all corpus sizes, gates and product thresholds in this PRD are proposed choices.

- **[S1] Joren et al., Sufficient Context.** Separates context sufficiency from a model's use of retrieved evidence. Supports measuring sufficient evidence separately from string presence and answer correctness. [Paper](https://arxiv.org/abs/2411.06037).
- **[S2] Trivedi et al., MuSiQue.** Motivates construction and controls that reduce single-hop shortcuts in multi-hop questions. [Paper](https://arxiv.org/abs/2108.00573).
- **[S3] Friel et al., RAGBench.** Provides precedent for evaluating relevance, utilisation and completeness separately across RAG settings. It is methodological context, not an eligible private test corpus. [Paper](https://arxiv.org/html/2407.11005v2).
- **[S4] Oren et al., Proving Test Set Contamination in Black Box Language Models.** Demonstrates a specific contamination test under assumptions about benchmark ordering. It does not provide a universal certificate of non-contamination for private documents. [Paper](https://proceedings.iclr.cc/paper_files/paper/2024/file/46e624c244cff669223d488defd4e835-Paper-Conference.pdf).
- **Project source:** John O'Hare, The Copy Ceiling, final manuscript `docs/research/gain-over-copy-paper/gain-over-copy-paper.pdf` (v9.2, 21 September 2026). Corpus counts and the motivation for this follow-on study come from that manuscript; new targets above are not findings of it.
