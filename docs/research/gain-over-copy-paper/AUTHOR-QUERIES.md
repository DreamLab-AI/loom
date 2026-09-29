# Author queries — v10 reframe

Open questions from `BRIEF-2026-09-29-reframe.md` §10. Each entry states what the record
established (via `notes/R6-corpus.md` unless stated otherwise) and what remains open. No answer
is guessed here; where the record is silent, that is stated as silence, not filled in.

---

## 1. The 0.87 floor

**What the record establishes.** `git log --all -S"0.87"` finds the figure's earliest appearance
in commit `53026db320f6c28c423a362520c4e085bfc94d10` (2026-08-17 10:15:01 UTC, introducing
ADR-137 and PRD-027). Every subsequent occurrence (`README.md:409`, `ADR-137:35`,
`.claude/evidence/EXP-008.evidence.md`, `EXPECTATIONS-rust-loom.md:81`) states it as a bare
"design floor" or "precondition for default-on" with no benchmark study, percentile or
threshold-selection method, or citation attached anywhere in `docs/design/`, `README.md`,
`.claude/evidence/` or `.claude/expectations/`. Measured recall against it is 0.816.

**What remains open.** The author's own account is that the figure was arbitrary. The record
contains nothing that confirms or contradicts that beyond the bare repeated assertion — there is
no derivation to recover.

**Query.** Confirm for the record: is 0.87 to be stated in the paper as an arbitrary
implementation choice with no calibration study, full stop, or is there a rationale that exists
outside version control (a conversation, an external note) that should be captured before v10
locks the wording?

---

## 2. Word count of the evaluated snapshot

**What the record establishes.** Two independent whitespace-token counts, both with frontmatter
or leading Logseq property lines stripped (`notes/R6-corpus.md` §2–3):

