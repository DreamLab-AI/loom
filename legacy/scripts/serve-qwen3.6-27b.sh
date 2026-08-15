#!/usr/bin/env bash
set -euo pipefail

# ── Qwen 3.6 27B (dense) — flagship-coding model, kept for occasional use ──
# DENSE: ~42 t/s on this box (all 27B active/token) vs ~140 for the MoE models.
# Use when you want its coding/reasoning quality and can tolerate the speed.
# Safe: serves on :8084 pinned to GPU0; does NOT touch qwen-MoE :8080 or Aria :8082.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp/build/bin/llama-server"
MODEL="${MODEL:-$ROOT_DIR/models/Qwen3.6-27B/Qwen3.6-27B-UD-Q4_K_XL.gguf}"
LOG_DIR="$ROOT_DIR/logs"

CACHE_TYPE="${CACHE_TYPE:-q8_0}"
CTX_SIZE="${CTX_SIZE:-16384}"
PORT="${PORT:-8084}"
CUDA_DEVICE="${CUDA_DEVICE:-0}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        -c|--ctx)  CTX_SIZE="$2"; shift 2 ;;
        -p|--port) PORT="$2"; shift 2 ;;
        --gpu)     CUDA_DEVICE="$2"; shift 2 ;;
        --q5)      CACHE_TYPE=q5_0; shift ;;
        --f16)     CACHE_TYPE=f16; shift ;;
        -h|--help) echo "Usage: $(basename "$0") [-c CTX] [-p PORT] [--gpu N] [--q5|--f16]"; exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

[[ -f "$MODEL" ]] || { echo "Error: model not found: $MODEL" >&2; exit 1; }
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/qwen27.log"

echo "── Qwen 3.6 27B dense @ :${PORT} (GPU${CUDA_DEVICE}, ${CACHE_TYPE} KV, ctx ${CTX_SIZE}) ──"
CUDA_VISIBLE_DEVICES="$CUDA_DEVICE" "$SERVER" \
    --model "$MODEL" \
    --host 0.0.0.0 --port "$PORT" \
    --gpu-layers 999 --ctx-size "$CTX_SIZE" \
    --threads 24 --parallel 1 \
    --batch-size 4096 --ubatch-size 2048 \
    --flash-attn on --jinja --reasoning-format deepseek \
    --cache-type-k "$CACHE_TYPE" --cache-type-v "$CACHE_TYPE" \
    --metrics --no-context-shift \
    --temp 0.6 --top-k 20 --top-p 0.95 --min-p 0 \
    > "$LOG_FILE" 2>&1 &
PID=$!
echo "PID $PID — waiting for load..."
for i in $(seq 1 90); do
    curl -s "http://127.0.0.1:${PORT}/health" 2>/dev/null | grep -q ok && { echo "Ready (${i}x2s) — http://0.0.0.0:${PORT}/v1"; exit 0; }
    kill -0 "$PID" 2>/dev/null || { echo "died; tail $LOG_FILE:" >&2; tail -20 "$LOG_FILE" >&2; exit 1; }
    sleep 2
done
echo "still loading — check $LOG_FILE"
