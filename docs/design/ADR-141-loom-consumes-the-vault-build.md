# ADR-141 — The Loom consumes the vault build: `visionGraph@<sha>`, OKF vocabulary, a ledger route, stdio dropped

Status: Proposed · 2026-09-22 · Extends ADR-135 (generation discipline), ADR-136 D4 (atomic
mirror), ADR-138 (grounding contract) and ADR-140 (the two planes) · Amends ADR-140 D1 and D4 in
place · Realises the Loom half of VisionFlow **PRD Sovereign Corpus** (Q8, Q10, §3.4) and serves
its contract C5 · Nightly promotion with a recorded stamp (amendment of 2026-11-16): **built and tested on branch `feat/nightly-promotion`, not merged, not deployed**

<!-- lands with ontomark step 10 -->
> Producer rename (EXP-10 step 10): the corpus build is `ontomark build`, a pinned released tag, and the `vault` name survives only as the governance adapter. The marker shapes, the served identity and the ledger described here are unchanged by the rename.

## Context

The estate has one corpus and four doors that disagree about it (PRD §1). Loom is the only door
that reasons, gates and stamps what it serves — and it is also the stalest: a bundle built
2026-08-22 with 8,146 classes against a vault carrying 8,433. ADR-140 §1.3 measured that and
ADR-140 D3 made the `ontology-bridge` re-point conditional on fixing it. The fix is not ours to
build: the sovereign-corpus one-shot replaces the upstream `jjohare/logseq` pipeline with a local
`vault build` over `/home/devuser/workspace/visionGraph`, which emits exactly the artefacts Loom
already loads (contract C3) plus a generation marker with a shape of its own.

Three further decisions land on Loom in the same change.

**Generation identity.** The corpus is no longer published from GitHub, so a generation named by a
bare ISO stamp (the mirror marker) or by a GitHub-anchored `wasDerivedFrom` asserts an origin the
bytes do not have. PRD Q8 fixes the identity as `visionGraph@<local sha>` plus a content digest.

**Vocabulary.** ADR-140 D4 minted `Mapping` and `Evidence` for the two content families
EvoOntology measured as load-bearing. The vault validates against Open Knowledge Format v0.2,
which already names both, and OKF's trust model (`generated` / `verified` by actor URI) is
strictly stronger than the single `corpusNature` honesty label Loom carries per unit: the label
asserted human direction uniformly and was never checkable per entry.

**The ledger had no writer.** `AttestationLedger` has been a build/CI-time port since ADR-136 D5.
Contract C5 ends the governance loop with `Loom AttestationLedger.attest({case_id, digest,
outcome})` — the node must expose a route, and the chain must outlive the restart that every
generation reload is.

And one retraction. ADR-140 P0 shipped a `loom-mcp-stdio` binary the same morning PRD Q10 settled
agent access estate-wide as **Rust CLI-first, no MCP inside the estate**.

## Decision

1. **The served generation is `visionGraph@<sha>`.** `loom-facade::mirror` reads the `vault build`
   commit marker (C3: `id`, `commit`, `content_digest`, `generated_at`, `class_count`,
   `page_count`, `vocabulary_version`, `stale_after`, `artifacts: [{name, sha256, bytes}]`) as a
   new `GenerationSource::VaultBuild`, and **still reads the legacy mirror marker** (`generation`,
   artefact map). This is a reader-side superset, not a shim in the served contract: there is one
   `/loom/generation` shape, one grounding envelope, and no consumer branches on which marker was
   on disk. The live node carries a mirror marker until the first vault-build promotion, and a
   reader that refused it would take the node down to gain nothing. `wasDerivedFrom` names the
   local repository — `file:///home/devuser/workspace/visionGraph@<sha>` — and never a GitHub URL.
2. **`stale_after` is carried and reported, never enforced.** `/health` gains
   `generation.stale_after` (the generation's own promise) and `generation.stale` (this node's
   judgement of it against its clock); the MCP manifest names `generation-stale` in its `degraded`
   list. The grounding contract is untouched: staleness is a property of the corpus, not of a
   retrieval, and a stale node answers exactly as it did before. A generation that declares no
   `stale_after` is not stale — it made no promise to break.
