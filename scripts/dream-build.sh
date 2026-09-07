#!/usr/bin/env bash
# Dream-cycle build step: capability diagnostics, printed in phase 0.
#
# Diagnostics only — this never fails the night, so every probe result reaches
# the report even when one of them is bad news.
#
# `set -o pipefail` here is NOT the redundant entrypoint-level pipefail (the
# dream engine already runs every entrypoint under `bash -o pipefail -c`). Shell
# options are not inherited across an exec, so this separate process needs its
# own if `npm ci ... | tail -5` is to report npm's status rather than tail's.
set -uo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd -- "${here}/.." && pwd)"
LOOM_URL="${LOOM_URL:-http://127.0.0.1:8084}"

echo
echo '== facade =='
echo "LOOM_URL=${LOOM_URL}"
bash "${here}/dream-health.sh" || echo 'FACADE UNREACHABLE'

echo
echo '== ontology-mcp =='
(cd -- "${repo}/app/ontology-mcp" && npm ci --ignore-scripts 2>&1 | tail -5) || echo 'NPM CI FAILED'

exit 0
