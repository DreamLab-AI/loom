#!/usr/bin/env bash
set -euo pipefail

# ── Gemma 4 26B-A4B-it — ARA-abliterated (uncensored), APEX-IQ quant ──
# DEFAULT uncensored model in the hybrid plan. Plain decode (no MTP — this is an
# abliterated community quant, not Google QAT; the boxwrench MTP heads don't apply).
#
# Safe by default: serves on :8082 and does NOT kill the qwen :8080 server that the
# email-mcp-gateway depends on. Pin to one GPU with --gpu0/--gpu1 to co-reside with
# qwen; for full 262K context, free VRAM first (stop qwen) and pass -c 262144.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp/build/bin/llama-server"
MODEL_DIR="$ROOT_DIR/models/gemma-4-26B-A4B-ara-abliterated"
LOG_DIR="$ROOT_DIR/logs"

MODEL="${MODEL:-$MODEL_DIR/gemma4-ara-2pass-APEX-IQ.gguf}"
MMPROJ="${MMPROJ:-$MODEL_DIR/mmproj-gemma4-f16.gguf}"

# q8_0 KV: measured fastest AND highest-fidelity here. On this hw q5_0 KV is a trap
# (Aria: q5_0=86 vs q8_0=140 t/s — no fast FA kernel for gemma4 head dims at q5_0),
# and q8_0 is also the article's high-tail-precision tier. Use --q5 ONLY to fit
# longer context when q8_0 KV won't fit VRAM (accepting the ~1.6x speed cost).
CACHE_TYPE="${CACHE_TYPE:-q8_0}"
CTX_SIZE="${CTX_SIZE:-32768}"
PORT="${PORT:-8082}"
PARALLEL="${PARALLEL:-1}"
GPU_ENV=""   # optional CUDA_VISIBLE_DEVICES pin

while [[ $# -gt 0 ]]; do
    case "$1" in
        --gpu0)      GPU_ENV="0"; shift ;;
        --gpu1)      GPU_ENV="1"; shift ;;
        --f16)       CACHE_TYPE=f16; shift ;;
        --q8)        CACHE_TYPE=q8_0; shift ;;
        --q4)        CACHE_TYPE=q4_0; shift ;;
        -c|--ctx)    CTX_SIZE="$2"; shift 2 ;;
        -p|--port)   PORT="$2"; shift 2 ;;
        -m|--model)  MODEL="$2"; shift 2 ;;
        --parallel)  PARALLEL="$2"; shift 2 ;;
        --no-vision) MMPROJ=""; shift ;;
        -h|--help)
            echo "Usage: $(basename "$0") [--gpu0|--gpu1] [--f16|--q8|--q4] [-c CTX] [-p PORT] [--parallel N]"
            echo "  Default: :8082, ctx 32768, q8_0 KV, vision on, does NOT touch :8080."
            exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ ! -f "$MODEL" ]]; then
    echo "Error: Model not found: $MODEL" >&2
    exit 1
fi

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/gemma4-abliterated.log"

MMPROJ_ARGS=""
if [[ -n "$MMPROJ" ]] && [[ -f "$MMPROJ" ]]; then
    MMPROJ_ARGS="--mmproj $MMPROJ"
fi

GPU_PREFIX=""
if [[ -n "$GPU_ENV" ]]; then
    GPU_PREFIX="CUDA_VISIBLE_DEVICES=$GPU_ENV"
fi

echo "══════════════════════════════════════════════════════"
echo "  Gemma 4 26B-A4B-it — ARA-abliterated (uncensored)"
echo "══════════════════════════════════════════════════════"
echo "  Model:    $(basename "$MODEL")"
echo "  Vision:   $([ -n "$MMPROJ_ARGS" ] && basename "$MMPROJ" || echo disabled)"
echo "  KV cache: ${CACHE_TYPE} (Hadamard auto)   Ctx: ${CTX_SIZE}   Parallel: ${PARALLEL}"
echo "  GPU pin:  ${GPU_ENV:-both (split layer)}"
echo "  Decode:   PLAIN (no MTP — see config/gemma4-qat-mtp.env for the fast path)"
echo "  API:      http://0.0.0.0:${PORT}/v1     Log: ${LOG_FILE}"
echo "══════════════════════════════════════════════════════"

# shellcheck disable=SC2086
env $GPU_PREFIX "$SERVER" \
    --model "$MODEL" \
    $MMPROJ_ARGS \
    --host 0.0.0.0 \
    --port "$PORT" \
    --gpu-layers 999 \
    --ctx-size "$CTX_SIZE" \
    --split-mode layer \
    --threads 24 \
    --parallel "$PARALLEL" \
    --batch-size 4096 \
    --ubatch-size 2048 \
    --flash-attn on \
    --jinja \
    --reasoning-format deepseek \
    --cache-type-k "$CACHE_TYPE" \
    --cache-type-v "$CACHE_TYPE" \
    --metrics \
    --no-context-shift \
    --temp 0.6 \
    --top-k 64 \
    --top-p 0.95 \
    --min-p 0 \
    > "$LOG_FILE" 2>&1 &

PID=$!
echo "Server PID: $PID  — waiting for load..."
for i in $(seq 1 90); do
    if curl -s "http://127.0.0.1:${PORT}/health" 2>/dev/null | grep -q ok; then
        echo "Ready after ${i}s — API at http://0.0.0.0:${PORT}/v1"
        exit 0
    fi
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "Server died during load. Tail of $LOG_FILE:" >&2
        tail -20 "$LOG_FILE" >&2
        exit 1
    fi
    sleep 2
done
echo "Still loading after 180s — check $LOG_FILE"
