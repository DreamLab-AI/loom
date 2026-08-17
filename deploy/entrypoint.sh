#!/bin/sh
# =============================================================================
# loom-facade entrypoint — the rvdb read-only-mount HAZARD mitigation.
#
# HAZARD (verified stage 3): opening data/ontology-corpus.rvdb via ruvector-core
# VectorDB MUTATES the redb file EVEN FOR READS — VectorDB::new(DbOptions{..})
# rebuilds/repacks the HNSW index on open and writes back to the redb store. Both
# compose profiles mount the generation read-only (`./data:/app/data:ro`), so a
# direct open of the mounted artifact would either fail (EROFS) or, worse, be
# attempted against an immutable file and abort the semantic index. There is NO
# code change permitted, so we solve it at the CONTAINER level:
#
#   1. COPY the .rvdb (+ its .generation.json sidecar) from the RO mount into a
#      writable tmpfs-friendly dir (/run/loom), which redb may freely mutate.
#   2. (belt-and-braces) If the sidecar records a sha256, VERIFY the copy against
#      it BEFORE the binary first opens the DB — catching a truncated/torn mirror
#      before redb rewrites it. (The current sidecar carries classCount/generatedAt
#      only, no sha256, so this step logs "no sha recorded" and proceeds — it
#      activates automatically the day the mirror starts stamping a digest.)
#   3. REPOINT LOOM_HNSW_ARTIFACT at the writable copy, then exec the binary.
#
# Everything ELSE in /app/data (scaffold-index.json, prose-index.json, the TTLs)
# is plain JSON/turtle read immutably — it stays on the RO mount, uncopied.
#
# Fail-open throughout: no rvdb present (retrieval-only node, or the semantic
# artifact simply not shipped) ⇒ skip the copy and serve the lexical floor.
# =============================================================================
set -eu

RUNTIME_DIR="${LOOM_RUNTIME_DIR:-/run/loom}"
SRC="${LOOM_HNSW_ARTIFACT:-/app/data/ontology-corpus.rvdb}"

log() { printf '%s loom-entrypoint: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }

mirror_rvdb() {
    if [ ! -f "$SRC" ]; then
        log "no HNSW artifact at $SRC — semantic fallback unavailable, serving lexical floor (fail-open)"
        return 0
    fi

    mkdir -p "$RUNTIME_DIR"
    dest="$RUNTIME_DIR/$(basename "$SRC")"
    sidecar="${SRC}.generation.json"

    log "copying rvdb from RO mount → writable $dest (redb mutates on open — must not touch the :ro mount)"
    cp -f "$SRC" "$dest"
    if [ -f "$sidecar" ]; then
        cp -f "$sidecar" "${dest}.generation.json"
    fi

    # Belt-and-braces: verify the COPY against a sha256 in the sidecar, if present.
    # We check the sidecar the standard `sha256`/`sha256sum` keys would use.
    recorded=""
    if [ -f "$sidecar" ]; then
        recorded="$(grep -oE '"sha256[a-z]*"[[:space:]]*:[[:space:]]*"[0-9a-fA-F]{64}"' "$sidecar" 2>/dev/null \
                    | grep -oE '[0-9a-fA-F]{64}' | head -n1 || true)"
    fi
    if [ -n "$recorded" ]; then
        actual="$(sha256sum "$dest" | awk '{print $1}')"
        if [ "$actual" != "$recorded" ]; then
            log "FATAL: rvdb sha256 mismatch — sidecar=$recorded actual=$actual (torn/corrupt mirror); refusing to open"
            exit 1
        fi
        log "rvdb sha256 verified against sidecar ($actual)"
    else
        log "sidecar records no sha256 — skipping digest verify (copy still isolates the RO mount)"
    fi

    export LOOM_HNSW_ARTIFACT="$dest"
    log "LOOM_HNSW_ARTIFACT repointed → $dest"
}

mirror_rvdb

log "exec loom-facade (profile=${LOOM_DEPLOY_PROFILE:-a} port=${LOOM_FACADE_PORT:-8080})"
exec /usr/local/bin/loom-facade "$@"