- Evaluated snapshot (commit `b7e6cfcf1`, 2026-08-15, the closest date match to
  `scaffold-index.json`'s generation stamp): **12,783,434 raw/dedup words**, 8,161 pages.
- Present-day visionGraph (HEAD `066602c0f`, 2026-09-28): **8,923,490 raw/dedup words**, 8,446
  pages.

Both figures are substantially higher than the author's publicly stated ~4M (earlier) / ~7M
(now), and — the reverse of the author's stated trend — the older snapshot counts *higher* than
the present one under this method. The likely cause is methodological: this counter is a
whitespace-token count, not a prose-word tool, and for Logseq-era pages it cannot fully strip
embedded block-level `key:: value` property lines and inline JSON-LD/ontology-relation markup
from mid-body text, inflating the older snapshot's count relative to the newer YAML-frontmatter
format. This is not confirmed; it is the most likely explanation, not a demonstrated one.

**What remains open.** The method behind the author's public ~4M/~7M figures is not documented
anywhere in the loom, knowledgeGraph or visionGraph repos. The two counting methods cannot be
reconciled from what is in the repos.

**Query.** What method produced the public ~4M/~7M figures (a specific tool, a different
stripping rule, a different corpus subset)? Until that is known, the brief's instruction stands:
omit the word count from the paper and report only page/class/triple counts for the evaluated
snapshot, citing the snapshot id/date (`scaffold-index.json` generated 2026-08-15T13:22:49Z,
8,146 classes).

---

## 3. Human verification of the corpus

**What the record establishes.** The documented gates are: (a) CI consistency checks plus a
Whelk OWL 2 EL reasoner rejecting logical contradictions before publication
(`gain-over-copy-paper.tex:95`); (b) an OKF/vocabulary source-gate (`vault validate`) run in CI
(`visionGraph/.github/workflows/publish.yml:155–163`); and (c) a stated
propose → consistency-check → human-PR-merge gate for admitting new content (ADR-136, ADR-137,
`README.md:415`). `PRD-028-does-loom-earn-its-complexity.md` (lines 195, 199, 277) *proposes* a
blinded, two-reviewer-plus-adjudicator human review protocol (5 min/response, ~200
reviewer-hours) explicitly as **future work for a follow-up study**, framed as a design for work
"not yet executed."

**What remains open.** No document specifies a systematic content-quality review procedure (a
checklist, a sampling rate, a per-page sign-off) that has actually been applied to the
LLM-generated prose itself, beyond the CI/reasoner gates and the PR-merge step. The "human
reviews and merges" step is confirmed as a gate on admission, not confirmed as a content-quality
review of the prose's factual claims.

**Query.** Beyond the CI/reasoner/OKF gates and PR-merge admission, has any systematic
content-quality review of corpus prose actually been performed to date (even informally,
outside PRD-028's proposed protocol)? If not, the paper should say so plainly, per the brief's
instruction never to imply per-assertion human verification.

---

## 4. Migration dates and artefacts

**What the record establishes** (`notes/R6-corpus.md` §3, all dates confirmed by git log in the
visionGraph repository):

- `485977fb6` — 2026-09-02 08:05:49 — "Convert mainKnowledgeGraph and workingGraph to Obsidian
  vaults (vault-migrate, in place)."
- `02386d1f7` — 2026-09-02 09:01:32 — "Port the publishing pipeline to the visionGraph vault
  layout."
- `ae913f93a` — 2026-09-22 21:44:22 — "feat(corpus): frontmatter-only OKF corpus, Obsidian-native
  vaults, Quartz publish" — the commit that switched pages from Logseq `key:: value` properties
  to YAML frontmatter.
- `d13a8d924` — 2026-09-23 10:20:59 — "fix(site): the explorer is narrativegoldmine.com again;
  Quartz renders the notes at /notes/."

Practical migration window: **2026-09-02 → 2026-09-23** — roughly five to six weeks after the
paper's evaluated 2026-08-15 snapshot. The public URL `https://narrativegoldmine.com` is
confirmed as the consistent, repeatedly-cited home for the published corpus across both the loom
and visionGraph repositories, both before and after the migration.

**What remains open.** Nothing further is open on the migration dates themselves; the record is
complete on this point. Not established: any file binding the evaluated sweep rows
(`uplift-results/sweep/`, mtimes 2026-08-16) to a specific corpus commit or generation manifest —
`app/data/.generation.json`, the sha-addressable manifest `app/mirror.sh` documents, is absent
from this checkout, and no `commitSha`/`generation-id`/`digest` value was found anywhere in
`uplift-results/`, `tools/paper/` or `app/mirror.sh` tying a sweep row to a corpus build.

**Query.** None — this is settled by the record. Use the dates and commits above if the paper
states the migration timeline; state plainly that the evaluated sweep's exact corpus build is
inferred (via `scaffold-index.json`'s generation timestamp and the nearest-in-time commit,
`b7e6cfcf1`, 2m26s prior), not recorded by an explicit identifier.

---

## 5. Later experiments

**What the record establishes.** The five-arm control rerun (`uplift-results/control-rerun-2026-09-21/`)
has been judged with `gpt-4.1`, analysed, and — per the author's decision for v10 — is now
included and reported as a completed study, not a pending one. The human annotation sample
remains unfilled: the brief records this as an author decision ("the human annotation sample
stays 'prepared, not performed'"), not an oversight.

**What remains open.** Nothing procedural — both dispositions (rerun completed and included;
human sample deliberately not performed) are settled author decisions, recorded here so v10 does
not silently revisit them.

**Query.** None outstanding; confirming for the record only.

---

## 6. Current code support for release/deployment claims

The claims ledger (`CLAIMS-LEDGER.md`) marks the following rows `UNVERIFIED` because no artefact
in this repository confirms them (as opposed to the tex prose alone). Each is listed with what
would close it.

| Row | Claim | Why unverified | What would close it |
|---|---|---|---|
| 33 | Re-judge agreement: 2026-09-21 re-judge of control completions agrees with the archived 2026-08-17 scores on 227/240 exactly, 240/240 within one point | `analysis-archived-2026-08-17.json` (the comparison file named in `MANIFEST.md`) was not opened and independently checked in the R7/ledger pass | Read `uplift-results/controls-rejudge-2026-09-21/analysis-archived-2026-08-17.json` directly and confirm the 227/240 and 240/240 figures against its contents |
| 37 | Gate correctly skipped 60 of 100 off-domain gradings | The engagement-rate figure was not located verbatim in the `analysis.md` excerpt pulled during verification; consistent with the narrative but not line-confirmed | Locate and quote the exact line in `uplift-results/general/analysis.md` (or equivalent) stating the 60/100 engagement figure |
| 38 | Podcast case study: term resolution 0.17 → 0.55 (3.2×); generation time roughly halves (173s vs 346s per episode) | No `uplift-results/casestudy` or `uplift-results/podcast` directory exists in this repository; the MANIFEST footnotes this artefact as living in the VisionFlow repository, not here | Either locate the per-arm table in the VisionFlow repository and cite it directly, or bring a copy of the artefact into this repository under version control so the loom-side claim has a local, checkable source |
| 39 | Artefact-name fidelity: 0 ASR-artefact entity names in the Loom-grounded Qwen arm vs 7/181, 3/131, 3/171 for other arms | Same as row 38 — source table not accessible in this repository | Same as row 38 |
| 40 | Judged page-integration gate: every arm degrades judged page quality, means −1.04 to −0.44; second judge 20% subset, 93% verdict agreement | Artefact location is a VisionFlow-side sandbox of before/after pairs, not present locally; no per-arm n stated in the tex prose either | Bring the before/after judged-pairs artefact (or a durable pointer with commit pin) into version control alongside the paper's other evidence, per the pattern already used for `uplift-results/` |
| 41 | Best prompt-optimisation variant remains negative under both rubrics: −0.40 dev, −0.90 held-out | Same as row 40 — no local artefact | Same as row 40 |
| 42 | Thin-page direct-enrichment rejected: mean −0.65 and −0.60 across twenty pairs under two rubrics; splice mechanic failed on 13/20 pairs | Same as row 40 — no local artefact | Same as row 40 |
| 44 | Semantic fallback disabled throughout: measured HNSW recall 0.816 vs a design floor described in tex as ≈0.87 | The 0.87/0.816 figures appear only in tex prose; no separate JSON/MD summary artefact is cited in MANIFEST for this specific pairing (distinct from query 1 above, which is about the floor's *derivation*, not its *artefact*) | Add a small released artefact (or a MANIFEST line pointing at the existing `.claude/evidence/EXP-008.evidence.md`) that states the 0.816 measurement and the 0.87 floor together, so the pairing has a citable source beyond the tex itself |

**General note on rows 38–42.** These all describe the write-path production case study, whose
raw evidence lives in the VisionFlow repository rather than this one. `docs/research/README.md`
already states this ("the write-path result is only checkable there"). The queries above are not
asking whether the claims are true — the tex's own framing is accepted — but whether the
underlying artefacts can be made locally checkable before v10 ships, consistent with the
evidence-versioning standard the rest of the paper's claims already meet
(`uplift-results/`, tracked and force-added per `docs/research/README.md`'s "Per-row evidence"
section).
