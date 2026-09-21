# Loom research: the evidence base

The node was measured. This directory holds the manuscript that measures it, its per-row
evidence, the harness that produced both, and the superseded versions kept for provenance.
Read the current manuscript before trusting a number quoted elsewhere in the docs, and read
its limitations before quoting a number from it.

## Current manuscript

**[`gain-over-copy-paper/gain-over-copy-paper.pdf`](gain-over-copy-paper/gain-over-copy-paper.pdf)** — *The Copy Ceiling: An Input-Exposure Control
for Ontology-Grounded Generation over Curated Corpora* (28 pp, 21 September 2026, v9.2).

This is the version to cite. Every table, figure and headline number in it maps to a released
artefact and a released script ([`gain-over-copy-paper/MANIFEST.md`](gain-over-copy-paper/MANIFEST.md)); nothing in it
is reported without its observations.

**What it establishes.** When a model answers from a curated corpus and the gold answers derive
from that same corpus, headline "grounding uplift" mixes delivery of facts already shown to the
model with reasoning over the injected structure. The paper proposes *exposure accounting*:
classify every gold item by whether the shown context exposed it and whether the answer
recovered it, and report the four counts; the copy ceiling, the recall a verbatim copy of the
shown context would score, is that table's per-item scalar and a copy baseline, not an upper
bound. Across ten models, unaided recall averages 0.26 and grounded recall 0.92, yet gain over
copy is uniformly negative (−0.067 to −0.022) and beyond-exposure recovery is three items in
11,360 observations of the same 1,136 target instances. The reading is the narrow one: such a
score does not establish reasoning beyond answer-name exposure, and does not establish its
absence either. A stratified, model-judged audit of the matcher (423 units, gpt-4.1, quote-gated)
finds its credit relationally correct in 0.971 [0.941, 0.986] of cases, no false credit on
unexposed items, and three characterised failure modes; the corrected exposed-item correctness
is 0.905 against the matcher's 0.931. A fresh paraphrase stress set collapses the lexical
ceiling from 0.964 to 0.328 while the absence-keyed fallback fires on 2 of 506; on gold targets
the corpus contains but the retrieved scaffold did not expose, recovery falls from 0.121 bare to
0.004 grounded; a paired production study lifts judged quality by +0.27 pooled. On the four-arm
negative-control cohort, re-judged with gpt-4.1 and corrected over the whole contrast family,
only the served path against no context survives; whether the scaffold's specific content
matters is not established at that power. What the paper does **not** establish: precision
against fabrication, attribution accuracy, transfer beyond one corpus, or any architectural
prescription.

