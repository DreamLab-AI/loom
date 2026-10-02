# PRD-028 Phase 0: candidate corpus inventory (visionGraph)

Date: 2 October 2026 · Corpus: `visionGraph@015ca2c1f2d7289955ebf16b98b6775a57ec0f7b` ·
Gate (PRD-028 §12, Phase 0): *a usable non-public corpus exists; source handling is authorised*.

This inventory covers the four strata held in the visionGraph vault. It reports counts only. No
title, path, sentence or URL from a non-public stratum appears here or in
[`corpus-profile.json`](corpus-profile.json), because this repository is public.

## Verdict

**The gate is not met from visionGraph.** None of its strata is a non-public corpus of local
facts at pilot scale. The published `knowledge/` vault is the regression reference that §3.3
describes, and it is now measured. The only private material, about 300 curator notes, falls
below the pilot floor of 400 units and is mostly commentary on public technology. Phase 0 has to
continue with the estate's operational records (see "What remains" below), and only the owner
can authorise handling them. The owner did so later the same day, and those sources do meet the
pilot floor; see [Private sources](#private-sources-owner-authorised-2-october-2026).

## Strata

Measured with [`profile_corpus.py`](profile_corpus.py) (body text only, frontmatter excluded;
tokens estimated as characters ÷ 4) and `vault validate --json` from VisionClaw source at
`271085c21`.

| Stratum | Units | Words | Est. tokens | Median / p90 words | Stubs < 50 words | Exact-duplicate units | Repeated long lines | URL-bearing lines | Visibility |
|---|---:|---:|---:|---|---:|---:|---:|---:|---|
| `knowledge/` (published OKF classes) | 9,380 | 9.06 M | 18.0 M | 238 / 1,732 | 10.2% | 7.2% | 11.7% | 4.1% | 9,366 `public: true` |
| `working/pages` | 786 | 0.97 M | 1.96 M | 769 / 2,471 | 17.0% | 2.9% | 21.7% | 13.0% | 597 `false`, 187 `true` |
| `working/journals` | 1,236 | 0.14 M | 0.37 M | 43 / 276 | 53.9% | 6.1% | 38.5% | 53.3% | all `false` |
| `transcripts/` | 241 | 0.94 M | 1.38 M | 3,964 / 5,821 | 0.4% | 0.0% | 26.8% | 26.1% | no frontmatter |

### `knowledge/`: the regression reference (§3.3, §3.4)

- 9,366 public classes across 19 `domain` values, the largest being blockchain (1,212),
  infrastructure (1,205), artificial intelligence (1,139) and spatial computing (1,078). The
  vault build reports 338,191 asserted and 83,555 inferred triples, 9,366 graph nodes and
  114,765 edges.
- That is 1,220 classes more than the 8,146 the Copy Ceiling paper evaluated. Any "0.5 to 2 times
  Loom's deduplicated token volume" target (§3.4) should be computed from this snapshot,
  deduplicated: about 16.7 M estimated tokens once the 7.2% exact duplicates are removed. So the
  full-study private corpus would need roughly 8 to 33 M tokens.
- Quality profile from `vault validate`: 0 errors, 29,036 warnings. 8,446 of them are
  `UNVERIFIED_STABLE` (stable status with no human entry in `verified`), 20,547 are
  `DANGLING_LINK` and 2,238 are `MULTI_PARENT`. No stable class has a human `verified` entry, so under
  OKF's trust model the reference corpus is process- or agent-generated and unverified.
- It is public and published, so it cannot serve as the primary corpus.

### `working/pages`: private curator notes

- 302 private `Note` pages (0.35 M words), 295 private `Episode` pages (0.31 M words) and 187
  `Note` pages already flagged `public: true` (0.33 M words). Only 19 pages carry a `domain`, and
  `vault validate` reports 765 `MISSING_DOMAIN` warnings.
- `Episode` pages are notes on third-party podcasts and talks, so their facts have public
  counterparts (§5.3). Of what remains, the 302 private notes are the only candidate. A random
  sample of 12 found link collections, pasted assistant answers and commentary on public
  technology. One held generic UK company guidance. None held facts particular to an
  organisation. That fails the "answers depend on local facts" test in
  §3.1.
- Provenance is weak for this purpose. Every page carries the same migration stamp
  (`process:vault-migrate/1.0`, 22 September 2026). Git history starts with the 2025 import,
  so neither gives the original authoring time that §2's temporal-novelty row needs.

### `working/journals`: private dated logs

- 1,236 entries, dated by filename from 2021 to 2026 (2023: 276, 2024: 349, 2025: 325, 2026: 122).
- Median 43 words, 54% stubs and 53% of lines carrying a URL: a link log, not a body of facts.
  It is personal, so the owner would have to attest to any use, and §3.1 does not favour it.

### `transcripts/`: third-party transcripts

- 241 long transcripts of published podcasts and articles. Their counterparts are public and
  the copyright belongs to third parties, so they are ineligible as a private corpus. They may
  serve as distractor background only, if licensing allows.

## Provenance and permission

| Requirement (§2, §3.4) | knowledge | working pages | journals | transcripts |
|---|---|---|---|---|
| Private access | no (published) | yes (597 units) | yes | no |
| Owner, source, timestamp, stable ID | `resource` IRI; migration timestamp only | migration timestamp only | filename date | none |
| Unseen, local facts | no | rarely | rarely | no |
| Source handling authorised | n/a | **owner to attest** | **owner to attest** | no |

## What remains for Phase 0

Phase 0's preferred candidates (§3.1) are internal deployment decisions, equipment
configurations, incident resolutions and versioned procedures. In this estate those are
operational records outside visionGraph. They are listed by category, because some of them live in
private repositories whose names do not belong in a public repo. The PRD-028 Disposition
carries the plan and its commands. The owner authorised those sources on 2 October 2026; the
result is the next section.

## Private sources (owner-authorised, 2 October 2026)

The owner authorised the remaining Phase 0 sources on 2 October 2026 (owner decision R3):
private-repository ADRs, PRDs and runbooks, the RuVector `project-state` namespace, and workspace
notes and incident histories, for **local and development use only**. This section reports
counts per source class and nothing else. The raw inventory, the profiling script and the
hand-read samples stay outside git, in the local workspace and in a RuVector `project-state` entry. Subdomains are lettered, not named, because their
names are private.

Method: the measures of `profile_corpus.py` (body text, tokens estimated as characters ÷ 4),
applied to tracked Markdown in the private checkouts and to the memory store read through the
memory MCP tools. Vendored agent tooling, third-party pages, archives and literature folders
were excluded after the first sample read found them. "Deduplicated tokens" removes exact
duplicate units and repeated paragraphs of 40 characters or more. "Local signal" is the fraction
of units carrying at least three kinds of local identifier (address, port, version, commit, date,
record ID, repository path). "Hand-read local" is how many of a random 20-unit sample per class
held facts particular to this estate.

| Source class | Units | Deduplicated tokens | Median words | Local signal | Hand-read local | Owner, timestamp, stable ID |
|---|---:|---:|---:|---:|---:|---|
| Private-repo decision records (ADR, PRD) | 89 | 0.28 M | 1,234 | 94% | 19 / 20 | git author, git date, path and record ID |
| Private-repo runbooks and site operations | 181 | 0.40 M | 714 | 46% | 15 / 20 | git author, git date, path |
| RuVector `project-state` | 1,937 | 0.76 M | 151 | 55% | 18 / 20 | agent-written; date in 85%; key |
| Workspace notes | 29 | 0.10 M | 1,405 | 83% | 11 / 20 | dated in 24%; path |
| Incident histories (dream reports, dream-inbox items, fix commits) | 387 | 0.42 M | 92 | 66% | 19 / 20 | engine or git author; date in 56%; night or commit ID |
| **Union, exact-deduplicated** | **2,623** | **≈ 1.99 M** | | | | |

Public counterparts (§5.3). Many `project-state` entries, dream reports and workspace notes
describe the public agentbox, VisionFlow, Loom and forum repositories, whose commits and ADRs are
public. Units whose subject is a private project, with no public counterpart, form the
**strict private subset: 837 units, about 0.90 M tokens**, drawn from all five classes (460
memory entries, 180 runbooks, 106 incident records, 89 decision records, 2 notes). It spans
nine subdomains: A 343 units, B 285, C 51, D 49, E 46, F 23, G 17, H 12, I 11. Six of those
subdomains have at least 20 units.

Quality flags from the hand reads: two of the 20 sampled memory entries had their whitespace
stripped; nine of the 20 workspace notes are 2025 agent-swarm reports whose claims are stale or
unreliable; the memory entries are agent-written summaries, so §3.4 source-owner review has to
fall on the human owner of each project. At least two subdomains, A and D, are client
engagements, so the owner must attest that their material may be handled even locally.

### Gate verdict

**The Phase 0 pilot floor is met, with conditions.** The strict private subset reaches 837 units
across six subdomains of 20 or more units, against a floor of 400 units across four subdomains,
and the hand reads found substantive local facts in every class except the workspace notes. A
pilot of 400 to 800 units drawn from it costs about **0.43 to 0.86 M tokens** of corpus (about
1,080 tokens per unit). The roughly 1,240 public-project units are available as nearby
distractors (§3.4). The conditions are:

1. Subdomains A and B supply 75% of the strict subset, so pilot sampling must be stratified by
   subdomain and must report results per subdomain.
2. Authorship is shared with the Loom's own authoring process (DreamLab and its agents), which §3.1
   requires to be disclosed, with transfer claims restricted.
3. The owner attests handling for the two client-engagement subdomains before Phase 1 reads them.

**The full-study band is not reachable from private estate material.** The whole union is about
2 M tokens, against the 8 to 33 M tokens that §3.4 derives from the measured Loom reference. The
full study will need a partner corpus, or the owner will need to justify a smaller corpus against
the measured profile, as §3.4 allows.
