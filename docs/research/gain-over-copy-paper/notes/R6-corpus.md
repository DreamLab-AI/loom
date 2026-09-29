# R6 — corpus provenance facts for gain-over-copy-paper

Working dir: `/home/devuser/workspace/loom`. Read-only on the paper. All facts below cite a file
path, commit hash or command output; anything not nailed down is in "Not established" at the end.

## 1. Evaluated snapshot

**scaffold-index.json** (`app/data/scaffold-index.json`, top-level keys):
```
"version": 1
"generated": "2026-08-15T13:22:49.334017+00:00"
"counts": {"classes": 8146}
```
(command: `cat app/data/scaffold-index.json | head -c 2000`, field read off the raw JSON.)

**Sweep artefacts** (`uplift-results/sweep/`): 10 models × {raw,scaffold} result/score JSONL files,
all `mtime` Aug 16 02:41–23:02 2026 (`ls -la uplift-results/sweep/`). No `commitSha`/`generation`/
`digest` field is embedded in the sweep JSONL rows themselves — grepped `uplift-results/`,
`tools/paper/`, `app/mirror.sh`, `README.md` for those three tokens; the only corpus-identifying
values found anywhere in the repo are the embedded generation timestamps in `scaffold-index.json`
(above) and the class/triple counts repeated in prose (README.md:407, ADR-137 line 35, and the paper
itself — see below). `app/mirror.sh` documents the mirror's *mechanism* (atomic staged fetch, one-
generation-cluster check within `GEN_TOL_SECONDS`, `data/.generation.json` manifest with per-artefact
sha256) but no `data/.generation.json` file currently exists in this checkout
(`find app/data -maxdepth 1` lists only `ontology-inferred.ttl`, `scaffold-index.json`,
`prose-index.json`, `ontology.ttl` — the manifest itself is absent).

**The paper's own numbers** (`docs/research/gain-over-copy-paper/gain-over-copy-paper.tex`):
- Line 95: "8,138 Logseq markdown pages compiled losslessly into a pure-TBox OWL 2 ontology of 8,146
  classes and no individuals" — CI gate = consistency tests + Whelk OWL 2 EL reasoner, "each
  generation ships its reasoned closure of 282,492 triples."
- Line 109: "8,146 concept classes and 282,492 triples in the Whelk-reasoned closure."
- Also repeated in `docs/design/PRD-028-does-loom-earn-its-complexity.md:73`: "approximately 8,138
  pages, 8,146 classes and 282,492 closure triples. These are reference measurements, not sufficient
  criteria for an equivalent corpus."
- `docs/research/gain-over-copy-paper/archive/v2/figures/FIGURES-REPORT.md:57-58` restates the same
  chain: "knowledgeGraph corpus (8,138 Logseq pages → pure-TBox OWL 2, 8,146 classes) → CI gate +
  Whelk EL reasoner → versioned generation (282,492-triple closure)."
- One coincidental, unrelated hit: `docs/research/evidence/production.log:3225` — `"8138.069 MiB"` is
  an llama.cpp server KV-cache memory-usage log line, not a page count. Noted so it isn't mistaken for
  corroboration.

**No file in this repo independently derives or logs "8,138" as a computed page count** — it appears
only as an asserted figure in the paper and its own restatements (PRD-028, FIGURES-REPORT). The
`scaffold-index.json` `generated` timestamp (2026-08-15T13:22:49Z) is the only hard machine-recorded
anchor for the evaluated snapshot's build time.

### Matching commit — knowledgeGraph / visionGraph

Both `/home/devuser/workspace/knowledgeGraph` (origin `github.com/DreamLab-AI/knowledgeGraph`) and
`/home/devuser/workspace/visionGraph` (origin `github.com/jjohare/visionGraph`) exist locally.
`visionGraph`'s git history is the older, continuous Logseq→Obsidian lineage (earliest commit
2023-11-29 "Auto saved by Logseq"; `git log --oneline | wc -l` → 2031 commits); `knowledgeGraph`'s
local clone only has history from 2026-08-11 onward and does **not** contain the mid-August commits —
`git worktree add` against a knowledgeGraph SHA taken from visionGraph's log failed
(`fatal: bad object`). The worktree was built from visionGraph instead.

