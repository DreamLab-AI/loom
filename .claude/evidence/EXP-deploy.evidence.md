# EXP-deploy — Stage 5 deployment layer evidence

**Verdict: PASS** — the multi-stage image builds end-to-end from the parent
context, the binary starts, and `/health` reports `index_classes: 8146` inside a
build-time smoke stage with the real generation baked in.

- Date (UTC): `2026-08-17T15:58:08Z`
- Repo HEAD at build: `0c63a1d72e4a95fa9db0b6c81c1d8f468b21260f` (branch `main`)
- Host toolchain (known-green): `cargo 1.97.0 (2026-06-30)` / `rustc 1.97.0`
- Docker: `29.6.2`, BuildKit (buildx 0.35). `DOCKER_CONFIG=/tmp/.docker-loom`
  (the container `$HOME` is read-only; the docker CLI needs a writable config dir).

## What was built

| Image | Target | Size | Notes |
|---|---|---|---|
| `loom:rust` | `runtime` | **102 MB** | the deployable artifact; non-root uid 65532; 13 MB binary |
| `loom:smoke` | `smoke` | 258 MB | runtime + baked 8146-class generation (build-time self-test only) |
| `loom:builder` | `builder` | 3.01 GB | intermediate compile stage, never shipped |

Binary + user inside `loom:rust`:
```
-rwxr-xr-x 1 root root 13M /usr/local/bin/loom-facade
uid=65532(nonroot) gid=65532(nonroot) groups=65532(nonroot)
```

## Build invocation (parent-context — load-bearing)

The facade path-depends on a SIBLING workspace
(`ruvector-core = { path = "../ruvector/crates/ruvector-core" }`), and a Docker
`COPY` cannot escape its context, so the build context is the PARENT of the repo:

```
DOCKER_CONFIG=/tmp/.docker-loom DOCKER_BUILDKIT=1 \
  docker build -f loom/deploy/Dockerfile -t loom:rust /home/devuser/workspace
# just docker-build wraps exactly this (context = justfile_dir/..).
```

BuildKit honours `deploy/Dockerfile.dockerignore` (an allowlist), so the context
transfer is **65 MB**, not the whole parent tree:
```
#6 [internal] load build context
#6 transferring context: 65.28MB done
```

### ruvector path-dep resolution

`ruvector-core` inherits 31 fields via `workspace = true`, so cargo needs a
ruvector workspace root to resolve inheritance. The REAL root lists ~120 members
we do not vendor, so the Dockerfile COPYs `deploy/ruvector-workspace.trimmed.toml`
over `/build/ruvector/Cargo.toml` — `members = ["crates/ruvector-core"]` with
`[workspace.package]` + `[workspace.dependencies]` kept byte-for-byte so
`--locked` resolution against `loom/Cargo.lock` is identical. The `[patch.crates-io]
hnsw_rs` override is dropped: loom is the root being built (only its patches
apply) and `loom/Cargo.lock` already pins `hnsw_rs 0.3.4` from the registry, so
the patch is inert here. Verified standalone via `cargo metadata` before building.

## musl vs glibc decision — **glibc (documented deviation from §13)**

§13 aspires to one static `x86_64-unknown-linux-musl` binary on `FROM scratch` /
distroless-static. We **deviate to a glibc build + `debian:bookworm-slim`
runtime**, forced by two independent facts:

1. **RocksDB C++.** `oxigraph 0.4` pulls `oxrocksdb-sys` (RocksDB, built via
   `cc`/`bindgen`); `ruvector-core` pulls `simsimd` (C SIMD). A static musl build
   of RocksDB needs a full musl C++ cross-toolchain + static libstdc++ — fragile
   and out of scope for a serving image.
2. **The entrypoint needs a shell.** The rvdb ro-mount hazard mitigation (below)
   is a `/bin/sh` script; distroless/static and distroless/cc have no shell, so
   they could not run it regardless.

`debian:bookworm-slim` gives glibc + libstdc++ + a shell + coreutils
(`cp`, `sha256sum`) for a few extra MB. Non-root, minimal, single binary — just
not `FROM scratch`. 102 MB total is a reasonable outcome.

### Second toolchain finding (real bug caught)

First build on `rust:1.88-bookworm` (the workspace `rust-version` floor) FAILED:
`ruvector-core/src/simd_intrinsics.rs` uses AVX-512 intrinsics (`_mm512_loadu_ps`,
`#[target_feature(enable = "avx512f")]`) that only **stabilised in Rust 1.89**
(E0658: `stdarch_x86_avx512` / `avx512f` unstable). The `simd` feature is enabled
on the ruvector-core path dep. Fixed by pinning the builder to `rust:1.97-bookworm`
to match the known-green host toolchain. (rust-version 1.88 remains the crate
metadata floor, but the `simd` feature needs ≥1.89 in practice.)

## rvdb read-only-mount HAZARD handling (verified stage 3, solved at container level)

