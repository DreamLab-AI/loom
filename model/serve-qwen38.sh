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

# Speculative decoding: mtp | dflash | off.
#  mtp    — embedded MTP head (blk.64.nextn.*); needs no -md. n_max swept on this
#           hardware 2026-08-15 (docs/research/evidence/mtp-sweep.txt): 3 is best
#           (29.9 tok/s greedy, 50% acceptance; 1.66x over no-spec 18.0).
#  dflash — DFlash2 block-diffusion drafter (llama.cpp PR #27342, merged 2026-08-27);
#           external draft GGUF via DRAFT_GGUF (official source: z-lab/Qwen3.8-27B-
#           DFlash2-GGUF — the incoai mirrors are known-broken per upstream). The
#           drafter targets base Qwen3.8-27B; finetunes (Heretic) share the tokenizer
#           so decoding stays lossless, acceptance just runs a little lower. This is
#           the only spec option for GGUFs whose MTP head was stripped (Heretic).
#           Do NOT combine with VISION=1 yet — image turns are broken/slow upstream
#           as of 2026-08-29 (M-RoPE draft positions, acceptance collapse).
SPEC="${SPEC:-mtp}"
DRAFT_N_MAX="${DRAFT_N_MAX:-3}"
DRAFT_GGUF="${DRAFT_GGUF:-/models/qwen3.8-27B/Qwen3.8-27B-DFlash2-Q4_K_M.gguf}"
# dflash n_max swept on this hardware 2026-08-29 (docs/research/evidence/
# dflash2-sweep.txt): 4 is best (38.9 tok/s greedy vs 19.4 no-spec = 2.0x,
# 44% acceptance); 3 is within noise, throughput falls monotonically above 4.
DFLASH_N_MAX="${DFLASH_N_MAX:-4}"

# Reasoning: thinking on by default. Server default effort is MEDIUM (2026-08-15 —
# xhigh can spiral to 17K-token traces on adversarial/incoherent prompts); clients
# override per request with chat_template_kwargs {"reasoning_effort": "xhigh|medium|low"}
# or disable with {"enable_thinking": false} ("none" is rejected by the template).
# Budget -1 = unrestricted; preserve keeps historical thinking blocks in context
# (Qwen's preserve_thinking — better KV reuse + consistency).
REASONING_EFFORT="${REASONING_EFFORT:-medium}"
REASONING_BUDGET="${REASONING_BUDGET:--1}"
REASONING_PRESERVE="${REASONING_PRESERVE:-1}"
# Thought-tag extraction: none | deepseek. Finetunes whose chat template llama.cpp
# does not recognise as reasoning-capable (e.g. qwen38-ara/Heretic — chat_format
# "Content-only") leak raw <think> blocks into message.content; "deepseek" parses
# <think>…</think> into message.reasoning_content instead (2026-08-18 cutover).
REASONING_FORMAT="${REASONING_FORMAT:-}"
# Override template for GGUFs shipped WITHOUT tokenizer.chat_template (the Heretic
# abliteration strips it — llama.cpp then falls back to generic ChatML and ALL
# reasoning controls dead-end). Point at the base Qwen3.8 template extracted from
# the reference GGUF (chat-template.jinja beside the weights).
CHAT_TEMPLATE_FILE="${CHAT_TEMPLATE_FILE:-}"

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
if [[ -n "$REASONING_EFFORT" && "$REASONING_EFFORT" != "default" ]]; then
    ARGS+=(--chat-template-kwargs "{\"reasoning_effort\":\"$REASONING_EFFORT\"}")
fi
if [[ -n "$REASONING_FORMAT" ]]; then
    ARGS+=(--reasoning-format "$REASONING_FORMAT")
fi
if [[ -n "$CHAT_TEMPLATE_FILE" ]]; then
    [[ -f "$CHAT_TEMPLATE_FILE" ]] || { echo "Error: CHAT_TEMPLATE_FILE not found: $CHAT_TEMPLATE_FILE" >&2; exit 1; }
    ARGS+=(--chat-template-file "$CHAT_TEMPLATE_FILE")
fi
# Unified KV: all slots share one pool so a single request can use the full -c.
if [[ "$KV_UNIFIED" == "1" ]]; then
    ARGS+=(-kvu)
fi
if [[ "$REASONING_PRESERVE" == "1" ]]; then
    ARGS+=(--reasoning-preserve)
fi
if [[ "$SPEC" == "mtp" ]]; then
    ARGS+=(--spec-type draft-mtp --spec-draft-n-max "$DRAFT_N_MAX")
elif [[ "$SPEC" == "dflash" ]]; then
    [[ -f "$DRAFT_GGUF" ]] || { echo "Error: DFlash2 draft GGUF not found: $DRAFT_GGUF" >&2; exit 1; }
    if [[ "$VISION" == "1" ]]; then
        echo "Error: SPEC=dflash with VISION=1 is broken upstream (2026-08-29) — use SPEC=mtp or VISION=0" >&2
        exit 1
    fi
    ARGS+=(-md "$DRAFT_GGUF" -ngld 99 --spec-type draft-dflash --spec-draft-n-max "$DFLASH_N_MAX")
fi
if [[ "$VISION" == "1" && -f "$MMPROJ" ]]; then
    ARGS+=(--mmproj "$MMPROJ")
fi

echo "Qwen3.8-27B — llama.cpp in Loom"
echo "  API: http://${HOST_ADDR}:${PORT}/v1 | ctx ${CTX_SIZE} | KV ${KV_TYPE} | spec ${SPEC} | vision ${VISION}"

exec llama-server "${ARGS[@]}"
