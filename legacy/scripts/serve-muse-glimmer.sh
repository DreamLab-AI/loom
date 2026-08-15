#!/usr/bin/env bash
set -euo pipefail

# ── Muse Glimmer 30B (dense + vision) — Unsloth UD-Q8_K_XL on MAINLINE llama.cpp ──
# Agentic model (Meta Superintelligence Labs, Apache-2.0) on :8085, alongside Gemma on :8084.
# Runs in the FOREGROUND (exec) so systemd can supervise it (see muse-glimmer.service).
# Manual background run: `./serve-muse-glimmer.sh &` or nohup.
#
# Config: config/muse-glimmer.env (override any var inline, e.g. CTX_SIZE=262144 ./serve-muse-glimmer.sh)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

ENV_FILE="${ENV_FILE:-$ROOT_DIR/config/muse-glimmer.env}"
if [[ -f "$ENV_FILE" ]]; then set -a; source "$ENV_FILE"; set +a; fi

SERVER="${LLAMA_SERVER:-$ROOT_DIR/llama.cpp/build/bin/llama-server}"
# NOTE: bartowski Q8_0 (~8-bit, near-lossless). The Unsloth UD-Q8_K_XL quant shipped broken
# tokenizer/chat metadata -> garbage chat + peg-native 500s (2026-08-10); bartowski's re-quant
# (built after the fixes) chats correctly. mmproj + dflash are the meta-models official files.
MODEL_DIR="${MODEL_DIR:-$ROOT_DIR/models/muse-glimmer-30B}"
MAIN="${MAIN_GGUF:-$MODEL_DIR/Muse-Glimmer-30B-Q8_0.gguf}"
MMPROJ="${MMPROJ:-$MODEL_DIR/mmproj-kquant.gguf}"
DRAFT="${DRAFT_GGUF:-$MODEL_DIR/dflash-kquant.gguf}"

HOST_ADDR="${HOST:-0.0.0.0}"
PORT="${PORT:-8085}"
CTX_SIZE="${CTX_SIZE:-131072}"
PARALLEL="${PARALLEL:-1}"
KV_TYPE="${KV_TYPE:-q8_0}"
CUDA_DEVICE="${CUDA_DEVICE:-0,1}"
SPLIT_MODE="${SPLIT_MODE:-layer}"
MAIN_GPU="${MAIN_GPU:-0}"
KV_UNIFIED="${KV_UNIFIED:-1}"
TEMP="${TEMP:-1.0}"
TOP_P="${TOP_P:-0.95}"
TOP_K="${TOP_K:-64}"
SPEC="${SPEC:-dflash}"
DRAFT_N_MAX="${DRAFT_N_MAX:-15}"
ROPE_SCALING="${ROPE_SCALING:-none}"
ROPE_SCALE="${ROPE_SCALE:-1}"
YARN_ORIG_CTX="${YARN_ORIG_CTX:-131072}"

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
    -a muse-glimmer-30B
)
# Unified KV cache: all server slots share one context pool, so a single request can use the
# full -c (otherwise -c is divided by --parallel -> tiny per-request ctx).
if [[ "$KV_UNIFIED" == "1" ]]; then
    ARGS+=(-kvu)
fi
# YaRN context extension past the GGUF's trained 131072 (EXPERIMENTAL — community-suggested,
# not officially validated; --override-kv lifts llama.cpp's clamp so -c can exceed n_ctx_train).
if [[ "$ROPE_SCALING" == "yarn" ]]; then
    ARGS+=(--rope-scaling yarn --rope-scale "$ROPE_SCALE" --yarn-orig-ctx "$YARN_ORIG_CTX"
           --override-kv muse-glimmer.context_length=int:"$CTX_SIZE")
fi
# DFlash block-diffusion speculative decoding. The MTP-style sidecar shares the target model's
# context; --spec-type draft-dflash selects the block-diffusion drafter (block size 16 -> n_max<=15).
if [[ "$SPEC" == "dflash" && -f "$DRAFT" ]]; then
    ARGS+=(-md "$DRAFT" --spec-type draft-dflash --spec-draft-n-max "$DRAFT_N_MAX")
fi
if [[ "${VISION:-1}" == "1" && -f "$MMPROJ" ]]; then
    ARGS+=(--mmproj "$MMPROJ")
fi

echo "Muse Glimmer 30B (dense + vision, Unsloth UD-Q8_K_XL) — mainline llama.cpp"
echo "  API: http://${HOST_ADDR}:${PORT}/v1  | GPU ${CUDA_DEVICE} | ctx ${CTX_SIZE} | KV ${KV_TYPE} | spec ${SPEC} | vision ${VISION:-1}"

exec env CUDA_VISIBLE_DEVICES="$CUDA_DEVICE" "$SERVER" "${ARGS[@]}"