Opening `data/ontology-corpus.rvdb` via ruvector-core `VectorDB` **mutates the
redb file even for reads** (it rebuilds/repacks the HNSW on open). Both compose
profiles mount the generation `:ro`. Zero code change permitted → solved in
`deploy/entrypoint.sh`:

1. copy the `.rvdb` (+ `.generation.json` sidecar) from the `:ro` `/app/data`
   into a writable `/run/loom` (tmpfs in compose);
2. **belt-and-braces** — if the sidecar records a `sha256`, verify the copy
   against it BEFORE first open (the current sidecar carries `classCount`/
   `generatedAt` only, so this logs "skipping digest verify" and activates
   automatically once the mirror stamps a digest);
3. repoint `LOOM_HNSW_ARTIFACT` at the writable copy, then `exec` the binary.

Everything else in `/app/data` (scaffold/prose JSON, TTLs) is read immutably and
stays on the `:ro` mount. The oxigraph store is **in-memory** (`Store::new`, TTLs
`File::open`'d read-only) so it introduces no second write hazard.

The smoke run proves the mitigation end-to-end — note the redb rebuild targets the
**writable copy**, never the `:ro` mount:
```
loom-entrypoint: copying rvdb from RO mount → writable /run/loom/ontology-corpus.rvdb (redb mutates on open — must not touch the :ro mount)
loom-entrypoint: sidecar records no sha256 — skipping digest verify (copy still isolates the RO mount)
loom-entrypoint: LOOM_HNSW_ARTIFACT repointed → /run/loom/ontology-corpus.rvdb
loom-entrypoint: exec loom-facade (profile=a port=8080)
ruvector_core::vector_db: Index rebuilt successfully
loom_facade: HNSW artifact ready artifact=/run/loom/ontology-corpus.rvdb enabled=false
```

## Smoke `/health` (build-time, no bind mount)

A `docker run -v` smoke is untrustworthy here: bind-mount paths resolve on the
HOST filesystem from inside the agentbox, so it would mount the wrong tree. The
smoke stage instead COPYs the real generation in at build time and curls `/health`
during a `RUN`:

```json
{"backend":null,"backend_reachable":null,"deploy_profile":"a",
 "generation":{"class_count":8146,"source":"ScaffoldIndex","generated_at":"2026-08-15T13:22:49.334017+00:00"},
 "graph":{"available":true,"loaded_files":["ontology.ttl","ontology-inferred.ttl"],"triples":282492},
 "index_classes":8146,"mode":"scaffold","ok":true,
 "semantic":{"generation":{"class_count":8146,"source":"MirrorManifest","verified_single_generation":true},"ready":true}}
```
```
test 1 = 1
grep -q "index_classes":8146 /tmp/health.json
SMOKE PASS: index_classes=8146
#44 DONE 4.5s
#45 writing image sha256:01292c811cfe0210ec958f714b4f810712814fbc3a9d24cc510d697536ce8d0c done
```

Observed and correct for a retrieval-only smoke (no `DISTILL_BACKEND_URL`, no
Xinference in the build sandbox): `backend:null`; `embedder verify failed
(non-fatal)` — fail-open; `index_classes:8146`; graph loaded 282,492 triples;
semantic artifact ready but `enabled=false` (gated off until the recall bench).

## cargo-chef

Evaluated and **dropped**. The recipe would have to span an 8-crate intra-workspace
path graph PLUS a cross-workspace path dep into `../ruvector` whose crate inherits
from a trimmed synthetic root — chef's skeletoniser is brittle across that seam.
Replaced with plain manifest-first layer caching (copy every `Cargo.toml` + the
lock, stub each crate's declared lib/bin, build deps only, then copy real sources).
Transparent, deterministic, and the heavy RocksDB/oxigraph/ruvector-core compile
lands in a reusable layer.

## Deliverables (owned paths only)

- `deploy/Dockerfile` — 3-stage (builder glibc → runtime debian-slim → smoke)
- `deploy/entrypoint.sh` — rvdb copy + sha-verify + exec
- `deploy/compose.profile-a.yml` — host-colocated HP (`network_mode: host`, :8084)
- `deploy/compose.profile-b.yml` — sidecar on `visionclaw_network` (:8080)
- `deploy/ruvector-workspace.trimmed.toml` — vendored trimmed ruvector root
- `deploy/Dockerfile.dockerignore` — allowlist for the parent build context
- `.dockerignore` — repo-root ignore (context=loom fallback)
- `justfile` — build/test/clippy/deny/bench + docker-build/-smoke/-run-a/-run-b + ci

## Divergences from §13 (all documented above)

1. glibc `debian:bookworm-slim` runtime instead of static-musl `scratch`/distroless
   (RocksDB C++ + shell-in-entrypoint).
2. Builder pinned to Rust 1.97, not the 1.88 floor (avx512 intrinsics need ≥1.89).
3. cargo-chef dropped for manifest-first caching (cross-workspace path-dep seam).
4. Compose volume path is `../data` (compose files live in `deploy/`), pointing at
   the fully mirrored generation dir; §13 shows `./data` from the repo root.
