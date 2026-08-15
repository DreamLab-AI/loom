#!/usr/bin/env bash
set -euo pipefail

# Benchmark TurboQuant KV cache on Qwen3.5-122B-A10B
# Runs llama-perplexity across different cache configurations
# Results written to logs/bench-turboquant-results.txt

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
TURBO_BIN="$ROOT_DIR/llama.cpp-turboquant/build/bin"
STANDARD_BIN="$ROOT_DIR/llama.cpp/build/bin"
MODEL_DIR="$ROOT_DIR/models/UD-Q4_K_XL"
LOG_DIR="$ROOT_DIR/logs"
RESULTS="$LOG_DIR/bench-turboquant-results.txt"

# Find model
MODEL=$(find "$MODEL_DIR" -name '*-00001-of-*.gguf' -print -quit 2>/dev/null || true)
if [[ -z "$MODEL" ]]; then
    echo "Error: No GGUF shards found in $MODEL_DIR"
    exit 1
fi

# Download wikitext-2 test data if not present
WIKITEXT="$ROOT_DIR/models/wikitext-2-raw-test.txt"
if [[ ! -f "$WIKITEXT" ]]; then
    echo "Downloading wikitext-2 test set..."
    curl -sL "https://huggingface.co/datasets/ggml-org/ci/resolve/main/wikitext-2-raw-test.txt" \
        -o "$WIKITEXT"
fi

mkdir -p "$LOG_DIR"

# Common args
COMMON_ARGS="--model $MODEL --gpu-layers 999 --threads 24 --flash-attn on"
# Short context for perplexity (fast iteration)
PPL_ARGS="--ctx-size 512 --ppl-stride 0 --chunks 8"

echo "══════════════════════════════════════════════════════" | tee "$RESULTS"
echo "  TurboQuant KV Cache Benchmark" | tee -a "$RESULTS"
echo "  Model: $(basename "$MODEL")" | tee -a "$RESULTS"
echo "  Date:  $(date -Iseconds)" | tee -a "$RESULTS"
echo "══════════════════════════════════════════════════════" | tee -a "$RESULTS"
echo "" | tee -a "$RESULTS"

run_ppl() {
    local label="$1"
    local cache_k="$2"
    local cache_v="$3"
    local binary="$4"
    local extra="${5:-}"

    echo "── $label (K=$cache_k V=$cache_v) ──" | tee -a "$RESULTS"
    echo "Binary: $binary" | tee -a "$RESULTS"

    local cmd="$binary/llama-perplexity $COMMON_ARGS $PPL_ARGS \
        --cache-type-k $cache_k --cache-type-v $cache_v \
        -f $WIKITEXT $extra"

    echo "Command: $cmd" | tee -a "$RESULTS"
    echo "" | tee -a "$RESULTS"

    if eval "$cmd" 2>&1 | tee -a "$RESULTS"; then
        echo "" | tee -a "$RESULTS"
        echo "✓ $label completed" | tee -a "$RESULTS"
    else
        echo "" | tee -a "$RESULTS"
        echo "✗ $label FAILED" | tee -a "$RESULTS"
    fi
    echo "" | tee -a "$RESULTS"
    echo "────────────────────────────────────────────────" | tee -a "$RESULTS"
}

# 1. Baseline: q8_0 K+V (standard llama.cpp, current config)
run_ppl "Baseline q8_0" "q8_0" "q8_0" "$STANDARD_BIN"

# 2. Baseline: q4_0 K+V (standard llama.cpp, for comparison)
run_ppl "Baseline q4_0" "q4_0" "q4_0" "$STANDARD_BIN"

# 3. TurboQuant: turbo4 K+V (best quality, 3.8x compression)
run_ppl "TurboQuant turbo4" "turbo4" "turbo4" "$TURBO_BIN"

# 4. TurboQuant: turbo3 K+V (4.6x compression)
run_ppl "TurboQuant turbo3" "turbo3" "turbo3" "$TURBO_BIN"

# 5. Asymmetric safe: q8_0 K + turbo4 V
run_ppl "Asymmetric q8_0/turbo4" "q8_0" "turbo4" "$TURBO_BIN"

# 6. Asymmetric: q8_0 K + turbo3 V
run_ppl "Asymmetric q8_0/turbo3" "q8_0" "turbo3" "$TURBO_BIN"

echo "" | tee -a "$RESULTS"
echo "══════════════════════════════════════════════════════" | tee -a "$RESULTS"
echo "  Benchmark complete. Results: $RESULTS" | tee -a "$RESULTS"
echo "══════════════════════════════════════════════════════" | tee -a "$RESULTS"
