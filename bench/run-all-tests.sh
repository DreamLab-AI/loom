#!/usr/bin/env bash
# Run the complete ontology-toolkit test system (the same suites used to build it).
# Suites: scaffold selftest · proxy integration tests · uplift-bench selftest ·
#         MCP server tests · pipeline unit suite (reasoner, conflicts, graph tiers).
# Exit nonzero if any suite fails.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

declare -A RESULT
fail=0

run() {
  local name="$1"; shift
  if "$@" >/tmp/ontology-test-"$name".log 2>&1; then
    RESULT[$name]="PASS"
  else
    RESULT[$name]="FAIL (see /tmp/ontology-test-$name.log)"
    fail=1
  fi
}

run scaffold    python3 ontology_scaffold.py --selftest
run proxy       python3 test_proxy.py
run bench       python3 bench_ontology_uplift.py --selftest
run confidence  python3 ../tests/test_confidence_injection.py
if [[ -d ontology-mcp/node_modules ]]; then
  run mcp     bash -c 'cd ontology-mcp && node test.js'
else
  RESULT[mcp]="SKIP (run: cd ontology-mcp && npm install)"
fi
if [[ -x .venv/bin/python ]]; then
  run pipeline ./.venv/bin/python -m pytest pipeline/tests -q
else
  RESULT[pipeline]="SKIP (run: python3 -m venv .venv && ./.venv/bin/pip install 'rdflib>=7' pytest)"
fi

echo "── ontology toolkit test system ──"
for s in scaffold proxy bench confidence mcp pipeline; do
  printf "  %-9s %s\n" "$s" "${RESULT[$s]}"
done
exit $fail
