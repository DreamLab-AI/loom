# syntax=docker/dockerfile:1.7
# =============================================================================
# Ontology Loom — Rust serving node (RUST-ARCHITECTURE §13).
#
# BUILD CONTEXT IS THE PARENT of this repo (`/home/devuser/workspace`), NOT the
# loom repo, because the facade path-depends on a SIBLING workspace:
#     loom/Cargo.toml → ruvector-core = { path = "../ruvector/crates/ruvector-core" }
# A COPY cannot escape its context, so the context must contain BOTH `loom/` and
# `ruvector/` at sibling paths, and the in-image layout preserves `../ruvector`.
#
#   docker build -f loom/deploy/Dockerfile -t loom:rust /home/devuser/workspace
#     (BuildKit honours loom/deploy/Dockerfile.dockerignore, which allowlists
#      only loom/ + ruvector/crates/ruvector-core out of the huge parent tree.)
#   just docker-build            # wraps exactly the above
#
# ---- musl decision (§13 aspiration vs reality) ------------------------------
# §13 aspires to ONE static x86_64-unknown-linux-musl binary on FROM scratch /
# distroless-static. We DEVIATE to a glibc build + debian:bookworm-slim runtime,
# forced by TWO independent facts:
#   1. `oxigraph 0.4` pulls `oxrocksdb-sys` — RocksDB, a C++ library built via
#      cc/bindgen (also `simsimd` C SIMD via ruvector-core). A static musl build
#      of RocksDB needs a full musl C++ cross-toolchain + static libstdc++; that
#      is fragile and out of scope for a serving image.
#   2. The rvdb ro-mount hazard mitigation (see entrypoint.sh) needs /bin/sh at
#      runtime. distroless/static and distroless/cc have NO shell, so we could
#      not run the copy+verify entrypoint there anyway.
# debian:bookworm-slim gives glibc + libstdc++ + a shell + coreutils(cp,sha256sum)
# for a few extra MB — the correct, working choice over the ideologically-static
# one. Non-root, minimal, single binary; just not FROM scratch.
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1 — builder (glibc). Pinned to 1.97 to MATCH the known-green host
# toolchain. NB: the workspace declares rust-version = 1.88 as a MINIMUM, but
# ruvector-core's `simd` feature uses AVX-512 intrinsics (_mm512_* in
# simd_intrinsics.rs) that only STABILISED in Rust 1.89 — 1.88 fails with E0658
# (stdarch_x86_avx512 / avx512f unstable). 1.97 is the host's green stable.
# -----------------------------------------------------------------------------
FROM rust:1.97-bookworm AS builder

# RocksDB (oxrocksdb-sys) + simsimd/bindgen need a C/C++ toolchain, clang for
# bindgen, and cmake. ca-certificates for crates.io TLS during `cargo fetch`.
RUN apt-get update && apt-get install -y --no-install-recommends \
        clang libclang-dev cmake pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The image already ships 1.88 as default; the repo's rust-toolchain.toml pins
# `channel = "stable"`, which would make rustup fetch *latest* stable at build
# time (a non-deterministic network step). We build with the image's pinned
# 1.88 instead, so the toolchain file is removed after copy (below).
WORKDIR /build

