# Gemma 4 Hybrid Setup (abliterated default + QAT-MTP fast)

> **RETIRED 2026-08-11.** the Gemma 4 hybrid setup no longer serves this network. The default model is now
> **Qwen3.8-27B** served by the loom docker stack (`~/githubs/loom`) on the same `:8085`
> endpoint. See `loom/docs/QWEN3.8-CONNECTION.md` and `loom/docs/REMOTE-CLIENT-SETUP.md`.
> This document is kept for historical reference only.


Overhaul of Gemma serving on this box, June 2026. Goal: run the community
**abliterated** Gemma 4 26B-A4B as the default uncensored model, AND have an
official **QAT + MTP** speculative-decoding endpoint for the speed-critical path.

## TL;DR — the one thing that surprised us

You **cannot** get "the abliterated model" *and* the cutting-edge QAT-MTP
speedups at the same time. The community MTP draft heads
(`boxwrench/gemma-4-qat-mtp-assistant-heads`) are matched to **Google's QAT
checkpoint**. Abliteration edits the weights (SVD removal of refusal
directions), so the heads no longer match, and the abliterated GGUF ships **no
MTP heads at all** → plain decode only. So we run **both**:

| Endpoint | Port | Model | Decode | Use for |
|---|---|---|---|---|
| **abliterated** (default) | 8082 | `gemma-4-26B-A4B-ara-abliterated` (APEX-IQ, ~21 GB) | plain | general / personal / uncensored |
| **QAT-MTP** (fast) | 8083 | unsloth `gemma-4-26B-A4B-it-qat` UD-Q4_K_XL (~14 GB) + boxwrench MTP head | MTP spec-decode | throughput, safety-aligned |
| qwen (untouched) | 8080 | `qwen3.6-35B-A3B-abliterix` | plain | **email-mcp-gateway** depends on this — left as-is |

## Launch

```bash
# Default uncensored endpoint (:8082). Does NOT touch qwen :8080.
scripts/serve-gemma4-abliterated.sh            # ctx 32768, both GPUs, q8_0 KV
scripts/serve-gemma4-abliterated.sh --gpu0 -c 8192   # co-reside test on GPU0

# Fast QAT + MTP endpoint (:8083), pinned to GPU0 by default.
scripts/serve-gemma4-qat-mtp.sh                # MTP on, turbo3 KV
scripts/serve-gemma4-qat-mtp.sh --baseline     # SPEC=off → A/B baseline (no MTP)
SPEC=off TEMP=0 scripts/serve-gemma4-qat-mtp.sh    # greedy baseline for accept-rate ceiling
```

Configs: `config/gemma4-abliterated.env`, `config/gemma4-qat-mtp.env`.

## Engine

The QAT-MTP endpoint uses a **separate** llama.cpp build so the stable
`llama.cpp/` that serves qwen :8080 is never disturbed:

- `llama.cpp/`      — stock build (commit b9151, May 14). Serves qwen + abliterated gemma.
- `llama.cpp-mtp/`  — AtomicBot-ai/atomic-llama-cpp-turboquant `feature/turboquant-kv-cache`.
  First-class Gemma 4 MTP + TurboQuant KV cache. CUDA arch 89.
  Flags: `--mtp-head <gguf> --spec-type mtp --draft-block-size 3 --draft-max 16`.
  **KV cache default is `q8_0`** (measured fastest + best tail quality here — see
  BENCHMARKS.md). `turbo3` is available for max compression but worse long-ctx tail;
  `q5_0` is a throughput trap on this hw (no fast FA kernel for gemma-4 head dims).

The boxwrench head was verified against the fork
(`verify-gemma4-assistant-gguf.py` → `ok ... embedding_length_kv=1024`).

## Hardware caveats vs the reddit post

- The post benchmarked **Strix Halo / Vulkan**; this box is **dual RTX 6000 Ada / CUDA**.
  Absolute tok/s differ (CUDA generally faster) and the post's
  `LLAMA_PIPELINE_DEPTH2=0` is a **Vulkan-only** deadlock workaround — not needed here.
- The post's QAT-matched-head acceptance numbers (e.g. 26B-A4B 91.8%) apply to the
  **official QAT** model. We use Unsloth UD-Q4_K_XL (85.6% top-1 vs 70.2% for naive
  q4_0) — same QAT family, so acceptance should hold. Measured CUDA numbers: see BENCHMARKS.md.

## PARALLEL=2 (deferred — needs a reviewed patch, not the reddit snippet)

The fork is at the **unfixed** line `gemma4-assistant.cpp:109`
(`ggml_reshape_3d(ctx0, Qcur, n_embd_head, n_head, n_tokens)`), so `--parallel 2`
will hit `GGML_ASSERT(ggml_nelements(a) == ne0*ne1*ne2)`. All our configs default
to `PARALLEL=1` and are unaffected.

The reddit fix changes `n_tokens` → `1`. We did **not** apply it: from the code
alone it's unclear whether the correct value is `1` or the number of active
sequences, and a wrong value would silently corrupt the 2nd slot's drafts rather
than crash. To enable PARALLEL=2, apply the **reviewed** fix from upstream
(Atomic fork PR #26 / stock llama.cpp PR #23398) and rebuild
`llama.cpp-mtp`, then validate both slots produce coherent output.

## VRAM (2× RTX 6000 Ada, 48 GB each)

qwen (:8080) + gpt-oss-safeguard (email gateway) already occupy most of one GPU's
worth. There is **not** enough VRAM to run qwen + abliterated-26B + QAT-26B all at
full context simultaneously. Practical pattern:
- Keep qwen :8080 up (email gateway).
- Run the QAT-MTP endpoint pinned to GPU0 (~17 GB, fits in spare) for the fast path.
- Treat the big abliterated 26B as a swap-in (free VRAM / stop qwen for full 262K ctx).

## Consumers — what changed and what didn't

- **openwebui (:3000)** — REWIRED. Recreated the container (data volume + WEBUI_SECRET_KEY
  preserved) with `OPENAI_API_BASE_URLS=:8082;:8084;:8083;:8080` → Aria (default), Qwen-27B
  dense, QAT-MTP, qwen-MoE. Each model appears only while its server is up; since VRAM can't
  hold them all, this is effectively a swap menu (e.g. stop Aria, start Qwen-27B to use it).
  A down endpoint just adds a brief probe timeout to the model-list refresh.
- **email-mcp-gateway (:8765)** — UNCHANGED (by request). Stays on qwen :8080
  (`QWEN_MODEL=APEX-Q5_K_M.gguf`); validated on qwen, egress sanitized by gpt-oss-safeguard.
- **gaussian / LichtFeld-Studio** — UNCHANGED, and already aligned on Gemma 4. Its pipeline
  uses an `agent-vlm:8080` service running the **official** `gemma-4-26B-A4B-it` Q8_0 + vision
  (`src/pipeline/config.py:43`, `endpoints.py:40`, `sota_registry.py:146`) as a transient VLM
  for 3D-artifact analysis. Not repointed to the abliterated model on purpose: that task needs
  **vision** (which disables MTP) and uncensored output is irrelevant for render analysis — the
  official VLM variant is correct here.
- **ComfyUI / vitrine** vision — UNCHANGED, keeps the official
  `comfyui-models-staging/gguf/gemma-4-26b-a4b-it` Q8_0 (load-bearing).

## Cleanup done

- Removed ~62 GB of stale Docker (4.4 GB stopped containers + 57.6 GB dangling images).
- The 20 KB orphaned `google/gemma-3-27b-it` HF cache stub is root-owned; left in place
  (needs sudo, not worth it).
