# Related-work positioning notes — the "copy ceiling" vs prior art

Prepared for the revision agent. Every claim below is grounded in a source I actually
fetched (URL + date). The bottom section states exactly how far the "first standing
control" claim can be pushed without overstating.

New/updated bib keys you can cite:
- `ragchecker2024`  — RAGChecker (NeurIPS 2024 D&B, arXiv 2408.08067)
- `mirage2025`      — MIRAGE (Findings of NAACL 2025, 2504.17137)
- `copyasdecode2026`— Copy-as-Decode (arXiv 2604.18170, **withdrawn**) — name-collision footnote only
- `answerpresence2026` — corrected title/authors (arXiv 2606.05633, **withdrawn**)
- `seedrg2026`      — corrected title/authors (arXiv 2605.08838)
- `es2023ragas`     — unchanged, verified accurate
- `laitenberger2025stronger` — authors added, now EMNLP 2025

---

## 1. RAGChecker (`ragchecker2024`)
Ru et al., *RAGChecker: A Fine-grained Framework for Diagnosing Retrieval-Augmented
Generation*, NeurIPS 2024 Datasets & Benchmarks (arXiv 2408.08067; 18 authors led by
Dongyu Ru). Proposes a **suite of diagnostic metrics** decomposed over the retrieval
module and the generation module — claim-level metrics obtained by **LLM-extracting
claims** from responses and checking them against retrieved context and ground truth
(claim recall, context precision, plus faithfulness/hallucination/noise-sensitivity
style generation metrics). Its headline validation is a meta-evaluation showing the
metrics correlate with human judgement better than prior automatic metrics. It is a
**pipeline diagnostic panel**, run over 8 RAG systems to expose design trade-offs.

*Copy-ceiling difference:* the copy ceiling is **judge-free and deterministic** — it
does not extract or LLM-score claims; it is a single per-item exposure number (the
recall a verbatim copy of the injected context would obtain under the same
generation-recall metric), reported as a standing control, not a multi-metric
diagnostic suite.

## 2. MIRAGE (`mirage2025`)
Park, Moon, Park & Lim, *MIRAGE: A Metric-Intensive Benchmark for RAG Evaluation*,
Findings of NAACL 2025, pp. 2883–2900 (DOI 10.18653/v1/2025.findings-naacl.157; arXiv
2504.17137). A **QA dataset** (7,560 instances over a 37,800-entry retrieval pool)
plus new **adaptability metrics** measuring how a reader responds to context quality:
noise vulnerability, context acceptability, context insensitivity, and context
misinterpretation, evaluated across retriever–LLM pairings. Note: the abstract frames
these as reader-behaviour metrics across configurations; it does not itself advertise a
verbatim "oracle-copy" upper bound — the closest analogue is *context insensitivity*
(reader ignores helpful context).

*Copy-ceiling difference:* MIRAGE is a dataset-plus-metric suite scored across model
pairs; the copy ceiling is a **per-item continuous exposure ceiling on a schema-level
ontology corpus**, printed beside each item's generation recall rather than aggregated
as a benchmark-level adaptability score.

## 3. RAGAS context recall (`es2023ragas`)
Es, James, Espinosa-Anke & Schockaert, *RAGAS: Automated Evaluation of RAG*, EACL 2024
System Demonstrations (arXiv 2309.15217). **Important precision point:** the *paper*
proposes three **reference-free** metrics — *faithfulness*, *answer relevance*, and
*context relevance* — and explicitly markets itself as not needing ground-truth
annotations. "**Context recall**" is *not* defined in the paper; it is a metric added
later in the RAGAS *library*, and it is **LLM-judged and ground-truth-dependent**: the
fraction of ground-truth-answer sentences that can be attributed to the retrieved
context. So if the paper cites "RAGAS context recall", attribute it to the library, not
to Es et al. 2023, and note it is an LLM-graded retrieval-coverage metric.

*Copy-ceiling difference:* context recall is **LLM-judged** and scores the *retriever's*
coverage of the gold answer; the copy ceiling is **deterministic** and scores the
*ceiling on generation recall* achievable by copying the injected context verbatim —
an exposure upper bound on the same axis the generator is scored on, with no judge in
the loop.

