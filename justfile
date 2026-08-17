# Ontology Loom — task runner (RUST-ARCHITECTURE §14 CI gates + §13 deploy).
# `just` runs recipes from this file's directory (the loom repo root).
#
#   just            # list recipes
#   just ci         # the full green bar (§14 gates 1-4)
#   just docker-build && just docker-run-a

set shell := ["bash", "-uc"]

# The deploy image builds from the PARENT context so the sibling ../ruvector path
# dep is reachable (a COPY cannot escape its context). See deploy/Dockerfile.
image      := "loom:rust"
context    := justfile_directory() + "/.."
dockerfile := "deploy/Dockerfile"

# Default: show the recipe list.
default:
    @just --list

# --- §14 gate 1 — compiles on BOTH feature planes ---------------------------
# all-features proves pg-write/attest/semantic-fallback still build; no-default
# proves the SERVING binary compiles without them (the boundary rule, §10).
build:
    cargo build --workspace --all-features --locked
    cargo build --workspace --no-default-features --locked

# --- §14 gate 2 — the full test suite (byte-golden scaffold, SPARQL clamp, router)
test:
    cargo test --workspace --all-features --locked

# --- §14 gate 3 — clippy, pedantic, warnings are errors ----------------------
clippy:
    cargo clippy --all-targets --all-features --locked -- -D warnings

# --- §14 gate 4 — licence + advisory gate (deny.toml) ------------------------
deny:
    cargo deny check

# --- §14 gate 5 — Criterion perf gate: match() p99 < 50ms on the 8k index ----
bench:
    cargo bench --workspace

# --- §14 gates 1-4 chained — what "green" means before a commit --------------
ci: build test clippy deny
    @echo "CI GREEN — build(both planes) + test + clippy + deny"

# --- §13 deploy: build the image from the parent context ---------------------
# Context = {{context}} (parent of the repo) so `../ruvector` resolves; BuildKit
# picks up deploy/Dockerfile.dockerignore automatically.
docker-build:
    DOCKER_BUILDKIT=1 docker build -f {{dockerfile}} -t {{image}} {{context}}

# Build-time smoke: bakes the real generation and asserts /health index_classes=8146
# WITHOUT a bind mount (bind paths resolve on the HOST from in here — a run -v smoke
# would mount the wrong tree). This is the trustworthy self-test.
docker-smoke:
    DOCKER_BUILDKIT=1 docker build -f {{dockerfile}} --target smoke -t {{image}}-smoke {{context}}

# --- §13 Profile A — host-colocated on HP (network_mode: host, :8084) ---------
# NOTE: run this ON THE DEPLOY HOST. Bind-mount paths resolve on the HOST
# filesystem, so launching from inside the agentbox would mount the wrong ./data.
docker-run-a:
    docker compose -f deploy/compose.profile-a.yml up -d
    @echo "Profile A up — curl http://127.0.0.1:8084/health"

# --- §13 Profile B — sidecar on visionclaw_network (:8080) --------------------
docker-run-b:
    docker compose -f deploy/compose.profile-b.yml up -d
    @echo "Profile B up — curl http://127.0.0.1:8080/health"

# Tear down either profile.
docker-down-a:
    docker compose -f deploy/compose.profile-a.yml down
docker-down-b:
    docker compose -f deploy/compose.profile-b.yml down
