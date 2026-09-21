# Routing-seam evidence (typed skill routing, 86 turns over 115 rubrics)

Salvaged 2026-09-21 from five Claude Code session scratchpads under the agentbox
checkout, where the routing study for paper v7 was run and where nothing was committed.
This directory is the authoritative home for that evidence. The write-up is
`docs/research/companion-routing/` (split from the paper after external review on
2026-09-21); the measurement rig snapshot is `tools/routing-eval/`.

Provenance: agentbox commit `5c273c704` (2026-09-21, ADR-2095) is the tree the rig and
corpus were taken from. The rig remains a live crate in agentbox
(`crates/system-one/system-one-eval`, `publish = false`) because it is wired to that
repo's skills tree; the snapshot here is for reproduction of the paper's numbers.

## corpus/

| File | What it is |
|---|---|
| `routing-cases.json` | the 86-turn labelled routing corpus (`_role`, `_schema`, `_provenance`, `cases`); self-authored, single seed; identical bytes to `agentbox/tests/system-one/routing-cases.json` at the provenance commit |
| `skill-rubrics.json` | the 115 skill rubrics shown to the judge as options (the exposure), snapshot at the same commit; 50 carry an exclusion clause |
| `mined-rows.json` | 86 per-turn rows joining judge pick/confidence/latency with the BM25 pick, score, margin and decline outcome (`lex_*`); the row set behind the discordant-pair, subgroup and label-audit analyses |

## runs/

| File | Engine / backend | Notes |
|---|---|---|
| `run-5260b398.json` | `sso` façade, model `openjev` (the local 4B cross-encoder, independent scoring), `http://systemone:8097` | `system-one-eval run --json`; overall / by_class / by_discriminator / none; p50 ≈ 5.0 s, mean input 12,938 tokens |
| `openjev-run-v2.json` | same engine, second pass | the run the repaired copy ceiling was read from (`copy-ceiling --from-report`) |
| `live-5260b398.json`, `repro-5260b398.json` | copy-ceiling output, `fair` and `naive` modes | `fair` = each judge-free ranker gets an oracle-tuned decline threshold; `naive` = no decline rule |
| `live-5260b398.txt`, `live-5260b398.err` | console rendering of the same | |
| `copy-ceiling-v2.txt` | console rendering of the FIRST (faulty) ceiling | lexical 76.7 %, gain +11.6: the fixed-grid threshold sweep and unstripped exclusion clauses; kept because the companion note reports these defects as its first finding |
| `sweep.txt`, `sweep-low.txt`, `sweep-v2.txt` | decline-threshold sweeps on the 4B engine | one whole corpus run per threshold; accuracy 88.4 % at the deployed 0.5 |
| `eval-report-stub-engine-e2e.json` | `sso-stubengine`, `laya-typed-decisions` | an end-to-end rig test against a stub engine, NOT a measurement; kept only because it documents the report schema |

Not located as per-item artefacts: the cloud judge (`jev-1.13.0`) run and the 421M
shared-head encoder run that fill the other two rows of the three-judge table. Their
numbers appear in the drafts (`docs/research/companion-routing/drafts/`) and in
`analysis/run*.py` which called the backends live; if per-item JSON for those two rows
exists it was never written to disk. Treat those two rows as reported-but-not-released
until re-run with `--json`.

## analysis/

Python scratch scripts, in the order they were written. They are the working that
produced the companion note's tables; they are not a maintained harness.

| File | Role |
|---|---|
| `copy-ceiling.py` | first ceiling implementation (fixed 41-point threshold grid; exclusion clauses indexed) |
| `copy-ceiling-fair.py` | breakpoint-enumerating sweep and fair mode |
| `mine.py` | joins judge and ranker picks into `corpus/mined-rows.json` |
| `lib.py` | shared loaders, tokeniser, BM25, McNemar exact test |
| `ceiling.py`, `strong_ceiling.py` | the ceiling with exclusion clauses stripped (the 83.7 % figure) and the strongest-ranker search |
| `run1.py` … `run9.py` | the numbered passes: three-judge table, discordant pairs, exact McNemar, subgroup decomposition with Holm, power at n = 86 |
| `leak.py`, `leak2.py` | prompt-to-rubric content-token overlap (37.1 % gold vs 2.7 % random) |
| `perm.py`, `cells.py`, `rw.py`, `dump.py` | permutation checks, 2×2 cell tables, label re-read helpers |

The Rust rig (`tools/routing-eval/src/copy.rs`) is the released, tested implementation of
the same procedure and supersedes these scripts where they disagree.

## embedding-cache/

`embed-cache.jsonl` (rig format) and `emb-cache.json` (script format): bge-small-en-v1.5
384-d embeddings of the 115 rubrics and 86 prompts, requested one text per call from the
estate's Xinference endpoint. With these the embedding ceiling (69.8 %) reproduces offline.
