# Recovery pass — 2026-09-21

Response to `docs/research/paper-v8/REVIEW-2026-09-21-external-2.md`'s "experimental
provenance" section, against the six gaps enumerated in
`docs/research/paper-v8/MANIFEST.md` ("What is not released"). Exhaustive search for
surviving artefacts; nothing in this repository's tracked tree, prior git history,
docs/paper drafts, or paper edits was touched. Everything below is either a *new* file
copied into `uplift-results/recovered/<study>/` or a documented negative result.

**Method:** (1) `git show`/`git log --all -p -S<term>` across every ref (`main`,
`master`, `dream/model-facade-2026-08-29`, `feat/confidence-contract`, all remotes) and
the reflog, for `fluent_noise`, `gemini-3.1`, `retry_budget`, `two_edge`/`two-edge`,
`chain_`, `paraphrase`, `stress`, `rescore`, `retry-policy`; (2) grep of all ~35 Claude
Code scratchpad session directories for `-home-devuser-workspace-loom*` and
`-home-devuser-workspace-dream-machine*` under `.tmp/claude-1000/`, plus top-level
`.tmp/*.txt`/`*.json`; (3) the pre-v6 paper snapshots (`paper-v4`, `paper-v5`,
`preprint`) and every `uplift-results/*` subdirectory not cited by the paper
(`quality`, `oracle`, `precise`, `focused`); (4) a filesystem-wide search for recent
large `.jsonl` files that might be host-side run logs. No SSH to the model host was
available or needed — no live logs of that kind exist on this container.

## Classification key (reviewer's own three situations)

- **(A)** Results retained, just unreleased → release as-is.
- **(B)** Inputs + code retained, outputs missing → recomputable by re-running.
- **(C)** Observations/questions/code not retained → must be re-run or the claim removed.

## Summary table

| # | Study | Found | mtime | Classification | Action |
|---|---|---|---|---|---|
| i | Control rerun (5-arm ITT, `gemini-3.1-pro-preview` judge) | Nothing beyond the historical 4-arm cohort already tracked | n/a | **C** — genuinely absent | Rerun, or cut the ITT/fluent-noise numbers |
| ii | Routing: cloud judge (`jev-1.13.0`) + 421M encoder per-item rows | Nothing (one unrelated 7-line smoke-test log found, not this corpus) | n/a | **C** for these 2 of 3 engines (3rd, `sso`/BM25, already **A**, salvaged at `ce137f5`) | Rerun with `--json` |
| iii | Semantic re-score results + embedding cache | Script + methodology doc already tracked (`uplift-results/semantic-rescore/`); `embed_lib.py`, `embed_cache.json`, `copy_contexts.json` never committed anywhere | n/a | **B** — inputs (sweep rows, questions) present, but the script is **not self-contained** (missing `embed_lib.py`) and the cache is gone | Rewrite `embed_lib.py` and re-run against the live embedding endpoint |
| iv | Sweep-generating harness (`bench_ontology_uplift.py`) | **Recovered** from git history, `e83bb32^` | 2026-09-02 21:50 UTC (parent commit) | **A** — fully recovered, confirmed by field-name match | Restore into the tree (see below) |
| v | Two-edge composition corpus, mining script, six-arm rows | Nothing beyond the aggregate numbers/plot coordinates already in the tracked paper text and figure | n/a | **C** for the corpus/rows/script; the **aggregate** per-model deltas and `fig:composition` coordinates are **A** (already released, in-paper) | Rerun to get per-item rows, or keep as reported-but-not-released |
| vi | Paraphrase stress-set, per-arm ceilings | Nothing found anywhere | n/a | **C** — genuinely absent | Rerun |

Three studies (i, v, vi) are confirmed to have **no artefact anywhere in the reachable
record** — not just this checkout. One (ii) is now two-thirds resolved by prior work
(`ce137f5`, `uplift-results/routing/`); the remaining third is absent. One (iii) sits at
B: the released `rescore.py` imports a module that was never committed, so it cannot
even be rerun without rewriting `embed_lib.py` first. One (iv) is now fully **A**.

