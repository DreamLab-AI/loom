#!/usr/bin/env bash
set -euo pipefail

# Nemotron-3-Super-120B-A12B (UD-IQ4_XS) via Unsloth's llama.cpp fork
# Architecture: Mamba-2 + MoE + Attention hybrid (120B total, 12B active)
# REQUIRES: unsloth/llama.cpp fork (standard llama.cpp won't work)
# Temperature: 1.0 for chat/reasoning, 0.6 for tool-calling

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp-unsloth/build/bin/llama-server"
MODEL_DIR="$ROOT_DIR/models/Nemotron-3-Super-120B-A12B-IQ4_XS"
LOG_DIR="$ROOT_DIR/logs"

# Find the first shard (split GGUF)
MODEL=$(find "$MODEL_DIR" -name '*-00001-of-*.gguf' -print -quit 2>/dev/null || true)
if [[ -z "$MODEL" ]]; then
    # Try single file
    MODEL=$(find "$MODEL_DIR" -name '*.gguf' -print -quit 2>/dev/null || true)
fi
if [[ -z "$MODEL" ]]; then
    echo "Error: No GGUF files found in $MODEL_DIR"
    echo "Download with:"
    echo "  huggingface-cli download unsloth/NVIDIA-Nemotron-3-Super-120B-A12B-GGUF \\"
    echo "    --include 'UD-IQ4_XS/*' --local-dir $MODEL_DIR"
    exit 1
fi

# Context size - default 262K (recommended max for 96GB VRAM), override with first arg
CTX_SIZE="${1:-262144}"

mkdir -p "$LOG_DIR"

echo "══════════════════════════════════════════════════════"
echo "  Nemotron-3-Super-120B-A12B (UD-IQ4_XS)"
echo "══════════════════════════════════════════════════════"
echo "  Model:        $(basename "$MODEL")"
echo "  Architecture: Mamba-2 + MoE + Attention hybrid"
echo "  Active:       12B / 120B parameters"
echo "  Context:      ${CTX_SIZE} tokens"
echo "  KV Cache:     q8_0 (Mamba layers need no KV)"
echo "  Fork:         unsloth/llama.cpp (required)"
echo "  API:          http://0.0.0.0:8080/v1"
echo "══════════════════════════════════════════════════════"

exec "$SERVER" \
    --model "$MODEL" \
    --host 0.0.0.0 \
    --port 8080 \
    --gpu-layers 999 \
    --ctx-size "$CTX_SIZE" \
    --split-mode layer \
    --threads 24 \
    --parallel 1 \
    --batch-size 4096 \
    --ubatch-size 2048 \
    --flash-attn on \
    --jinja \
    --reasoning-format none \
    --chat-template-kwargs '{"enable_thinking": false}' \
    --cache-type-k q8_0 \
    --cache-type-v q8_0 \
    --metrics \
    --no-context-shift \
    --temp 1.0 \
    --top-k 0 \
    --top-p 0.95 \
    --min-p 0 \
    2>&1 | tee "$LOG_DIR/nemotron-3-super.log"