Checked commits from 2026-08-15 (the date matching the scaffold-index generation stamp), each
counted via `git show <sha>:knowledge/pages | grep -c '\.md$'`:

| commit | date (UTC) | `knowledge/pages/*.md` |
|---|---|---|
| 858fbd24 | 09:31:00 | 8161 |
| a1b9c6ef | 09:36:08 | 8161 |
| 56c2e8c1 | 09:55:30 | 8161 |
| f2fa463a | 10:13:00 | 8161 |
| af54f70a | 12:40:58 | 8161 |
| **b7e6cfcf1a09606182f5f25b2617b34551de265c** | **13:20:23** | **8161** |

`b7e6cfcf1` ("formal correctness + llms.txt honesty: root-under-root fixed, disjointness retired") is
the last commit before the scaffold-index generation stamp (13:22:49Z), only 2 min 26 s earlier —
the closest temporal match found. Page count in that checkout does not filter to a bare `find`:

```
$ git worktree add <scratchpad>/wt-aug15 b7e6cfcf1a09606182f5f25b2617b34551de265c
$ find <wt>/knowledge/pages -maxdepth 1 -name '*.md' | wc -l   → 8161
$ grep -rl "public:: true" <wt>/knowledge/pages | wc -l         → 8140
$ grep -rl "public:: false" <wt>/knowledge/pages | wc -l        → 19
$ grep -rL "public::" <wt>/knowledge/pages | wc -l              → 54   (no tag either way)
```

`pipeline/build.py` (in that checkout) reports `f"{len(pages)} pages ({with_oc} OntologyClass,
{public} public)"` — i.e. the pipeline's own published-page count is the `public::true` subset, not
the raw directory listing. **8,140 public pages is the closest match found to the paper's 8,138**
(off by 2); the raw directory count (8,161) is off by 23. No build log recording the pipeline's own
printed count at this commit was found in either repo, so I could not confirm the exact 8,138 via a
captured build run — 8,140 is the nearest empirical figure and b7e6cfcf1 is the best-matching commit
by both date and count.

Worktree removed after use: `git worktree remove <scratchpad>/wt-aug15 --force` (confirmed via
`git worktree list`, which now shows only the pre-existing unrelated VisionFlow worktree from another
session).

## 2. Word count of the evaluated snapshot

Command (`<scratchpad>/wc.py`, strips YAML `---` frontmatter or a leading run of `key:: value` Logseq
property lines, then splits remaining text on whitespace; dedup groups by SHA-256 of the stripped
body):

```
$ python3 wc.py <wt-aug15>/knowledge/pages
files=8161 raw_words=12783434 unique_content_hashes=8161 dedup_words=12783434 duplicate_pages=0
```

No exact-duplicate page bodies at this snapshot (8,161 unique content hashes for 8,161 files). Raw
word count: **12,783,434** (== dedup word count, since there were no duplicates). This count includes
Logseq page-property lines that survive the frontmatter strip (block-level `type::`, `tags::`,
relation lines etc. are still inside the body for Logseq's outliner format, unlike YAML frontmatter),
so it is not a pure-prose count — flagged under §3 against the author's public word-count claims.

## 3. Present-day corpus (visionGraph)

```
$ cd visionGraph && git log -1 --format="%H %ad %s" --date=iso
066602c0f8cee27415d55ef7411aec5766f4eaed 2026-09-28 13:19:11 +0000 Introduction to Agents: QR opens the plain page, not presentation mode
$ find knowledge/pages -maxdepth 1 -name '*.md' | wc -l   → 8446
$ grep -rl "^public: true" knowledge/pages | wc -l         → 8432   (YAML frontmatter now, not Logseq `::`)
$ python3 wc.py knowledge/pages
files=8446 raw_words=8923490 unique_content_hashes=8446 dedup_words=8923490 duplicate_pages=0
```

Present-day: **8,446 pages** (8,432 marked `public: true`), **8,923,490 raw/dedup words** (no
duplicate page bodies), HEAD `066602c0f` dated **2026-09-28**.

**Migration date.** `knowledgeGraph` commit `3a266fc3a` "ci: retire build.yml with the archived
Logseq pipeline" is confirmed in the local `knowledgeGraph` clone's log (`git log --oneline -5`,
top-of-log entries `3a266fc3a`/`721d8074c`/`75a5c1f1a`/`cd0928aa6`/`d09a47612`). In visionGraph's own
(longer, continuous) history the actual Logseq→Obsidian conversion commits are:
- `485977fb6c58de4ec7f94764c1e988655ccafdaa` 2026-09-02 08:05:49 "Convert mainKnowledgeGraph and
  workingGraph to Obsidian vaults (vault-migrate, in place)"
