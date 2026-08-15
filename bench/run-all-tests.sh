#!/usr/bin/env bash
# Run the complete ontology-toolkit test system (the same suites used to build it).
# Suites: scaffold selftest · proxy integration tests · uplift-bench selftest ·
#         confidence-injection unit test · MCP server tests · pipeline unit suite.
# This copy is wired for the Loom repo layout (code in ../app, tests in ../tests).
# All suites are vendored here (llm-server retired 2026-08-15); pipeline needs the
# repo-root .venv (python3 -m venv .venv && .venv/bin/pip install 'rdflib>=7' pytest).
# Exit nonzero if any suite fails.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
APP=../app

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

run scaffold    python3 "$APP"/ontology_scaffold.py --selftest
if [[ -f "$APP"/test_proxy.py ]]; then
  run proxy     python3 "$APP"/test_proxy.py
else
  RESULT[proxy]="SKIP (app/test_proxy.py missing)"
fi
run bench       env PYTHONPATH="$APP" python3 bench_ontology_uplift.py --selftest
run confidence  python3 ../tests/test_confidence_injection.py
if [[ -d "$APP"/ontology-mcp/node_modules ]]; then
  run mcp     bash -c "cd '$APP'/ontology-mcp && node test.js"
else
  RESULT[mcp]="SKIP (run: cd app/ontology-mcp && npm install)"
fi
if [[ -x "$APP"/../.venv/bin/python && -d "$APP"/pipeline/tests ]]; then
  run pipeline "$APP"/../.venv/bin/python -m pytest "$APP"/pipeline/tests -q
else
  RESULT[pipeline]="SKIP (run: python3 -m venv .venv && .venv/bin/pip install 'rdflib>=7' pytest)"
fi

echo "── ontology toolkit test system ──"
for s in scaffold proxy bench confidence mcp pipeline; do
  printf "  %-10s %s\n" "$s" "${RESULT[$s]}"
done
exit $fail
