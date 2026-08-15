#!/usr/bin/env bash
set -euo pipefail

# Qwen3.5-122B-A10B (UD-Q4_K_XL) with TurboQuant KV cache
# Uses spiritbuun/llama-cpp-turboquant-cuda fork
# Available cache types: turbo2, turbo3, turbo4, q8_0, q4_0, f16

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp-turboquant/build/bin/llama-server"
MODEL_DIR="$ROOT_DIR/models/UD-Q4_K_XL"
LOG_DIR="$ROOT_DIR/logs"

# Find first shard
MODEL=$(find "$MODEL_DIR" -name '*-00001-of-*.gguf' -print -quit 2>/dev/null || true)
if [[ -z "$MODEL" ]]; then
    MODEL=$(find "$MODEL_DIR" -name '*.gguf' -print -quit 2>/dev/null || true)
fi
if [[ -z "$MODEL" ]]; then
    echo "Error: No GGUF files found in $MODEL_DIR"
    exit 1
fi

if [[ ! -x "$SERVER" ]]; then
    echo "Error: TurboQuant server not built at $SERVER"
    echo "Build with: cd $ROOT_DIR/llama.cpp-turboquant && cmake -B build -DGGML_CUDA=ON -DCMAKE_BUILD_TYPE=Release && cmake --build build -j\$(nproc)"
    exit 1
fi

# Defaults — override with env vars or args
CACHE_K="${CACHE_K:-turbo4}"
CACHE_V="${CACHE_V:-turbo4}"
CTX_SIZE="${CTX_SIZE:-262144}"
PORT="${PORT:-8080}"

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --turbo2)   CACHE_K=turbo2; CACHE_V=turbo2; shift ;;
        --turbo3)   CACHE_K=turbo3; CACHE_V=turbo3; shift ;;
        --turbo4)   CACHE_K=turbo4; CACHE_V=turbo4; shift ;;
        --safe)     CACHE_K=q8_0;   CACHE_V=turbo4; shift ;;  # asymmetric safe mode
        --baseline) CACHE_K=q8_0;   CACHE_V=q8_0;   shift ;;  # baseline comparison
        -c|--ctx)   CTX_SIZE="$2"; shift 2 ;;
        -p|--port)  PORT="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $(basename "$0") [--turbo2|--turbo3|--turbo4|--safe|--baseline] [-c CTX] [-p PORT]"
            echo ""
            echo "  --turbo4    turbo4 K+V (3.8x compression, best quality) [default]"
            echo "  --turbo3    turbo3 K+V (4.6x compression)"
            echo "  --turbo2    turbo2 K+V (6.4x compression, experimental)"
            echo "  --safe      q8_0 K + turbo4 V (safest for low-bit weight quants)"
            echo "  --baseline  q8_0 K+V (no TurboQuant, for comparison)"
            echo "  -c CTX      Context size (default: 262144)"
            echo "  -p PORT     Port (default: 8080)"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/qwen3.5-122b-${CACHE_K}-${CACHE_V}.log"

echo "══════════════════════════════════════════════════════"
echo "  Qwen3.5-122B-A10B (UD-Q4_K_XL) — TurboQuant KV"
echo "══════════════════════════════════════════════════════"
echo "  Model:        $(basename "$MODEL")"
echo "  Architecture: MoE (122B total, 10B active)"
echo "  KV Cache K:   ${CACHE_K}"
echo "  KV Cache V:   ${CACHE_V}"
echo "  Context:      ${CTX_SIZE} tokens"
echo "  Fork:         spiritbuun/llama-cpp-turboquant-cuda"
echo "  API:          http://0.0.0.0:${PORT}/v1"
echo "  Log:          ${LOG_FILE}"
echo "══════════════════════════════════════════════════════"

exec "$SERVER" \
    --model "$MODEL" \
    --host 0.0.0.0 \
    --port "$PORT" \
    --gpu-layers 999 \
    --ctx-size "$CTX_SIZE" \
    --split-mode layer \
    --threads 24 \
    --parallel 1 \
    --batch-size 4096 \
    --ubatch-size 4096 \
    --flash-attn on \
    --jinja \
    --reasoning-format none \
    --cache-type-k "$CACHE_K" \
    --cache-type-v "$CACHE_V" \
    --metrics \
    --no-context-shift \
    --temp 0.6 \
    --top-k 20 \
    --top-p 0.95 \
    --min-p 0 \
    2>&1 | tee "$LOG_FILE"
