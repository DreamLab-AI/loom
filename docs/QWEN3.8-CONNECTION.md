# Qwen3.8-27B — LAN Connection & Integration Guide

> **Identity (2026-10-29):** the loaded weights are **Hemmingway-1.5 Q8_0**, GGUF sha256
> `8b3c689c88395942c3e3caf3dac1f1d70fe16f86f490bd783bf44afa32834b4e`, served with the
> Unsloth-fixed Qwen3.8 template (sha256 `12827f24…aaadce`, file
> `/models/hemmingway-1.5/chat-template.jinja`). The wire alias is `hemmingway-1.5-q8_0`.
> Both digests were checked on the HP at cutover; see the 2026-10-29 addendum below.
>
> **Identity qualification (2026-09-07, historical):** `qwen3.8-27B` is the compatibility wire alias.
> The documented weight variant was then **Heretic abliterated Qwen3.8-27B Q8_0**,
> which differs from the base model. The August deployment statement below is
> historical evidence; a `/v1/models` alias alone cannot attest the loaded GGUF.
> The façade provides ontology grounding, not proof of base-model refusal behaviour
> or a substitute for authorisation. Verify the loaded weight digest and serving
> template before asserting a new deployment identity.

**Replaces Muse Glimmer 30B as the network's default model** (cutover 2026-08-14), and the
model engine now runs **inside the Loom docker stack** (`loom-model` container) instead of a
host systemd unit. Alibaba's Qwen3.8-27B: dense 27B hybrid (Gated DeltaNet + Gated
Attention), native vision, Apache-2.0, served by llama.cpp over an OpenAI-compatible API.
Beats Muse Glimmer on every shared published benchmark (Terminal Bench 73.0 vs 51.7,
SWE-bench Pro 61.7 vs 51.2, OSWorld 84.3 vs 65.9, GPQA 89.2 vs 83.5, IFBench 79.5 vs 77.0).

> **Model-is-a-URL, unchanged by the Rust re-platform.** Under the Rust Loom façade
> (`ADR-137` / `PRD-027`) the model is still just `DISTILL_BACKEND_URL` and the `loom-model`
> container is unchanged — the re-platform swaps the *façade* substrate (stdlib Python → a Rust
> binary), not the model behind it. Everything in this guide holds verbatim; swap the model and
> no consumer changes.

