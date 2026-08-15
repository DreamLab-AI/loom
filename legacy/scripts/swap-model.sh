#!/usr/bin/env bash
set -euo pipefail

# Swap between models on llama-server (port 8080)
# Usage: ./swap-model.sh [9b|122b|nemotron]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp/build/bin/llama-server"
LOG_DIR="$ROOT_DIR/logs"
mkdir -p "$LOG_DIR"

MODEL_CHOICE="${1:-}"

UNSLOTH_SERVER="$ROOT_DIR/llama.cpp-unsloth/build/bin/llama-server"

usage() {
    echo "Usage: $(basename "$0") [9b|122b|nemotron|gemma4|status|stop]"
    echo ""
    echo "  9b        Qwen3.5-9B-Uncensored (Q8_0) + Vision, 262K ctx"
    echo "  122b      Qwen3.5-122B-A10B (UD-Q4_K_XL), 262K ctx"
    echo "  nemotron  Nemotron-3-Super-120B-A12B (UD-IQ4_XS), 1M ctx [unsloth fork]"
    echo "  gemma4    Gemma 4 31B-it (Q8_0) + Vision, 262K ctx, rotated q4_0 KV"
    echo "  status    Show which model is currently loaded"
    echo "  stop      Stop the running server"
    exit 0
}

stop_server() {
    local pid
    pid=$(pgrep -f "llama-server.*--port 8080" || true)
    if [[ -n "$pid" ]]; then
        echo "Stopping llama-server (PID: $pid)..."
        kill "$pid"
        # Wait for it to actually stop
        for i in {1..30}; do
            if ! kill -0 "$pid" 2>/dev/null; then
                echo "Server stopped."
                return 0
            fi
            sleep 1
        done
        echo "Force killing..."
        kill -9 "$pid" 2>/dev/null || true
    else
        echo "No llama-server running on port 8080."
    fi
}

