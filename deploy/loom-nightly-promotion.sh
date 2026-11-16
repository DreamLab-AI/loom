#!/usr/bin/env bash
# Nightly promotion of the visionGraph default branch into the Loom (ADR-141 D6).
#
#   fetch visionGraph -> (nothing new? exit 0) -> build with the PINNED ontomark
#   -> promote_vault_build -> promote_generation (validate, install, stamp, log)
#
# It never restarts the service: the reload tick (loom-generation-reload.timer,
# ADR-141 D6) does that once the new generation is installed. Safe to run any
# time and as often as you like:
#   * nothing new on the default branch  -> prints "current", exits 0, builds
#     nothing, writes no log line;
#   * the build happens in a private directory outside DATA_DIR and is renamed
#     to its final name only when complete, so the service never sees a
#     half-built bundle; a failed build leaves the installed generation alone;
#   * promote_generation refuses an invalid or older bundle (non-zero exit).
#
# Configuration (environment; defaults suit the HP checkout layout):
#   LOOM_HOME         ~/githubs/loom          checkout holding target/release binaries and deploy/
#   LOOM_DATA_DIR     $LOOM_HOME/data         the directory the service reads (bind-mounted :ro)
#   LOOM_SOURCE_REPO  ~/githubs/visionGraph   visionGraph checkout (fetched, never modified)
#   LOOM_SOURCE_REF   origin/HEAD             the ref to promote (the remote default branch)
#   LOOM_STATE_DIR    ~/.local/state/loom     bundles, ontomark cache, lock
#   LOOM_KEEP_BUNDLES 5                       built bundles kept for rollback
set -euo pipefail

LOOM_HOME="${LOOM_HOME:-$HOME/githubs/loom}"
DATA_DIR="${LOOM_DATA_DIR:-$LOOM_HOME/data}"
SOURCE_REPO="${LOOM_SOURCE_REPO:-$HOME/githubs/visionGraph}"
SOURCE_REF="${LOOM_SOURCE_REF:-origin/HEAD}"
STATE_DIR="${LOOM_STATE_DIR:-$HOME/.local/state/loom}"
KEEP="${LOOM_KEEP_BUNDLES:-5}"
BIN="$LOOM_HOME/target/release"

log() { printf '%s loom-nightly: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

for tool in git sha256sum flock; do
    command -v "$tool" >/dev/null || die "$tool is required"
done
for exe in promote_generation reload_published_generation promote_vault_build; do
    [[ -x "$BIN/$exe" ]] || die "$BIN/$exe is missing; build it (see docs/published-generation-reload.md)"
done
[[ -d "$SOURCE_REPO/.git" || -f "$SOURCE_REPO/.git" ]] || die "$SOURCE_REPO is not a git checkout"

# The pin. Sourced, never floated: the tag and the digest both come from the file.
# shellcheck source=deploy/ontomark-release.env
source "$LOOM_HOME/deploy/ontomark-release.env"
: "${ONTOMARK_RELEASE_REPO:?}" "${ONTOMARK_RELEASE_TAG:?}" "${ONTOMARK_ASSET:?}" "${ONTOMARK_SHA256:?}"

umask 022
mkdir -p "$STATE_DIR/bundles" "$STATE_DIR/ontomark"
exec 9>"$STATE_DIR/nightly.lock"
flock -n 9 || die "another nightly promotion is running"

# 1. What is on the default branch now?
git -C "$SOURCE_REPO" fetch --quiet --prune origin || die "fetch of $SOURCE_REPO failed"
SHA="$(git -C "$SOURCE_REPO" rev-parse --verify --quiet "$SOURCE_REF^{commit}")" \
    || die "$SOURCE_REF does not resolve in $SOURCE_REPO"
WANT="visionGraph@$SHA"

# 2. Is it already installed (and does the installed bundle still verify)?
if INSTALLED="$("$BIN/reload_published_generation" verify --data-dir "$DATA_DIR" 2>/dev/null)" \
    && [[ "$INSTALLED" == *"\"generation\":\"$WANT\""* ]]; then
    echo "current $WANT"
    exit 0
fi

# 3. The pinned ontomark, checksum-verified, cached by tag.
ONTOMARK="$STATE_DIR/ontomark/$ONTOMARK_RELEASE_TAG/ontomark"
verify_ontomark() {
    [[ -x "$ONTOMARK" ]] \
        && printf '%s  %s\n' "$ONTOMARK_SHA256" "$ONTOMARK" | sha256sum --check --status --strict
}
if ! verify_ontomark; then
    log "installing ontomark $ONTOMARK_RELEASE_TAG"
    mkdir -p "$STATE_DIR/ontomark/$ONTOMARK_RELEASE_TAG"
    DL="$(mktemp -d "$STATE_DIR/ontomark/.download.XXXXXX")"
    if command -v gh >/dev/null; then
        gh release download "$ONTOMARK_RELEASE_TAG" --repo "$ONTOMARK_RELEASE_REPO" \
            --pattern "$ONTOMARK_ASSET" --dir "$DL"
    else
        curl -fsSL -o "$DL/$ONTOMARK_ASSET" \
            "https://github.com/$ONTOMARK_RELEASE_REPO/releases/download/$ONTOMARK_RELEASE_TAG/$ONTOMARK_ASSET"
    fi
    printf '%s  %s\n' "$ONTOMARK_SHA256" "$DL/$ONTOMARK_ASSET" | sha256sum --check --status --strict \
        || { rm -rf "$DL"; die "ontomark download does not match the pinned sha256"; }
    install -m755 "$DL/$ONTOMARK_ASSET" "$ONTOMARK"
    rm -rf "$DL"
    verify_ontomark || die "installed ontomark does not match the pinned sha256"
fi

# 4. Build in a private directory, from a clean detached clone at exactly $SHA.
WORK="$(mktemp -d "$STATE_DIR/bundles/.build-${SHA:0:9}.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
git clone --quiet --shared --no-checkout "$SOURCE_REPO" "$WORK/src"
git -C "$WORK/src" checkout --quiet --detach "$SHA"
log "building $WANT with ontomark $ONTOMARK_RELEASE_TAG"
"$ONTOMARK" --repo "$WORK/src" build --out "$WORK/out" --with-rvdb --stats
[[ -f "$WORK/out/data/.generation.json" ]] || die "the build produced no data/.generation.json"
"$BIN/promote_vault_build" --data "$WORK/out/data"

# Only a complete bundle gets a final name.
FINAL="$STATE_DIR/bundles/$SHA"
rm -rf "$FINAL"
mv "$WORK/out" "$FINAL"

# 5. Validate, install atomically, stamp, log. Non-zero if it refuses.
"$BIN/promote_generation" \
    --bundle "$FINAL/data" \
    --data-dir "$DATA_DIR" \
    --source-repo "$SOURCE_REPO" \
    --log "$DATA_DIR/promotions.jsonl" \
    --promoted-by loom-nightly

# 6. Keep the newest $KEEP built bundles (rollback needs them); never the one just installed.
# shellcheck disable=SC2012
ls -1dt "$STATE_DIR"/bundles/[0-9a-f]* 2>/dev/null | tail -n +"$((KEEP + 1))" | while read -r old; do
    [[ "$old" == "$FINAL" ]] || rm -rf "$old"
done