---

## i. Control rerun (5-arm ITT) — **ABSENT, confirmed independently**

This exact question was already answered, exhaustively, by a prior session and is
committed at `docs/research/paper-v8/notes/R3-data.md` §3 ("Control rerun cohort —
ABSENT from this checkout"). I re-ran the same searches independently (git history
across all refs/reflog for `fluent_noise`, `gemini-3.1`, `retry_budget`,
`common-retry`/`retry-policy`) and found nothing beyond what R3-data.md already
documents:

- `uplift-results/paper-v2/control-results.jsonl` — 228 rows, **4 arms only**
  (true/shuffled/masked/irrelevant), no `retry_budget` field, no `fluent_noise` rows.
  This is the pre-rerun, attrition-biased 1536-token cohort — already tracked, already
  cited by the paper as "historical".
- `tools/paper/control_harness.py` — generates only the 4-arm cohort
  (`ARMS = ("true", "shuffled", "masked", "irrelevant")`), no 4096/8192 retry-budget
  logic, no fifth arm. Already tracked.
- `tools/paper/analyze_controls.py` — analyses only the 1536-budget cohort; explicitly
  skips any row carrying a `retry_budget` field. Already tracked.
- Git history: only the three original paper-v2 commits touch these files
  (`48ed93a`, plus two more referenced in R3-data.md); no rerun commit exists on any
  branch, in the reflog, or in a dangling blob reachable by `-S` search.