> **Addendum (2026-10-29): the deployed model is now Hemmingway-1.5 Q8_0, with MTP on.**
> The `loom-model` container serves
> [`HemmingwayAI/Hemmingway-1.5`](https://huggingface.co/HemmingwayAI/Hemmingway-1.5), a
> Qwen3.8-27B finetune trained to write prose that reads as human, quantised by
> [`bartowski`](https://huggingface.co/bartowski/HemmingwayAI_Hemmingway-1.5-GGUF) at **Q8_0**.
> Same façade, port and serving script. Model id is now **`hemmingway-1.5-q8_0`**.
>
> - **Weights:** `/models/hemmingway-1.5/HemmingwayAI_Hemmingway-1.5-Q8_0.gguf`, 29,116,389,728
>   bytes, sha256 `8b3c689c88395942c3e3caf3dac1f1d70fe16f86f490bd783bf44afa32834b4e`
>   (matches the Hugging Face etag). Arch `qwen35`, 65 blocks; the MTP head (`blk.64.nextn.*`)
>   is embedded at Q8_0. Q8_0 is the largest quant that fits: bf16 is ~54 GB against 48 GB of VRAM.
> - **Fit:** full 262,144 context, q8_0 KV, layer split, vision on: 22.9 + 23.4 GB of 24.6 GB per card.
>   `-sm tensor` was not tried: it requires f16 KV, estimated (not measured) at ~17 GB for 262K
>   against ~8.5 GB at q8_0, which leaves no room beside 29 GB of weights.
> - **Chat template:** the Unsloth-fixed Qwen3.8 template is kept, copied beside the weights as
>   `chat-template.jinja`. The GGUF embeds the stock template, which raises on multiple system
>   messages and on the `developer` role.
> - **Vision:** Hemmingway was published text-only. The base `mmproj-BF16` loads (`n_embd` matches)
>   and answers a red-square probe correctly, so image input works but is smoke-tested only.
> - **MTP:** speculative decoding works and is now on (`SPEC=mtp`, `DRAFT_N_MAX=3`). Sweep on
>   2026-10-29 at deployed sampling (temp 1.0, top-p 0.95, top-k 20), thinking off,
>   `max_tokens` 700, median of 3 runs per prompt:
>
>   | Config | Prose tok/s | Technical tok/s | Draft acceptance |
>   |---|---|---|---|
>   | no spec | 16.7 | 16.7 | – |
>   | n=2 | 22.4 | 24.2 | 31% / 39% |
>   | **n=3** | 21.4 | **26.7** | 22% / 36% |
>   | n=4 | 19.7 | 23.4 | 17% / 25% |
>
>   A recheck after the final restart (median of 2) gave 21.1 prose and 24.9 technical tok/s at n=3.
>   Heretic with no speculation measured 16.5 tok/s on the same harness. Acceptance (22–36%) is
>   well below the 2026-08-15 base-model sweep (`docs/research/evidence/mtp-sweep.txt`, 58.8% at
>   n=2), whose sampling settings are not recorded, so the two are not directly comparable.
> - **Verified through the façade (`:8084`):** `/v1/models`, chat with `reasoning_content` split
>   out, `enable_thinking:false`, tool calling, an image turn with MTP on, and a grounded delegate
>   (`served_mode: delegated`, four corpus seeds injected, 22.4 tok/s, 97/393 drafts accepted).
> - **Old model names still work.** llama-server does not check the requested `model`, so clients
>   sending `qwen3.8-27b-heretic-q8_0` or `qwen3.8-27B` were served by Hemmingway on both `:8085` and
>   `:8084` (tested 2026-10-30). The response's `model` field now reads `hemmingway-1.5-q8_0`.
>
> **Before relying on it, note:**
>
> - **Licence is CC-BY-NC-4.0** (non-commercial). Qwen3.8 and the Heretic variant were Apache-2.0.
> - **Not abliterated.** Refusal behaviour reverts toward base Qwen3.8; the Heretic variant's
>   uncensoring does not carry over.
> - **Voice.** Prose aims to read as human, which can include an invented first-person persona
>   (one grounded test opened with "I'm still learning to type"). Consumers that need a neutral
>   register should set a system prompt.
> - **Training-data leakage on bare prompts.** With no system prompt and thinking off, "Say ok."
>   sent straight to `:8085` returned an invented email signed "Suzanne" about "Enron UK's bank
>   account", which suggests the Enron email corpus is in the finetune data. The same prompt through
>   the façade answered "ok".
> - **Grounding uplift is unmeasured on this model.** The repository's grounding results were
>   measured on Qwen3.8; none have been re-run on Hemmingway.
>
> **Rollback:** in `deploy/compose.model-vision.yml` set
> `MAIN_GGUF=/models/qwen3.8-27B-heretic/RVN-Q8_0.gguf`,
> `CHAT_TEMPLATE_FILE=/models/qwen3.8-27B-heretic/chat-template.jinja`,
> `ALIAS=qwen3.8-27b-heretic-q8_0`, `SPEC=off`, then recreate the `model` service. Those weights
> remain on disk.

> **Addendum (2026-08-18): the deployed variant is now Heretic abliterated Qwen3.8-27B Q8_0.**
> The `loom-model` container serves
> [`0bserverx/Qwen3.8-27B-Heretic-Abliterated-Uncensored-GGUF`](https://huggingface.co/0bserverx/Qwen3.8-27B-Heretic-Abliterated-Uncensored-GGUF)
> at Q8_0 on `:8085` (262 K context, q8_0 KV, ~19.5 tok/s), the same façade and port as before.
> The base unsloth `UD-Q8_K_XL` weights below are retained on disk as rollback. Model id and every
> route in this guide are unchanged. Two serving knobs were needed and added to the serve script
> (`model/serve-qwen38.sh`, commit `5a4d24e`):
>
> - The Heretic GGUF **ships with no embedded chat template**, so llama.cpp fell back to generic
>   ChatML and leaked raw `<think>` blocks into `message.content` (and every reasoning control
>   dead-ended). The fix serves it with `--chat-template-file` pointed at the base Qwen3.8
>   template (`CHAT_TEMPLATE_FILE` knob) so the template and its reasoning controls are live again.
> - `--reasoning-format deepseek` (`REASONING_FORMAT` knob) parses `<think>…</think>` into
>   `message.reasoning_content` instead of leaking it into `.content`. With both knobs set,
>   `reasoning_content` is populated separately and `enable_thinking:false` is honoured, exactly
>   as §3 describes. Both knobs default empty (base weights need neither); the compose env sets
>   them for the Heretic variant.

- **Model:** `Qwen/Qwen3.8-27B` — 27B dense, 64 layers (48 linear-attention DeltaNet +
  16 full-attention), vision encoder, MTP head embedded.
- **Quant:** deployed model **Hemmingway-1.5 Q8_0** (2026-10-29 addendum); previously **Heretic abliterated Q8_0**; base
  unsloth **`UD-Q8_K_XL`** (31.5 GB, Unsloth Dynamic v3, maximal for 48 GB with full context)
  retained as rollback. Vision: `mmproj-BF16`.
- **Backend:** llama.cpp `llama-server` (release b11982, commit `8a199ad3`, CUDA sm_75; image `loom-model:b11982`, rollback `loom-model:local` = `cc83d7b4`) in the
  `loom-model` container (`~/githubs/loom/docker-compose.yml`).
- **Hardware:** 2× Quadro RTX 6000 (Turing, 24 GB each), layer-split.
- **Capabilities:** text + **vision** (images/video frames), **tool calling**,
  **reasoning with per-request effort control**, **262 K native context** (no YaRN),
  **MTP speculative decoding** (embedded head, server-side, transparent).

---

## 1. Connection

| Field        | Value                                             |
|--------------|---------------------------------------------------|
| Host (LAN)   | `10.10.10.1` (25 G fibre link)                    |
| Port         | `8085` (unchanged from Muse — clients keep working) |
| Base URL     | `http://10.10.10.1:8085/v1`                       |
| Chat route   | `POST /v1/chat/completions`                       |
| Models route | `GET /v1/models`  → id `hemmingway-1.5-q8_0` (2026-10-29) |
| Auth         | none                                              |
| Health       | `GET http://10.10.10.1:8085/health`               |
| Web UI       | `http://10.10.10.1:8085/`  (llama.cpp built-in)   |
| Loom façade  | `http://10.10.10.1:8084/v1` (ontology-grounded proxy, unchanged) |

---

## 2. Sampling (Qwen official recommendations)

| Mode | temperature | top_p | top_k | min_p | presence_penalty |
|------|-------------|-------|-------|-------|------------------|
| Thinking (default) | 1.0 | 0.95 | 20 | 0.0 | 0.0 |
| Instruct (thinking off) | 0.7 | 0.80 | 20 | 0.0 | 1.5 |

Server defaults are the thinking-mode set; override per request for instruct mode.

```bash
curl http://10.10.10.1:8085/v1/chat/completions -H 'Content-Type: application/json' -d '{
  "model":"qwen3.8-27B",
  "messages":[{"role":"user","content":"Explain entropy simply."}],
  "max_tokens": 1024 }'
```

```python
from openai import OpenAI
c = OpenAI(base_url="http://10.10.10.1:8085/v1", api_key="not-needed")
r = c.chat.completions.create(model="qwen3.8-27B",
    messages=[{"role":"user","content":"Explain entropy simply."}])
print(r.choices[0].message.content)          # clean answer
print(r.choices[0].message.reasoning_content) # chain-of-thought (if any)
```

**Vision** — standard OpenAI multimodal content parts (mmproj is loaded server-side):
```jsonc
{"messages":[{"role":"user","content":[
  {"type":"text","text":"What is in this image?"},
  {"type":"image_url","image_url":{"url":"data:image/png;base64,<BASE64>"}}]}]}
```

**Tool calling** — standard OpenAI `tools` / `tool_choice` → `tool_calls`.

---

## 3. Reasoning control

Thinking is **ON by default at effort `medium`** (server-side default since 2026-08-15 —
the model's own `xhigh` default can spiral to 17K-token traces on adversarial prompts;
request `xhigh` explicitly for hard tasks). Chain-of-thought arrives in
`message.reasoning_content`; the answer in `.content`.

- **Per-request effort** (`xhigh` | `medium` | `low` — verified live; `"none"` is REJECTED
  by the chat template):
  ```jsonc
  { "chat_template_kwargs": {"reasoning_effort": "low"}, ... }
  ```
- **Disable thinking per request** (verified live — then use instruct-mode sampling):
  ```jsonc
  { "chat_template_kwargs": {"enable_thinking": false}, ... }
  ```
- **Server-side budget:** `REASONING_BUDGET` env on the `model` service
  (`-1` unrestricted, `0` off globally).
- **preserve_thinking:** the server runs `--reasoning-preserve` — historical reasoning
  blocks are retained in context (Qwen recommends this; improves KV-cache reuse).
  Clients should still only *display* `.content`.
- **Output room:** give agentic/thinking requests generous `max_tokens` (Qwen suggests up
  to 262 K reasoning + 131 K final within long contexts; practically: 1024+ minimum).

---

## 4. Operational notes

- **Runs in docker:** `cd ~/githubs/loom && docker compose up -d` starts both `loom-model`
  (engine, :8085) and `loom` (façade, :8084). Logs: `docker logs -f loom-model`.
- **Verified at cutover (2026-08-14):** chat ✓, tool calling ✓, vision ✓ ("Red" on a red
  test image), `reasoning_effort` low/medium/xhigh ✓, `enable_thinking:false` ✓, MTP
  drafting active (~4 tok/draft) ✓, façade grounding against the new backend ✓ (683 tok
  injected, top_score 10.75). Throughput ~42 tok/s end-to-end short-prompt. VRAM 45.4/48 GB
  at full 262 K ctx — snug (GPU1 at 23.9/24.6); the server auto-fell-back from pipeline
  parallelism on load, which is expected and fine.
- **262 144 context is native** (no YaRN, unlike Muse's extended 256 K). Extension to 1 M
  is possible via YaRN but the KV cache would not fit alongside Q8 weights on 48 GB.
- **MTP speculative decoding** (`--spec-type draft-mtp`, embedded `blk.64.nextn.*` head)
  is transparent to clients. Tune `DRAFT_N_MAX` (1–6) via compose env.
- **Model store:** GGUFs live in `~/models/qwen3.8-27B/` (moved from the retired
  `llm-server` repo 2026-08-15), mounted read-only into the container at `/models`.
- **Legacy retired 2026-08-14:** `muse-glimmer.service`, `gemma4-31b.service`, and the
  host `ontology-proxy*` units are removed. Rollback = re-download nothing: Muse GGUFs
  are still on disk in `~/models/`; the legacy serve scripts, env configs, and the
  muse-glimmer unit file are preserved in this repo under `legacy/`.

---

_Server: `loom-model` container (llama.cpp b11982 `8a199ad3`, CUDA sm_75) · model
`bartowski/HemmingwayAI_Hemmingway-1.5-GGUF:Q8_0` + Qwen3.8 `mmproj-BF16` · host `10.10.10.1:8085`._
