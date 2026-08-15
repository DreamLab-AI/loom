#!/bin/bash
set -e
pkill -9 -f llama-server 2>/dev/null || true
pkill -f "curl.*bf16" 2>/dev/null || true
sleep 3

SERVER=/home/john/githubs/llm-server/llama.cpp/build/bin/llama-server
MODEL=/home/john/githubs/llm-server/models/gemma-4-26B-A4B-ara-abliterated/gemma4-ara-2pass-bf16.gguf
MMPROJ=/home/john/githubs/llm-server/models/gemma-4-26B-A4B-ara-abliterated/mmproj-gemma4-f16.gguf
BENCH=/home/john/githubs/llm-server/scripts/bench-quality-v2.py
PYTHON=/home/john/githubs/llm-server/.venv/bin/python3

$SERVER --model $MODEL --mmproj $MMPROJ \
  --host 127.0.0.1 --port 8090 \
  --gpu-layers 999 --ctx-size 4096 --threads 24 \
  --split-mode layer \
  --flash-attn on --jinja --reasoning-format deepseek \
  --cache-type-k q8_0 --cache-type-v q8_0 \
  > /dev/null 2>&1 &
PID=$!
echo "Server PID: $PID"

for i in $(seq 1 60); do
    if curl -s http://127.0.0.1:8090/health 2>/dev/null | grep -q ok; then
        echo "Ready after ${i}s"
        break
    fi
    sleep 2
done

$PYTHON $BENCH http://127.0.0.1:8090 "Gemma4-26B-A4B-BF16-abliterated"

kill $PID 2>/dev/null