3. **ADR-140 D4 adopts OKF's vocabulary** (amended in place, dated): `Evidence` becomes
   `AttestedComputation { runtime, parameters, computation, executor{resource, receipt},
   attester{resource} }`; `Mapping` keeps its `Locus` enum and addresses by OKF `resource` rather
   than a second `iri`; `CoverageReport` reports `terms`, `attested_computations`,
   `verified_machine`, `verified_human`; and `CanonicalUnit.corpus_nature` is replaced by
   `Provenance { generated, verified }`, read from the scaffold index's OKF trust keys where
   present and otherwise `generated: process:vault, verified: []`.
4. **`POST /loom/attest` records one human-signed governance decision** —
   `{case_id, digest, outcome, signer, at}` → `201 {entry_id, chain_head}` — and
   `GET /loom/attest/verify` re-hashes the chain → `{ok, length}`. It compiles behind the `attest`
   feature, which is now **on by default**, and appends to `LOOM_LEDGER_PATH`
   (default `data/ledger.jsonl`) so the chain survives a reload. `passed` on an entry means *this
   decision admitted content into the served corpus*: true for `promote`, false for `demote`,
   `reject` and `expired`.
5. **`loom-mcp-stdio` is deleted.** Inside the estate agents use the `vault` CLI and
   `loom-client`; `/mcp` remains, and is kept solely as the door for hosts outside the estate,
   which are remote by definition and speak HTTP. A stdio transport serves an agent running beside
   the corpus — exactly the case that now uses the CLI — so it had no remaining consumer.
6. **The reload timer's precondition is named and its commands are written down.** It is enabled
   after the **first clean promotion of a `vault build` bundle** verified by one manual tick whose
   receipt reads `served`; `docs/published-generation-reload.md` carries the exact
   install/enable/disable commands, and `scripts/reload-published-generation.py` validates both
   marker shapes, refusing a declared `content_digest` that disagrees with the artefact set even
   when every individual file hashes correctly.

## Consequences

- Loom stops being the authoritative-but-stale door. Once the vault build promotes, `/health`,
  `vault build --stats` and VisionClaw's class count are the same number by construction, and the
  PRD's acceptance 2 becomes checkable rather than argued.
- A consumer can tell an old answer from a current one without reading the bundle: `/health` and
  the manifest say so. Nothing is refused for being stale, which keeps the failure mode "an
  honestly-labelled old answer" rather than "a node that went quiet".
- Per-unit trust replaces a corpus-wide honesty label. This is a **weaker** default claim —
  `process:vault`-generated and unverified — and that is the point: it is the claim we can
  actually support, and a human signature now shows up where it was earned.
- The ledger acquires a serving-path writer. It is one append under an internal lock, the route is
  feature-gated, and the file lives beside the corpus. A node built `--no-default-features` has no
  writer at all.
- Two marker shapes is real reader complexity. It is bounded (one function, table-documented in
  `ONTOLOGY-LOOM-PIPELINE.md`) and temporary by intent: it can be deleted once the live node has
  promoted a vault build, and this ADR is where to look for permission to do so.
- `CorpusNature` is gone from the public domain crate. Anything projecting `corpus_nature` must
  move to `provenance`; the field was never part of the markdown block, so THE PRIZE is unchanged.

## Verification

1. `crates/loom-facade/tests/adr141_generation_and_ledger.rs` — 7 integration tests: a vault-build
   marker activates and reports `visionGraph@<sha>` with commit, counts, vocabulary version and
   both digests; the legacy mirror marker still activates and reports the new keys as `null`; a
   past `stale_after` shows `generation.stale: true` on a node that still answers `200`, and
   `generation-stale` in the manifest's degraded list while the grounding status stays
   `navigated`; a signed decision ledgers and verifies; every outcome is recorded; the chain
   survives a restart; a case without an id or digest is refused and nothing is written.