# ---- dependency layer (manifest-first caching) ------------------------------
# cargo-chef was evaluated and DROPPED: the recipe has to span an 8-crate
# intra-workspace path graph PLUS a cross-workspace path dep into ../ruvector
# whose crate inherits from a trimmed synthetic root — chef's skeletoniser is
# brittle across that seam. Plain manifest-first layer caching is transparent,
# deterministic, and fully under our control. We copy every Cargo.toml + the
# lockfile, stub each crate's declared lib/bin target, and build deps only; the
# heavy RocksDB/oxigraph/ruvector-core compile lands in THIS layer and is reused
# until a manifest or the lockfile changes.
COPY loom/Cargo.toml loom/Cargo.lock /build/loom/
COPY loom/crates/loom-domain/Cargo.toml                 /build/loom/crates/loom-domain/
COPY loom/crates/loom-scaffold/Cargo.toml               /build/loom/crates/loom-scaffold/
COPY loom/crates/loom-graph-oxigraph/Cargo.toml         /build/loom/crates/loom-graph-oxigraph/
COPY loom/crates/loom-vector-ruvector/Cargo.toml        /build/loom/crates/loom-vector-ruvector/
COPY loom/crates/loom-embed-xinference/Cargo.toml       /build/loom/crates/loom-embed-xinference/
COPY loom/crates/loom-backend-openai/Cargo.toml         /build/loom/crates/loom-backend-openai/
COPY loom/crates/loom-attest-proofgate/Cargo.toml       /build/loom/crates/loom-attest-proofgate/
COPY loom/crates/loom-facade/Cargo.toml                 /build/loom/crates/loom-facade/
# The sibling core crate + our TRIMMED workspace root (see the .toml header for
# why the real 120-member root is not used).
COPY ruvector/crates/ruvector-core/Cargo.toml           /build/ruvector/crates/ruvector-core/
COPY loom/deploy/ruvector-workspace.trimmed.toml        /build/ruvector/Cargo.toml

# Stub the exact lib/bin target each manifest declares, so the dependency graph
# compiles without the real sources. loom-facade declares BOTH a lib and a bin;
# ruvector-core has optional benches/bins gated behind features we don't enable.
RUN set -eux; \
    for c in loom-domain loom-scaffold loom-graph-oxigraph loom-vector-ruvector \
             loom-embed-xinference loom-backend-openai loom-attest-proofgate; do \
        mkdir -p "/build/loom/crates/$c/src"; \
        echo '' > "/build/loom/crates/$c/src/lib.rs"; \
    done; \
    mkdir -p /build/loom/crates/loom-facade/src; \
    echo '' > /build/loom/crates/loom-facade/src/lib.rs; \
    echo 'fn main() {}' > /build/loom/crates/loom-facade/src/main.rs; \
    mkdir -p /build/ruvector/crates/ruvector-core/src; \
    echo '' > /build/ruvector/crates/ruvector-core/src/lib.rs

# Build ONLY the serving binary's dependency closure (default features: no
# pg-write / attest / semantic-fallback), locked to the committed lockfile.
WORKDIR /build/loom
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --locked --bin loom-facade || true

# ---- real source layer ------------------------------------------------------
# Copy the true sources over the stubs. Overwrite the stub root with the trimmed
# root AGAIN (the crates/* COPY below would otherwise not touch it) and drop the
# non-deterministic toolchain file. `touch` forces cargo to recompile the crates
# whose stubbed sources just changed.
COPY loom/ /build/loom/
COPY ruvector/crates/ruvector-core/ /build/ruvector/crates/ruvector-core/
COPY loom/deploy/ruvector-workspace.trimmed.toml /build/ruvector/Cargo.toml
RUN rm -f /build/loom/rust-toolchain.toml

WORKDIR /build/loom
# The release profile already strips symbols (strip = "symbols"); we DO NOT run
# the binary here — main.rs ignores argv and binds the port, so any invocation
# would block the build. The real start/serve/curl self-test is stage 3 (smoke).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --locked --bin loom-facade \
    && install -m 0755 /build/loom/target/release/loom-facade /usr/local/bin/loom-facade

