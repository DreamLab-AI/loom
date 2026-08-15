# Muse Glimmer 30B — LAN Connection & Integration Guide

> **RETIRED 2026-08-14.** Muse Glimmer 30B no longer serves this network. The default model is now
> **Qwen3.8-27B** served by the loom docker stack (`~/githubs/loom`) on the same `:8085`
> endpoint. See `loom/docs/QWEN3.8-CONNECTION.md` and `loom/docs/REMOTE-CLIENT-SETUP.md`.
> This document is kept for historical reference only.


**Replaces Gemma 4 31B as the network's default model** (cutover 2026-08-11). Meta
Superintelligence Labs' 30B dense agentic + vision model, Apache-2.0, served by mainline
llama.cpp over an OpenAI-compatible HTTP API. Standard `max_tokens`, `temperature`,
streaming, tool calling, and concurrent requests all behave as expected.

- **Model:** `meta-models/Muse-Glimmer-30B` — 30B dense, instruction-tuned, agentic.
- **Quant:** **bartowski `Q8_0`** (near-lossless 8-bit). *(Unsloth's `UD-Q8_K_XL` had broken
  chat metadata — don't use it; bartowski's re-quant is correct.)*
- **Backend:** llama.cpp `llama-server` (mainline `ggml-org/llama.cpp`, CUDA sm_75).
- **Hardware:** 2× Quadro RTX 6000 (Turing, 24 GB each), layer-split.
- **Capabilities:** text + **vision** (images), **tool/function calling**, **reasoning** (on by
  default, controllable), **256 K context** (via YaRN), **DFlash** speculative decoding.

---

## 1. Connection

| Field        | Value                                             |
|--------------|---------------------------------------------------|
| Host (LAN)   | `10.10.10.1`  ⚠️ (25 G fibre link — **not** the old `192.168.2.48`) |
| Port         | `8085`                                            |
| Base URL     | `http://10.10.10.1:8085/v1`                       |
| Chat route   | `POST /v1/chat/completions`                       |
| Models route | `GET /v1/models`  → id `muse-glimmer-30B`         |
| Auth         | none                                              |
| Health       | `GET http://10.10.10.1:8085/health`               |
| Web UI       | `http://10.10.10.1:8085/`  (llama.cpp built-in)   |

> **IP note:** this host's only global address is `10.10.10.1` (interface `enp65s0f0np0`,
> the 25 G point-to-point fibre to `ml`). The old Gemma doc's `192.168.2.48` is stale.

---

## 2. Request schema (OpenAI Chat Completions)

```jsonc
{
  "model": "muse-glimmer-30B",
  "messages": [{"role": "user", "content": "Explain entropy simply."}],
  "max_tokens": 512,
  "temperature": 1.0,   // Meta default; DO NOT use 0/greedy — it causes repetition loops
  "top_p": 0.95,
  "top_k": 64,
  "stream": false
}
```

**Sampling: use `temperature=1.0, top_p=0.95, top_k=64`.** Greedy (temp 0) makes this model loop.

---

## 3. Examples

**cURL**
```bash
curl http://10.10.10.1:8085/v1/chat/completions -H 'Content-Type: application/json' -d '{
  "model":"muse-glimmer-30B",
  "messages":[{"role":"user","content":"Explain entropy simply."}],
  "temperature":1.0,"top_p":0.95,"top_k":64 }'
```

**Python (OpenAI SDK)**
```python
from openai import OpenAI
c = OpenAI(base_url="http://10.10.10.1:8085/v1", api_key="not-needed")
r = c.chat.completions.create(model="muse-glimmer-30B",
    messages=[{"role":"user","content":"Explain entropy simply."}],
    temperature=1.0, top_p=0.95, extra_body={"top_k":64})
print(r.choices[0].message.content)
```

**Vision** — OpenAI multimodal content parts (server has the mmproj loaded):
```jsonc
{"messages":[{"role":"user","content":[
  {"type":"text","text":"What is in this image?"},
  {"type":"image_url","image_url":{"url":"data:image/png;base64,<BASE64>"}}]}]}
```

**Tool / function calling** — standard OpenAI `tools` + `tool_choice`; returns `tool_calls`
with `finish_reason:"tool_calls"`. Verified working.

---

## 4. Reasoning mode

Thinking is **ON by default**. The chain-of-thought goes in
`choices[0].message.reasoning_content`; the clean answer in `.content` — **use `.content`**.

- **Control effort** with a system message: `"Reasoning strength: low|medium|high|xhigh"`
  (use `high`/`xhigh` for hard coding/agentic tasks).
- **Multi-turn:** do **not** send `reasoning_content` back — keep only `.content` in history.
- **Budget tokens:** with thinking on, too-small `max_tokens` can be consumed by reasoning,
  leaving `.content` empty with `finish_reason:"length"`. Give it room (512+).

---

## 5. Operational notes

- **One model, one endpoint.** `GET /v1/models` → `muse-glimmer-30B`. `GET /health` → `{"status":"ok"}`.
- **256 K context via YaRN** (`--rope-scaling yarn`, rope-scale 2). Native trained ctx is 131 K;
  the 256 K extension is *community-configured, not officially validated by Meta* — needle
  retrieval passes at 157 k tokens, but treat very-long-context quality as best-effort.
- **DFlash speculative decoding** is server-side and transparent (no request changes). ~1.55×
  decode speedup on this Turing hardware (~39 tok/s; ~905 tok/s prefill).
- **Replaces Gemma 4 31B.** Gemma's `:8084` service is stopped/disabled (the two 30B models
  can't co-reside on 48 GB). Fallback: `sudo systemctl stop muse-glimmer && sudo systemctl start gemma4-31b`.
- **Security note:** Muse Glimmer has weaker prompt-injection resistance than Gemma — add
  guardrails / human-in-the-loop for irreversible agentic actions.

---

_Server: `llama-server` (mainline llama.cpp, CUDA sm_75) · model
`bartowski/Muse-Glimmer-30B-GGUF:Q8_0` + official mmproj/dflash · host `10.10.10.1:8085`._