The paper also carries the **write path**: a production case study in which scaffold grounding
raises ontology-term resolution 0.17 → 0.55 while every arm degrades judged page quality, and
the write-path lifecycle that result forced. That part is deliberately embedded in the stack it
was measured on: the ingest pipeline and ledger writer run in
[VisionFlow](https://github.com/DreamLab-AI/VisionFlow) and promotion runs through the
ontology-bridge governed write path. The instrument is corpus-general; the write-path result is
only checkable there.

**What left the paper, and why.** The two-edge composition study and the five-arm control rerun
of earlier versions had no surviving observations and are not reported. The control rerun is
being repeated with rows persisted and will be added as v9.1 when it lands; the composition
study would be a new experiment. The typed-skill-routing study is a companion note
([`companion-routing/`](companion-routing/)) measuring a choice task against a ranker baseline,
a different instrument from the exposure scalar.

Alongside the manuscript:

| File | What it is |
|---|---|
| [`gain-over-copy-paper/MANIFEST.md`](gain-over-copy-paper/MANIFEST.md) | table-to-artefact manifest: every table, figure and headline number mapped to its released rows and generating script |
| [`gain-over-copy-paper/CHANGES-2026-09-21-v9.md`](gain-over-copy-paper/CHANGES-2026-09-21-v9.md) | what left, what was re-run, what changed, 26 pp → 24 pp |
| [`gain-over-copy-paper/gain-over-copy-paper-arxiv.zip`](gain-over-copy-paper/gain-over-copy-paper-arxiv.zip) | the arXiv bundle, verified to build standalone |
| [`gain-over-copy-paper/archive/v7/REVIEW-2026-09-21-external-1.md`](gain-over-copy-paper/archive/v7/REVIEW-2026-09-21-external-1.md), [`gain-over-copy-paper/archive/v8/REVIEW-2026-09-21-external-2.md`](gain-over-copy-paper/archive/v8/REVIEW-2026-09-21-external-2.md) | the two external reviews, verbatim |
| [`gain-over-copy-paper/archive/v8/REMEDIATION-2026-09-21.md`](gain-over-copy-paper/archive/v8/REMEDIATION-2026-09-21.md) | the first review's item-by-item disposition; the second review's dispositions are the v9 change log and the artefacts below |
| [`gain-over-copy-paper/archive/v8/notes/`](gain-over-copy-paper/archive/v8/notes/) | the verification passes: fact-check, prior-work, recomputation, number provenance (`R5-numbers.md`) |

## Companion note

**[`companion-routing/main.pdf`](companion-routing/main.pdf)** — *Gain over a Judge-Free
Ranker: Measuring a Typed-Decision Seam against a Strong Simple Baseline* (5 pp,
21 September 2026).

Split out of the paper on the reviewer's finding that it had quietly substituted a ranker
baseline for the exposure scalar and kept the name. On a discriminative choice over a curated
option set every candidate is shown by construction, so the exposed fraction is 1 and the
exposure-based gain degenerates to accuracy minus one; something else has to play the role of
the copy baseline. **The quantity the note reports is ranker-relative and is a different
instrument from the copy ceiling.** Its first finding is a fault in its own control: two
defects — a threshold sweep that could not reach its own optimum, and exclusion clauses
indexed as positive evidence — had manufactured 6.9 of an apparent 11.6-point advantage.
Repaired, a BM25 ranker reaches 83.7% against the local 4B engine's 88.4%. The corpus is
self-authored and single-seed, which is the binding limitation and is stated as such.
See [`companion-routing/HARNESS-NOTES.md`](companion-routing/HARNESS-NOTES.md) for the rig.

## Reviewed versions (superseded)

**[`gain-over-copy-paper/archive/v8/main-v8.tex`](gain-over-copy-paper/archive/v8/main-v8.tex)** — the v8 text (26 pp, source only) the second external review
read. It still reported the five-arm control rerun, the two-edge composition study and the
original paraphrase stress set, none of which had surviving observations; v9 removes the first
two and replaces the third with a fresh set. Do not cite it.

**[`gain-over-copy-paper/archive/v7/main-v7.tex`](gain-over-copy-paper/archive/v7/main-v7.tex)** — the v7 text (31 pp, 21 September 2026, source only). This
is the manuscript the first external review read, kept so the review can be checked against what it
reviewed. Its §Ideal State architecture section and its routing section do not survive into
v8, and several of its claims are retracted there — notably the "thirty times worse outside
the corpus" reading of the suppression result, the content-specificity reading of the
controls, and the description of the placebo as a correctly specified floor. Do not cite it.
Its own predecessor's review and ledger are [`gain-over-copy-paper/archive/v6/REVIEW-2026-09-11.md`](gain-over-copy-paper/archive/v6/REVIEW-2026-09-11.md)
and [`gain-over-copy-paper/archive/v6/REMEDIATION-2026-09-11.md`](gain-over-copy-paper/archive/v6/REMEDIATION-2026-09-11.md).

## Archive of earlier versions

Every earlier manuscript lives as source under [`gain-over-copy-paper/archive/`](gain-over-copy-paper/archive/): `v2/`
(the arXiv preprint as first posted, 2026-08-18), `v3/` (a model-re-voiced edition of v2,
generated by `tools/paper/rewrite_v4.py`), `v4/` and `v5/` (intermediate drafts), `v6/` and
`v7/` (the text the first external review read, with its review and ledger), `v8/` (the text
the second review read, with both reviews, its ledger, manifest and the verification notes),
`preprint/` (the earliest scaffold and survey notes) and `uplift-report/` (the LaTeX of the
precursor uplift report). The PDFs of those versions are not kept; each source records what it
claimed at the time and several of those claims were retracted. Do not cite them. The third
review is beside the current paper as `REVIEW-2026-09-21-external-3.md`.

## Design notes

[`design-notes/private-corpus-programme.md`](design-notes/private-corpus-programme.md) preserves
the private-corpus architecture programme from v7's "Ideal State" section as requirements, each
marked implemented, tested or untested, with the three retracted claims stated up front so they
are not reconstructed. The two-edge composition design survives as an appendix protocol in the
current paper; its earlier execution is not reported.

## Per-row evidence

**As of commits `ce137f5` and `0acd8da` (21 September 2026), the paper's per-row evidence is
under version control for the first time** — 278 tracked files under
[`../../uplift-results/`](../../uplift-results/). That directory is listed in `.gitignore`
and the evidence files are force-added, so a clone gets them but `git status` will not notice
new ones. Deliberately excluded: `uplift-results/ingest/` (52 MB of corpus SQL and concept
records, not per-row evidence) and `app/data/` (the 7.9 MB scaffold index and ontology TTLs,
which several scripts nonetheless need).

| Path | What it is |
|---|---|
| [`../../uplift-results/questions.jsonl`](../../uplift-results/questions.jsonl) | the frozen 510-question set (seed 42, 15 domains) |
| [`../../uplift-results/sweep/`](../../uplift-results/sweep/) | the ten-model sweep: results, scores, summaries, per-provider logs, `sweep-analysis.json` |
| [`../../uplift-results/paper-v2/`](../../uplift-results/paper-v2/) | Study 2 and the historical controls (below) |
| [`../../uplift-results/routing/`](../../uplift-results/routing/) | the routing-seam evidence for the companion note: 86-turn corpus, 115 rubrics, mined rows, per-run reports, embedding caches, 21 analysis scripts, and its own [`README.md`](../../uplift-results/routing/README.md) manifest |
| `../../uplift-results/{general,arcane,arcane-prose,thin-prose,quality}/` | the judged-arm study directories and the case-study candidates |
| `../../uplift-results/semantic-rescore/` | `rescore.py`, the reconstructed `embed_lib.py` and `make_copy_contexts.py`, `rescore_results.json`, `RESCORE-2026-09-21.md` (recomputed 2026-09-21; replicates); the 571 MB embedding cache is regenerable and excluded |
| `../../uplift-results/semantic-audit/` | the stratified model-judged matcher audit: frame, 423-unit sample, verdicts, judge cache, `SUMMARY.md`, blind human sample and guide; `tools/paper/semantic_audit.py` |
| `../../uplift-results/controls-rejudge-2026-09-21/` | the four-arm control cohort re-judged with gpt-4.1, family-wide Holm, common-intersection analysis, per-arm accounting |
| `../../uplift-results/paraphrase-stress/` | the fresh paraphrase stress set (506 accepted), per-question ceilings, `PARAPHRASE-2026-09-21.md`; `tools/paper/paraphrase_stress.py` |
| `../../uplift-results/recovered/` | the sweep-generating harness recovered from git history, and `RECOVERY.md` recording what could not be recovered |

Inside `uplift-results/paper-v2/`:

| File | What it is |
|---|---|
| `live-results.jsonl` | the production paired-study rows, 234; mutated in place by the budget re-runs |
| `control-results.jsonl` | the **historical** four-arm control rows, 228 (true / shuffled / masked / irrelevant, 1536-token budget) |
| `judged.json` | the `gpt-4.1` rubric gradings, 360 |
| `judged-claude-opus46.json` | the third-family re-judge, 360, plus its six shard files |
| `analysis.json` | paired bootstrap, exact signed-rank, rank-biserial, Holm; control contrasts merged in under `"controls"` |
| `decomposition.json`, `DECOMPOSITION-SUMMARY.md` | the item-level exposure/recovery decomposition |

The five-arm control rerun with a length-matched noise arm is being repeated with every row
persisted (`uplift-results/control-rerun-2026-09-21/`, `tools/paper/control_rerun.py`); it is
not in v9 and will be reported as v9.1 when complete.

## The harness

Under [`../../tools/paper/`](../../tools/paper/):

| Tool | Role |
|---|---|
| `ontology_scaffold_v1.py` | the vendored scaffold engine; regenerates the served block for scoring |
| `live_harness.py` | the production paired study (loom vs raw) |
| `control_harness.py` | the four negative-control arms |
| `retry_empties.py` | the budget-exhaustion re-run at 4096, preserving pairing; rewrites `live-results.jsonl` in place |
| `judge_v2.py` | the cross-family reference-guided rubric judge, resumable and shardable |
| `analyze.py`, `analyze_controls.py` | the paired and control-contrast statistics; `analyze.py` implements the exact conditional signed-rank test |
| `decompose_exposure.py` | the exposure/recovery decomposition, with the copy-as-answer invariant as a hard gate |
| `parametric_suppression.py` | the bare-versus-grounded comparison on unexposed items; **prints to stdout, writes no file** |
| `rewrite_v4.py` | the paragraph re-voicing harness that produced the archived v3 edition |

Study 1's sweep is driven by `bench/sweep/{launch-sweep.sh,run-one-model.sh,analyse.py}` and
the judged arms by `bench/quality/`. **Caveat:** the Python harness both call,
`bench/bench_ontology_uplift.py`, has been deleted from the repository (see
`bench/LEGACY-PYTHON-NOTE.md`), and `bench/sweep/analyse.py` still imports it. The sweep rows
are released as data without their generator. The routing rig is snapshotted at
[`../../tools/routing-eval/`](../../tools/routing-eval/); the live crate is in the agentbox
repository.

## Earlier uplift reports (precursors)

These predate the instrument and use the raw-versus-scaffold framing it exists to re-centre.
Read them as precursors, not as independent corroboration.

- [`ontology-uplift-report.pdf`](ontology-uplift-report.pdf) — the typeset two-study report.
- [`report.md`](report.md) — the local-model uplift report (Gemma, Muse).
- [`report-gemini-3.7-flash.md`](report-gemini-3.7-flash.md) — the first cloud-model companion.
- [`evidence/`](evidence/), [`preprint/`](preprint/) — per-run logs, and the earlier preprint
  scaffold and survey notes.