show_status() {
    local pid
    pid=$(pgrep -f "llama-server.*--port 8080" || true)
    if [[ -n "$pid" ]]; then
        local model
        model=$(ps -p "$pid" -o args= | grep -oP '(?<=--model )\S+' | xargs basename)
        echo "Running: $model (PID: $pid)"
        curl -s http://127.0.0.1:8080/v1/models 2>/dev/null | python3 -c "
import json, sys
d = json.load(sys.stdin)
for m in d.get('data', []):
    caps = ', '.join(d.get('models', [{}])[0].get('capabilities', []))
    print(f\"  Model ID: {m['id']}\")
    print(f\"  Context: {m['meta']['n_ctx_train']}\")
    print(f\"  Params: {m['meta']['n_params']:,}\")
    print(f\"  Capabilities: {caps}\")
" 2>/dev/null || true
    else
        echo "No llama-server running on port 8080."
    fi
}

case "${MODEL_CHOICE}" in
    9b|9B)
        stop_server
        echo ""
        echo "Loading Qwen3.5-9B-Uncensored (Q8_0) + Vision..."
        MODEL="$ROOT_DIR/models/Qwen3.5-9B-Uncensored/Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q8_0.gguf"
        MMPROJ="$ROOT_DIR/models/Qwen3.5-9B-Uncensored/mmproj-Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-BF16.gguf"

        nohup "$SERVER" \
            --model "$MODEL" \
            --mmproj "$MMPROJ" \
            --host 0.0.0.0 \
            --port 8080 \
            --gpu-layers 999 \
            --ctx-size 262144 \
            --split-mode layer \
            --threads 24 \
            --parallel 1 \
            --batch-size 4096 \
            --ubatch-size 2048 \
            --flash-attn on \
            --jinja \
            --reasoning-format none \
            --rope-scaling yarn \
            --rope-scale 4 \
            --rope-freq-base 10000000 \
            --yarn-orig-ctx 262144 \
            --cache-type-k q8_0 \
            --cache-type-v q8_0 \
            --metrics \
            --no-context-shift \
            --temp 0.6 \
            --top-k 20 \
            --top-p 0.95 \
            --min-p 0 \
            > "$LOG_DIR/qwen3.5-9b-vision.log" 2>&1 &

        echo "Server starting (PID: $!)..."
        echo "Waiting for model load..."
        for i in {1..120}; do
            if curl -s http://127.0.0.1:8080/v1/models >/dev/null 2>&1; then
                echo "Ready! API: http://0.0.0.0:8080/v1"
                show_status
                exit 0
            fi
            sleep 2
        done
        echo "Warning: Server may still be loading. Check logs: $LOG_DIR/qwen3.5-9b-vision.log"
        ;;

    122b|122B)
        stop_server
        echo ""
        echo "Loading Qwen3.5-122B-A10B (UD-Q4_K_XL)..."
        MODEL="$ROOT_DIR/models/UD-Q4_K_XL/Qwen3.5-122B-A10B-UD-Q4_K_XL-00001-of-00003.gguf"

        nohup "$SERVER" \
            --model "$MODEL" \
            --host 0.0.0.0 \
            --port 8080 \
            --gpu-layers 999 \
            --ctx-size 262144 \
            --split-mode layer \
            --threads 24 \
            --parallel 2 \
            --batch-size 4096 \
            --ubatch-size 4096 \
            --flash-attn on \
            --jinja \
            --reasoning-format none \
            --cache-type-k bf16 \
            --cache-type-v bf16 \
            --metrics \
            --no-context-shift \
            --temp 0.6 \
            --top-k 20 \
            --top-p 0.95 \
            --min-p 0 \
            > "$LOG_DIR/qwen3.5-122b.log" 2>&1 &

        echo "Server starting (PID: $!)..."
        echo "Waiting for model load (this takes a while for 122B)..."
        for i in {1..300}; do
            if curl -s http://127.0.0.1:8080/v1/models >/dev/null 2>&1; then
                echo "Ready! API: http://0.0.0.0:8080/v1"
                show_status
                exit 0
            fi
            sleep 2
        done
        echo "Warning: Server may still be loading. Check logs: $LOG_DIR/qwen3.5-122b.log"
        ;;

    nemotron|nem)
        stop_server
        echo ""
        echo "Loading Nemotron-3-Super-120B-A12B (UD-IQ4_XS)..."
        NEMOTRON_DIR="$ROOT_DIR/models/Nemotron-3-Super-120B-A12B-IQ4_XS"
        MODEL=$(find "$NEMOTRON_DIR" -name '*-00001-of-*.gguf' -print -quit 2>/dev/null || true)
        if [[ -z "$MODEL" ]]; then
            MODEL=$(find "$NEMOTRON_DIR" -name '*.gguf' -print -quit 2>/dev/null || true)
        fi
        if [[ -z "$MODEL" ]]; then
            echo "Error: No GGUF found in $NEMOTRON_DIR"
            exit 1
        fi
        if [[ ! -x "$UNSLOTH_SERVER" ]]; then
            echo "Error: Unsloth llama.cpp server not found at $UNSLOTH_SERVER"
            echo "Build with: cd $ROOT_DIR/llama.cpp-unsloth && cmake -B build -DGGML_CUDA=ON && cmake --build build -j"
            exit 1
        fi

        nohup "$UNSLOTH_SERVER" \
            --model "$MODEL" \
            --host 0.0.0.0 \
            --port 8080 \
            --gpu-layers 999 \
            --ctx-size 262144 \
            --split-mode layer \
            --threads 24 \
            --parallel 1 \
            --batch-size 4096 \
            --ubatch-size 2048 \
            --flash-attn on \
            --jinja \
            --reasoning-format none \
            --cache-type-k q8_0 \
            --cache-type-v q8_0 \
            --metrics \
            --no-context-shift \
            --temp 1.0 \
            --top-k 0 \
            --top-p 0.95 \
            --min-p 0 \
            > "$LOG_DIR/nemotron-3-super.log" 2>&1 &

        echo "Server starting (PID: $!)..."
        echo "Waiting for model load (120B MoE, this takes a while)..."
        for i in {1..300}; do
            if curl -s http://127.0.0.1:8080/v1/models >/dev/null 2>&1; then
                echo "Ready! API: http://0.0.0.0:8080/v1"
                show_status
                exit 0
            fi
            sleep 2
        done
        echo "Warning: Server may still be loading. Check logs: $LOG_DIR/nemotron-3-super.log"
        ;;

    status|s)
        show_status
        ;;

    gemma4|gemma)
        stop_server
        echo ""
        echo "Loading Gemma 4 31B-it (Q8_0) + Vision..."
        GEMMA_MODEL="$ROOT_DIR/models/gemma-4-31B-it/gemma-4-31B-it-Q8_0.gguf"
        GEMMA_MMPROJ="$ROOT_DIR/models/gemma-4-31B-it/mmproj-BF16.gguf"
        if [[ ! -f "$GEMMA_MODEL" ]]; then
            echo "Error: Model not found at $GEMMA_MODEL"
            exit 1
        fi

        MMPROJ_ARGS=""
        if [[ -f "$GEMMA_MMPROJ" ]]; then
            MMPROJ_ARGS="--mmproj $GEMMA_MMPROJ"
        fi

        nohup "$SERVER" \
            --model "$GEMMA_MODEL" \
            $MMPROJ_ARGS \
            --host 0.0.0.0 \
            --port 8080 \
            --gpu-layers 999 \
            --ctx-size 262144 \
            --split-mode layer \
            --threads 24 \
            --parallel 1 \
            --batch-size 4096 \
            --ubatch-size 2048 \
            --flash-attn on \
            --jinja \
            --reasoning-format deepseek \
            --cache-type-k q8_0 \
            --cache-type-v q8_0 \
            --metrics \
            --no-context-shift \
            --temp 0.6 \
            --top-k 20 \
            --top-p 0.95 \
            --min-p 0 \
            > "$LOG_DIR/gemma4-31b.log" 2>&1 &

        echo "Server starting (PID: $!)..."
        echo "Waiting for model load..."
        for i in {1..120}; do
            if curl -s http://127.0.0.1:8080/v1/models >/dev/null 2>&1; then
                echo "Ready! API: http://0.0.0.0:8080/v1"
                show_status
                exit 0
            fi
            sleep 2
        done
        echo "Warning: Server may still be loading. Check logs: $LOG_DIR/gemma4-31b.log"
        ;;

    stop)
        stop_server
        ;;

    *)
        usage
        ;;
esac
