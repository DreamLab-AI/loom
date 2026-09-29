#!/usr/bin/env bash
# Judge + analyse + render the five-arm rerun. Idempotent and resumable:
# judging skips (set,id,arm) triples already in judged.json.
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=uplift-results/control-rerun-2026-09-21
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY must be set (OPENAI_API_KEY 401s here)}"

python3 tools/paper/judge_openrouter.py \
    --rows "$OUT/rows.jsonl" uplift-results/paper-v2/live-results.jsonl \
    --sets arcane,thin --model openai/gpt-4.1 --concurrency 6 \
    --out "$OUT/judged.json"

python3 tools/paper/analyze_controls_v2.py \
    --judged "$OUT/judged.json" \
    --rows "$OUT/rows.jsonl" uplift-results/paper-v2/live-results.jsonl \
    --label "rerun-4096-8192-5arm-2026-09-21" \
    --out-json "$OUT/analysis.json"

python3 tools/paper/render_controls_md.py \
    --analysis "$OUT/analysis.json" \
    --preamble "$OUT/preamble.md" \
    --title "Negative controls — five-arm rerun under the common retry policy, 2026-09-21" \
    --out "$OUT/ANALYSIS.md"

python3 tools/paper/exposure_summary.py --rows "$OUT/rows.jsonl" \
    --out "$OUT/exposure.json" --md "$OUT/EXPOSURE.md"