2. `crates/loom-domain/src/okf.rs` — 5 unit tests over the OKF vocabulary: actor tiers read off
   the URI prefix (an unrecognised prefix is neither tier), the default provenance is
   vault-generated and unverified, human and machine verification are distinguishable, `Locus`
   round-trips as a tagged object, and an attested computation keeps executor and attester apart.
3. `tests/test_reload_published_generation.py` — 13 tests, five of them new, covering the C3
   marker: acceptance, a disagreeing declared content digest, an id that does not name its own
   commit, a missing C3 field, and a semantic sidecar declaring a different generation.
4. The frozen `tests/golden-python/` byte-parity goldens stay green — the injection plane is not
   permitted to drift while the corpus source changes underneath it.
5. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean, and
   `cargo clippy -p loom-facade --no-default-features` clean, so the ledger route really is
   optional rather than merely gated in prose.

## References

- VisionFlow `docs/PRD-sovereign-corpus.md` (Q8, Q10, Q11, §3.4) and
  `docs/engineering/sovereign-corpus-contracts.md` (C3, C5).
- ADR-135 (node boundary, generation discipline), ADR-136 D4/D5, ADR-138 (grounding contract),
  ADR-139 (per-request opt-out), ADR-140 (two planes; D1 and D4 amended here).
- `docs/design/ONTOLOGY-LOOM-PIPELINE.md` (amended: the two marker shapes, local `wasDerivedFrom`),
  `docs/published-generation-reload.md` (the timer precondition and commands).
- Open Knowledge Format v0.2 — the vocabulary D4 now speaks.

## Disposition — 2026-10-02

- **Suitability:** fits
- **Priority:** P1 — this cycle (demonstrator path, planning-cycle §9: the Loom is the door every consumer holds; TODO row N-1, corpus/vault landing)
- **Why:** The record is implemented (`1a6a470`, `4ec3a00` `promote_vault_build`) and in force on the live node. On 2026-10-02, `GET http://192.168.2.132:8084/loom/generation` returned `source: VaultBuild`, `id: visionGraph@ae913f93…`, `atomicity_verified: true` and `drift.ok: true`. `GET /loom/attest/verify` returned `{"ok":true,"length":0}`. D6's precondition, the first clean vault-build promotion, is met. Whether the reload timer is enabled is unverified. The served generation dates from 2026-09-23, while `visionGraph` has moved on (HEAD `015ca2c1f`), which suggests it is not. The ledger is empty: no governance decision has been attested yet. Verified against loom `main` at `8c618fa`.
- **Next:** Ready to accept on the live `/loom/generation` and `/loom/attest/verify` reads above. Before that, enable the reload timer per `docs/published-generation-reload.md` and record one `served` tick.
- **2026-10-02, later — next generation staged, not yet served.** `visionGraph@015ca2c1f`
  (9,366 classes) is built, promoted and verified in the agentbox workspace at
  `loom-publish/015ca2c1f/data`. `verified_bundle` accepts it with artefact digest `519692687fb1…`,
  and a local `loom-facade` over it satisfies `served_matches` (same id, digest, drift and
  embedding contract). The door still reports `visionGraph@ae913f93…`. HP is reachable only
  from the machinelearn host, so the copy, the manual tick and the timer install are owner
  steps, written down in `docs/published-generation-reload.md` § Publishing a new generation.
  The baked agentbox `vault` 0.1.0 predates the portable-records file and cannot produce a
  promotable bundle; build `vault` from source until the image is rebuilt. Status stays
  **Proposed** until the receipt reads `served` and the door reports the new id.

## Amendment proposed — 2026-10-12: identity is not distribution

