# Remote Client Setup — connecting LAN machines to the Loom stack

How to point any machine on the LAN at the network's model endpoint (Qwen3.8-27B in the
Loom docker stack). No auth, no client-side install beyond an OpenAI-compatible library.
Full model reference: [`QWEN3.8-CONNECTION.md`](QWEN3.8-CONNECTION.md).

## 1. Pick your endpoint

| Endpoint | Use when |
|---|---|
| `http://10.10.10.1:8085/v1` | **Default.** Raw model API (OpenAI-compatible): chat, tools, vision, streaming. |
| `http://10.10.10.1:8084/v1` | **Ontology-grounded.** Same API, but the Loom façade injects knowledge-graph context for on-ontology queries (responses carry a `loom` provenance block). Prefer for narrativegoldmine-domain work. |

Model id (both): **`qwen3.8-27B`**. Health checks: `GET /health` on either port.

```bash
# smoke test from any LAN machine
curl -s http://10.10.10.1:8085/health
curl -s http://10.10.10.1:8085/v1/chat/completions -H 'Content-Type: application/json' \
  -d '{"model":"qwen3.8-27B","messages":[{"role":"user","content":"ping"}],"max_tokens":32}'
```

## 2. Environment-variable wiring (most OpenAI-compatible apps)

Many tools (SDKs, aider, LiteLLM, shell-gpt, etc.) honour these:

```bash
export OPENAI_BASE_URL="http://10.10.10.1:8085/v1"   # some older tools use OPENAI_API_BASE
export OPENAI_API_KEY="not-needed"                    # must be non-empty; value is ignored
export OPENAI_MODEL="qwen3.8-27B"                     # tool-specific; often set in the tool's own config
```

## 3. SDK snippets

**Python** (`pip install openai`):
```python
from openai import OpenAI
c = OpenAI(base_url="http://10.10.10.1:8085/v1", api_key="not-needed", timeout=600)
r = c.chat.completions.create(model="qwen3.8-27B",
    messages=[{"role": "user", "content": "Explain entropy simply."}])
print(r.choices[0].message.content)
```

**Node** (`npm install openai`):
```js
import OpenAI from "openai";
const c = new OpenAI({ baseURL: "http://10.10.10.1:8085/v1", apiKey: "not-needed", timeout: 600000 });
const r = await c.chat.completions.create({
  model: "qwen3.8-27B",
  messages: [{ role: "user", content: "Explain entropy simply." }],
});
console.log(r.choices[0].message.content);
```

Streaming (`"stream": true`), tool calling (`tools`/`tool_choice`), and OpenAI multimodal
image parts all work as standard. For vision, send `image_url` content parts (base64 data
URLs fine); the mmproj is loaded server-side.

## 4. Reasoning control (important)

Server default is **thinking ON at `medium` effort**. Reasoning arrives separately in
`message.reasoning_content` — display/keep only `.content`.

```jsonc
// harder task → deeper thinking
{ "chat_template_kwargs": {"reasoning_effort": "xhigh"}, ... }   // xhigh | medium | low

// fast instruct mode (no thinking) → also switch sampling
{ "chat_template_kwargs": {"enable_thinking": false},
  "temperature": 0.7, "top_p": 0.8, "presence_penalty": 1.5, ... }
```

Notes:
- `reasoning_effort: "none"` is **rejected** — use `enable_thinking: false`.
- With thinking on, give it room: `max_tokens` 1024+ (reasoning consumes the budget first).
- Set client **timeouts ≥ 600 s** for xhigh/agentic calls; thinking can be long.
- Sampling defaults (thinking mode: temp 1.0, top-p 0.95, top-k 20) are set server-side;
  don't override unless you're switching to instruct mode.

## 5. Common clients

- **Open WebUI**: Admin → Connections → OpenAI API → URL `http://10.10.10.1:8085/v1`,
  key `not-needed`. Model appears as `qwen3.8-27B`. (A compose lives in
  `llm-server/openwebui-docker/` on the host.)
- **aider**: `aider --openai-api-base http://10.10.10.1:8085/v1 --openai-api-key not-needed --model openai/qwen3.8-27B`
- **LiteLLM / proxies**: provider `openai`, `api_base` as above.
- **Anything speaking "Ollama"**: not exposed; use the OpenAI-compatible route above.

## 6. Operational notes for remote consumers

- **One model, one GPU box**: single-stream serving (`--parallel 1`, unified KV). Long
  requests queue behind each other — don't fan out concurrent bulk jobs; batch politely.
- Context: 262 K tokens shared budget (prompt + reasoning + response).
- Throughput: ~30 tok/s decode, ~800 tok/s prefill (MTP speculative decoding is
  server-side and transparent).
- The endpoint host is `10.10.10.1` (25 G fibre). If you're on a different segment and
  can't reach it, route via the `ml` host or ask for a port forward.
- Provenance: responses via `:8084` include a `loom` block (grounding scores, corpus
  generation id) — log it if you need auditable grounding.
