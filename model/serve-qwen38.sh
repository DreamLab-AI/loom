#!/usr/bin/env bash
set -euo pipefail

# ── Qwen3.8-27B (dense hybrid DeltaNet+Gated-Attention, vision, embedded MTP) ──
# The Loom stack's model engine — llama-server on :8085, the network default endpoint.
# Runs in the foreground so the container supervises it. All knobs are env vars
# (set in docker-compose.yml; override with `docker compose up -d -e ...` or compose env).
#
# Quant: unsloth UD-Q8_K_XL (31.5GB, maximal for 48GB with 262K ctx).
# MTP: the GGUF embeds the 1-layer MTP head (blk.64.nextn.*) — draft-mtp needs no -md.
# Vision: mmproj-BF16 (qwen3vl_merger projector).
# Hybrid attention (48/64 layers linear DeltaNet, 16 full-attention) keeps KV small:
# ~34KB/token at q8_0 -> ~8.5GB for the full native 262144 context.

MODEL_DIR="${MODEL_DIR:-/models/qwen3.8-27B}"
MAIN="${MAIN_GGUF:-$MODEL_DIR/Qwen3.8-27B-UD-Q8_K_XL.gguf}"
MMPROJ="${MMPROJ:-$MODEL_DIR/mmproj-BF16.gguf}"

HOST_ADDR="${HOST:-0.0.0.0}"
PORT="${PORT:-8085}"
ALIAS="${ALIAS:-qwen3.8-27B}"
CTX_SIZE="${CTX_SIZE:-262144}"          # native trained ctx — no YaRN needed
PARALLEL="${PARALLEL:-1}"
KV_TYPE="${KV_TYPE:-q8_0}"
KV_UNIFIED="${KV_UNIFIED:-1}"
SPLIT_MODE="${SPLIT_MODE:-layer}"       # NVLink inactive -> layer split, not row
MAIN_GPU="${MAIN_GPU:-0}"

# Qwen thinking-mode sampling (instruct-mode clients override per request:
# temp 0.7, top-p 0.80, presence_penalty 1.5).
TEMP="${TEMP:-1.0}"
TOP_P="${TOP_P:-0.95}"
TOP_K="${TOP_K:-20}"
MIN_P="${MIN_P:-0.0}"

# Speculative decoding via the embedded MTP head: mtp | off. n_max 1-6, hardware-
# dependent (tune with bench); 4 was the sweet spot for Gemma 4 MTP on these cards.
SPEC="${SPEC:-mtp}"
DRAFT_N_MAX="${DRAFT_N_MAX:-4}"

# Reasoning: thinking on by default (per-request reasoning_effort xhigh/medium/low/none
# via the chat template). Budget -1 = unrestricted; preserve keeps historical thinking
# blocks in context (Qwen's preserve_thinking — better KV reuse + consistency).
REASONING_BUDGET="${REASONING_BUDGET:--1}"
REASONING_PRESERVE="${REASONING_PRESERVE:-1}"

VISION="${VISION:-1}"

[[ -x /usr/local/bin/llama-server ]] || { echo "Error: llama-server missing" >&2; exit 1; }
[[ -f "$MAIN" ]] || { echo "Error: model GGUF not found: $MAIN (is the models volume mounted?)" >&2; exit 1; }

ARGS=(
    -m "$MAIN"
    --host "$HOST_ADDR" --port "$PORT"
    -a "$ALIAS"
    -ngl 99
    -sm "$SPLIT_MODE" -mg "$MAIN_GPU"
    -c "$CTX_SIZE"
    -ctk "$KV_TYPE" -ctv "$KV_TYPE"
    -fa on
    --parallel "$PARALLEL" --cont-batching
    --jinja --metrics --slots
    --temp "$TEMP" --top-p "$TOP_P" --top-k "$TOP_K" --min-p "$MIN_P"
    --reasoning-budget "$REASONING_BUDGET"
)
# Unified KV: all slots share one pool so a single request can use the full -c.
if [[ "$KV_UNIFIED" == "1" ]]; then
    ARGS+=(-kvu)
fi
if [[ "$REASONING_PRESERVE" == "1" ]]; then
    ARGS+=(--reasoning-preserve)
fi
if [[ "$SPEC" == "mtp" ]]; then
    ARGS+=(--spec-type draft-mtp --spec-draft-n-max "$DRAFT_N_MAX")
fi
if [[ "$VISION" == "1" && -f "$MMPROJ" ]]; then
    ARGS+=(--mmproj "$MMPROJ")
fi

echo "Qwen3.8-27B (UD-Q8_K_XL, vision, MTP spec-decode) — llama.cpp in Loom"
echo "  API: http://${HOST_ADDR}:${PORT}/v1 | ctx ${CTX_SIZE} | KV ${KV_TYPE} | spec ${SPEC} | vision ${VISION}"

exec llama-server "${ARGS[@]}"
