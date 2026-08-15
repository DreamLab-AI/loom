#!/usr/bin/env bash
set -euo pipefail

# ── Gemma 4 31B (dense) it — QAT UD-Q4_K_XL on MAINLINE llama.cpp ──
# Replaces the former DiffusionGemma service on :8084. Runs in the FOREGROUND
# (exec) so systemd can supervise it directly (see gemma4-31b.service). For a
# manual background run: `./serve-gemma4-31b.sh &` or use nohup.
#
# Config: config/gemma4-31b.env (override any var inline, e.g. PORT=8085 CUDA_DEVICE=0 ...)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

ENV_FILE="${ENV_FILE:-$ROOT_DIR/config/gemma4-31b.env}"
if [[ -f "$ENV_FILE" ]]; then set -a; source "$ENV_FILE"; set +a; fi

SERVER="${LLAMA_SERVER:-$ROOT_DIR/llama.cpp/build/bin/llama-server}"
MODEL_DIR="${MODEL_DIR:-$ROOT_DIR/models/gemma-4-31B-it-qat}"
MAIN="${MAIN_GGUF:-$MODEL_DIR/gemma-4-31B-it-qat-UD-Q4_K_XL.gguf}"
MMPROJ="${MMPROJ:-$MODEL_DIR/mmproj-BF16.gguf}"

HOST_ADDR="${HOST:-0.0.0.0}"
PORT="${PORT:-8084}"
CTX_SIZE="${CTX_SIZE:-131072}"
PARALLEL="${PARALLEL:-4}"
KV_TYPE="${KV_TYPE:-f16}"
CUDA_DEVICE="${CUDA_DEVICE:-0,1}"
SPLIT_MODE="${SPLIT_MODE:-layer}"
MAIN_GPU="${MAIN_GPU:-0}"
KV_UNIFIED="${KV_UNIFIED:-1}"
TEMP="${TEMP:-1.0}"
TOP_P="${TOP_P:-0.95}"
TOP_K="${TOP_K:-64}"
SPEC="${SPEC:-mtp}"
DRAFT="${DRAFT_GGUF:-$MODEL_DIR/mtp-gemma-4-31B-it.gguf}"
DRAFT_N_MAX="${DRAFT_N_MAX:-4}"

export PATH="/opt/cuda/bin:$PATH"

[[ -f "$SERVER" ]] || { echo "Error: llama-server not built: $SERVER" >&2; exit 1; }
[[ -f "$MAIN"   ]] || { echo "Error: model GGUF not found: $MAIN" >&2; exit 1; }

ARGS=(
    -m "$MAIN"
    --host "$HOST_ADDR" --port "$PORT"
    -ngl 99
    -sm "$SPLIT_MODE" -mg "$MAIN_GPU"
    -c "$CTX_SIZE"
    -ctk "$KV_TYPE" -ctv "$KV_TYPE"
    -fa on
    --parallel "$PARALLEL" --cont-batching
    --jinja --metrics --slots
    --temp "$TEMP" --top-p "$TOP_P" --top-k "$TOP_K"
    -a gemma-4-31B-it-qat
)
# Unified KV cache: let all server slots share one context pool, so a single request
# can use the full -c (otherwise -c is divided by --parallel → tiny per-request ctx).
if [[ "$KV_UNIFIED" == "1" ]]; then
    ARGS+=(-kvu)
fi
# Multi-Token-Prediction speculative decoding (needs --spec-type draft-mtp; the MTP
# head shares the main model's context, so a bare -md would fail on ctx_other).
if [[ "$SPEC" == "mtp" && -f "$DRAFT" ]]; then
    ARGS+=(-md "$DRAFT" --spec-type draft-mtp --spec-draft-n-max "$DRAFT_N_MAX")
fi
if [[ "${VISION:-1}" == "1" && -f "$MMPROJ" ]]; then
    ARGS+=(--mmproj "$MMPROJ")
fi

echo "Gemma 4 31B (dense, QAT UD-Q4_K_XL) — mainline llama.cpp"
echo "  API: http://${HOST_ADDR}:${PORT}/v1  | GPU ${CUDA_DEVICE} | ctx ${CTX_SIZE} | KV ${KV_TYPE} | vision ${VISION:-1}"

exec env CUDA_VISIBLE_DEVICES="$CUDA_DEVICE" "$SERVER" "${ARGS[@]}"
