# Notes — v10 reframe verification passes

Working notes supporting the v10 revision (`BRIEF-2026-09-29-reframe.md`). Each is a verification
pass against released artefacts or the running text; none is a source of evidence in its own
right and none should be cited from the paper.

| File | Role |
|---|---|
| [`R6-corpus.md`](R6-corpus.md) | Corpus provenance facts: the evaluated snapshot's identity and date, its word/page counts against two independently-run counting methods, the present-day visionGraph corpus for comparison, migration dates, the documented human-review procedure, and the 0.87 floor's first appearance and (missing) derivation. Read-only against the paper; everything is sourced to a file path, commit hash or command output, with an explicit "Not established" section for what the record does not settle. Feeds `AUTHOR-QUERIES.md` directly. |
| [`../CLAIMS-LEDGER.md`](../CLAIMS-LEDGER.md) (R7) | One row per numbered claim in the paper (v9.2), each checked against a released JSON/MD summary and marked `VERIFIED` or `UNVERIFIED` with a reason, plus permitted wording and wording to avoid drawn from the brief. Also records discrepancies (`D1`–`D9`) between the current running text and the brief's banned-phrase list, most of which are confirmed clean rather than live violations. The rows marked `UNVERIFIED` for lack of a local artefact feed `AUTHOR-QUERIES.md` §6. |
| [`R8-phrases.md`](R8-phrases.md) | A line-anchored sweep of the paper, its figures and both README files for the specific misreading-prone phrases the brief names in §§3, 4 and 8 (stale title, "verifiably", pooled tables, "five providers", "human-judged", "pretraining contamination" and others), each with a proposed replacement and cross-referenced to the brief clause it corresponds to. Closes with a scored check of the brief's §12 acceptance test against the paper's title, abstract, captions and conclusion read alone. The README-facing hits from this sweep are the ones fixed directly in the top-level `README.md` and `docs/research/README.md`. |

## Reading order

`R6-corpus.md` establishes the facts; `CLAIMS-LEDGER.md` (R7) checks the paper's numbers against
artefacts and assigns wording; `R8-phrases.md` sweeps the prose (paper and both READMEs) for the
specific misreadings the brief warns against. `AUTHOR-QUERIES.md`, one level up, collects what
none of the three could settle and states it as open rather than guessed.
