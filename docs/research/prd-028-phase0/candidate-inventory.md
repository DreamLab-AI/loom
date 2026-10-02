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
can authorise handling them.

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
carries the plan and its commands.
