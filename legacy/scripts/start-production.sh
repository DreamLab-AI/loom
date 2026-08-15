#!/bin/bash
set -e

# ── Gemma 4 26B-A4B MoE (ARA Abliterated, APEX-IQ) ──
# Architecture: 128 experts, 8 active (~4B per token), 30 layers
#   25 SWA layers (window=1024, head_dim=256, 8 KV heads)
#   5 global layers (full context, head_dim=512, 2 KV heads)
# Context: 262K | Vision: mmproj (SigLIP) | Uncensored: ARA 2-pass
# Hadamard rotation: enabled for quantized KV (PR #21513)

SERVER=/home/john/githubs/llm-server/llama.cpp/build/bin/llama-server
MODEL=/home/john/githubs/llm-server/models/gemma-4-26B-A4B-ara-abliterated/gemma4-ara-2pass-APEX-IQ.gguf
MMPROJ=/home/john/githubs/llm-server/models/gemma-4-26B-A4B-ara-abliterated/mmproj-gemma4-f16.gguf
LOG=/home/john/githubs/llm-server/logs/production.log

mkdir -p /home/john/githubs/llm-server/logs
pkill -9 -f llama-server 2>/dev/null || true
sleep 2

$SERVER \
  --model $MODEL \
  --mmproj $MMPROJ \
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
  --temp 0.6 \
  --top-k 64 \
  --top-p 0.95 \
  --min-p 0 \
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
