#!/bin/bash
set -e

# ── Qwen 3.6 35B-A3B MoE (Abliterix-EGA, Q5_K_M) ──
# Architecture: 256 experts, 8 active + 1 shared (~3B per token), 40 layers
#   30 linear attention (Gated DeltaNet), 10 full attention (GQA)
# Context: 262K | Vision: mmproj | Uncensored: Abliterix-EGA
# Thinking: enabled by default, --reasoning-format deepseek

SERVER=/home/john/githubs/llm-server/llama.cpp/build/bin/llama-server
MODEL=/home/john/githubs/llm-server/models/qwen3.6-35B-A3B-abliterix/APEX-Q5_K_M.gguf
MMPROJ=/home/john/githubs/llm-server/models/qwen3.6-35B-A3B-abliterix/mmproj-qwen36-F16.gguf
LOG=/home/john/githubs/llm-server/logs/qwen36-production.log

mkdir -p /home/john/githubs/llm-server/logs
pkill -9 -f llama-server 2>/dev/null || true
sleep 2

MMPROJ_ARGS=""
if [ -f "$MMPROJ" ]; then
    MMPROJ_ARGS="--mmproj $MMPROJ"
fi

$SERVER \
  --model $MODEL \
  $MMPROJ_ARGS \
  --host 0.0.0.0 --port 8080 \
  --gpu-layers 999 \
  --ctx-size 262144 \
  --split-mode layer \
  --threads 24 \
  --parallel 2 \
  --batch-size 8192 \
  --ubatch-size 4096 \
  --flash-attn on \
  --jinja \
  --reasoning-format deepseek \
  --cache-type-k q8_0 \
  --cache-type-v q8_0 \
  --metrics \
  --no-context-shift \
  --temp 0.7 \
  --top-k 20 \
  --top-p 0.95 \
  --min-p 0 \
  --presence-penalty 1.5 \
  > $LOG 2>&1 &

PID=$!
echo "Server PID: $PID"
echo "Waiting for load..."

for i in $(seq 1 60); do
    if curl -s http://127.0.0.1:8080/health 2>/dev/null | grep -q ok; then
        echo "Ready after ${i}s"
        echo ""
        grep -E "kv_cache:|attn_rot|model loaded|listening" $LOG | tail -10
        echo ""
        echo "API: http://0.0.0.0:8080/v1"
        exit 0
    fi
    sleep 2
done

echo "Still loading... check $LOG"
