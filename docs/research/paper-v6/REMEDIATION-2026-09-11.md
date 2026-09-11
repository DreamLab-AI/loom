# Remediation of REVIEW-2026-09-11

Response to `REVIEW-2026-09-11.md`, applied 11 September 2026. Every numbered review
item is dispositioned below as **fixed** (manuscript/code/bib edited), **verified**
(recomputed from released artifacts; review claim confirmed or resolved), or
**blocked** (needs artifacts absent from this checkout; manuscript re-worded to claim
only what the present data support).

## Headline: the central results stand

The review's two most consequential items (1.1, 1.2) were flagged "impact needs
data". This checkout **does** contain the frozen questions (`uplift-results/questions.jsonl`,
510), all ten model sweep rows (`uplift-results/sweep/`), and the scaffold index
(`app/data/scaffold-index.json`), so both were decided by recomputation:

- **1.1 scorer asymmetry — numerically inert.** Gain over copy is identical to four
  decimal places for all ten models under (a) the published asymmetric scorer,
  (b) the symmetric any-collapsed scorer, and (c) scaffold-only exposure. Cause:
  every `gold_type=="any"` question's alternatives are exposed all-or-nothing in the
  frozen scaffolds (the 17 multi-alternative questions included), so the fractional
  vs collapsed ceiling never differ. The copy-as-answer invariant (copy text scored
  as an answer equals its reported ceiling) holds 510/510 under both definitions.
  `decompose_exposure.py` now runs both invariants as hard gates.
- **1.2 prompt-inclusive exposure — numerically inert for the scaffold arm; raw-arm
  claim corrected.** Exposure flags are identical for full-input vs scaffold-only on
  all 510 questions. Prompt-only (raw-arm) exposure is however NOT ≈0: 35/1,136 gold
  items appear in the bare question (mean raw ceiling 0.037). §3.1 of the paper now
  states the definition (full visible input) and both measurements.
- **3.6 suppression figures — verified exactly**: 92/760 raw vs 3/760 grounded,
  seven models at zero, recomputed from the released sweep rows (per-model 1–14/76).
- **2.1 Study-2 accounting — review confirmed**: 28 marked retry pairs, 25 complete
  rerun pairs + 89 original-budget = 114; exclusions asymmetric (loom 1, raw 2).
  Manuscript corrected (was "39 pairs", "empty in both arms").
- **2.2 statistics — review's exact p-values reproduced independently** (arcane
  0.0322265625, thin 0.1304931641, general 0.5, pooled 0.0023037884; rank-biserial
  0.670/0.431/1.0/0.589). The paper now documents and reports the exact conditional
  signed-rank test; `analyze.py` implements it (`exact_signed_rank`), corrects the
  "Pratt" mislabel, renames `cliffs_delta`→`sign_dominance` (alias kept), and Holm
  now runs over exact p-values. The advertised script reproduces Table `tab:live`.

## Manuscript changes (main.tex)

- **Abstract** rewritten to one paragraph, 1,897 chars (< arXiv's 1,920 form limit);
  exact p=0.002; "eight developer families"; raw range 0.151–0.375; seed-disjoint
  placebo (no "verified irrelevant"); margin claim as interval-inclusion with the
  58/60-ties caveat; composition sentence without "where copying cannot help".
- **§Ceiling**: raw-arm ceiling corrected (0.037, not ≈0); new "Scoring symmetry,
  stated and checked" paragraph; A2 rewritten as surface-proxy (substring/word-bag
  false positives; no strict lower-bound claim); "rules out reasoning" replaced with
  the attribution-scope reading; low ceiling = regime-exit flag, not synthesis
  sufficiency (1.5, 1.6).
- **§Rescore** rewritten to the saved threshold table: 0.90 is stricter and lowers
  both recall and ceiling below lexical; all 30 combos negative with CIs excluding
  zero; pooled n01 low single digits per threshold (3 at 0.85, max single-model 4);
  bge-small-en-v1.5 span method described; framed as robustness probe (1.3).
