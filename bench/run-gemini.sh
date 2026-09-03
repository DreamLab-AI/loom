#!/usr/bin/env bash
# FROZEN (2026-09-03): the harness this drives, bench/bench_ontology_uplift.py, was
# DELETED (broken import of the retired app/ontology_scaffold.py). This script is kept
# as the paper's cited reproduction driver (docs/research/latex/report.tex) and as the
# record of the exact configuration behind uplift-results/. It CANNOT run as-is; see
# bench/LEGACY-PYTHON-NOTE.md for the checkout recipe that restores the harness.
# Drive the ontology-uplift benchmark against Gemini 3.7 Flash (cloud, OpenAI-compat).
# Config locked from live smoke test 2026-08-16:
#   - max_tokens 2048 : mandatory thinking would truncate answers at 400 (finish=length)
#   - reasoning_effort low : minimise thinking overhead / cost
#   - temp 1.0 : Google-recommended for Gemini 3.x (temp<1.0 is off-label; may loop/degrade)
#   - auth via GOOGLE_API_KEY env (Bearer), passed by NAME so the secret stays out of argv
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export PYTHONPATH=app
B=bench/bench_ontology_uplift.py
IDX=app/data/scaffold-index.json
Q=uplift-results/questions.jsonl
BASE=https://generativelanguage.googleapis.com/v1beta/openai/
MODEL=gemini-3.7-flash
COMMON="--base-url $BASE --model-name $MODEL --auth-bearer-env GOOGLE_API_KEY \
  --reasoning-effort low --temp 1.0 --max-tokens 2048 --timeout 120 --retries 3 --sleep 0.4"

echo "[$(date -u +%H:%M:%S)] RAW run ..."
python3 $B run --questions "$Q" --mode raw --outdir uplift-results $COMMON

echo "[$(date -u +%H:%M:%S)] SCAFFOLD run ..."
python3 $B run --questions "$Q" --mode scaffold --index "$IDX" --outdir uplift-results $COMMON

echo "[$(date -u +%H:%M:%S)] SCORE ..."
for f in uplift-results/results-gemini-3.7-flash-raw.jsonl uplift-results/results-gemini-3.7-flash-scaffold.jsonl; do
  python3 $B score --questions "$Q" --results "$f" --outdir uplift-results
done

echo "[$(date -u +%H:%M:%S)] REPORT ..."
python3 $B report \
  --scores "gemini-3.7-flash/raw=uplift-results/scores-gemini-3.7-flash-raw.jsonl" \
  --scores "gemini-3.7-flash/scaffold=uplift-results/scores-gemini-3.7-flash-scaffold.jsonl" \
  --out uplift-results/report-gemini-3.7-flash.md

echo "[$(date -u +%H:%M:%S)] DONE"
