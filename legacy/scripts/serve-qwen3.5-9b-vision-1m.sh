#!/usr/bin/env bash
set -euo pipefail

# Qwen3.5-9B-Uncensored-HauhauCS-Aggressive with Vision + 1M RoPE Context
# Architecture: Hybrid linear/full attention (3:1) - efficient for long context
# Vision: mmproj encoder for image/video understanding
# RoPE: YaRN scaling factor 4 (262K native -> 1M extended)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SERVER="$ROOT_DIR/llama.cpp/build/bin/llama-server"
MODEL_DIR="$ROOT_DIR/models/Qwen3.5-9B-Uncensored"
LOG_DIR="$ROOT_DIR/logs"

MODEL="$MODEL_DIR/Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-Q8_0.gguf"
MMPROJ="$MODEL_DIR/mmproj-Qwen3.5-9B-Uncensored-HauhauCS-Aggressive-BF16.gguf"

# Context size - default 1M, override with first arg
CTX_SIZE="${1:-1048576}"

mkdir -p "$LOG_DIR"

# Validate files exist
for f in "$MODEL" "$MMPROJ"; do
    if [[ ! -f "$f" ]]; then
        echo "Error: Missing file: $f"
        echo "Download with:"
        echo "  huggingface-cli download HauhauCS/Qwen3.5-9B-Uncensored-HauhauCS-Aggressive \\"
        echo "    --include '*.gguf' --local-dir $MODEL_DIR"
        exit 1
    fi
done

echo "══════════════════════════════════════════════════════"
echo "  Qwen3.5-9B Vision (Uncensored) — 1M Context"
echo "══════════════════════════════════════════════════════"
echo "  Model:        $(basename "$MODEL") (Q8_0, 8.9 GB)"
echo "  Vision:       $(basename "$MMPROJ") (BF16, 880 MB)"
echo "  Context:      ${CTX_SIZE} tokens"
echo "  RoPE:         YaRN x4 (262K → 1M)"
echo "  KV Cache:     q8_0 (reduced memory for long ctx)"
echo "  API:          http://0.0.0.0:8080/v1"
echo "══════════════════════════════════════════════════════"

exec "$SERVER" \
    --model "$MODEL" \
    --mmproj "$MMPROJ" \
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
    2>&1 | tee "$LOG_DIR/qwen3.5-9b-vision-1m.log"