# -----------------------------------------------------------------------------
# Stage 2 — runtime (debian-slim, glibc, HAS a shell for the entrypoint).
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# libstdc++ + libgcc are pulled by RocksDB at runtime; ca-certificates for the
# backend/embedder TLS; curl for a clean HEALTHCHECK (a couple of MB, and the
# entrypoint already needs a shell so there is no static-image purity to keep).
# coreutils (cp, sha256sum) ships in the base and powers the rvdb copy+verify.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libstdc++6 curl \
    && rm -rf /var/lib/apt/lists/*

# Non-root serving user. UID/GID 65532 mirrors the distroless `nonroot` id so a
# ro bind-mount of ./data (owned by the host user) stays readable and /run/loom
# (created below, owned by this user) stays writable for the rvdb copy.
RUN groupadd --gid 65532 nonroot \
    && useradd --uid 65532 --gid 65532 --home-dir /home/nonroot --create-home nonroot \
    && mkdir -p /run/loom /app/data \
    && chown -R nonroot:nonroot /run/loom

COPY --from=builder /usr/local/bin/loom-facade /usr/local/bin/loom-facade
COPY loom/deploy/entrypoint.sh /usr/local/bin/loom-entrypoint.sh
RUN chmod +x /usr/local/bin/loom-entrypoint.sh /usr/local/bin/loom-facade

USER nonroot
WORKDIR /app

# §10 defaults baked so a bare `docker run` (no compose) is already sane. The rvdb
# hazard: LOOM_HNSW_ARTIFACT is REPOINTED to the writable copy by the entrypoint
# (see entrypoint.sh); the value here is only the fallback if the entrypoint is
# bypassed. Everything else reads the ro mount directly (JSON, immutable on read).
ENV LOOM_FACADE_PORT=8080 \
    LOOM_DEPLOY_PROFILE=a \
    ONTOLOGY_INDEX=/app/data/scaffold-index.json \
    ONTOLOGY_PROSE_INDEX=/app/data/prose-index.json \
    LOOM_SEMANTIC_FALLBACK=0 \
    LOOM_HNSW_ARTIFACT=/app/data/ontology-corpus.rvdb \
    LOOM_RUNTIME_DIR=/run/loom \
    RUST_LOG=info

EXPOSE 8080
# curl-based liveness on the facade's own /health. --start-period covers index load.
HEALTHCHECK --interval=30s --timeout=5s --start-period=25s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${LOOM_FACADE_PORT:-8080}/health" || exit 1

ENTRYPOINT ["/usr/local/bin/loom-entrypoint.sh"]

# -----------------------------------------------------------------------------
# Stage 3 — smoke (build-time self-test, §14 gate 8 spirit).
# Bakes the REAL generation into the image and starts the binary to assert
# /health returns index_classes == 8146 (the 8146-class scaffold index) WITHOUT
# any bind mount — because bind-mount paths resolve on the HOST filesystem from
# inside this container, so a `docker run -v` smoke would silently mount the
# wrong tree. A build-time COPY + in-RUN curl is the only trustworthy smoke here.
#   docker build -f loom/deploy/Dockerfile --target smoke -t loom:smoke /home/devuser/workspace
# -----------------------------------------------------------------------------
FROM runtime AS smoke
USER root
# The full mirrored generation: scaffold+prose indices and the ontology TTLs live
# in loom/app/data; the HNSW rvdb + its sidecar live in loom/data (stage-3 output).
COPY loom/app/data/scaffold-index.json   /app/data/scaffold-index.json
COPY loom/app/data/prose-index.json      /app/data/prose-index.json
COPY loom/app/data/ontology.ttl          /app/data/ontology.ttl
COPY loom/app/data/ontology-inferred.ttl /app/data/ontology-inferred.ttl
COPY loom/data/ontology-corpus.rvdb                 /app/data/ontology-corpus.rvdb
COPY loom/data/ontology-corpus.rvdb.generation.json /app/data/ontology-corpus.rvdb.generation.json
RUN chown -R nonroot:nonroot /app/data
USER nonroot
# Retrieval-only smoke: no DISTILL_BACKEND_URL, so /health reports backend:null but
# still serves the lexical floor. Start, poll /health, assert the class count, stop.
RUN set -eux; \
    /usr/local/bin/loom-entrypoint.sh & \
    pid=$!; \
    ok=0; \
    for i in $(seq 1 30); do \
        if curl -fsS "http://127.0.0.1:8080/health" -o /tmp/health.json 2>/dev/null; then ok=1; break; fi; \
        sleep 1; \
    done; \
    echo "=== /health ==="; cat /tmp/health.json; echo; \
    test "$ok" = "1"; \
    grep -q '"index_classes":8146' /tmp/health.json; \
    kill "$pid" 2>/dev/null || true; \
    echo "SMOKE PASS: index_classes=8146"
