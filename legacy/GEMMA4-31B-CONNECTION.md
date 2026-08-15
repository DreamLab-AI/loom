# Gemma 4 31B (dense) — LAN Connection & Integration Guide

> **RETIRED 2026-08-14.** Gemma 4 31B no longer serves this network. The default model is now
> **Qwen3.8-27B** served by the loom docker stack (`~/githubs/loom`) on the same `:8085`
> endpoint. See `loom/docs/QWEN3.8-CONNECTION.md` and `loom/docs/REMOTE-CLIENT-SETUP.md`.
> This document is kept for historical reference only.


Replaces the former **DiffusionGemma** service on this box. This is a **normal
autoregressive** chat model served by **mainline llama.cpp** over an
OpenAI-compatible HTTP API — so unlike DiffusionGemma, standard `max_tokens`,
`temperature`, streaming, and concurrent requests all behave the way you expect.

- **Model:** `unsloth/gemma-4-31B-it-qat` — Gemma 4, 31B **dense**, instruction-tuned.
- **Quant:** `UD-Q4_K_XL` (QAT — Quantization-Aware Trained, so 4-bit ≈ bfloat16 quality).
- **Backend:** llama.cpp `llama-server` (mainline `ggml-org/llama.cpp`, CUDA, sm_89).
- **Hardware:** 2× Quadro RTX 6000 (Turing, 24 GB each), model layer-split across both.
- **Capabilities:** text + **vision** (image input), **thinking/reasoning** mode (on
  by default), the model's full **256K** context (served at 256K), and **MTP
  speculative decoding** (active — ~25–75 tok/s depending on content; ~0.9 draft
  acceptance on predictable text).

---

## 1. Connection

| Field        | Value                                             |
|--------------|---------------------------------------------------|
| Host (LAN)   | `192.168.2.48`                                    |
| Port         | `8084`                                            |
| Base URL     | `http://192.168.2.48:8084/v1`                     |
| Chat route   | `POST /v1/chat/completions`                       |
| Models route | `GET /v1/models`                                  |
| Auth         | none (no API key — send any/no `Authorization`)   |
| Model id     | `gemma-4-31B-it-qat`                              |
| Protocol     | HTTP/1.1 JSON, OpenAI Chat Completions shape      |
| Health       | `GET http://192.168.2.48:8084/health`             |
| Web UI       | `http://192.168.2.48:8084/`  (llama.cpp built-in) |

No firewall change needed — TCP 8084 is already open on this host.

---

## 2. What's different from a hosted model (and from the old DiffusionGemma)

- **It's a standard causal LM.** Left-to-right token generation, real per-token
  streaming, first-token latency is low. `max_tokens` truncates like you expect.
- **Sampling is honored.** `temperature`, `top_p`, `top_k`, penalties all work.
  Google/Unsloth recommended defaults for Gemma 4: **`temperature=1.0`,
  `top_p=0.95`, `top_k=64`**. If you don't pass them the server defaults are used.
- **Concurrency is fine.** llama.cpp serves multiple slots; you may issue
  parallel requests (they share the KV budget). No single-request lock like the
  old diffusion wrapper.
- **Full 256K context is served** (the model's native maximum, `n_ctx_slot=262144`,
  unified KV so a single request can use all of it). Keep prompt+answer under 256K
  or you'll get a context-overflow error.
- **QAT quality:** the 4-bit weights are quantization-aware-trained, so quality
  tracks bf16 closely — this is the intended way to run the model, not a compromise.

---

## 3. Request schema (OpenAI Chat Completions)

```jsonc
{
  "model": "gemma-4-31B-it-qat",          // optional; only one model is loaded
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user",   "content": "Explain entropy simply."}
  ],
  "max_tokens": 512,        // optional, standard behaviour
  "temperature": 1.0,       // recommended
  "top_p": 0.95,            // recommended
  "top_k": 64,              // recommended (llama.cpp extension)
  "stream": false           // set true for SSE token streaming
}
```

Streaming (`"stream": true`) returns OpenAI `chat.completion.chunk` SSE events,
per **token**, terminated by `data: [DONE]`.

---

## 4. Calling examples

**cURL:**
```bash
curl http://192.168.2.48:8084/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "gemma-4-31B-it-qat",
    "messages": [{"role":"user","content":"Explain entropy simply."}],
    "temperature": 1.0, "top_p": 0.95, "top_k": 64
  }'
```

**Python (OpenAI SDK — just repoint `base_url`):**
```python
from openai import OpenAI
client = OpenAI(base_url="http://192.168.2.48:8084/v1", api_key="not-needed")
r = client.chat.completions.create(
    model="gemma-4-31B-it-qat",
    messages=[{"role": "user", "content": "Explain entropy simply."}],
    temperature=1.0, top_p=0.95, extra_body={"top_k": 64},
)
print(r.choices[0].message.content)
```

**Vision (image input)** — OpenAI multimodal content parts are supported because
the server is started with the vision projector (`mmproj`):
```bash
curl http://192.168.2.48:8084/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "gemma-4-31B-it-qat",
    "messages": [{"role":"user","content":[
      {"type":"text","text":"What is in this image?"},
      {"type":"image_url","image_url":{"url":"data:image/png;base64,<BASE64>"}}
    ]}]
  }'
```

---

## 5. Thinking / reasoning mode

Gemma 4 reasons before answering, and **thinking is ON by default.**

- The server splits the output: the chain-of-thought goes in
  `choices[0].message.reasoning_content`, and the clean final answer in
  `choices[0].message.content`. **Use `content` as the answer**; `reasoning_content`
  is optional context.
- **Multi-turn: do NOT send `reasoning_content` (thought blocks) back** in later
  requests — keep only the final `content` in conversation history. Feeding prior
  thoughts back degrades the model (official Gemma 4 guidance).
- **Budget enough tokens.** With thinking on, a too-small `max_tokens` can be
  consumed entirely by reasoning, leaving `content` empty and `finish_reason:"length"`.
  Give it room (e.g. 512+), or disable thinking for terse/tool replies.
- **Disable it:** pass `"chat_template_kwargs": {"enable_thinking": false}` in the
  request body — you then get the answer directly in `content` with no
  `reasoning_content`.

```jsonc
// terse, no reasoning:
{ "messages": [...], "chat_template_kwargs": {"enable_thinking": false} }
```

---

## 6. Operational notes

- **One model, one endpoint.** `GET /v1/models` reports `gemma-4-31B-it-qat`.
- **Health/liveness:** `GET /health` returns `{"status":"ok"}` when ready.
- **Full 256K is available**, using q8_0 KV cache (near-lossless) to fit the model's
  max context across the two 24 GB cards. Requests beyond 256K are rejected.
- **Speculative decoding** (MTP draft) is a server-side speed optimization and is
  transparent to clients — no request changes needed.
- **This replaces DiffusionGemma.** If you previously hit `:8084` expecting the
  diffusion model (256-token canvas / `n_blocks` knobs), those parameters no longer
  apply — this is a standard chat model now.

---

_Server: `llama-server` (mainline llama.cpp, CUDA) · model
`unsloth/gemma-4-31B-it-qat-GGUF:UD-Q4_K_XL` · host `HP-Desktop` (192.168.2.48)._
