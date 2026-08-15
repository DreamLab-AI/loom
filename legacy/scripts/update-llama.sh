#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LLAMA_DIR="$(dirname "$SCRIPT_DIR")/llama.cpp"

echo "Updating llama.cpp..."
cd "$LLAMA_DIR"
git pull --ff-only

echo "Rebuilding with CUDA..."
cmake -B build -DGGML_CUDA=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j$(nproc)

echo "Done."
"$LLAMA_DIR/build/bin/llama-server" --version
