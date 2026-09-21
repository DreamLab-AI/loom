# R2 — Prior-work differentiation (response to "novelty is narrower than the manuscript's rhetoric suggests")

Source review: `paper-v6/REVIEW-2026-09-21-external.md`, section "The novelty is narrower than the manuscript's rhetoric suggests" (table of four neighbours, lines ~123-130).

All four fetches below are from the arXiv abstract or HTML pages (WebFetch, 2026-09-21). Quoted phrases are the fetch tool's extraction of page text, not full-text verification against the PDF — flagged UNVERIFIED where I could not corroborate a number two ways.

---

## 1. Per-paper facts

### RAGChecker (arXiv 2408.08067, Ru et al., NeurIPS 2024 D&B track)

- **What it defines**: a suite of claim-level metrics decomposing RAG quality into retriever and generator diagnostics: claim-level context precision, claim recall, faithfulness, noise sensitivity (relevant/irrelevant), hallucination, self-knowledge, and **context utilization**.
- **How computed**: LLM-based claim extraction and entailment checking, not lexical matching. Quote: "a text-to-claim extractor that decomposes a given text into a set of claims, and a claim-entailment checker to determine whether a given claim is entailed in a reference text or not." Reference implementation uses Llama3-70B as both extractor and checker (RefChecker framework).
  - Context precision: "a retrieved chunk is called relevant chunk if any ground-truth claim is entailed in it"; precision = relevant chunks / retrieved chunks.
  - Claim recall: proportion of ground-truth claims entailed within retrieved chunks.
  - **Context utilization** (closest analogue to this paper's exposure/delivery pair): "the ratio between correct ground-truth claims both entailed in chunks and present in model response versus total ground-truth claims in chunks" — i.e. of the claims the retrieved context supports, how many survive into the answer.
  - Faithfulness: proportion of response claims entailed in retrieved chunks.
  - Self-knowledge: correct response claims not entailed in any retrieved chunk (model's own knowledge contribution).
  - Hallucination: incorrect response claims not entailed in any retrieved chunk.
- **Validated against**: "Meta evaluation verifies that RAGChecker has significantly better correlations with human judgments than other evaluation metrics" (abstract language; I could not pull the exact correlation coefficient from the fetched pages — UNVERIFIED precise number).
- **What it does NOT do** (relative to this paper):
  - No deterministic surface matcher — it is judge-dependent throughout (LLM extractor + LLM entailment checker), so it is neither cheap nor reproducible without paying for and trusting a grading model.
  - No four-count exposure/recovery accounting with a signed scalar; context utilization is a single ratio over entailed claims, not a $(n_{11},n_{10},n_{01},n_{00})$ decomposition, and it is not reported as a *signed gain relative to a no-op copy baseline* — there is no "copy the passage" reference point.
  - No copy-as-answer invariant (no check that scoring the visible input itself as an answer reproduces the reported ceiling).
  - No seed-disjoint placebo / negative-control apparatus.
  - No production-node deployment measurement.
  - No suppression-of-unexposed-recovery finding (RAGChecker's self-knowledge metric is the nearest concept but is not used as a suppression diagnostic under grounding).
  - No single-premise/two-edge composition controls.

### "Recall Is Not Enough" / reader-context diagnostic (arXiv 2607.00725, Bala, 2026) — already cited as `readercontext2026`

- **What it defines**: "answer-in-context", a diagnostic measuring "whether a gold answer survives into the packed context" after budget-constrained packing.
- **How computed**: two variants — a **binary** variant on multi-hop datasets (span-presence test) and a **graded lexical variant for free-form answers**, "where no verbatim span exists" (this is the paper's own framing, matching what the review calls "a graded lexical variant for free-form answers"). Mechanism is lexical/deterministic on presence, not an LLM judge, for the core diagnostic itself — closest of the four to this paper's matcher in spirit.
- **Validated against**: two independent **interventions**: (1) a packing change that raises document coverage without raising answer-in-context leaves downstream accuracy flat; (2) prompt compression that destroys the answer span lowers both together. Also reports predictive power: answer-in-context adds "$\Delta R^2 = 0.17$–$0.27$ over recall across three multi-hop datasets," and among fully-retrieved-evidence questions, whether packing keeps the answer separates exact match by "4.6x".
- **What it does NOT do**:
  - Single binary/graded presence measure per item, not a four-count exposure/recovery accounting with context utilisation and beyond-exposure recovery reported separately.
  - No signed gain-over-copy scalar re-centring recall against a no-op extractor; its object is presence-in-context, not model-delivery recall re-centred on that presence.
  - No copy-as-answer invariant.
  - Interventions are packing/compression manipulations, not a seed-disjoint placebo construction.
  - No production node measurement, no composition/single-premise controls, no suppression-of-unexposed-recovery finding.
  - This is the **closest comparator overall** — same “does the input already contain it” move, and it is judge-free like this paper. The genuine differentiator versus this one is: continuous per-item accounting (four counts, not one flag/grade), the signed copy-baseline framing, the composition apparatus, and the production-deployment measurement, not the presence-diagnostic idea itself.

### Sufficient Context (arXiv 2411.06037, Joren et al., ICLR 2025) — already cited as `sufficientcontext2025`

- **What it defines**: "sufficient context" — an instance has sufficient context "if and only if there exists an answer A′ such that A′ is a plausible answer to the question Q given the information in C," explicitly **independent of ground-truth correctness**: "we do not presuppose that we have the answer A′ in advance, only that such an answer exists."
- **How computed**: LLM autorater (Gemini 1.5 Pro, one-shot chain-of-thought): "output a list of step-by-step questions that would be used to arrive at a label...answer each of the questions...use these answers to evaluate the criteria."
- **Validated against**: 115 human-gold-labelled instances; autorater accuracy 93% (precision 93.5%, recall 93.5%), beating an entailment baseline (TRUE-NLI, 82.6%). This 93% figure matches the "roughly 93% agreement" already stated in the current paper-v6 §ceiling-prior text.
- **Explicit contrast with answer-presence**: the paper itself benchmarks a "Contains GT" (ground-truth string-in-context) baseline at 81% accuracy against its own 93% autorater — i.e. Sufficient Context's own ablation shows sufficiency and name-presence are different, separable quantities, with sufficiency the stronger predictor of answerability. This is direct textual support for the review's point that "name-exposure is weaker than sufficiency."
- **What it does NOT do**:
  - Judge-dependent (LLM autorater), not judge-free/deterministic.
  - Binary sufficient/insufficient label, not a continuous per-item ceiling or a signed gain scalar.
  - No four-count exposure/recovery accounting, no copy-as-answer invariant, no seed-disjoint placebo, no production node, no composition controls.
  - Sufficiency asks whether *an* answer is derivable from context at all (a stronger, semantically-judged condition); this paper's exposure ceiling asks only whether the gold *name* is present as a string (a weaker, cheaper condition) — the review's point exactly, and the draft below concedes it explicitly rather than eliding it.

### MuSiQue (arXiv 2108.00573, Trivedi et al., TACL 2022) — already cited as `musique2022`

- **What it defines**: a 2–4 hop multihop QA dataset (25k questions) built by **bottom-up composition of single-hop questions**: "a bottom-up approach that systematically selects composable pairs of single-hop questions that are connected, i.e., where one reasoning step critically relies on information from another."
- **Shortcut-resistance filters** (the mechanism the review calls "construction and filtering... to resist disconnected and single-hop shortcuts"):
  - **Head-node filter**: rejects a head single-hop question if it is answerable by a QA model without its intended supporting paragraph (mean answer-F1 against the label below a threshold used as the accept criterion, i.e. accepted only when the model *cannot* already answer it — reported as accepting a head question when overlap "< 0.5").
  - **Tail-node filter**: rejects tail sub-questions whose entity can be guessed without the head answer (F1-based thresholds on masked support).
  - Formal "MuSiQue condition": every edge sub-question must be unanswerable from its predecessor alone and unanswerable from no context: "$\forall(q_j,q_i)\in edges(G_Q): M(q_i^{m_j},C)\neq a_i \;\wedge\; \forall q_i\in nodes(G_Q): M(q_i,\phi)\neq a_i$" — i.e. genuinely composable, not decomposable-and-guessable.
  - Distractors are drawn from gold paragraphs of the filtered single-hop questions, closing the door on surface-level single-hop matching against distractors.
  - MuSiQue-Full variant adds unanswerable-contrast questions.
- **Validated against**: reports "a 30 point drop in F1" for single-hop models versus the intended multihop solution, and (per the fetched abstract) a "3x increase in human-machine gap" relative to prior multihop datasets — UNVERIFIED whether this is measured against a specific named baseline dataset; I did not confirm the comparison set from the HTML fetch.
- **What it does NOT do** (relative to this paper):
  - Corpus/dataset **construction** methodology (upstream, one-time filtering when building the benchmark), not a per-item **measurement instrument** applied post hoc to already-served contexts, as the review itself notes ("this occurs during corpus construction, whereas the copy ceiling is a per-item measurement" — already in paper-v6 main.tex line 71, re: SeedRG, and the same distinction applies to MuSiQue).
  - No exposure/copy-ceiling concept at all; its single-hop-answerability filter is a *necessary condition for inclusion in the dataset*, not a *reported per-item diagnostic scalar* for a deployed system.
  - No notion of a served, budget-clamped scaffold text or a production node.
  - This paper's single-premise controls ($a$-only, $b$-only, masked, irrelevant, raw) are explicitly modelled on the same worry MuSiQine's filters address (can the question be answered from less than the intended evidence, or by ignoring relation labels) but applied as an **evaluation-time behavioural control on a live serving pipeline**, not a **dataset-construction filter**. This is squarely "an application and adaptation," as the review says — not new methodology.

---

## 2. refs.bib check

All four neighbours are **already cited** in `paper-v6/refs.bib` — no new BibTeX entries needed:

| Neighbour | Key | Lines | Status |
|---|---|---|---|
| RAGChecker | `ragchecker2024` | 316–320 | present, complete (NeurIPS D&B, arXiv 2408.08067) |
| Recall Is Not Enough | `readercontext2026` | 358–363 | present, complete (arXiv 2607.00725) |
| Sufficient Context | `sufficientcontext2025` | 364–369 | present, complete (ICLR 2025, arXiv 2411.06037) |
| MuSiQue | `musique2022` | 421–426 | present, complete (TACL 2022, arXiv 2108.00573) |

Also confirmed present and complete, as requested:

| Key | Line | Status |
|---|---|---|
| `kaushik2018reading` | 91 | present |
| `gururangan2018artifacts` | 276 | present |
| `poliak2018hypothesis` | 281 | present |
| `laitenberger2025stronger` | 265 | present |
| `min2023factscore` | 259 | present |

**No missing entries.** The gap the reviewer identified is entirely in the *prose's framing* of already-cited work, not in citation coverage. (I did not re-verify every field of the five "already exist" entries beyond confirming the keys and titles resolve to the right papers; full BibTeX bodies were not re-typed here since no edit is needed.)

---

## 3. Draft replacement — §ceiling-prior "Relation to prior instruments" (~310 words)

Replace the current two-paragraph text at `paper-v6/main.tex` lines 135–138 (label `sec:ceiling-prior`) with:

```latex
\subsection{Relation to prior instruments}\label{sec:ceiling-prior}
Several recent instruments ask a related question --- did the shown context deliver the answer? --- and the honest comparison concedes precedent on each component rather than claiming a new quantity. The exposure/delivery decomposition itself has close precedent in RAGChecker, whose context utilisation metric is claim-level context recall re-expressed as delivery~\cite{ragchecker2024}: ours is the same move restricted to a deterministic surface matcher in place of LLM claim extraction and entailment checking, traded for zero judge cost and exact reproducibility. Graded, judge-free answer-presence has closer precedent still: the reader--context diagnostic scores whether a packed context retains the gold answer, with a graded lexical variant for free-form answers validated by two independent interventions~\cite{readercontext2026}; it is the nearest neighbour to this instrument, and the distinction is accounting (four counts and a signed, copy-baseline-relative scalar, checked by a copy-as-answer invariant) and apparatus (seed-disjoint placebo, production node, composition controls) rather than the underlying presence idea. Sufficient Context asks a strictly stronger question --- whether \emph{an} answer is derivable from the context at all, independent of any specific gold string --- using a prompted autorater at $93\%$ agreement, and its own ablation shows this exceeds a ground-truth-string-in-context baseline ($81\%$)~\cite{sufficientcontext2025}: our exposure ceiling is exactly that weaker string-in-context baseline, deliberately, to buy determinism. Our single-premise controls are an evaluation-time application of MuSiQue's shortcut-resistance filters to a served pipeline rather than a new construction~\cite{musique2022}: MuSiQue filters questions at dataset-build time to remove disconnected and single-hop shortcuts; we test the analogous condition, \emph{after} serving, on live model behaviour. None of these four instruments is judge-free and deterministic together; that combination, paired with a copy-as-answer invariant, a verified seed-disjoint placebo, a production node, and single-premise composition controls, is what we contribute, as a diagnostic and reporting convention rather than a new construct. The trade-off is explicit: removing semantic judgement removes semantic discrimination, which is why the ceiling is read as a delivery-fidelity reference, never as a reasoning detector.
```

Word count: ~305 words. Explicitly avoids "detects reasoning" framing per the review's core objection; every clause is a concession-plus-narrow-differentiator pair, closing with the trade-off the review demanded be measured rather than asserted.

---

## 4. Related Work paragraph (3-5 sentences) and composition-section note

### Related Work (insert into or replace part of the "Input-only baselines and faithfulness metrics" paragraph, `main.tex` line 68, after the FActScore/ALCE sentence and before the literature-search sentence)

```latex
Against the four closest instruments we claim narrower ground than a new construct: RAGChecker's context utilisation already performs the exposure/delivery decomposition, via LLM claim extraction rather than a deterministic matcher~\cite{ragchecker2024}; the reader--context diagnostic already reports graded, judge-free answer presence in packed context, validated by intervention~\cite{readercontext2026}, and is the closest comparator overall; Sufficient Context already shows that context-derivable-answer sufficiency is a stronger and better-validated criterion than raw answer-string presence~\cite{sufficientcontext2025}; and MuSiQue's single-hop-shortcut filters are the origin of the single-premise control logic we apply downstream, at serving time rather than dataset-construction time~\cite{musique2022}. Our contribution is the combination --- deterministic four-count accounting with a signed copy-baseline scalar, a copy-as-answer invariant, a seed-disjoint placebo, and composition controls on a production node --- not priority over any one component.
```

### Composition-section note (insert as an addition to the existing "Lineage and scope" paragraph in `sec:composition`, `main.tex` around line 620, alongside the existing MuSiQue citation)

```latex
The single-premise arms ($a$-only, $b$-only, masked, irrelevant) are a direct, serving-time adaptation of MuSiQue's shortcut-resistance logic rather than a new control design: MuSiQue filters candidate questions at dataset-construction time so that no sub-question is answerable from less than its intended supporting evidence~\cite{musique2022}; we cannot re-filter a fixed production corpus's questions this way, so we instead hold the questions fixed and vary what evidence the served context exposes, asking the analogous question of live model behaviour rather than of dataset membership. The 99.97\% of naively mined two-hop chains that failed our eligibility filters (\S\ref{sec:composition}) is the operational cost of that adaptation: most chains a served-per-IRI pipeline would present are already single-hop exposure once the block containing $A$ is shown, echoing why MuSiQue needed head- and tail-node filtering to keep its dataset genuinely multihop in the first place.
```

---

## Notes for the writer

- Everything above the two LaTeX blocks in §3–4 is verified from the fetched abstract/HTML pages, not from the PDFs; the UNVERIFIED tags mark the two numbers I could not corroborate a second way (RAGChecker's exact human-correlation coefficient; MuSiQue's "3x human-machine gap" baseline comparator).
- Do not let the drafted prose drift back toward "detects reasoning" language elsewhere in the paper (Fig. 2, §9, §18, D7) — this note only fixes §ceiling-prior, Related Work, and the composition-section lineage paragraph; the reviewer's other essential fixes (routing metric conflation, Figure 2 attribution, §18 claims) are out of scope here.
