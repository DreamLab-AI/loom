#!/usr/bin/env bash
set -euo pipefail

# ── Gemma 4 26B-A4B-it — official QAT + MTP speculative decoding ("FAST") ──
# Safety-aligned (NOT abliterated). Reproduces the community MTP speedups using
# Google QAT weights (Unsloth UD-Q4_K_XL) + boxwrench QAT-matched assistant head,
# on the Atomic TurboQuant fork. See config/gemma4-qat-mtp.env.
#
# Safe by default: serves on :8083, pinned to GPU0, does NOT touch qwen :8080.
# A/B: SPEC=off ./serve-gemma4-qat-mtp.sh   → baseline (no speculative decode)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
FORK="$ROOT_DIR/llama.cpp-mtp"
SERVER="${LLAMA_SERVER:-$FORK/build/bin/llama-server}"
MODEL_DIR="$ROOT_DIR/models/gemma-4-26B-A4B-it-qat"
LOG_DIR="$ROOT_DIR/logs"
HOST_ADDR="${HOST:-0.0.0.0}"

MAIN="${MAIN_GGUF:-$MODEL_DIR/gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf}"
DRAFT="${DRAFT_GGUF:-$MODEL_DIR/gemma-4-26B-A4B-it-qat-assistant-MTP-Q8_0.gguf}"
# Vision OFF by default: loading mmproj marks every prompt "multimodal", which
# DISABLES MTP ("skipping speculative prime for multimodal prompt"). Use --vision
# (or MMPROJ=<path>) only if you need images AND are willing to lose MTP speedup.
MMPROJ="${MMPROJ:-}"

SPEC="${SPEC:-mtp}"               # mtp | off
# q8_0 default — combines anbeeld's tail-KLD quality finding with measured throughput
# on THIS fork/CUDA (greedy, ctx 16384):
#   turbo3=188 t/s (75.5% acc, poor tail) · q8_0=184 (70.8%, excellent tail)
#   · f16=176 (72.8%) · q5_0=77 (!) (80%, but ~2.4x slower — no fast kernel here)
# q8_0 ≈ turbo3 speed with far better long-context fidelity. Use KV_TYPE=turbo3 only
# if you need extreme KV compression for very long context (accepting tail loss).
KV_TYPE="${KV_TYPE:-q8_0}"
CTX_SIZE="${CTX_SIZE:-16384}"
PORT="${PORT:-8083}"
PARALLEL="${PARALLEL:-1}"
CUDA_DEVICE="${CUDA_DEVICE:-0}"
TEMP="${TEMP:-}"                  # set TEMP=0 to measure max MTP acceptance (greedy)
DRAFT_BLOCK_SIZE="${DRAFT_BLOCK_SIZE:-3}"
DRAFT_MAX="${DRAFT_MAX:-16}"
DRAFT_MIN="${DRAFT_MIN:-0}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --baseline|--off) SPEC=off; shift ;;
        --q8)   KV_TYPE=q8_0; shift ;;
        --f16)  KV_TYPE=f16; shift ;;
        -c|--ctx) CTX_SIZE="$2"; shift 2 ;;
        -p|--port) PORT="$2"; shift 2 ;;
        --gpu) CUDA_DEVICE="$2"; shift 2 ;;
        --parallel) PARALLEL="$2"; shift 2 ;;
        --temp) TEMP="$2"; shift 2 ;;
        --no-vision) MMPROJ=""; shift ;;
        --vision)    MMPROJ="$MODEL_DIR/mmproj-BF16.gguf"; shift ;;
        -h|--help)
            echo "Usage: $(basename "$0") [--baseline] [--q8|--f16] [-c CTX] [-p PORT] [--gpu N] [--temp T]"
            echo "  Default: :8083, GPU0, ctx 16384, turbo3 KV, MTP on. Doesn't touch :8080."
            exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ ! -f "$SERVER" ]]; then
    echo "Error: MTP server not built yet: $SERVER" >&2
    echo "  Build: cmake -S $FORK -B $FORK/build -G Ninja -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=89 && cmake --build $FORK/build --target llama-server -j" >&2
    exit 1
fi
if [[ ! -f "$MAIN" ]]; then echo "Error: main GGUF not found: $MAIN" >&2; exit 1; fi

# Verify the MTP head matches the gemma4_assistant shape before serving.
if [[ "$SPEC" == "mtp" ]]; then
    if [[ ! -f "$DRAFT" ]]; then echo "Error: MTP head not found: $DRAFT" >&2; exit 1; fi
    if [[ -f "$FORK/scripts/verify-gemma4-assistant-gguf.py" ]]; then
        PYTHONPATH="$FORK/gguf-py" "$ROOT_DIR/.venv/bin/python3" \
            "$FORK/scripts/verify-gemma4-assistant-gguf.py" "$DRAFT" || {
            echo "Error: MTP head failed verification" >&2; exit 1; }
    fi
fi

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/gemma4-qat-mtp.log"

MMPROJ_ARGS=()
[[ -n "$MMPROJ" && -f "$MMPROJ" ]] && MMPROJ_ARGS=(--mmproj "$MMPROJ")

ARGS=(
    -m "$MAIN"
    "${MMPROJ_ARGS[@]}"
    --host "$HOST_ADDR" --port "$PORT"
    -c "$CTX_SIZE"
    -ngl 99 -ngld 99
    -ctk "$KV_TYPE" -ctv "$KV_TYPE" -ctkd "$KV_TYPE" -ctvd "$KV_TYPE"
    -fa on
    --parallel "$PARALLEL" -np "$PARALLEL"
    --cont-batching
    --jinja
    --metrics --slots
)
if [[ "$SPEC" == "mtp" ]]; then
    ARGS+=(--mtp-head "$DRAFT" --spec-type mtp
           --draft-block-size "$DRAFT_BLOCK_SIZE" --draft-max "$DRAFT_MAX" --draft-min "$DRAFT_MIN")
fi
[[ -n "$TEMP" ]] && ARGS+=(--temp "$TEMP")

echo "══════════════════════════════════════════════════════"
echo "  Gemma 4 26B-A4B-it — QAT + MTP (FAST, safety-aligned)"
echo "══════════════════════════════════════════════════════"
echo "  Engine:  $(basename "$FORK") (TurboQuant MTP fork)"
echo "  Main:    $(basename "$MAIN")"
echo "  Draft:   $([ "$SPEC" = mtp ] && basename "$DRAFT" || echo 'DISABLED (baseline)')"
echo "  KV:      ${KV_TYPE}   Ctx: ${CTX_SIZE}   GPU: ${CUDA_DEVICE}   Parallel: ${PARALLEL}"
echo "  API:     http://${HOST_ADDR}:${PORT}/v1   Log: ${LOG_FILE}"
echo "══════════════════════════════════════════════════════"

CUDA_VISIBLE_DEVICES="$CUDA_DEVICE" "$SERVER" "${ARGS[@]}" > "$LOG_FILE" 2>&1 &
PID=$!
echo "Server PID: $PID — waiting for load..."
for i in $(seq 1 90); do
    if curl -s "http://127.0.0.1:${PORT}/health" 2>/dev/null | grep -q ok; then
        echo "Ready after ${i}s — API at http://${HOST_ADDR}:${PORT}/v1"
        exit 0
    fi
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "Server died during load. Tail of $LOG_FILE:" >&2; tail -25 "$LOG_FILE" >&2; exit 1
    fi
    sleep 2
done
echo "Still loading after 180s — check $LOG_FILE"
