# Qwen3.8-27B — LAN Connection & Integration Guide

**Replaces Muse Glimmer 30B as the network's default model** (cutover 2026-08-14), and the
model engine now runs **inside the Loom docker stack** (`loom-model` container) instead of a
host systemd unit. Alibaba's Qwen3.8-27B: dense 27B hybrid (Gated DeltaNet + Gated
Attention), native vision, Apache-2.0, served by llama.cpp over an OpenAI-compatible API.
Beats Muse Glimmer on every shared published benchmark (Terminal Bench 73.0 vs 51.7,
SWE-bench Pro 61.7 vs 51.2, OSWorld 84.3 vs 65.9, GPQA 89.2 vs 83.5, IFBench 79.5 vs 77.0).

- **Model:** `Qwen/Qwen3.8-27B` — 27B dense, 64 layers (48 linear-attention DeltaNet +
  16 full-attention), vision encoder, MTP head embedded.
- **Quant:** unsloth **`UD-Q8_K_XL`** (31.5 GB, Unsloth Dynamic v3 — maximal for 48 GB
  with full context). Vision: `mmproj-BF16`.
- **Backend:** llama.cpp `llama-server` (pinned commit `030ebb55`, CUDA sm_75) in the
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
| Models route | `GET /v1/models`  → id `qwen3.8-27B`              |
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

Thinking is **ON by default** at effort `xhigh`. Chain-of-thought arrives in
`message.reasoning_content`; the answer in `.content`.

- **Per-request effort** (`xhigh` | `medium` | `low` | `none`):
  ```jsonc
  { "chat_template_kwargs": {"reasoning_effort": "low"}, ... }
  ```
  `"none"` disables thinking for that request (then use instruct-mode sampling).
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
- **262 144 context is native** (no YaRN, unlike Muse's extended 256 K). Extension to 1 M
  is possible via YaRN but the KV cache would not fit alongside Q8 weights on 48 GB.
- **MTP speculative decoding** (`--spec-type draft-mtp`, embedded `blk.64.nextn.*` head)
  is transparent to clients. Tune `DRAFT_N_MAX` (1–6) via compose env.
- **Model store:** GGUFs live in `~/githubs/llm-server/models/qwen3.8-27B/`, mounted
  read-only into the container at `/models`.
- **Legacy retired 2026-08-14:** `muse-glimmer.service`, `gemma4-31b.service`, and the
  host `ontology-proxy*` units are removed. Rollback = re-download nothing: Muse GGUFs
  are still on disk; recreate a systemd unit from git history of `llm-server` if needed,
  or run its serve script manually.

---

_Server: `loom-model` container (llama.cpp `030ebb55`, CUDA sm_75) · model
`unsloth/Qwen3.8-27B-GGUF:UD-Q8_K_XL` + `mmproj-BF16` · host `10.10.10.1:8085`._
