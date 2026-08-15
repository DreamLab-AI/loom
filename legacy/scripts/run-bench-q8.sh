#!/bin/bash
set -e
SERVER=/home/john/githubs/llm-server/llama.cpp/build/bin/llama-server
MODEL=/home/john/githubs/llm-server/models/gemma-4-31B-it/gemma-4-31B-it-Q8_0.gguf
BENCH=/home/john/githubs/llm-server/scripts/bench-quality.py
PYTHON=/home/john/githubs/llm-server/.venv/bin/python3

$SERVER --model $MODEL --host 127.0.0.1 --port 8090 --gpu-layers 999 --ctx-size 4096 --threads 24 --flash-attn on --jinja --reasoning-format deepseek > /dev/null 2>&1 &
PID=$!
echo "Server PID: $PID"

for i in $(seq 1 40); do
    if curl -s http://127.0.0.1:8090/health 2>/dev/null | grep -q ok; then
        echo "Ready after ${i}s"
        break
    fi
    sleep 2
done

$PYTHON $BENCH http://127.0.0.1:8090 "Gemma4-31B-original-Q8_0"

kill $PID 2>/dev/null
