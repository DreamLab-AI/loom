# Ontology Loom — the Generation-Identity Contract the Mirror Consumes

## Status: Contract (consumer-side). The **build authority moved upstream to `jjohare/logseq`**; this doc is the generation-identity contract the Rust `loom-facade` mirror consumes and verifies.
## Date: 2026-08-11 (authored); retargeted 2026-08-17 (Rust re-platform)
## Author: Loom capstone workstream (WS-A), retargeted for ADR-137 / PRD-027
## Depends on: OCP Revised Design Brief (2026-08-11); ADR-135 (façade + generation discipline); ADR-136 D4 (SSOT + atomic mirror); ADR-137 (Rust re-platform)
## Scope: **the contract** — `build-manifest.json`, `urn:ngm:generation:<sha>`, the atomic never-mixed generation, and how the Rust mirror verifies them. The *builder implementation* (`pipeline/*`, `publish.yml`) is **upstream in `jjohare/logseq`** and is reproduced below only as the contract the mirror relies on, not as Loom work.

---

> **Retargeting note (2026-08-17).** This document was authored as the WS-A *build-stage* spec
> back when the Loom vendored a `pipeline/` copy. That copy is retired (#21): the Loom is a
> **serving mirror, not a builder**, and the canonical builder is `jjohare/logseq` (`publish.yml`
> runs `pytest pipeline/tests` + `pipeline.validate` before deploy; `enrich-gate.yml` gates
> enrichment PRs). So the *authority* for everything in §3–§8 lives upstream; those sections stay
> here as the **contract the mirror consumes and verifies** — the manifest shape, the generation
> identity, the atomic never-mixed guarantee — not as a build to run in this repo. The Rust
> realisation of the consumer side is `app/mirror.sh` (still the shipped promote mechanism — the
> Rust node implements the *read* side of this contract) and `loom-facade::mirror` /
> `GenerationStore` (RUST-ARCHITECTURE §11.6): fetch the artifacts, verify their generation stamps
> cluster within tolerance, verify per-artifact sha256, promote atomically, expose the stamp on
> `/health` + `/loom/generation`. Read this doc as **"what a served generation guarantees, and how
> the mirror proves it,"** and read the builder detail as the upstream contract it checks against.

## 1. Context

The Loom is a portable VisionFlow node with a stable, model-swappable façade. In the pre-Rust
design it was described as *owning* the corpus lifecycle including the build; the Rust
re-platform sharpens that (ADR-137 D7): **the build is upstream (`jjohare/logseq`) and the Loom
is a serving mirror of its output.** What the Loom owns is the **consumer side** of the
generation contract: pull the published generation, verify it is one whole never-mixed
generation, serve it, and expose which generation it is serving. It satisfies agentbox ADR-112
(one-brain / no hot-path LLM) by construction: the upstream pipeline is the authoritative slow
path; the Loom's in-process retrieval reads *this* generation instead of re-deriving it.

The **generation-identity** the contract turns on: each upstream artifact must agree on one
generation stamp and one commit anchor, bound by a `build-manifest.json`. Without it a single
`www/` build carried several disagreeing timestamps and no commit anchor, nothing bound the
artifacts to the corpus commit, and a file-by-file mirror could expose a half-swapped
mixed-build `data/`. The contract closes all three: **one shared build context stamped into
every artifact (upstream), a `build-manifest.json` that names the generation, and an atomic
mirror that publishes `data/` as one verified generation (the Loom's job).** The Rust mirror's
realisation of the verify-all-then-flip discipline is `app/mirror.sh` + `GenerationStore`
(RUST-ARCHITECTURE §11.6); the sections below specify what it verifies against.

This is the root fix in the brief's dependency order (§WS-A): *"no envelope rule is expressible without it."* The distillation result envelope's `corpusSha_used` / `corpus_generation` fields (brief §"Result envelope"), the `corpusSha_match` admission rule (§"Corpus identity & the mirror"), and the content-address identity core (`corpusSha` is a hashed field, brief §"Content-address identity core") all resolve to the identity this stage mints.

## 2. Generation identity model

A **generation** is the immutable output of one pipeline run over one corpus commit. It is named by the commit and content-addressed by artifact hashes.

Introduce a single frozen context object in a new module `pipeline/manifest.py`, constructed once at the top of `build.py` and threaded into every emitter:

```python
# pipeline/manifest.py
@dataclass(frozen=True)
class BuildContext:
    commit_sha: str        # GITHUB_SHA in CI; `git rev-parse HEAD` locally; "0"*40 if dirty/absent
    generated_at: str      # ISO-8601 UTC; see §2.1 (reproducible: commit committer-date, not wall clock)
    build_id: str          # f"{commit_sha[:12]}-{pipeline_version}"
    pipeline_version: str   # single source: PIPELINE_VERSION ("ng-1.0.0")
    corpus_nature: str      # "synthetic-ai-generated-human-directed"  (corpus-honesty, §6)
    repo_iri: str           # "https://github.com/jjohare/logseq"

    @classmethod
    def from_env(cls, pipeline_version: str) -> "BuildContext": ...

    def version_iri(self) -> str:      # "urn:ngm:generation:<commit_sha>"
        return f"urn:ngm:generation:{self.commit_sha}"
    def derived_from_iri(self) -> str:  # "https://github.com/jjohare/logseq@<commit_sha>"
        return f"{self.repo_iri}@{self.commit_sha}"
```

`from_env` reads `GITHUB_SHA`; when absent (local dev) it shells `git rev-parse HEAD` and, if the tree is dirty, sets `commit_sha = "0"*40` and marks the generation **local/unpinnable** (a local generation must never be mirrored as authoritative — the mirror rejects the all-zero sha, §5).

### 2.1 Deterministic `generatedAt` (correctness — reproducible generations)

`generated_at` is derived from the **commit committer-date** (`git show -s --format=%cI <sha>`), normalised to UTC `Z`, **not** wall-clock `now()`. This is load-bearing: the manifest content-addresses each artifact by `sha256`, and an embedded wall-clock timestamp makes the artifact bytes — and therefore its hash — differ on every rebuild of the *same* commit, defeating both sha-pinning and the reasoner conformance test (§7). Sourcing the timestamp from the commit makes every artifact **byte-reproducible per commit**: rebuild commit `X` and you get identical hashes. Wall-clock is the documented fallback only for dirty local builds, which are already flagged unpinnable. This is the concrete meaning of the brief's "thread ONE shared timestamp + commitSha through every emitter."

### 2.2 `corpusSha` binding

`corpusSha` in the distillation job/result envelopes (brief §"Content-address identity core", §"Result envelope") **is** `BuildContext.commit_sha`. The mirror's active generation pointer (§5) is the same value, so `corpusSha_match: exact | at_least | latest` (brief §"Corpus identity & the mirror") resolves against the published `commitSha`. The pipeline emits full 64-char `sha256` artifact digests; the 12-char truncation (`sha256-12`) and the `urn:agentbox:job:*` / bead URN grammar are consumer-side concerns owned by agentbox `uris.js` (ADR-013 §6) — the pipeline neither mints nor truncates those.

## 3. `pipeline/manifest.py` — the build manifest (written LAST)

The manifest is the last artifact written, after every generation artifact exists and has been flushed. It walks the **generation set** (the `data/` and `api/` outputs that constitute the corpus generation — *not* the SPA/WASM assets, which are presentation, §7) and records identity + a hash/size/count triple per artifact.

`www/api/build-manifest.json`:

```json
{
  "schemaVersion": 1,
  "commitSha": "4f5e1150e84a4531...   (40-hex GITHUB_SHA)",
  "buildId": "4f5e1150e84a-ng-1.0.0",
  "generatedAt": "2026-08-11T12:41:07Z",
  "pipelineVersion": "ng-1.0.0",
  "corpusNature": "synthetic-ai-generated-human-directed",
  "versionIRI": "urn:ngm:generation:4f5e1150e84a4531...",
  "wasDerivedFrom": "https://github.com/jjohare/logseq@4f5e1150e84a4531...",
  "artifacts": {
    "data/ontology.ttl":            {"sha256": "…", "bytes": 4821334, "count": 98214},
    "data/ontology-inferred.ttl":   {"sha256": "…", "bytes": 1120044, "count": 21877},
    "data/ontology.json":           {"sha256": "…", "bytes": 39221114, "count": 5975},
    "data/scaffold-index.json":     {"sha256": "…", "bytes": 2103992, "count": 5975},
    "data/prose-index.json":        {"sha256": "…", "bytes": 5540012, "count": 2411},
    "data/graph/overview.json":     {"sha256": "…", "bytes": 41221, "count": 40},
    "data/graph/stats.json":        {"sha256": "…", "bytes": 3120, "count": null},
    "data/graph/full.bin":          {"sha256": "…", "bytes": 812004, "count": null},
    "data/graph/bridges.json":      {"sha256": "…", "bytes": 88210, "count": 612},
    "api/search-index.json":        {"sha256": "…", "bytes": 3221004, "count": 6144},
    "api/pages/":                   {"sha256": "…", "bytes": 61220114, "count": 6144}
  }
}
```

Rules:

- **`count`** is the semantically meaningful cardinality where one exists (JSON array length for `search-index.json`; `counts.classes` for `scaffold-index.json`; triple count for the TTLs; WebVOWL `class` count for `ontology.json`), else `null` (binary tiers, `stats.json`).
- **High-cardinality `api/pages/`** (~6k per-page files) is *not* enumerated file-by-file — that would bloat the manifest and race the atomic mirror. Instead the directory gets one **rollup entry**: `sha256` = SHA-256 over the newline-joined, path-sorted list of `"<relpath>  <filehash>"` lines (a flat Merkle digest), `count` = file count, `bytes` = total. The singleton generation artifacts get per-file entries. The mirror verifies per-file entries directly and verifies the page directory against the rollup digest (§5).
- **`versionIRI`** and **`wasDerivedFrom`** are the same strings the TTL headers carry (§4), so the OWL artifacts and the manifest agree on generation identity.
- Written LAST via a temp-write-then-rename inside `www/api/` so a reader never sees a partial manifest.

`build.py` integration (end of `build()`):

```python
from .manifest import BuildContext, write_manifest
ctx = BuildContext.from_env(PIPELINE_VERSION)
# … all existing stages, now receiving ctx (§4) …
write_manifest(output_dir, ctx)   # walks data/ + api/, hashes, writes api/build-manifest.json LAST
```

## 4. Threading the context through every emitter

Every emitter that currently stamps its own time takes `ctx` and stamps `ctx.generated_at` + generation identity instead. This is a mechanical change with a frozen list:

| File / function | Change |
|---|---|
| `reason.py::emit_inferred_ttl` | Replace `datetime.now(...)` (`:196`) with `ctx.generated_at`. Add to the `owl:Ontology` header: `owl:versionIRI <ctx.version_iri()>`, `owl:versionInfo <commit_sha>`, `prov:wasDerivedFrom <ctx.derived_from_iri()>`, `vc:corpusNature <ctx.corpus_nature>`. Keep `vc:inferenceMethod "transitive-subclass-closure"`. |
| `jsonld_to_turtle.py::build_graph` | Same header block on the `ontology.ttl` `owl:Ontology` node: `owl:versionIRI`, `owl:versionInfo`, `prov:wasDerivedFrom`, `vc:generatedAt`, `vc:corpusNature`. (Today `ontology.ttl` carries no version/provenance header at all — this is where the brief's "add `owl:versionIRI` + `prov:wasDerivedFrom <repo@sha>` to both TTL headers" lands.) |
| `scaffold_index.py::emit_scaffold_index` | Replace `datetime.now(...)` (`:96`) with `ctx.generated_at`; add top-level `"commitSha"`, `"pipelineVersion"`, `"corpusNature"`. Class-body caps (`DEFINITION_CAP=400`, `BACKLINK_CAP=20`) unchanged — they are the ADR-116 tier-budget discipline the Loom owns once (§9). |
| `prose_index.py::emit_prose_index` | Replace `datetime.now(...)` (`:87`) with `ctx.generated_at`; add `"commitSha"`, `"pipelineVersion"`, `"corpusNature"`. |
| `emit_graph_tiers.py` (`_build_overview`, `emit_graph_tiers` stats) | Replace `date.today().isoformat()` (`:885`, `:952`) with `ctx.generated_at`. `PROVENANCE.corpusNature` already ships (`:82`); add `commitSha` + `versionIRI` alongside it in `overview.json` and `stats.json`. `PIPELINE_VERSION` becomes `ctx.pipeline_version` (single source). NGG1 binary layout is untouched — the golden fixture `pipeline/tests/fixtures/ngg1-3n2e.bin` still pins byte-for-byte; only the JSON date/identity fields change, and the overview-contract test tolerates an injected date. |
| `jsonld_to_page_api.py::build_page_api` | Stamp `_domain-index.json` with `{"commitSha", "generatedAt", "pipelineVersion", "corpusNature", "domains": …}` (today it is a bare map). Per-page files stay shape-frozen (SPA contract); identity lives in the manifest + domain index. |
| `jsonld_to_search.py::build_search_index` | **Envelope change.** Today it returns a bare `list[dict]`. Wrap: `{"version": 1, "commitSha", "generatedAt", "pipelineVersion", "corpusNature", "count", "entries": [...]}`. This is a breaking shape change for the SPA fetch in `publishing-tools/WasmVOWL/modern` (`pageService.ts`) — that reader must switch to `.entries`. Coordinated in the same PR; see §11 open item. |

`PIPELINE_VERSION` currently lives in `emit_graph_tiers.py:45`. Promote it to `pipeline/manifest.py` (or a tiny `pipeline/version.py`) as the single source, imported by both.

## 5. The atomic mirror (`mirror.sh`) — kills the mixed-build window

The Loom host (HP reference deployment, or a `visionclaw_network` sidecar — brief §"Deployment topologies") pulls the published generation locally so scaffold retrieval and jobd read from a **whole, verified** `data/`. Today's naive `cp`-per-file leaves a window where a consumer reads half of build *N* and half of build *N+1*. The manifest makes atomicity expressible; `mirror.sh` implements it as **verify-all-then-flip**:

```sh
#!/usr/bin/env sh
set -eu
SRC="https://narrativegoldmine.com"          # cloud read-replica; or the Loom façade
ROOT="$HOME/loom/corpus"                       # generations/<sha>/ + a `current` symlink
TMP="$(mktemp -d "$ROOT/.stage.XXXXXX")"

# 1. Manifest FIRST — it names the generation and every hash.
curl -fsS "$SRC/api/build-manifest.json" > "$TMP/build-manifest.json"
SHA="$(jq -r .commitSha "$TMP/build-manifest.json")"
[ "$SHA" = "0000000000000000000000000000000000000000" ] && { echo "refusing unpinnable local generation"; exit 3; }

# 2. Already current? No-op (corpus changes slowly; stale reads are fine, brief §0).
[ "$(readlink -f "$ROOT/current" 2>/dev/null | xargs -r basename)" = "$SHA" ] && { rm -rf "$TMP"; exit 0; }

# 3. Fetch every artifact into the staged generation and verify sha256 BEFORE it is ever visible.
GEN="$ROOT/generations/$SHA"
jq -r '.artifacts | to_entries[] | "\(.key)\t\(.value.sha256)"' "$TMP/build-manifest.json" |
while IFS="$(printf '\t')" read -r path want; do
  case "$path" in
    */) fetch_dir_and_verify_rollup "$SRC" "$TMP" "$path" "$want" ;;   # api/pages/ Merkle rollup
    *)  curl -fsS "$SRC/$path" > "$TMP/$path.part"
        got="$(sha256sum "$TMP/$path.part" | cut -d' ' -f1)"
        [ "$got" = "$want" ] || { echo "HASH MISMATCH $path"; rm -rf "$TMP"; exit 4; }   # fail-labelled: keep old generation
        mkdir -p "$(dirname "$TMP/$path")"; mv "$TMP/$path.part" "$TMP/$path" ;;
  esac
done

# 4. Stamp the generation pointer and flip ATOMICALLY (single rename of a symlink).
mkdir -p "$(dirname "$GEN")"; mv "$TMP" "$GEN"
jq '{commitSha,generatedAt,corpusNature,pipelineVersion}' "$GEN/build-manifest.json" > "$GEN/.generation"
ln -sfn "$GEN" "$ROOT/current.new" && mv -T "$ROOT/current.new" "$ROOT/current"   # atomic swap

# 5. Retain N generations for rollback; prune the rest.
ls -1dt "$ROOT/generations"/*/ | tail -n +6 | xargs -r rm -rf
```

Properties:

- **No mixed-build window.** Consumers always read through `current/`, which flips in one atomic `rename(2)` only after *every* artifact hash verified. A failed fetch or hash mismatch aborts and leaves the previous generation live (fail-labelled, never fail-open — brief §constraints).
- **`corpusSha` resolution** (brief §"Corpus identity & the mirror") reads `current/.generation.commitSha`. jobd's admission compares `job.corpusSha` to it; on mismatch it invokes `mirror.sh` once on demand (cron cadence irrelevant); `exact` + still-mismatched → terminal `corpus-unavailable`. That admission logic is WS-E (HP jobd) — this doc owns the pointer and the atomic guarantee it reads.
- **Retention** keeps the last 5 generations so a distillation job pinned to `at_least: <older sha>` can still resolve, and so a bad publish can be rolled back by re-pointing `current`.

## 6. Corpus-honesty — `corpusNature` propagates into every generation

`emit_graph_tiers.py` already single-sources `CORPUS_NATURE = "synthetic-ai-generated-human-directed"` (`:67`) into `overview.json` / `stats.json` `provenance`. WS-A **widens** that to the whole generation so the honesty label cannot be stripped by reading any single artifact:

- `build-manifest.json` carries top-level `corpusNature` (§3).
- Both TTL headers carry `vc:corpusNature` (§4).
- `scaffold-index.json`, `prose-index.json`, the search envelope, and `_domain-index.json` each carry `corpusNature`.
- The mirror's `.generation` stamp carries it, so jobd copies it into every result envelope's `corpusNature` field (brief §"Result envelope") without re-deriving.

This is the pipeline-side half of the brief's rule that `corpusNature` "must propagate into build-manifest and every generation" — the distillate can never present as anything other than synthetic-AI-generated-human-directed, sourced from data rather than hand-typed copy (ADR-NG-001 §7 honesty posture, already CI-gated in `publish.yml`).

## 7. One reasoner authority + conformance test (OPEN DECISION)

The corpus is reasoned **twice** in the estate today and this caused real drift (the `8152-vs-5975` class-count divergence; brief §0b): this repo's `reason.py` computes a **transitive `subClassOf` closure** (cycle-safe BFS, `INHERITED_RELATION_CAP=8`) and emits `ontology-inferred.ttl`; VisionClaw runs **Whelk EL++** and asserts a `:inferred` graph. The target invariant is **one authority, conformance-tested so the two never drift again** (brief §0b).

**OPEN DECISION (owner: operator; lands in VisionClaw ADR-135, the façade-contract / one-reasoner decision):** which engine is canonical — promote Whelk into the Loom and make `reason.py` a conformance oracle, or make `reason.py` authoritative and have VisionClaw load `ontology-inferred.ttl` as its `:inferred` source. This doc does **not** pick; it specifies the anti-drift mechanism that must hold *whichever* wins.

**Conformance contract (`pipeline/tests/test_reasoner_conformance.py`, CI-gated).** For a pinned golden corpus fixture:

1. Run `reason.py::compute_closure` → normalise to the set `P_py = {(sub, super)}` of inferred (non-direct) `subClassOf` pairs.
2. Run the other engine (Whelk EL++) over the *same* corpus → its inferred `subClassOf` set `P_whelk`.
3. Assert the **precise subset relationship**, not naive equality: `reason.py` implements only transitive `subClassOf`, whereas EL++ additionally infers via `equivalentClass`, existential subsumption and property chains. So the contract is:
   - **`P_py ⊆ P_whelk`** — every pair `reason.py` emits MUST be inferable by EL++. A pair in `P_py \ P_whelk` is a **drift bug → hard fail** (the transitive path emitted something the sound reasoner rejects).
   - **`P_whelk \ P_py`** is the **documented EL++ enrichment delta**, checked into `pipeline/tests/fixtures/reasoner-delta.json` and classified. An *un*documented new entry there → fail (forces a human to look before the two engines silently diverge again).

"Same corpus in → identical inferred closure out, or an explicit documented delta" is exactly this: identity on the transitive-`subClassOf` fragment, an enumerated-and-reviewed delta on the EL++ remainder. The test is the wire that keeps `ontology-inferred.ttl` and VisionClaw `:inferred` derived from **one** closure (brief §0b: "published `ontology-inferred.ttl` AND VisionClaw `:inferred` both DERIVE from that one generation").

## 8. `conflicts.py` as the Loom pre-publish gate

`pipeline/conflicts.py` already exposes the typed `ConflictReport` (`analyse(pages) → ok()/blocking()/exit_code/to_json`) and is wired into `publish.yml` as the **semantic conflict gate** (`--severity high` hard-fails on `SUBCLASS_CYCLE` / `DUPLICATE_CONCEPT`; `RELATION_CONTRADICTION` / `TYPE_CONFLICT` reported non-blocking). In the Loom model this is the **pre-publish gate**: no generation reaches the manifest/mirror carrying a high-severity semantic conflict. It composes with — does not duplicate — VisionClaw Whelk **consistency** as the **pre-assert gate** (Loom §0b: conflicts.py = Loom pre-publish gate; Whelk consistency = Loom pre-assert gate — composed, not duplicated).

Concretely, `build.py` should fold the report into the build result so a blocking conflict fails the build *before* `write_manifest`, making "no generation is minted from a conflicted corpus" a structural guarantee rather than a separate CI step that could be reordered:

```python
from .conflicts import analyse
report = analyse(pages, severity_gate="high")
if not report.ok():
    print(report.to_json()); raise SystemExit(report.exit_code)   # gate BEFORE emit + manifest
```

The existing `publish.yml` step stays (defence in depth); the point is that the gate is now *inside* the generation-mint boundary. This keeps the write path from being widened (brief §constraints): the gate only *blocks*, it never rewrites the corpus.

## 9. Binding constraints honoured

| Constraint (brief §constraints) | How this stage honours it |
|---|---|
| agentbox ADR-112 one-brain / no hot-path LLM | Pipeline is pure `rdflib`+stdlib, batch-CI, zero LLM/network at build. It is the slow authoritative path; in-process retrieval stays the fast path and reads *this* generation (§1, brief §0b). |
| agentbox ADR-116 tier budgets | Index caps owned once, here: `DEFINITION_CAP=400`, `BACKLINK_CAP=20` (scaffold), `DEFAULT_CL_CAP=1500` (prose), `INHERITED_RELATION_CAP=8` (reason), `RELATION_TOPK`/`MAX_NODES` (tiers). Consumers stop re-deriving indices. |
| write-path-never-widened | This stage only reads corpus + writes `www/`. Corpus write-back (elevation) is WS-H through the PRD-020 ADR-121 propose spine — not a new pipeline door. |
| fail-labelled-not-fail-open | Conflict gate hard-fails high severity; mirror aborts on any hash mismatch and keeps the prior generation; local/dirty builds are flagged unpinnable, never mirrored. |
| URI/DID grammar closed | Pipeline emits `urn:ngm:class:*`, `urn:ngm:generation:<sha>`, `did:nostr:jjohare` (existing `ATTRIBUTED_TO`) and full 64-hex `sha256`. Bead/job URNs, `sha256-12`, `did:nostr` wrapping are agentbox `uris.js` (ADR-013 §6) — not minted here. |
| corpus-honesty `corpusNature` | Propagated into manifest + every generation artifact (§6). |
| SPARQL SERVICE forbidden | The static site serves TTL as a downloadable artifact only; no live query endpoint, no federation (§10). |
| RuVector MCP-only | The pipeline writes files, never RuVector. Ingesting `scaffold-index`/`prose-index` into RuVector (the "RuVector-condense" of brief §0b) is a **Loom-host lifecycle task over MCP**, keyed by the same `commitSha` so the vector store version matches the published generation. Not part of `publish.yml` (CI has no MCP). |
| agentbox ADR-090 ring order | Out of scope for the build stage; the pipeline emits no cross-ring calls. |

Consumer-side security/correctness posture (sig-verify at the derived door **and** the RuVector read, distiller-provider allowlist, strict-nip98 provider door, CAS bead close + `lease_epoch` fencing, outcome-aware `getReady`, reconciliation janitor + deadline reaper, RuVector TTL law, no-synchronous-await law, release-build CI gate, NIP-59 gift-wrap) is **out of scope for WS-A** but *anchored* by it: those checks verify BIP-340 envelopes whose `corpusSha_used` / `corpus_generation` pin to this manifest's `commitSha`, and the sig binds a distillate to a specific, hash-verified generation. The distillation result envelope is carried on VisionClaw ACSP kind **31409 `DistillJobResult`** (allocated NEW alongside **31408 `DistillJobRequest`**; 31406/31407 SPARQL `semantic_query` untouched — brief §"Cross-repo hygiene") — the pipeline provides the generation those envelopes reference, nothing more.

## 10. What does NOT change

- **`publish.yml` still builds `www/` and deploys to `DreamLab-AI/knowledgeGraph` gh-pages** at narrativegoldmine.com. WS-A adds two things — the `BuildContext` threading (§4) and a final `manifest` stage (§3) — and folds the conflict gate inside the mint boundary (§8). The honesty gate, pytest/cargo gates, WASM+React build, the markdown-mirror-by-Title count gate, and the Playwright explorer smoke all stay exactly as they are.
- **The static site stays a static cloud read-replica.** No live SPARQL, no `SERVICE`, no query engine. `ontology.ttl` / `ontology-inferred.ttl` remain downloadable artifacts. In Loom terms the published site is the always-available fallback: corpus changes slowly, stale reads are fine, and only fresh distillation pauses when the Loom host is down (brief §0).
- **NGG1 binary tiers stay byte-frozen** (`FORMAT-NGG1.md`, golden fixture `ngg1-3n2e.bin`). WS-A changes only JSON identity/date fields in `overview.json` / `stats.json`, never the `.bin` layout.
- **Per-page API JSON shape** (`jsonld_to_page_api.py`) stays frozen for the SPA; generation identity lives in the manifest and `_domain-index.json`, not in per-page files.

## 11. Open decisions (for the operator)

1. **Canonical reasoner engine** (§7): promote Whelk EL++ into the Loom vs make `reason.py` authoritative + VisionClaw loads `ontology-inferred.ttl`. The invariant (one authority + conformance delta) holds either way; the choice is VisionClaw ADR-135's. This doc ships the conformance test regardless.
2. **`corpusSha` = raw `commitSha` vs a content digest.** Recommended: keep `commitSha` (`GITHUB_SHA`) as the generation *identity* and add a derived `corpusDigest` = RFC 8785 JCS canonicalisation over the `artifacts` map for a *content* address that is stable even if two commits produce identical corpus output. The job-envelope content-address core already uses RFC 8785 JCS (brief §"Content-address identity core"); reusing it here keeps one canonicalisation across the estate. Flagged for confirmation.
3. **Search-index envelope migration** (§4): wrapping the bare array is a breaking change for `pageService.ts`. Land the wrap + SPA reader change in one PR, or ship a sidecar `search-index.meta.json` and defer the wrap. Recommended: wrap now, it is a one-line SPA change and the brief calls for the envelope.
4. **Manifest artifact scope for `api/pages/`**: the flat-Merkle rollup (§3) vs a separate `api/pages/manifest.json`. Rollup keeps the atomic mirror simple; revisit if a consumer needs per-page pinning.

## 12. WS-A build order

1. `pipeline/manifest.py`: `BuildContext` + `write_manifest` + per-directory rollup hashing. Unit-test hashing determinism (same commit → identical manifest, modulo nothing).
2. Thread `ctx` through the seven emitters (§4 table); replace all `datetime.now`/`date.today` call sites.
3. TTL provenance headers in `reason.py` and `jsonld_to_turtle.py` (§4).
4. Fold `conflicts.analyse` into `build.py` before emit (§8).
5. `mirror.sh` verify-all-then-flip on the Loom host (§5); systemd/cron the on-demand refresh path for WS-E jobd.
6. `pipeline/tests/test_reasoner_conformance.py` + `fixtures/reasoner-delta.json` (§7); wire into `publish.yml` alongside the existing pytest gate.
7. SPA `pageService.ts` search-envelope reader change (§11 item 3).

Everything above is additive to a working pipeline: the generation this stage mints is the identity every downstream Loom workstream — corpus mirror, distillation `corpusSha` admission, signed result envelopes, corpus elevation — is defined against.
