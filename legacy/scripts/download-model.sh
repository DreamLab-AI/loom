#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODEL_DIR="$(dirname "$SCRIPT_DIR")/models"

REPO="${1:-bartowski/Qwen_Qwen3.5-122B-A10B-GGUF}"
QUANT="${2:-Q5_K_M}"

# derive filename from repo and quant
REPO_BASENAME="$(echo "$REPO" | sed 's|.*/||; s|-GGUF$||')"
FILENAME="${REPO_BASENAME}-${QUANT}.gguf"

echo "Downloading $FILENAME from $REPO ..."
echo "  Destination: $MODEL_DIR"

if ! command -v huggingface-cli &>/dev/null; then
    echo "Installing huggingface-cli..."
    pip install -U "huggingface_hub[cli]"
fi

huggingface-cli download "$REPO" \
    --include "$FILENAME" \
    --local-dir "$MODEL_DIR"

echo "Done. Model saved to: $MODEL_DIR/$FILENAME"
