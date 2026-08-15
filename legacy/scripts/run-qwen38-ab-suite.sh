#!/usr/bin/env bash
# Qwen3.8-27B A/B suite vs the recorded Muse baselines (BENCHMARKS.md 2026-08-11).
# Detached-safe: launch with `setsid nohup ./scripts/run-qwen38-ab-suite.sh &` so it
# survives SSH/session death. Throughput probe already done (logs/throughput-qwen3.8.txt).
# Writes logs/qwen38-suite-status.txt as a progress marker, logs/QWEN38-SUITE-DONE on finish.
set -u
cd "$(dirname "$0")/.."
mark() { echo "$(date +%F\ %T)  $*" >> logs/qwen38-suite-status.txt; }

mark "suite start (bullshit restarted from scratch after tmux migration)"
python3 scripts/bench-bullshit.py 8085 qwen3.8-27B > logs/bullshit-qwen3.8-run.log 2>&1
mark "bullshit done rc=$?"
( cd ontology && python3 bench-uplift.py 8085 qwen3.8-27B > ../logs/uplift-qwen3.8-run.log 2>&1 )
mark "uplift done rc=$?"
( cd ontology && python3 bench-agentic-uplift.py 8085 qwen3.8-27B > ../logs/agentic-uplift-qwen3.8-run.log 2>&1 )
mark "agentic-uplift done rc=$?"
python3 scripts/bench-headhead.py 8085 qwen3.8-27B > logs/headhead-qwen3.8-run.log 2>&1
mark "headhead done rc=$?"
python3 scripts/bench-headhead-hard.py 8085 qwen3.8-27B > logs/headhead-hard-qwen3.8-run.log 2>&1
mark "headhead-hard done rc=$?"
touch logs/QWEN38-SUITE-DONE
mark "ALL DONE"
