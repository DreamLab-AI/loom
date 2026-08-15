#!/usr/bin/env bash
set -euo pipefail

# Benchmark Hadamard-rotated KV cache quantization on Gemma 4 31B-it
# Tests: f16 baseline, q8_0, q5_0, q4_0 — all with automatic Hadamard rotation
# SWA layers stay F16 regardless (PR #21277)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
BIN="$ROOT_DIR/llama.cpp/build/bin/llama-perplexity"
MODEL="$ROOT_DIR/models/gemma-4-31B-it/gemma-4-31B-it-Q8_0.gguf"
WIKITEXT="$ROOT_DIR/models/wikitext-2-raw-test.txt"
RESULTS="$ROOT_DIR/logs/bench-gemma4-kv-results.txt"

if [[ ! -f "$MODEL" ]]; then
    echo "Error: Model not found: $MODEL"
    exit 1
fi

if [[ ! -f "$WIKITEXT" ]]; then
    echo "Error: wikitext-2 not found: $WIKITEXT"
    exit 1
fi

mkdir -p "$ROOT_DIR/logs"

echo "══════════════════════════════════════════════════════════" | tee "$RESULTS"
echo "  Gemma 4 31B-it KV Cache Benchmark (Hadamard Rotation)" | tee -a "$RESULTS"
echo "  Model: $(basename "$MODEL")" | tee -a "$RESULTS"
echo "  Date:  $(date -Iseconds)" | tee -a "$RESULTS"
echo "══════════════════════════════════════════════════════════" | tee -a "$RESULTS"
echo "" | tee -a "$RESULTS"

run_bench() {
    local label="$1"
    local cache_k="$2"
    local cache_v="$3"

    echo "── $label (K=$cache_k V=$cache_v) ──" | tee -a "$RESULTS"

    $BIN --model "$MODEL" \
        --gpu-layers 999 --threads 24 --flash-attn on --split-mode layer \
        --ctx-size 512 --chunks 16 \
        --cache-type-k "$cache_k" --cache-type-v "$cache_v" \
        -f "$WIKITEXT" >> "$RESULTS" 2>&1

    # Extract and display PPL
    local ppl=$(grep "Final estimate" "$RESULTS" | tail -1)
    echo "  $ppl" | tee -a /dev/stderr
    echo "" | tee -a "$RESULTS"
    echo "────────────────────────────────────────────────────" | tee -a "$RESULTS"
}

# 1. Baseline: f16 KV (no quantization, no rotation)
run_bench "Baseline f16" "f16" "f16"

# 2. q8_0 KV (with Hadamard rotation)
run_bench "q8_0 (rotated)" "q8_0" "q8_0"

# 3. q5_0 KV (with Hadamard rotation)
run_bench "q5_0 (rotated)" "q5_0" "q5_0"

# 4. q4_0 KV (with Hadamard rotation) — best compression
run_bench "q4_0 (rotated)" "q4_0" "q4_0"

echo "" | tee -a "$RESULTS"
echo "══════════════════════════════════════════════════════════" | tee -a "$RESULTS"
echo "  Summary:" | tee -a "$RESULTS"
grep "Final estimate" "$RESULTS" | tee -a /dev/stderr
echo "" | tee -a "$RESULTS"
echo "  Results saved: $RESULTS" | tee -a "$RESULTS"
echo "══════════════════════════════════════════════════════════" | tee -a "$RESULTS"