Owner decision, recorded in [Ontomark ADR-OM-005](https://github.com/DreamLab-AI/ontomark/blob/ffe06e4/docs/adr/ADR-OM-005-estate-alignment-one-producer-explicit-projections.md). **Proposed**; this ADR's status is unchanged until the amendment is accepted.

"Never a GitHub URL" protected the *identity* of a generation from a mutable pointer. That stands: the served generation is still `visionGraph@<sha>`, content-addressed. What this amendment separates is **distribution**: a bundle carrying that identity and a sha256 may be served by Agentbox or attached to a release tag, and a consumer that verifies the digest has the same generation whichever path delivered it. The identity is in the bundle (`owl:versionIRI`, `.generation.json`), not in the URL.

The owner also records that identifiers need not be stable over time at this stage: re-minting across the synthetic corpus to best practice is permitted, provided each re-mint ships the identity mapping, visibility diff and rollback artefact VisionClaw ADR-2137 §4 requires. The ledger route keeps recording what was served under which id.

Producer: the bundle is produced by the released `ontomark` CLI (visionGraph `publish.yml`; VisionClaw's own build is retired behind gates). A **private** `--vault all` machine bundle, never published to the site, exists for VisionClaw's GPU view; the Loom continues to serve the public generation.

## Amendment — 2026-11-16: nightly promotion with a recorded stamp

Owner decision V-1 (16 November 2026): activate the semantic bundle with nightly promotion and a
recorded promotion stamp. **Status: built and tested on branch `feat/nightly-promotion`; not merged,
not deployed, nothing enabled on the Loom's host.**

**What was wrong.** On 2026-11-15 the door's `/health` reported `visionGraph@c2cef6c4…` (built
2026-10-21), 26 days and 196 commits behind `visionGraph`, with `promoted_at: null`. Nothing built or
installed a newer closure, and nothing recorded when or by whom the served one was installed. D6
named the reload timer's precondition but no step that produced a bundle to reload.

**What this amendment adds (all in `loom-facade`).**

1. `promote_generation` validates a built bundle, installs it with the existing `BundlePromoter`
   protocol (in-flight sentinel, write beside, rename, marker last) and appends one JSON line to
   `<data>/promotions.jsonl`. Validation is the strict reload check, plus: an `ontomark build`
   marker that declares a source commit; the scaffold index, prose index and both Turtle files parse;
   the commit exists in the source repository; and the bundle is not older than the installed
   generation (its commit is not a strict ancestor of the installed one) unless `--allow-rollback`.
   A refusal exits 1 and leaves the installed generation byte-for-byte untouched. The same
   generation already installed is a no-op (exit 0, no log line).
2. The marker it commits carries `promoted_at`, `promoted_by` and `previous_generation`. The service
   reads them with the rest of the marker, so `/health` (`generation.promoted_at`, `promoted_by`,
   `previous_generation`) and `/loom/generation` report them once a promoted bundle is loaded.
3. `deploy/loom-nightly-promotion.{sh,service,timer}` fetch visionGraph, build with the pinned
   release in `deploy/ontomark-release.env`, run `promote_vault_build` and `promote_generation`.

**What did not change, and why it matters.** Reload is still a process boundary (ADR-135 closeout):
a running process never swaps bundles, so it keeps serving the generation it loaded, and the reload
tick (D6) refuses to restart onto a bundle that does not verify. A promotion therefore takes effect
at the next restart, which the reload timer performs within five minutes; **the nightly is only
useful with the reload timer enabled**. A restart onto a corrupted data directory is handled by the start-up fallback below. Rollback is
`promote_generation --allow-rollback` on a retained bundle directory (see
`docs/published-generation-reload.md`).

**Drift corrected.** The operations document named `v0.1.0-rc.1` as the pinned `ontomark`; the pin is
`deploy/ontomark-release.env` (`v0.9.0`), which the nightly sources.

### Further decisions — 2026-11-16 (owner): start-up fallback, and the build flags

**Status: built and tested on branch `feat/nightly-promotion`; not merged, not deployed, no image
rebuilt, no timer enabled.** These two decisions supersede the paragraph above that left a start-up
fallback to the owner.

**A. Start-up fallback.** If the generation in the data directory fails to activate at start, the
service serves the most recent retained previous generation that validates, and says so everywhere.

- *Where previous generations are.* `loom-nightly-promotion.sh` keeps the newest `LOOM_KEEP_BUNDLES`
  (default 5) built bundles at `$LOOM_STATE_DIR/bundles/<commit>/data`. They are outside the data
  directory the container mounts, so the compose files mount that directory read-only at
  `/app/retained-bundles` and set `LOOM_RETAINED_BUNDLES_DIR`. Unset or missing means no fallback
  and the previous behaviour: refuse to serve.
- *Which one.* The promotion log (`<data>/promotions.jsonl`) is read newest first; the generation
  that failed is skipped; the first whose retained bundle passes promotion's own checks
  (`promote::validate_bundle`: strict reload check, an `ontomark build` marker naming a hex source
  commit, indexes and Turtle parse, id equal to the logged one) is served. A directory under the
  root that the log does not name is never chosen. The one promotion check it cannot repeat is that
  the commit exists in the source repository (the service has no checkout); the log line is its
  stand-in, because promotion wrote it only after that check passed.
- *Trigger.* Only an activation error with a commit marker present (a missing or mis-hashed
  artefact, a promotion in flight), or a marker file that is not valid JSON. A data directory with no marker at all
  still degrades as before, and the current generation's checks are not tightened.
- *Reporting.* `/health` gains `status` (`serving` or `fallback`) and a `fallback` block (failed
  generation or null if its marker is unreadable, the activation error, the generation served, and
  where from). `ok` stays `true` so that the container health check and clients keep the node:
  it answers, and what it serves is declared. `/loom/generation` carries the same `fallback` block;
  the loaded phase stays `activated` and never becomes `served`; `serving_bundle.disk_matches_loaded`
  is `false`; the MCP manifest lists `generation-fallback` as degraded; a warning is logged at start.
- *Reload tooling.* `served_matches` rejects any report with a `fallback` block, so the reload tool
  never records `served` for a fallback. If a restart lands in a fallback it exits 1 with
  `the service is serving a fallback generation ...` and leaves `reload-pending`. If the data
  directory itself no longer verifies, the tool refuses before touching the receipt, as it always
  has, so the last `served` receipt then describes the generation that was previously served, not
  the failed one; the non-zero exit and the `/health` fallback block are the signals.
- *Recovery.* No state is persisted. A successful `promote_generation` (a newer commit, or the
  same generation rebuilt, which repairs an installed copy that no longer verifies) replaces the data
  directory and the next restart serves it normally. The nightly's "already current" test uses
  `verify`, which fails on a corrupt install, so it rebuilds and promotes.
- *Never.* The service never writes into the data directory or a retained bundle. The failed bundle
  is left for inspection; the retained semantic database is copied to the runtime directory before it
  is opened (redb mutates on open), and when there is no writable runtime directory it is not opened
  and the semantic fallback is reported unavailable.
- *Cost.* Retained bundles are now the full publish-flag build (about 0.9 GB each with the markdown
  mirror and certificate; `data/` is about 160 MB of it), up to `LOOM_KEEP_BUNDLES` of them.

**B. Build flags.** The nightly builds with the flags of visionGraph's own publish workflow
(`.github/workflows/publish.yml` on `origin/main`, step "ontomark build → artefacts + content
staging"; no other workflow there calls ontomark), the same pinned `ontomark` (`v0.9.0`,
`deploy/ontomark-release.env`):

| | Flags |
|---|---|
| before | `ontomark --repo <clone> build --out <out> --with-rvdb --stats` |
| after | `ontomark --repo <clone> build --vault all --out <out> --publish-out <work>/publish --with-markdown-mirror --certificate-out <out>/certificate --stats --with-rvdb` |

From the publish workflow: `--vault all`, `--out`, `--publish-out`, `--with-markdown-mirror`,
`--certificate-out` (below `--out`, as there), `--stats`. Only the output paths differ (a private
directory; the staging tree sits beside the bundle so it is not retained). Loom addition:
`--with-rvdb`, because the bundle check requires `ontology-corpus.rvdb`, which the publish workflow
does not produce. `--repo` selects the clean clone and is not a build flag. The workflow's
`ontomark validate` and `repair bodies --check` are gates on the source and stay in visionGraph's CI.
The served bundle is unchanged by it: built both ways on the same visionGraph commit, `class_count`,
`page_count` and both digests in the marker are identical and the shared artefacts differ only in the
build-time stamp (`--vault all` widens only the `publish/` staging, which the nightly discards). The
retained bundles do grow, to about 0.9 GB each.

