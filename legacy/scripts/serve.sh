#!/usr/bin/env bash
set -euo pipefail

# ── paths ──
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp/build/bin/llama-server"
MODEL_DIR="$ROOT_DIR/models"
LOG_DIR="$ROOT_DIR/logs"
CONFIG_DIR="$ROOT_DIR/config"

mkdir -p "$LOG_DIR"

# ── defaults (override via env or config) ──
MODEL="${MODEL:-}"
HOST="${HOST:-0.0.0.0}"
PORT="${PORT:-8080}"
GPU_LAYERS="${GPU_LAYERS:-999}"
CTX_SIZE="${CTX_SIZE:-32768}"
SPLIT_MODE="${SPLIT_MODE:-layer}"
REASONING_FORMAT="${REASONING_FORMAT:-none}"
PARALLEL="${PARALLEL:-2}"
BATCH_SIZE="${BATCH_SIZE:-4096}"
UBATCH_SIZE="${UBATCH_SIZE:-4096}"
THREADS="${THREADS:-24}"
EXTRA_ARGS="${EXTRA_ARGS:-}"

# ── load config file if present ──
CONFIG_FILE="${CONFIG_DIR}/default.env"
if [[ -f "$CONFIG_FILE" ]]; then
    set -a
    source "$CONFIG_FILE"
    set +a
fi

# ── usage ──
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS] [MODEL_NAME_OR_PATH]

Serve a GGUF model via llama.cpp with OpenAI-compatible API.

Options:
  -m, --model PATH      Path to GGUF file (or filename in models/)
  -p, --port PORT       API port (default: $PORT)
  -c, --ctx-size N      Context size (default: $CTX_SIZE)
  -sm, --split-mode M   Split mode: row|layer|none (default: $SPLIT_MODE)
  -j, --parallel N      Concurrent request slots (default: $PARALLEL)
  -l, --list            List available models in models/
  -h, --help            Show this help

Examples:
  $(basename "$0") Qwen_Qwen3.5-122B-A10B-Q5_K_M.gguf
  $(basename "$0") -m /path/to/model.gguf -p 8081 -c 65536
  MODEL=mymodel.gguf PORT=9090 $(basename "$0")

API will be available at:
  http://<host>:PORT/v1/chat/completions
  http://<host>:PORT/v1/completions
  http://<host>:PORT/v1/models
EOF
    exit 0
}

# ── list models ──
list_models() {
    echo "Models in $MODEL_DIR:"
    if compgen -G "$MODEL_DIR/*.gguf" > /dev/null 2>&1; then
        for f in "$MODEL_DIR"/*.gguf; do
            size=$(du -h "$f" | cut -f1)
            echo "  $(basename "$f")  ($size)"
        done
    else
        echo "  (none found — download GGUFs into $MODEL_DIR)"
    fi
    exit 0
}

# ── parse args ──
while [[ $# -gt 0 ]]; do
    case "$1" in
        -m|--model)      MODEL="$2"; shift 2 ;;
        -p|--port)       PORT="$2"; shift 2 ;;
        -c|--ctx-size)   CTX_SIZE="$2"; shift 2 ;;
        -sm|--split-mode) SPLIT_MODE="$2"; shift 2 ;;
        -j|--parallel)   PARALLEL="$2"; shift 2 ;;
        -l|--list)       list_models ;;
        -h|--help)       usage ;;
        -*)              echo "Unknown option: $1"; usage ;;
        *)               MODEL="$1"; shift ;;
    esac
done

# ── resolve model path ──
if [[ -z "$MODEL" ]]; then
    # try single .gguf first, then look for split shards in subdirectories
    MODEL=$(find "$MODEL_DIR" -maxdepth 1 -name '*.gguf' -print -quit 2>/dev/null || true)
    if [[ -z "$MODEL" ]]; then
        MODEL=$(find "$MODEL_DIR" -maxdepth 2 -name '*-00001-of-*.gguf' -print -quit 2>/dev/null || true)
    fi
    if [[ -z "$MODEL" ]]; then
        echo "Error: No model specified and no .gguf files found in $MODEL_DIR"
        echo "  Download one:  ./scripts/download-model.sh unsloth/Qwen3.5-122B-A10B-GGUF UD-Q4_K_XL"
        exit 1
    fi
fi

if [[ ! "$MODEL" = /* ]] && [[ ! -f "$MODEL" ]]; then
    # check as direct file
    if [[ -f "$MODEL_DIR/$MODEL" ]]; then
        MODEL="$MODEL_DIR/$MODEL"
    # check as subdirectory with split shards
    elif [[ -d "$MODEL_DIR/$MODEL" ]]; then
        MODEL=$(find "$MODEL_DIR/$MODEL" -name '*-00001-of-*.gguf' -print -quit 2>/dev/null || true)
        if [[ -z "$MODEL" ]]; then
            echo "Error: No split GGUF shards found in $MODEL_DIR/$MODEL"
            exit 1
        fi
    else
        echo "Error: Model not found: $MODEL (also checked $MODEL_DIR/$MODEL)"
        exit 1
    fi
fi

MODEL_NAME="$(basename "$MODEL" .gguf)"
LOG_FILE="$LOG_DIR/${MODEL_NAME}.log"

echo "══════════════════════════════════════════════════════"
echo "  llama.cpp server"
echo "══════════════════════════════════════════════════════"
echo "  Model:        $(basename "$MODEL")"
echo "  Size:         $(du -h "$MODEL" | cut -f1)"
echo "  API:          http://${HOST}:${PORT}/v1"
echo "  Context:      ${CTX_SIZE}"
echo "  GPU layers:   ${GPU_LAYERS}"
echo "  Split mode:   ${SPLIT_MODE}"
echo "  Batch:        ${BATCH_SIZE} / ${UBATCH_SIZE}"
echo "  Parallel:     ${PARALLEL} slots"
echo "  Threads:      ${THREADS}"
echo "  Log:          ${LOG_FILE}"
echo "══════════════════════════════════════════════════════"

exec "$SERVER" \
    --model "$MODEL" \
    --host "$HOST" \
    --port "$PORT" \
    --gpu-layers "$GPU_LAYERS" \
    --ctx-size "$CTX_SIZE" \
    --split-mode "$SPLIT_MODE" \
    --threads "$THREADS" \
    --parallel "$PARALLEL" \
    --batch-size "$BATCH_SIZE" \
    --ubatch-size "$UBATCH_SIZE" \
    --flash-attn on \
    --jinja \
    --reasoning-format "${REASONING_FORMAT}" \
    --cache-type-k bf16 \
    --cache-type-v bf16 \
    --metrics \
    --no-context-shift \
    --temp 0.6 \
    --top-k 20 \
    --top-p 0.95 \
    --min-p 0 \
    $EXTRA_ARGS \
    2>&1 | tee "$LOG_FILE"
