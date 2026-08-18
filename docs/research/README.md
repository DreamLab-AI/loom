# Loom research: the evidence base

The node was measured. This directory holds the paper that measures it, its data and harness,
and the earlier uplift reports that preceded it. The paper is the node's primary evidence base;
read it before trusting a number quoted elsewhere in the docs.

## The paper (primary evidence)

**[`paper-v2/main.pdf`](paper-v2/main.pdf)**: *The Copy Ceiling: An Input-Exposure Control for
Ontology-Grounded Generation over Private Corpora* (22 pp, 2026-08-18). The frozen preprint.

What it establishes, in its own softened register:

- **A serving-regime verdict.** On a private in-domain corpus, unaided models score ≈ 0.26 and
  grounded ≈ 0.92, but across ten models from five providers the *gain over copy* (grounded
  recall minus the recall a verbatim copy of the shown context would already score) is uniformly
  negative (−0.067 to −0.022). A direct exposure/recovery decomposition of 11,360 gold items
  finds three unexposed items recovered in total, with per-model context utilisation of
  0.900 to 0.955. The reading is measured, not inferred: the model adds faithful delivery of the
  exposed facts, not reasoning over the injected structure.
- **A copy-fidelity deficit.** Models differ almost entirely in how much surfaced gold they drop:
  the paper's phrase is that a model "drops about one exposed item in fourteen". That deficit,
  not any beyond-exposure recovery, is what the negative gain measures.
- **A production paired study.** Holding the model constant and varying only the serving path,
  loom−raw lifts judged quality by +0.27 pooled (p = 0.004) and +0.79 where curation is deepest
  (the arcane set), scored by a cross-family judge (`openai/gpt-4.1`). A four-arm negative-control
  design is consistent with content-specific transfer: the served block lifts quality +0.59; the
  placebo, verified irrelevant by seed-disjoint construction (not demonstrated empirically inert),
  has a point estimate near zero, +0.04, though differential attrition leaves the contrast
  imprecise.
- **Out-of-domain non-regression.** The gated node does not regress off domain: the general-set
  delta is +0.05, [0.00, +0.13], inside a pre-specified ±0.25 equivalence margin (TOST).
- **A budget interaction.** At a fixed reasoning budget, think-tokens can exhaust the budget on
  long scaffolds and return empty (42 of 234 completions at `max_tokens=1536`); the empty rate
  rises with scaffold length and disorder. Reasoning-capable backends need `max_tokens ≥ 1536`.

These findings drive three shipped features (verbatim serving, exposure telemetry, thinking and
budget control); see the [README serving controls](../../README.md#findings-driven-serving-controls) and
[`design/LOOM-POSITIONING.md`](../design/LOOM-POSITIONING.md).

LaTeX source (`paper-v2/main.tex`) and figures ship alongside the PDF. `paper-v2/` is the frozen
preprint; do not edit it.

## The data and analysis

Per-row data and the derived decompositions live under
[`../../uplift-results/paper-v2/`](../../uplift-results/paper-v2/):

| File | What it is |
|---|---|
| `live-results.jsonl` | the production paired study rows (loom vs raw, per question) |
| `control-results.jsonl` | the four-arm negative-control rows (true / shuffled / masked / irrelevant) |
| `judged.json` | the cross-family judge's 0 to 5 gradings per answer |
| `analysis.json` | paired bootstrap, Wilcoxon, matched-pairs rank-biserial, Holm |
| `decomposition.json` | the item-level exposure/recovery decomposition (11,360 gold items) |
| `DECOMPOSITION-SUMMARY.md` | the 2×2 exposure/recovery summary in prose |

## The harness

The tools that produced the paper live under [`../../tools/paper/`](../../tools/paper/):

| Tool | Role |
|---|---|
| `ontology_scaffold_v1.py` | fetches the LLM-free `/loom/scaffold` block used by every arm |
| `live_harness.py` | runs the production paired study (loom vs raw) |
| `control_harness.py` | runs the four negative-control arms |
| `retry_empties.py` | the budget-exhaustion re-run at 4096 (preserves pairing) |
| `judge_v2.py` | the cross-family reference-guided rubric judge |
| `analyze.py`, `analyze_controls.py` | the paired and control-contrast statistics |
| `decompose_exposure.py` | the item-level exposure/recovery decomposition |
| `rewrite_v4.py` | the paragraph re-voicing harness (produces paper-v3, below) |

## paper-v3: the model-re-voiced edition (generated)

`paper-v3/` is a model-re-voiced edition of the paper: each rewriteable paragraph passed through
`rewrite_v4.py` (Qwen3.8-27B Heretic, `--no-think`) under mechanically-enforced invariant checks,
chosen over thinking mode for equal acceptance at roughly 5.5× less wall-clock (see
[`paper-v4-smoke-notes.md`](paper-v4-smoke-notes.md) and `paper-v3/REWRITE-REPORT.md`). It is
generated output, not a re-measurement: the claims and numbers are paper-v2's. Do not edit
`paper-v3/`; it is produced by the harness.

## Earlier uplift reports (precursors)

These predate the paper and use the earlier raw-vs-scaffold framing. Read them as the precursor
studies the paper re-centres against a copy ceiling, not as independent corroboration of a
reasoning claim.

- [`ontology-uplift-report.pdf`](ontology-uplift-report.pdf): the typeset two-study report
  (*Does Grounding an LLM in a Formal Ontology Actually Work?*).
- [`report.md`](report.md): the local-model uplift report (Gemma, Muse).
- [`report-gemini-3.7-flash.md`](report-gemini-3.7-flash.md): the first cloud-model companion.
- [`evidence/`](evidence/): per-run logs and JSON for the model sweep.
- [`preprint/`](preprint/): the earlier preprint scaffold, survey and differentiation notes.
