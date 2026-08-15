#!/bin/bash
set -e
pkill -9 -f llama-server 2>/dev/null || true
sleep 2
/home/john/githubs/llm-server/llama.cpp/build/bin/llama-server \
  --model /home/john/githubs/llm-server/models/gemma-4-31b-it-abliterated/gemma-4-31b-it-abliterated-t126-Q4_K_M.gguf \
  --host 127.0.0.1 --port 8090 --gpu-layers 999 --ctx-size 4096 --threads 24 --flash-attn on --jinja --reasoning-format deepseek > /dev/null 2>&1 &
for i in $(seq 1 40); do curl -s http://127.0.0.1:8090/health 2>/dev/null | grep -q ok && echo "Ready" && break; sleep 2; done
/home/john/githubs/llm-server/.venv/bin/python3 /home/john/githubs/llm-server/scripts/bench-quality-v2.py http://127.0.0.1:8090 "Gemma4-31B-abliterated-Q4_K_M"
kill $(pgrep -f "llama-server.*8090") 2>/dev/null
