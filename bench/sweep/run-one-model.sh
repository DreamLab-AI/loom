#!/usr/bin/env bash
# Run raw+scaffold+score for ONE model through the instrumented uplift harness.
# Uniform config for the cross-model sweep: temp=0 (deterministic/reproducible),
# max_tokens=2048 (headroom for any thinking), seed-42 510-question set.
# Args: <label> <base_url> <model_id> <key_env_var> [reasoning_effort]
set -uo pipefail
LABEL="$1"; BASE="$2"; MODEL="$3"; KEYENV="$4"; EFFORT="${5:-}"
cd /home/devuser/workspace/loom
export PYTHONPATH=app
B=bench/bench_ontology_uplift.py
IDX=app/data/scaffold-index.json
Q=uplift-results/questions.jsonl
OUT=uplift-results/sweep
mkdir -p "$OUT"
COMMON="--base-url $BASE --model-name $MODEL --auth-bearer-env $KEYENV \
  --temp 0 --max-tokens 2048 --timeout 120 --retries 3 --sleep 0.3 --outdir $OUT"
[ -n "$EFFORT" ] && COMMON="$COMMON --reasoning-effort $EFFORT"

echo "[$(date -u +%H:%M:%S)] $LABEL RAW ..."
python3 $B run --questions "$Q" --mode raw $COMMON \
  --out "$OUT/results-$LABEL-raw.jsonl"
echo "[$(date -u +%H:%M:%S)] $LABEL SCAFFOLD ..."
python3 $B run --questions "$Q" --mode scaffold --index "$IDX" $COMMON \
  --out "$OUT/results-$LABEL-scaffold.jsonl"
for m in raw scaffold; do
  python3 $B score --questions "$Q" --results "$OUT/results-$LABEL-$m.jsonl" \
    --out "$OUT/scores-$LABEL-$m.jsonl"
done
echo "[$(date -u +%H:%M:%S)] $LABEL DONE"