- `02386d1f78ca1e11ab362654b10f75650093c62e` 2026-09-02 09:01:32 "Port the publishing pipeline to the
  visionGraph vault layout"
- `ae913f93a23e7cd604f4c5ec94d0d48dfee8aec8` 2026-09-22 21:44:22 "feat(corpus): frontmatter-only OKF
  corpus, Obsidian-native vaults, Quartz publish" — the commit that switched pages from
  Logseq `key:: value` properties to YAML frontmatter (confirmed by inspecting a page header in the
  current checkout: `type: Class`, `public: true`, `generated: {by: process:vault-migrate/1.0, at:
  2026-09-22T12:41:39Z}`).
- `d13a8d92478b904102e24c32783f78c9e30db1ee` 2026-09-23 10:20:59 "fix(site): the explorer is
  narrativegoldmine.com again; Quartz renders the notes at /notes/"
So the practical migration window is **2026-09-02 → 2026-09-23** (Obsidian vault conversion on
Sept 2, frontmatter-only OKF landing Sept 22–23), i.e. roughly five to six weeks after the paper's
evaluated Aug-15 snapshot.

**Against the author's public word-count claims (~4M earlier, ~7M now):** my counts (12.78M for the
Aug-15 snapshot, 8.92M present-day) are both substantially higher than either stated figure, and —
oddly — the earlier snapshot counts *higher* than the present one under this method, the reverse of
the author's stated trend. The likely cause is methodological, not a real corpus shrink: my counter
(a) is whitespace-token count, not a prose-word tool, and (b) for the Logseq-era pages it cannot fully
strip embedded block-level `key:: value` property lines and inline JSON-LD/ontology-relation markup
from mid-body text (only a leading run of such lines is stripped), which inflates the older snapshot's
token count relative to the newer YAML-frontmatter format where non-prose fields are cleanly separated
into a stripped header. **I cannot reconcile my counts with the author's ~4M/~7M figures from what is
in the repos; that reconciliation is not established.**

## 4. Public URL

`README.md:49`: `` | [knowledgeGraph](https://github.com/DreamLab-AI/knowledgeGraph) | The published
corpus at [narrativegoldmine.com](https://narrativegoldmine.com) — Obsidian-vault→OWL build (`vault
build`), 8,100+ pages, ODbL-1.0 | `` — confirms the URL string `https://narrativegoldmine.com`
exactly. It recurs as the citable public home throughout: loom `README.md` lines 12, 22, 318, 324,
394; visionGraph's own `README.md` lines 1, 5, 7, 31, 48, 113, 127, 279, 431 (e.g. line 1: "narrative-
goldmine: the published export of an Obsidian corpus that is also an OWL ontology", line 7: published
"from the `gh-pages` branch"; IRIs at line 279: `https://narrativegoldmine.com/class/<slug>`). This is
consistently the stated public home for the corpus across both repos — no conflicting URL was found.

## 5. Human review procedure

The corpus is explicitly labelled synthetic, human-directed content, not human-authored prose. Exact
quotes found:

- `docs/design/ADR-136-loom-tooling-allocation.md:29` (loom repo): "The one canonical, load-bearing
  artifact of this system is the per-IRI human-scrutible unit: one block of curated research prose
  (`dfull`, `corpusNature: synthetic-ai-generated-human-directed`) ... that a human can read, review
  and audit end-to-end at single-entity granularity ... our attribution granularity — one reviewable
  markdown per IRI, behind a **propose→consistency-check→human-PR-merge gate** — is the real,
  non-eroding moat." (Same passage repeated verbatim in `ADR-137-loom-rust-replatform.md:23`,
  `ddd-ontology-loom-context.md:13`, `PRD-026-loom-consolidation.md:62`, `PRD-027-...md:63`.)
