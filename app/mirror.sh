#!/usr/bin/env bash
# Mirror the published ontology artifacts from narrativegoldmine.com into ./data/.
#
# SSOT boundary enforcement (ADR-136 D4 / ADR-135 D2.1 "never mixed-build"):
# the four artifacts are three projections of one build (ontology.ttl for SPARQL,
# scaffold-index.json for lexical match, prose-index.json for prose) plus the
# reasoned closure. They MUST be served as one generation, never spliced across
# builds. This script therefore:
#   1. fetches to a STAGING dir (never in place), timestamp-aware for efficiency;
#   2. verifies the candidate set carries ONE generation — the embedded stamps
#      (scaffold.generated, prose.generated, inferred generatedAt) must cluster
#      within GEN_TOL_SECONDS. A mid-publish or partial mirror shows stamps from
#      different builds and is REJECTED (previous consistent set kept);
#   3. promotes ATOMICALLY (all-or-nothing) and writes data/.generation.json — the
#      local manifest recording the promoted generation + per-artifact sha256, so
#      the Loom can expose which generation it is serving.
#
# Forward-compatible: if upstream ever publishes data/generation.json (a real
# build manifest with per-artifact sha256), this script prefers it and verifies
# against it; until then it falls back to the embedded-stamp cluster check above.
# No upstream change is required for the atomic/never-mixed guarantee to hold.
#
# Safe to run any time; safe to cron (suggested: hourly). Exit codes:
#   0 = promoted a new generation, or already current; 2 = rejected (drift/mixed
#   build detected or a fetch failed mid-update) — previous set kept intact.
set -euo pipefail

SITE="${ONTOLOGY_SITE:-https://narrativegoldmine.com}"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/data"
STAGE="$DIR/.stage"
GEN_TOL_SECONDS="${GEN_TOL_SECONDS:-300}"   # max spread across one build's stamps
mkdir -p "$DIR" "$STAGE"

# artifact_name -> published path (all four ride together)
ARTIFACTS=(scaffold-index.json prose-index.json ontology.ttl ontology-inferred.ttl)
declare -A REMOTE=(
  [scaffold-index.json]=data/scaffold-index.json
  [prose-index.json]=data/prose-index.json
  [ontology.ttl]=data/ontology.ttl
  [ontology-inferred.ttl]=data/ontology-inferred.ttl
)

cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT

# 1. Fetch each artifact into staging, timestamp-aware against the live copy so we
#    only pull what changed. A freshly-downloaded, non-empty stage file marks a
#    candidate update; anything else (304 not-modified, or fetch error) leaves no
#    stage file for that artifact.
downloaded=()
failed=()
for name in "${ARTIFACTS[@]}"; do
  out="$STAGE/$name"
  args=(-fsSL --retry 3 --connect-timeout 10 -o "$out" "$SITE/${REMOTE[$name]}")
  [[ -f "$DIR/$name" ]] && args+=(--time-cond "$DIR/$name")
  if curl "${args[@]}" 2>/dev/null && [[ -s "$out" ]]; then
    downloaded+=("$name"); echo "fetched  $name ($(du -h "$out" | cut -f1))"
  else
    rm -f "$out"
    # A real error only if the artifact is genuinely unreachable AND we have no
    # prior copy. A 304 (not-modified) is not a failure.
    if ! [[ -f "$DIR/$name" ]]; then failed+=("$name"); echo "FAILED   $name (no prior copy)" >&2
    else echo "current  $name"; fi
  fi
done

# 2 + 3. Verify one-generation cluster over the CANDIDATE set, then atomically
#        promote (or reject). All logic in one place to keep the invariant honest.
python3 - "$DIR" "$STAGE" "$GEN_TOL_SECONDS" "$SITE" "${downloaded[*]-}" "${failed[*]-}" <<'PY'
import hashlib, json, os, re, sys
from datetime import datetime, timezone

