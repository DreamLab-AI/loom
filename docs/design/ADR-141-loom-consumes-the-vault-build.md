# ADR-141 — The Loom consumes the vault build: `visionGraph@<sha>`, OKF vocabulary, a ledger route, stdio dropped

Status: Proposed · 2026-09-22 · Extends ADR-135 (generation discipline), ADR-136 D4 (atomic
mirror), ADR-138 (grounding contract) and ADR-140 (the two planes) · Amends ADR-140 D1 and D4 in
place · Realises the Loom half of VisionFlow **PRD Sovereign Corpus** (Q8, Q10, §3.4) and serves
its contract C5

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