- Scratchpads: zero hits for `fluent_noise`, `gemini-3.1`, or `retry_budget` referring
  to actual data (the only scratchpad hits for `fluent-noise`/`gemini-3.1` are inside
  copies of the paper's own prose in LaTeX build scratch — see "Non-findings" below).

**Verdict: C.** No completions, no judge outputs, no generating script for the 5-arm
ITT rerun exist anywhere searched. The historical 4-arm cohort (situation A/B) remains
available and is already released.

## ii. Routing — cloud judge + 421M encoder — **ABSENT for 2 of 3 engines, confirmed**

`uplift-results/routing/README.md` (committed at `ce137f5`, 2026-09-21) already states
this precisely: the `sso`/BM25 engine's 86-turn corpus, judge picks and analysis
scripts are fully salvaged and released (situation A); "Not located as per-item
artefacts: the cloud judge (`jev-1.13.0`) run and the 421M shared-head encoder run...
if per-item JSON for those two rows exists it was never written to disk."

I searched independently for `jev-1.13.0` and `421M`/`421m-encoder` across all loom and
agentbox scratchpad sessions. One file matched literally:
`.../-home-devuser-workspace-project-agentbox/27dbd76e-.../scratchpad/route.jsonl` — 7
lines, 3 `"outcome":"routed"` rows, all `model:"jev-1.13.0"`, spanning 20 seconds on
2026-09-16. This is a live-hook smoke test of the routing wiring (fields:
`ts, consumer, outcome, model, choice, confidence, candidates, chars, ms, input_tokens,
usd`), unrelated to the 86-turn labelled corpus the paper's routing table needs — wrong
shape (no ground-truth label, no corpus ID), wrong size (3 rows vs. 86), and a different
purpose (infra debug, not an evaluation run). Not copied in; noted here as a
non-finding so it isn't mistaken for the missing data if searched again.

**Verdict: C** for the cloud-judge and 421M-encoder rows specifically; **A** already for
the third engine (unchanged from `ce137f5`).

## iii. Semantic re-score — script tracked, cache and dependency missing — **B, but incomplete**

`uplift-results/semantic-rescore/rescore.py` and `RESCORE.md` are already tracked
(commit `9ef671e`, 2026-08-23). Reading `rescore.py`: it imports
`from embed_lib import EmbedCache, cosine, extract_candidates, normalise_title` from
its own directory, and reads `copy_contexts.json` and writes/reads
`embed_cache.json`, also expected in that same directory.

None of `embed_lib.py`, `copy_contexts.json`, or `embed_cache.json` were ever committed
to this repository (`git log --all --diff-filter=A` for each: zero hits; the only two
files `9ef671e` added are `RESCORE.md` and `rescore.py` themselves) and none exist in
any scratchpad session searched.

**What is genuinely recomputable (B):** the two required inputs the re-score depends
on — `uplift-results/sweep/results-*-scaffold.jsonl` (the 510×10 sweep rows) and
`uplift-results/questions.jsonl` (gold sets) — are both present and tracked. The
embedding endpoint (`bge-small-en-v1.5` via Xinference, `192.168.2.132:9997`) is a live
service per this container's CLAUDE.md, so the embedding cache is regenerable in
principle.

**What is not recomputable without new work:** `rescore.py` as released is **not
self-contained** — it cannot run today because `embed_lib.py` (candidate-span
extraction, cosine similarity, title normalisation) does not exist anywhere in the
reachable record. `RESCORE.md`'s method section describes what `embed_lib.py` must do
in enough detail to rewrite it (chunk on structural delimiters, word n-grams 1–6 within
each chunk, cosine ≥ threshold), but that is a re-implementation, not a recovery.

**Verdict: B**, with a caveat the reviewer's taxonomy doesn't quite have a slot for:
the "code retained" half is only partially true (the driver script is retained, a
required library module is not). Recomputing the table requires writing `embed_lib.py`
from the method description before `rescore.py` can be re-run.

## iv. Sweep-generating harness — **RECOVERED, A**

`bench/sweep/analyse.py` (still tracked, still live) imports
`bench_ontology_uplift as bu` for `bootstrap_ci`/`bootstrap_ci_clustered`. That module
was deleted at commit `e83bb32` (2026-09-03, "chore(cleanup): delete the dead Python
bench tree and its wrappers") along with three sibling scripts and
`bench/run-all-tests.sh`. The commit message states the three uplift harnesses "could
not execute" because they imported a retired serving module
(`app/ontology_scaffold.py`) — true for the *live-serving* code path, but irrelevant to
`bench_ontology_uplift.py`'s `generate`/`run`/`score`/`compare` CLI, which talks to a
model over HTTP directly and does not import that module.

**Confirmed identity, by field match.** `bench_ontology_uplift.py`'s `cmd_run` writes
rows with exactly the fields found in `uplift-results/sweep/results-*.jsonl`: `id`,
`model`, `mode`, `n_gold`, `n_gold_exposed`, `scaffold_engaged`, `injected_tokens`,
`finish_reason`. Its `cmd_score` produces the `scores-*.jsonl` fields (`recall`,
`n_hits`, etc.) matching the released scores files. This is the harness that generated
both.

**Recovered to `uplift-results/recovered/sweep-harness/`** (via
`git show e83bb32^:<path>`, mtime set to the parent commit's authored timestamp,
2026-09-02 21:50 UTC, since git does not preserve original filesystem mtimes):

| File | Role |
|---|---|
| `bench_ontology_uplift.py` | **The sweep-generating harness** — confirmed by field-name match against `uplift-results/sweep/*.jsonl` |
| `bench-uplift.py` | A different, older, unrelated harness (raw `urllib` against a since-deleted `ontology_scaffold` module; single-question A/B, not the sweep) — recovered because it was deleted in the same commit, kept for completeness, **not** the sweep generator |
| `bench-agentic-uplift.py` | Also unrelated — agentic tool-use A/B harness, same deleted `ontology_scaffold` dependency, not the sweep generator |
| `run-all-tests.sh` | The deleted CI test runner named in the commit message |

**Verdict: A.** The sweep rows (`uplift-results/sweep/*.jsonl`) were already released;
this recovery adds their generator back under version control, resolving gap (iv) as
originally scoped by the manifest. Note it is placed in `uplift-results/recovered/`
rather than restored to `bench/` per instruction — restoring it to its original path is
a decision for the paper maintainers, not this recovery pass.

## v. Two-edge composition — **ABSENT beyond what the paper already publishes, C**

No per-item rows, no 6,514-chain corpus, no mining script exist anywhere searched.
Two things were found, both already fully accounted for in the tracked paper and
**not new evidence**:

1. `docs/research/paper-v8/figures/fig-composition.tex` (tracked) — the exact 12
   per-model `(b-only, both)` coordinate pairs plotted in Figure `fig:composition`.
   An identical copy exists in an old LaTeX-build scratchpad
   (`.../6303bdf0-.../scratchpad/arxiv-final/figures/fig-composition.tex` and three
   sibling build dirs) — same file, not new data, not copied in.
2. `docs/research/paper-v8/main.tex` §11 ("Two-Edge Composition") — the full narrative
   with per-model Δ values and CIs (Gemini 3 Flash +0.33, Qwen3.8-27B +0.29, DeepSeek
   v4 +0.26, Sonnet 5 +0.25, Opus 4.8 +0.20; GPT-4.1-mini −0.47, Mistral-24B −0.29,
   GLM-4.7 −0.18, Llama-70B −0.12; three at floor). Identical text found in the same
   scratchpad session's `main.tex` variants — again, not new, and the section itself
   states "the two-edge chain corpus, the mining script that produced it and the
   six-arm per-model rows are not released" (its own words, already in the tracked
   paper).

Git history for `two-edge`/`two_edge`/`chain_` surfaces only paper-drafting commits
(`74952f9` "Study D two-edge composition", `25109eb` "Gemma replication numbers") —
prose and figure commits, no data commits. No `.jsonl`, `.json` or `.py` file for this
study was ever added in any commit on any ref.

**Verdict: C for the corpus, mining script and per-item rows.** The aggregate
per-model deltas and their bootstrap CIs are already released as prose/figure content
in the paper itself (situation A, but at the aggregate level only — this is a
pre-existing fact about the paper, not a new recovery).

## vi. Paraphrase stress-set — **ABSENT, C**

Nothing found. `docs/research/paper-v8/main.tex` §"Vocabulary Mismatch" (`sec:paraphrase`)
states outright: "the paraphrase stress-set behind the 0.96-to-0.34 collapse, and the
per-arm ceilings computed on it, are **reported but not yet released**." No script, no
question set, no per-arm ceiling file exists under `tools/paper/`, in git history for
any `paraphrase`/`stress`/`vocab` term (git log hits are all prose commits or unrelated
uses of the word "paraphrase" as a scoring caveat, e.g.
`.claude/expectations/EXPECTATIONS-rust-loom.md`), or in any scratchpad session (the
handful of scratchpad hits for "paraphrase" are all about lexical-matcher scoring
methodology in adversarial-review documents, not this study's data).

**Verdict: C.** Confirms the paper's own disclosure; nothing to add.

---

## Non-findings worth recording (so they aren't re-searched)

- `uplift-results/{quality,oracle,precise,focused}/` — four other, unrelated studies
  (quality-judge pilot shards, an oracle-context ablation, a "precise" oracle variant,
  and a stub "focused" study with only `cloud.err`/`oracle-context.json`). None contain
  composition, paraphrase, control-rerun, or routing data. Not part of the six gaps;
  flagged in the manifest's "present but not cited" section, out of scope here.
- `docs/research/paper-v4`, `paper-v5`, `preprint` — LaTeX build directories only
  (`.aux`/`.bbl`/`.pdf`/`main.tex`); no data files, no console logs with tables beyond
  what the corresponding `main.tex` already states in prose.
- The `-home-devuser-workspace-dream-machine*` scratchpad sessions — all hits for the
  search terms are false positives (unrelated repo, coincidental substring matches on
  "chain_", "stress", "paraphrase" in Dream Machine desktop/website documentation).
- No host-side run logs: `find / -xdev -newermt 2026-08-20 -name '*.jsonl' -size +50k`
  (excluding `node_modules`/`target`) returns nothing outside this repository's own
  tracked/scratchpad files. The container has no independent copy of runs made against
  the Loom façade or the HP model host.