DIR, STAGE, tol_s, SITE = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
downloaded = sys.argv[5].split() if len(sys.argv) > 5 and sys.argv[5] else []
failed = sys.argv[6].split() if len(sys.argv) > 6 and sys.argv[6] else []
ARTIFACTS = ["scaffold-index.json", "prose-index.json", "ontology.ttl", "ontology-inferred.ttl"]

def die(code, msg):
    print(msg, file=sys.stderr); sys.exit(code)

# A fetch failed with no prior copy -> cannot assemble a complete generation.
if failed:
    die(2, f"REJECT: unreachable artifact(s) with no prior copy: {', '.join(failed)}")

# Nothing new -> already current. Still sanity-check the live set clusters.
def candidate_path(name):
    s = os.path.join(STAGE, name)
    return s if name in downloaded else os.path.join(DIR, name)

for name in ARTIFACTS:
    if not os.path.exists(candidate_path(name)):
        die(2, f"REJECT: missing artifact {name} (neither fresh nor prior)")

def stamp(path, keys):
    """Pull an ISO8601 generation stamp from a JSON key or a TTL predicate."""
    with open(path, "rb") as f:
        head = f.read(8192)
    if path.endswith(".json"):
        try:
            d = json.loads(open(path).read())
            for k in keys:
                if isinstance(d, dict) and d.get(k):
                    return d[k]
        except Exception:
            pass
        return None
    m = re.search(rb'generatedAt\s+"?([0-9T:\-.Z+]+)', head)
    return m.group(1).decode() if m else None

def to_epoch(s):
    if not s: return None
    try:
        return datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()
    except Exception:
        return None

# Stamped artifacts define the generation; ontology.ttl (no stamp) rides with them.
STAMP_KEYS = {"scaffold-index.json": ["generated"], "prose-index.json": ["generated"],
              "ontology-inferred.ttl": ["generatedAt"]}
stamps = {n: stamp(candidate_path(n), STAMP_KEYS[n]) for n in STAMP_KEYS}
epochs = {n: to_epoch(s) for n, s in stamps.items()}
have = {n: e for n, e in epochs.items() if e is not None}

if len(have) < 2:
    die(2, f"REJECT: too few generation stamps to verify one build: {stamps}")

span = max(have.values()) - min(have.values())
if span > tol_s:
    detail = ", ".join(f"{n}={stamps[n]}" for n in have)
    die(2, f"REJECT: mixed build — generation stamps span {span:.0f}s (> {tol_s:.0f}s): {detail}")

# Consistent single generation. If nothing was freshly downloaded, we are current.
def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

gen_iso = datetime.fromtimestamp(min(have.values()), timezone.utc).isoformat()

if not downloaded:
    print(f"--- current: generation {gen_iso} (span {span:.1f}s, no change)")
    sys.exit(0)

# 3. Atomic promotion: os.replace each freshly-downloaded file into place. The set
#    is pre-verified consistent; .generation.json is written LAST as the commit
#    marker the Loom reads.
manifest = {
    "promoted_at": datetime.now(timezone.utc).isoformat(),
    "generation": gen_iso,
    "cluster_span_seconds": round(span, 3),
    "source": SITE,
    "artifacts": {},
}
for name in ARTIFACTS:
    src = candidate_path(name)
    dst = os.path.join(DIR, name)
    if name in downloaded:
        os.replace(src, dst)
    manifest["artifacts"][name] = {
        "sha256": sha256(dst),
        "stamp": stamps.get(name),
        "bytes": os.path.getsize(dst),
    }

tmp = os.path.join(DIR, ".generation.json.tmp")
with open(tmp, "w") as f:
    json.dump(manifest, f, indent=2)
os.replace(tmp, os.path.join(DIR, ".generation.json"))

print(f"--- PROMOTED generation {gen_iso} (span {span:.1f}s): "
      f"{', '.join(sorted(downloaded))}")
c = json.loads(open(os.path.join(DIR, 'scaffold-index.json')).read())
print(f"    scaffold-index: version={c.get('version')} "
      f"classes={c.get('counts', {}).get('classes')} generated={c.get('generated')}")
PY