## 4. Answer-presence audit (`answerpresence2026`) — WITHDRAWN
Li et al., *Answer Presence Drives RAG Rewriting Gains* (arXiv 2606.05633).
A **causal intervention audit**: it asks whether the F1 lift from an LLM rewriter is
driven by the gold answer string literally appearing in the rewritten context rather
than by curation. Method uses **controlled edits** — remove the gold span, substitute a
**length-matched random non-answer placebo**, or inject the gold — and a five-sentinel
leakage probe. **The authors withdrew it (2026-08-03) after finding errors in
Sections 3–4 that undermine the main conclusions.** Cite with care, or only for the
*method* (paired placebo / intervention design), flagging the withdrawal.

*Copy-ceiling difference:* this is the nearest neighbour on **experimental design** —
its length-matched placebo is the same instinct as the paper's seed-disjoint placebo.
But it is an **intervention on a rewriter's output** to explain a pipeline gain, not a
**standing per-item exposure ceiling** computed and printed for every item regardless of
outcome. The copy ceiling reports the upper bound as a permanent control column; the
answer-presence audit perturbs the context to attribute a gain.

## 5. SeedRG (`seedrg2026`)
Liu, Zhang, Jin & Neville, *Generating Leakage-Free Benchmarks for Robust RAG
Evaluation* (arXiv 2605.08838). Introduces **SeedRG**, a semi-synthetic **benchmark
generator** that fights knowledge leakage (questions answerable from parametric memory
without retrieval) and benchmark aging: it extracts a **reasoning graph** from
question–context pairs and applies **type-constrained entity replacement**, with a
reasoning-graph consistency check and a leakage filter. (The old bib entry's subtitle
"a Gold-Context Upper Bound" was fabricated — the real paper has no gold-context
upper-bound construct; corrected.)

*Copy-ceiling difference:* SeedRG operates at the **corpus-construction** level to make
benchmarks leakage-free; the copy ceiling operates at the **per-item measurement** level
and is deterministic. The shared spirit is anti-leakage — the paper's verified
seed-IRI-disjoint placebo is the measurement-side analogue of SeedRG's leakage filter —
but SeedRG builds datasets whereas the copy ceiling is a standing per-item control.

## 6. Copy-as-Decode (`copyasdecode2026`) — WITHDRAWN, name collision only
Liu, *Copy-as-Decode: Grammar-Constrained Parallel Prefill for LLM Editing* (arXiv
2604.18170, withdrawn 2026-05-24 over an authorship dispute). Unrelated domain:
a **decoding-layer editing mechanism** where the model emits `<copy lines="i-j"/>` /
`<gen>…</gen>` programs and copy spans are resolved by parallel prefill. It uses the
term **"copy ceiling"** to mean *token reachability* — the fraction of gold output
tokens reachable under a line-level copy primitive (74–98% on its edit corpora).

*Use:* a one-line footnote disambiguating terminology only. Same words, different
construct (token reachability in code/text editing vs graph-grounded RAG exposure
coverage). Note it is withdrawn if you cite it.

---

## How far "first standing control" can be pushed

**Do not claim "first standing control" unqualified.** Adjacent constructs exist:
- RAGAS-library *context recall* already reports a per-item retrieval-coverage number
  (but LLM-judged, retrieval-side, ground-truth-dependent).
- MIRAGE's *context insensitivity* already probes whether the reader uses available
  context (but as a benchmark-level adaptability score).
- The answer-presence audit already pairs a **length-matched placebo** with a gold-span
  manipulation (but as a one-off causal intervention, not a standing column; and it is
  withdrawn).

**What is defensibly novel** is the *specific combination*, none of which any single
prior work reports together:
1. **Deterministic / judge-free** — no LLM extracts claims or grades coverage;
2. **Per-item and continuous** — one exposure number per item, not a dataset-level
   aggregate;
3. On a **schema-level ontology corpus** (graph-grounded RAG), not free-text passages;
4. Reported as a **standing per-item control printed beside generation recall** — a
   permanent column, not a diagnostic run or an intervention;
5. **Paired with a verified seed-disjoint (cross-domain) placebo**, so exposure is read
   against a matched null.

**Suggested retreat wording:** replace "the first standing control" with something like
"a *deterministic, judge-free per-item exposure ceiling* reported as a standing control
beside generation recall — to our knowledge the first time an exposure upper bound is
printed per item alongside the generation metric it bounds, rather than as an LLM-graded
retrieval metric (RAGAS context recall), a benchmark-level adaptability score (MIRAGE),
or a one-off causal intervention (answer-presence audit)." Then cite `es2023ragas`,
`mirage2025`, `ragchecker2024`, and `answerpresence2026` as the adjacent-but-distinct
prior art, with the copy-as-decode footnote for the name clash.
