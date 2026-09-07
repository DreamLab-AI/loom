#!/usr/bin/env bash
# Emit the loom facade /health payload on stdout, and nothing else.
#
# The facade base URL is parameterised so a dream-cycle runner that is not on
# the HP host can reach it: LOOM_URL=http://192.168.2.132:8084. The default is
# byte-identical to the previously hard-coded value, so an HP-local run is
# unchanged. This matches crates/loom-facade/tests/contract_live.rs, which has
# read LOOM_URL with the same default since it was written.
#
# Exit status is curl's, so callers can distinguish "facade down" from
# "facade answered". Nothing is printed on failure — a caller that wants a
# human marker prints its own.
set -euo pipefail

LOOM_URL="${LOOM_URL:-http://127.0.0.1:8084}"

exec curl -sS --max-time "${LOOM_HEALTH_TIMEOUT:-10}" "${LOOM_URL}/health"
