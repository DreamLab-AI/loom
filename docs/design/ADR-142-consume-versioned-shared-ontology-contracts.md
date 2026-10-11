---
id: ADR-142
title: Consume versioned shared ontology contracts without moving authored authority
date: 2026-10-11
decision_status: proposed
implementation_status: partial
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
verified_paths: []
owner: jjohare
review_trigger: Before shared workspace extraction or any consumer dependency or generation migration
repo: loom
---

# ADR-142 — Consume versioned shared ontology contracts without moving authored authority

## Context

Loom consumes asserted/inferred Turtle, scaffold/prose indexes and generation markers produced elsewhere. It verifies an activated bundle and reports loaded content. A shared ontology workspace can align these contracts without moving authorship into Loom or making its graph store mandatory for authoring.

Estate proposal: [VisionFlow ADR-2014](../../../VisionFlow/docs/adr/ADR-2014-shared-ontology-workspace.md). Evidence: [workspace investigation](../../../visionGraph/planning/ontology-workspace/README.md). Proposed extension to [ADR-141](ADR-141-loom-consumes-the-vault-build.md) and [pipeline contract](ONTOLOGY-LOOM-PIPELINE.md); no accepted contract is superseded.

## Decision

This extension is partially implemented. Step 10 uses the shared typed index contracts and the pinned publishing CLI; IRI and provenance adoption remain separate migrations. Existing accepted invariants remain in force, and no live activation is claimed.

<!-- lands with ontomark step 10 -->
When the ontomark migration (EXP-10) lands, the extension is adopted in three steps. Class IRIs are minted by one function that follows `ontomark-model` (step 2). The provenance default names the producer from the bundle manifest through `ontomark-compile`, taken without its reasoner (step 6). The scaffold-index and prose-index readers take their typed structs from `ontomark-compile` (step 10, decision D9 of the deprecation inventory, not step 6), and the publishing runbook builds with a pinned released `ontomark` (step 10). Once all three steps are verified, the front matter becomes `decision_status: accepted` and `implementation_status: implemented`. `activation_status` becomes `active` only when the first clean promotion of an `ontomark build` bundle is served and its reload receipt reads `served` (the event in `docs/published-generation-reload.md`), and `verified_commit` and `verified_paths` are filled from that run. Until all three migration steps are verified, this ADR stays proposed; step-10 implementation is recorded as partial and activation stays inactive. The reload binary is step 11 and is not part of this adoption.

1. Consume versioned bundle contracts from the independent ontology workspace. Markdown and machine-readable headers remain the sole authored ontology authority. Oxigraph, lexical/vector indexes and loaded bundles remain rebuildable serving projections; Loom does not become an alternate ontology authoring database.
2. Prefer the existing artefact boundary initially. Shared contract crates may replace duplicated schema/identity logic only after licence and API review. This MIT/Apache workspace must not silently import the producer's AGPL implementation. No copy of ontology parsing or reasoning is introduced to avoid that boundary.
3. Preserve rich optional estate capabilities independently of the Obsidian plugin. Consumers negotiate or reject unsupported contract versions explicitly. Open-world truth, asserted/inferred basis, provenance and ontology version IRI survive projection; an empty response does not become a negative assertion.
4. Retain stage, digest verification and activation as separate operations. Report the loaded generation and ontology version IRI, not merely the latest files on disk. Coordinate producer release, reader compatibility and reload receipt; retain the prior bundle for rollback. Source commit alone is insufficient identity when vocabulary, policy or emitter versions change.
5. Test the same released fixture bundle across native, WASM and serving consumers. A compatible optimisation or richer native reasoner must label its semantic profile and completeness. Existing-vault proposal automation may use established external governance; OCR and document ingestion are out of scope.

## Consequences

Loom keeps its serving architecture and licence boundary while gaining an explicit producer compatibility contract. Shared types reduce drift only when they describe actual consumed bytes; wholesale dependency merger would increase coupling. Optional indexes remain valuable, but rebuilding them must never require reconstructing authored knowledge from a database.

## Verification

Step-10 reader and publishing adoption is implemented: `loom-scaffold` aliases the producer's scaffold/prose structs, binds its own OKF trust types, and disables the compiler reasoner. `deploy/ontomark-release.env` pins the CLI tag and asset digest; promotion refusal text names `ontomark build`. The release-identity reader also supports linked worktrees and watches the selected branch ref. Native fixture, lifecycle and migration contracts verify these behaviours. No IRI/provenance adoption or activation is claimed. Earlier source inspected at `37a320ce3f15787a1ee10fac0faae043ab1ac12e`: `crates/loom-graph-oxigraph/src/lib.rs:38` fixes the Turtle allowlist; `crates/loom-facade/src/bundle.rs:16` defines verification and loaded identity. Before adoption run graph allowlist/inferred-closure integration tests, `mirror_generation`, MCP ancestor/provenance tests and producer-fixture compatibility. Add vocabulary-only and emitter-only identity changes, unsupported-version rejection, mixed-bundle rejection and rollback/restart tests. A reload receipt must prove the intended generation is served; no deployment was exercised here.

<!-- lands with ontomark step 10 -->
After step 10, the verification record names the executed evidence: the three migration tests (`ontomark_iri_migration`, `ontomark_rdf_migration`, `ontomark_cli_publish_migration`) green, the pre-existing suites listed above green, and the receipt of the first served `ontomark build` bundle. Those fields stay empty until that evidence exists.
