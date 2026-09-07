#!/usr/bin/env bash
# Run one loom-facade evaluator binary against the live facade /health payload.
#
#   scripts/dream-eval.sh <bin-name>        # graph-check | confidence-check | generation-check
#
# Why a script rather than an inline dream.config.json entrypoint: on 2026-08-28
# and again on 2026-09-06 inline commands lost their quoting crossing the annexe
# `ssh bash -lc` boundary (`unexpected EOF while looking for matching '`), which
# killed three evaluators and the build step outright. A checked-in script is
# invoked quote-free, so there is nothing left to mis-parse.
#
# Exit-status policy, deliberately two-tier:
#   * facade unreachable  -> print SKIP, exit 0. Nightly evaluators degrade
#     rather than fail red for being offline (the policy stated in
#     crates/loom-facade/tests/contract_live.rs).
#   * evaluator non-zero  -> print the historical "<SLOT> EVALUATOR FAILED"
#     marker AND propagate the status. The old `|| echo '... FAILED'` tail
#     swallowed the status, so a red evaluator was recorded as PASSED; on
#     2026-09-06 that laundering turned environment rot into green receipts.
#     False-green is worse than red.
set -uo pipefail

bin="${1:?usage: dream-eval.sh <graph-check|confidence-check|generation-check>}"
here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd -- "${here}/.." && pwd)"
LOOM_URL="${LOOM_URL:-http://127.0.0.1:8084}"

health="$(mktemp)"
trap 'rm -f "${health}"' EXIT

if ! bash "${here}/dream-health.sh" >"${health}" 2>/dev/null; then
  echo "[${bin}] SKIP - no facade at ${LOOM_URL}"
  exit 0
fi

cd -- "${repo}" || exit 1
cargo run -q -p loom-facade --bin "${bin}" <"${health}"
status=$?

if [ "${status}" -ne 0 ]; then
  label="$(printf '%s' "${bin%-check}" | tr '[:lower:]' '[:upper:]')"
  echo "${label} EVALUATOR FAILED (exit ${status})"
  exit "${status}"
fi