- `README.md:415` (loom): "**Provenance.** The corpus it serves is **AI-generated synthetic content
  produced under human direction, by design** — an ontology testbed, not an authoritative
  encyclopaedia. Every grounded answer is traceable to a corpus generation; that provenance attests
  traceable generation, not human authorship."
- `docs/design/ADR-137-loom-rust-replatform.md:123`: "the human reviews and merges the canonical
  markdown in one place (`jjohare/logseq` PR gate); the Loom never mints a second, divergent copy."
- visionGraph `README.md:186`: "it is not a curated human-authored encyclopaedia" (of the corpus, used
  "as a testbed for the pipeline").
- visionGraph `.github/workflows/publish.yml` (`publish.yml:155-163`): a CI step literally named
  **"vault validate (source gate)"** that runs `vault validate --vault all`, checking (per the
  workflow's own comments, line ~149) OKF/vocabulary violations, `((block-ref))` integrity and
  json-ld fences, failing the build (`::error::`) on any finding.
- The paper's own §2.1 (`gain-over-copy-paper.tex:95`): "A CI gate combines consistency tests with a
  Whelk OWL 2 EL reasoner ..., rejecting contradictions before publication."
- Governed enrichment / propose path: loom `README.md:394`: "Uplift *into* the ontology happens
  through VisionClaw's governed propose door, the forum/ACSP surface, or direct agentic writes into
  the corpus — never the Loom."

**What is and is not documented:** the documented gates are (a) CI consistency + Whelk EL reasoner
checks (formal-logic contradiction rejection, not content-quality review), (b) an OKF/vocabulary
source-gate (`vault validate`) in CI, and (c) a PR-merge step described as where "the human reviews"
— but no document found specifies a systematic content-quality review procedure (e.g., a checklist,
sampling rate, or per-page sign-off) applied to the LLM-generated prose itself. `PRD-028-does-loom-
earn-its-complexity.md` (lines 195, 199, 277) *proposes* a blinded, two-reviewer-plus-adjudicator human
review protocol (5 min/response, ~200 reviewer-hours) as **future work for a follow-up study**, not as
a procedure that has been run on the corpus to date — the PRD explicitly frames it as a design for
work not yet executed ("If human review funding is insufficient, reduce scope before testing...").

## 6. The 0.87 figure

Earliest occurrence found via `git log --all -S"0.87" -- README.md docs/design/`:
- `53026db320f6c28c423a362520c4e085bfc94d10`, 2026-08-17 10:15:01 UTC, "design: Rust Loom
  re-engineering (PRD-027 + ADR-137 + DDD-rev + RUST-ARCH + DOC-PLAN) + legacy ADR fix" — this commit
  introduces ADR-137 and the associated expectations file together.

Statements of the figure, verbatim:
- `README.md:409`: "Recall gate RED: `rgb-protocol 0.816`, below `0.87` design floor. Wiring done and
  tested; the default does not change until the multivariate bench passes."
- `docs/design/ADR-137-loom-rust-replatform.md:35`: "Validated recall via `hnsw-xinference`:
  `rgb-protocol` 0.87, decoys ~0.45." (line 3, header caveat): "the recall gate is RED (measured
  `0.816 < 0.87`, §D5 + Erratum D below)."
- `.claude/evidence/EXP-008.evidence.md`: "measured in-domain recall is cos 0.8160 for `rgb-protocol`
  vs the **0.87 design floor** → the recall gate is RED", and: "The cosine ≥ 0.87 floor is the
  **PRECONDITION FOR DEFAULT-ON**, not a current guarantee" (`EXPECTATIONS-rust-loom.md:81`).

**No stated derivation was found anywhere.** I grepped `docs/design/`, `README.md`,
`.claude/evidence/`, `.claude/expectations/EXPECTATIONS-rust-loom.md` for any explanation of how 0.87
was chosen (no benchmark study, no percentile/threshold-selection methodology, no citation) — every
occurrence simply asserts "0.87 design floor" or "0.87 floor" as a given precondition for turning the
semantic fallback on. This is consistent with the author's own account that the figure was likely
arbitrary; the record contains no derivation to confirm or contradict that beyond its bare repeated
assertion. `docs/research/gain-over-copy-paper/archive/preprint/differentiation.md` also contains a
"design floor"/"recall gate" mention but restates the same ADR-137 fact rather than adding a
derivation.

---

## Facts the paper may state

1. Evaluated snapshot's `scaffold-index.json` was generated 2026-08-15T13:22:49.334017+00:00, with
   `counts.classes = 8146` (`app/data/scaffold-index.json`).
2. The knowledgeGraph/visionGraph commit `b7e6cfcf1a09606182f5f25b2617b34551de265c` (2026-08-15
   13:20:23 UTC, "formal correctness + llms.txt honesty: root-under-root fixed, disjointness retired")
   is the closest-matching commit found by date to the scaffold-index generation stamp (2m26s prior),
   with 8,161 total `.md` pages under `knowledge/pages/` and 8,140 marked `public:: true` — the
   pipeline's own published-page metric (per `pipeline/build.py`'s print statement) — the closest
   empirical match to the paper's stated 8,138 (off by 2), though not an exact reproduction.