- **§Method/Statistics**: exact conditional signed-rank documented; TOST claim
  replaced by the actual 95%-interval-inclusion criterion (noted stricter than
  conventional TOST's 90%); bootstrap vs rank test targets distinguished; per-script
  bootstrap seeds (7 legacy, 42 decomposition/rescore) disclosed (2.2, 2.3, 4.1).
- **§Results**: 25/89 retry split; asymmetric exclusions; control attrition rates in
  correct order without the monotonic length/disorder claim; "common retry policy"
  not "uniform budget"; envelope difference (system-message controls vs last-user-
  message node injection) disclosed; controls-table additivity note (pairwise-
  complete subsets); Table `tab:live` exact p-values; general-set insensitivity
  (58/60 ties, means 5.0/4.95) stated (1.4, 2.1, 2.2, 3.2).
- **§Controls/analysis**: "confirmed underpowered", "underpowered artefact, not a
  suppressed signal", "fluent-noise confirms" → non-establishment wording; forest
  caption gradient softened, interaction-test absence stated (3.1).
- **§Judges**: "three independent families agree" → direct OpenAI-vs-Anthropic
  check; controls judge identified as different-cohort; judgeprov table: wrong
  dagger removed, Opus re-judge row added, identifiers harmonised, "ten extraction
  families" → "ten serving arms across six model families"; limitations item updated
  for the Opus re-judge; §controls no longer says Study 2 "was not re-judged" (3.4).
- **§OOD/bar**: five-model worst case explicitly does NOT establish equivalence;
  no matched always-inject ablation noted; bar row 2 and "excellent, attributable
  delivery" requalified (2.3, 3.1, 4.5).
- **§Composition**: contrast explained as netting out exposure (abstract fixed);
  "leads the composing class" → "sits with" (Gemini 3 Flash +0.33 > Qwen +0.29, no
  inter-model test); GPT-5.6 "ceiling effect" → "consistent with limited headroom";
  masked "collapses" → "sharply reduces"; 99.97% exclusion re-worded to the
  eligibility filters; context-bearing arms wording; multiplicity non-adjustment
  disclosed (2.5, 3.3).
- **§Gemma**: Qwen3.8-27B delivery saturation explicitly an inference (its sweep row
  is Qwen-2.5-72B); Gemma is the only measured local-family sweep (3.5).
- **§Practitioner**: input context vs completion allowance separated; operational
  17.9% first-attempt failure rate; complete-case vs service-effectiveness;
  temperature-0 determinism scoped (3.7).
- **§Suppression**: observed-behaviour framing; "disagree by construction" removed;
  provenance caveat for bare-arm hits; figures marked as recomputed from released
  rows (3.6).
- **§Write-path/lifecycle**: dilution "is the mechanism" → "offers a mechanism
  consistent with"; event-sourcing analogy scoped to the assertion layer with
  curated prose retained as a source (3.8).
- **§Limitations**: "Every in-domain target" → 96.4% with 76/model unexposed;
  1,136×10 instance pooling stated; bootstrap-scope sentence corrected; asymmetric
  exclusions; new local-model-identity item (alias vs quantised derivative
  checkpoint) (2.5, 3.5, 4.5).
- **Related work**: sweeping no-precedent claims narrowed; RAGChecker named as
  closest comparator with search scope stated (4.4).
- **Figures**: fig:gain whisker offsets regenerated at 4 d.p. from
  `decomposition.json` (labels were already correct) and `\addplot+` → explicit
  burnt style (fixes the blue-bar rendering); fig-forest caption/labels (exact p,
  margin wording, gradient); fig-ceiling cancellation caveat (2.4, 4.5).
- **Appendix**: retitled "excerpted" with elisions marked; worked-example notes on
  T-TAX gold scope and the prose flag (4.2).
- **Metadata**: fixed `\date{11 September 2026}` (no `\today`); author block single
  author with affiliation line (no `\and` second author); `pdftitle`/`pdfauthor`
  set (5.2).

## Bibliography (refs.bib)

- `jiang2025practicalgraphrag` → `min2025practicalgraphrag` with correct title
  (*Towards Practical GraphRAG: Efficient Knowledge Graph Construction and Hybrid
  Retrieval at Scale*) and authors (Min, Bansal, Pan, Keshavarzi, Mathew, Kannan);
  cite site updated.
- `entityquestions2022` → `entityquestions2021` as `@inproceedings`, EMNLP 2021,
  ACL Anthology URL; cite site updated.
- `copyasdecode2026` note aligned to the arXiv record's wording ("authorship and
  contribution review").

## Code (tools/paper/)

- `decompose_exposure.py`: copy-as-answer invariant (both ceiling definitions) and
  prompt-vs-scaffold exposure consistency run as hard gates; output unchanged
  (verified: same pooled 2×2, empty `errors`).
- `analyze.py`: `exact_signed_rank()` (exact conditional enumeration, DP over tied
  half-integer ranks); corrected reduced-sample-vs-Pratt docstring; `p_exact`
  reported and used for Holm; `p_raw` → `p_normal_approx`; `cliffs_delta` →
  `sign_dominance` with a documented compatibility alias. Verified to reproduce
  Table `tab:live` from `judged.json` + `live-results.jsonl`.

## Blocked on artifacts absent from this checkout

These cannot be recomputed here; the manuscript now claims only what present data
support, and the intro release sentence marks not-yet-present artifacts as
reported-but-not-yet-released:

- Uniform-budget control rerun rows + Gemini re-judgements (1.4): per-contrast n
  and question IDs not publishable from here; table caption now flags
  pairwise-complete subsets instead.
- Semantic rescore regeneration (1.3): `embed_lib.py`/caches absent; §rescore
  re-worded to the saved threshold table only.
- Opus re-judge outputs, composition chains/six-arm rows, Gemma rows,
  paraphrase/generation-check rows, page-quality outputs (4.1): release manifest
  work, not manuscript work.
- Exact request-level model provenance (3.5): deferred to the release manifest;
  limitations item added.

## arXiv packaging (5.x)

- Abstract now fits the 1,920-char metadata form.
- `\pdfoutput=1` retained (harmless; arXiv advises selecting pdfLaTeX explicitly —
  do so on the form).
- Rebuild + repackaged `arxiv-v6.zip` from this source; BBL regenerated with the
  BibTeX backend (format 3.3, aligned with TeX Live 2025's biblatex 3.20).
- Replacement workflow: use Replace on the existing record, ≤1 replacement/week,
  describe corrections in the comments field, review arXiv's generated PDF.
