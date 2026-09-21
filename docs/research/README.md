# Loom research: the evidence base

The node was measured. This directory holds the manuscript that measures it, its per-row
evidence, the harness that produced both, and the superseded versions kept for provenance.
Read the current manuscript before trusting a number quoted elsewhere in the docs, and read
its limitations before quoting a number from it.

## Current manuscript

**[`paper-v8/main.pdf`](paper-v8/main.pdf)** — *The Copy Ceiling: An Input-Exposure Control
for Ontology-Grounded Generation over Curated Corpora* (26 pp, 21 September 2026).

This is the version to cite. It is the response to the first external review and it claims
considerably less than its predecessors did.

**What it establishes, in its own register.** When a model answers from a curated corpus and
the gold answers derive from that same corpus, headline "grounding uplift" mixes delivery of
facts already shown to the model with reasoning over the injected structure, and nothing in
the usual reporting separates them. The paper proposes *exposure accounting*: classify every
gold item by whether the shown context exposed it and whether the answer recovered it, and
report the four counts. The copy ceiling — the recall a verbatim copy of the shown context
would score — is that table's scalar. Measured across ten models, unaided recall averages
0.26 and grounded recall 0.92, yet gain over copy is uniformly negative (−0.067 to −0.022),
and beyond-exposure recovery is three items in 11,360 observations of the same 1,136 target
instances. **The defensible reading is the narrow one: such a score does not establish
reasoning beyond answer-name exposure, and equally does not establish its absence.** Zero
gain is compatible with successful reasoning over names the context already shows. Three
results stand without the ceiling: rephrasing questions out of the graph's vocabulary
collapses exposure from 0.96 to 0.34 while the fallback designed for that case fires on 3 of
510 questions; on gold targets the corpus contains but the retrieved scaffold did not expose,
recovery falls from 0.121 bare to 0.004 grounded, so injection is followed by loss of recall
the bare model demonstrably has; and a paired production study lifts judged quality by +0.27
pooled. What the paper does **not** establish: relational correctness, precision against
fabrication, attribution accuracy, transfer beyond one corpus, or any architectural
prescription. "Delivery" throughout means recall of exposed gold target names and nothing
more.

The paper also carries the **write path**: a production case study in which scaffold grounding
raises ontology-term resolution 0.17 → 0.55 while *every* arm degrades judged page quality, and
the write-path lifecycle that result forced — assertions landing on unserved ledger pages,
a two-instrument pre-filter, and admission only through the graph's governed propose/approve
path. That part of the paper is deliberately embedded in the stack it was measured on: the
ingest pipeline and ledger writer run in [VisionFlow](https://github.com/DreamLab-AI/VisionFlow)
and promotion runs through the ontology-bridge governed write path. The instrument is
corpus-general; the write-path result is only checkable there.

**Disclosure that travels with the numbers.** The negative-control rerun reported in
§Negative controls — the five-arm common-retry-policy cohort, including the fluent-noise arm,
judged by `gemini-3.1-pro-preview` — has **no released per-row evidence**. Its completions,
its judge outputs and its generating script were never written to disk. Those figures are
reported-but-not-released and cannot be independently reproduced; only the historical
four-arm complete-case cohort at the 1536-token budget can. The manuscript discloses this in
its abstract, its release statement, the section itself and its limitations. Two further
holes are disclosed there: two of the routing companion's three judge rows, and the semantic
re-score's saved results and embedding cache.

Alongside the manuscript:

| File | What it is |
|---|---|
| [`paper-v8/REVIEW-2026-09-21-external.md`](paper-v8/REVIEW-2026-09-21-external.md) | the external review, verbatim |
| [`paper-v8/REMEDIATION-2026-09-21.md`](paper-v8/REMEDIATION-2026-09-21.md) | item-by-item disposition: fixed / verified / deferred-as-owed-experiment / disagreed |
| [`paper-v8/CHANGES-2026-09-21-v8.md`](paper-v8/CHANGES-2026-09-21-v8.md) | what left the paper, what was reworded, 31 pp → 26 pp |
| [`paper-v8/MANIFEST.md`](paper-v8/MANIFEST.md) | table-to-artefact manifest: every table and figure mapped to its rows and generating script, or marked not-released |
| [`paper-v8/notes/`](paper-v8/notes/) | the three verification passes behind the ledger: fact-check, prior-work differentiation, recomputation from released rows |

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

## Reviewed version (superseded)

**[`paper-v6/main.pdf`](paper-v6/main.pdf)** — the v7 text (31 pp, 21 September 2026). This
is the manuscript the external review read, kept so the review can be checked against what it
reviewed. Its §Ideal State architecture section and its routing section do not survive into
v8, and several of its claims are retracted there — notably the "thirty times worse outside
the corpus" reading of the suppression result, the content-specificity reading of the
controls, and the description of the placebo as a correctly specified floor. Do not cite it.
Its own predecessor's review and ledger are [`paper-v6/REVIEW-2026-09-11.md`](paper-v6/REVIEW-2026-09-11.md)
and [`paper-v6/REMEDIATION-2026-09-11.md`](paper-v6/REMEDIATION-2026-09-11.md).

## Frozen preprint

**[`paper-v2/main.pdf`](paper-v2/main.pdf)** — *…over Private Corpora* (22 pp, 2026-08-18).
The arXiv preprint as first posted. Frozen; do not edit. Read as a historical record: it
describes the placebo as verified irrelevant, reads the four-arm controls as consistent with
content-specific transfer, and frames the headline as delivery rather than reasoning. The
control rerun retracted the first two readings and the external review retracted the third.

`paper-v3/` is a model-re-voiced edition of paper-v2, generated by `tools/paper/rewrite_v4.py`
under mechanically-enforced invariant checks (see [`paper-v4-smoke-notes.md`](paper-v4-smoke-notes.md)
and `paper-v3/REWRITE-REPORT.md`). Generated output, not a re-measurement; its claims are
paper-v2's. `paper-v4/` and `paper-v5/` are intermediate drafts.

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
| `../../uplift-results/semantic-rescore/` | `rescore.py` and `RESCORE.md` only — the saved results, cache and one imported module are **absent** |

Inside `uplift-results/paper-v2/`:

| File | What it is |
|---|---|
| `live-results.jsonl` | the production paired-study rows, 234; mutated in place by the budget re-runs |
| `control-results.jsonl` | the **historical** four-arm control rows, 228 (true / shuffled / masked / irrelevant, 1536-token budget) |
| `judged.json` | the `gpt-4.1` rubric gradings, 360 |
| `judged-claude-opus46.json` | the third-family re-judge, 360, plus its six shard files |
| `analysis.json` | paired bootstrap, exact signed-rank, rank-biserial, Holm; control contrasts merged in under `"controls"` |
| `decomposition.json`, `DECOMPOSITION-SUMMARY.md` | the item-level exposure/recovery decomposition |

The five-arm control rerun's rows are **not here and do not exist**; see the disclosure above
and [`paper-v8/MANIFEST.md`](paper-v8/MANIFEST.md) for the full list of what is missing.

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
| `rewrite_v4.py` | the paragraph re-voicing harness that produced paper-v3 |

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