3. No file in the loom, knowledgeGraph or visionGraph repos independently records a build log or
   manifest confirming the literal figure "8,138"; it is asserted in the paper and repeated verbatim
   in PRD-028 and the archived FIGURES-REPORT, with no independent corroborating source found.
4. `app/data/.generation.json` (the sha-addressable generation manifest `app/mirror.sh` documents) is
   absent from this checkout; no `commitSha`/`generation-id`/`digest` value tying the evaluated sweep
   rows to a specific corpus build was found anywhere in `uplift-results/`, `tools/paper/` or
   `app/mirror.sh` itself.
5. Present-day visionGraph (2026-09-28, HEAD `066602c0f`) has 8,446 pages (8,432 `public: true`) —
   larger in page count than the Aug-15 snapshot (8,161/8,140), and the corpus has undergone a
   Logseq→Obsidian frontmatter-only migration between 2026-09-02 (`485977fb6`) and 2026-09-23
   (`d13a8d92`), roughly five to six weeks after the paper's evaluated snapshot.
6. `https://narrativegoldmine.com` is confirmed as the consistent, repeatedly-cited public home for
   the published corpus in both the loom and visionGraph README files.
7. The documented review/governance procedure on the LLM-generated corpus is: CI consistency checks +
   Whelk OWL 2 EL reasoner (contradiction rejection), a `vault validate` OKF/vocabulary source-gate in
   CI (`publish.yml`), and a stated propose→consistency-check→human-PR-merge gate for admitting new
   content — explicitly not a documented systematic content-quality review of the corpus prose itself;
   a blinded two-reviewer human-review protocol exists only as a *proposed*, not-yet-executed design in
   PRD-028.
8. The "0.87 design floor" / "recall gate" figure first appears in commit `53026db320f6c28c423a362520
   c4e085bfc94d10` (2026-08-17 10:15:01 UTC, introducing ADR-137/PRD-027), asserted throughout as a
   fixed precondition for enabling semantic fallback, with no derivation, benchmark study or citation
   found anywhere in the record.

## Not established

- The exact provenance/derivation method behind the paper's literal "8,138" page figure (closest
  reproducible figure found is 8,140 public pages at commit `b7e6cfcf1`, not an exact match).
- Any `commitSha`/generation-id/digest explicitly binding the `uplift-results/sweep/` rows (Aug 16
  2026) or `uplift-results/paper-v2/` artefacts to a specific corpus commit or generation manifest —
  none was found; the link established here is inferential (via `scaffold-index.json`'s `generated`
  timestamp and the nearest-in-time commit), not a recorded identifier.
- Reconciliation of my word counts (12.78M at the Aug-15 snapshot; 8.92M present-day, by whitespace
  tokenisation with frontmatter/property-line stripping) against the author's publicly stated ~4
  million words earlier / ~7 million words now — the methods evidently differ and I could not
  determine what word-counting method underlies the author's public figures.
- Any documented systematic (non-PR-merge, non-CI) human content-quality review procedure actually
  applied to the corpus prose to date, beyond the CI/PR gates quoted in §5; PRD-028's blinded
  reviewer protocol is future work, not an executed procedure.
- Why the pipeline's raw directory count (8,161) and public-only count (8,140) at the matching commit
  both differ from the paper's 8,138 — whether the paper's figure reflects a slightly different or
  later build than any commit checked, a manual adjustment, or a different filter than `public::true`
  was not determinable from the repos alone.
