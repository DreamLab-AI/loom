#!/bin/bash
set -e
BENCH=/home/john/githubs/llm-server/scripts/bench-quality-v2.py
PYTHON=/home/john/githubs/llm-server/.venv/bin/python3
PORT=8090

start_and_bench() {
    local label="$1"
    local server="$2"
    local model="$3"
    shift 3
    local extra_args="$@"

    echo ""
    echo "================================================================"
    echo "  Starting: $label"
    echo "================================================================"
    pkill -9 -f "llama-server.*$PORT" 2>/dev/null || true
    sleep 3

    $server --model $model --host 127.0.0.1 --port $PORT \
        --gpu-layers 999 --ctx-size 4096 --threads 24 \
        --flash-attn on --jinja --reasoning-format deepseek \
        $extra_args > /dev/null 2>&1 &
    local pid=$!

    for i in $(seq 1 60); do
        if curl -s http://127.0.0.1:$PORT/health 2>/dev/null | grep -q ok; then
            echo "  Ready after ${i}s (PID $pid)"
            break
        fi
        sleep 2
    done

    $PYTHON $BENCH http://127.0.0.1:$PORT "$label"
    kill $pid 2>/dev/null || true
    sleep 2
}

# Nemotron (needs unsloth fork)
start_and_bench "Nemotron-3-Super-120B-A12B-IQ4_XS" \
    /home/john/githubs/llm-server/llama.cpp-unsloth/build/bin/llama-server \
    "/home/john/githubs/llm-server/models/Nemotron-3-Super-120B-A12B-IQ4_XS/UD-IQ4_XS/NVIDIA-Nemotron-3-Super-120B-A12B-UD-IQ4_XS-00001-of-00003.gguf" \
    "--split-mode layer --cache-type-k q8_0 --cache-type-v q8_0"

# Qwen 122B (standard llama.cpp)
start_and_bench "Qwen3.5-122B-A10B-UD-Q4_K_XL" \
    /home/john/githubs/llm-server/llama.cpp/build/bin/llama-server \
    "/home/john/githubs/llm-server/models/UD-Q4_K_XL/Qwen3.5-122B-A10B-UD-Q4_K_XL-00001-of-00003.gguf" \
    "--split-mode layer --cache-type-k q8_0 --cache-type-v q8_0"

echo ""
echo "================================================================"
echo "  All benchmarks complete"
echo "================================================================"
