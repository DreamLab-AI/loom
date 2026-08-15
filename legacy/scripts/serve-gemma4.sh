#!/usr/bin/env bash
set -euo pipefail

# Gemma 4 31B-it with Hadamard-rotated KV cache quantization
# Architecture: Hybrid SWA/global attention, 60 layers (50 SWA + 10 global)
# SWA cache kept in F16 (1024 token window), global cache quantized
# head_dim: 256 (SWA) / 512 (global), attention_k_eq_v on global layers
# Hadamard rotation (PR #21038) is always-on for quantized KV caches

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp/build/bin/llama-server"
MODEL_DIR="$ROOT_DIR/models/gemma-4-31B-it"
LOG_DIR="$ROOT_DIR/logs"

MODEL="${MODEL_DIR}/gemma-4-31B-it-Q8_0.gguf"
MMPROJ="${MODEL_DIR}/mmproj-BF16.gguf"

# KV cache type for global attention layers (SWA stays F16 automatically)
CACHE_TYPE="${CACHE_TYPE:-q4_0}"
CTX_SIZE="${CTX_SIZE:-262144}"
PORT="${PORT:-8080}"

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --f16)      CACHE_TYPE=f16; shift ;;
        --q8)       CACHE_TYPE=q8_0; shift ;;
        --q4)       CACHE_TYPE=q4_0; shift ;;
        --q5)       CACHE_TYPE=q5_0; shift ;;
        -c|--ctx)   CTX_SIZE="$2"; shift 2 ;;
        -p|--port)  PORT="$2"; shift 2 ;;
        -m|--model) MODEL="$2"; shift 2 ;;
        --no-vision) MMPROJ=""; shift ;;
        -h|--help)
            echo "Usage: $(basename "$0") [--f16|--q8|--q4|--q5] [-c CTX] [-p PORT]"
            echo ""
            echo "  --q4        q4_0 KV cache with Hadamard rotation [default]"
            echo "  --q5        q5_0 KV cache with Hadamard rotation"
            echo "  --q8        q8_0 KV cache with Hadamard rotation"
            echo "  --f16       f16 KV cache (baseline, no quantization)"
            echo "  --no-vision Disable vision/mmproj"
            echo "  -c CTX      Context size (default: 262144)"
            echo "  -p PORT     Port (default: 8080)"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ ! -f "$MODEL" ]]; then
    echo "Error: Model not found: $MODEL"
    echo "Download with:"
    echo "  curl -L -H 'Authorization: Bearer \$HF_TOKEN' \\"
    echo "    https://huggingface.co/unsloth/gemma-4-31B-it-GGUF/resolve/main/gemma-4-31B-it-Q8_0.gguf \\"
    echo "    -o $MODEL"
    exit 1
fi

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/gemma4-31b-${CACHE_TYPE}.log"

MMPROJ_ARGS=""
if [[ -n "$MMPROJ" ]] && [[ -f "$MMPROJ" ]]; then
    MMPROJ_ARGS="--mmproj $MMPROJ"
fi

echo "══════════════════════════════════════════════════════"
echo "  Gemma 4 31B-it (Q8_0) — Hadamard-Rotated KV Cache"
echo "══════════════════════════════════════════════════════"
echo "  Model:        $(basename "$MODEL")"
echo "  Vision:       $([ -n "$MMPROJ_ARGS" ] && basename "$MMPROJ" || echo 'disabled')"
echo "  Architecture: Hybrid SWA (50) + Global (10) attention"
echo "  KV Cache:     ${CACHE_TYPE} (global), f16 (SWA, auto)"
echo "  Rotation:     Hadamard (always-on for quantized KV)"
echo "  Context:      ${CTX_SIZE} tokens"
echo "  API:          http://0.0.0.0:${PORT}/v1"
echo "  Log:          ${LOG_FILE}"
echo "══════════════════════════════════════════════════════"

exec "$SERVER" \
    --model "$MODEL" \
    $MMPROJ_ARGS \
    --host 0.0.0.0 \
    --port "$PORT" \
    --gpu-layers 999 \
    --ctx-size "$CTX_SIZE" \
    --split-mode layer \
    --threads 24 \
    --parallel 1 \
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
    --top-k 20 \
    --top-p 0.95 \
    --min-p 0 \
    2>&1 | tee "$LOG_FILE"
