# DiffusionGemma — Docker-managed inference service

Containerized, self-recovering replacement for the old unsupervised
`diffusiongemma-serve.sh` host process. Same OpenAI-compatible endpoint on `:8084`, but now
Docker manages the lifecycle and a supervisor + management API recover the model backend
automatically instead of leaving `/v1/models` up while completions 500 (`backend not running`).

## Why this exists

The diffusion fork's `llama-diffusion-gemma-visual-server` isn't a network server — it speaks a
line protocol over stdin/stdout. The Python front-end (`dg_server.py`) owns it as a child and
exposes HTTP. Previously, if that child crashed, nothing restarted it. Now recovery is layered:

1. **Supervisor thread** — respawns the backend within seconds of a crash, with exponential backoff.
2. **Management API** — `POST /admin/restart` forces a clean reload on demand.
3. **Docker** — `restart: unless-stopped` + a healthcheck that probes the *backend child* (not just
   the HTTP shell) recycle the whole container if the process itself wedges.

## Layout

```
dg_server.py                 self-recovering OpenAI front-end + management API (stdlib only)
Dockerfile                   multi-stage: build the fork for sm_89 (CUDA-devel) -> slim CUDA-runtime
docker-compose.yml           service on the `visionflow_runtime` network, GPU0, model bind-mounted
llama-diffgemma-src.tar.gz   build input: `git archive` of the fork (gitignored; regenerate as below)
.env.example                 DG_ADMIN_TOKEN (optional admin auth)
```

The 26 GB GGUF is **bind-mounted, not baked** (see compose `volumes:`). The image is built
**from source** (the diffusion fork is not in mainline llama.cpp as of 2026-06) so the binary's
glibc matches the runtime image — host artifacts (built on CachyOS glibc 2.43) won't run on
Ubuntu's 2.39.

## Build & run

```bash
# 1. (re)generate the source tarball from the fork — only when the fork changes
git -C ../llama.cpp-diffgemma archive --format=tar HEAD | gzip > llama-diffgemma-src.tar.gz

# 2. ensure the shared network exists
docker network create visionflow_runtime 2>/dev/null || true

# 3. build (sm_89, CUDA 13; ~15-25 min the first time)
docker compose build

# 4. start. NOTE: GPU0 fits ONE copy of the model (~41 GB of 48 GB), so stop the legacy host
#    process first or the model load will OOM / the port will be taken:
pkill -f diffusiongemma-lan-server     # retire the old host process
docker compose up -d

# 5. verify (allow ~1 min for the model to load)
curl -s localhost:8084/health | python3 -m json.tool
```

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| POST | `/v1/chat/completions` | OpenAI chat (stream + non-stream). `n_blocks`/`seed` knobs; `temp`/`top_p` ignored. |
| GET | `/v1/models` | OpenAI model discovery. |
| GET | `/` | live denoise UI. |
| POST | `/ui/generate` | SSE: per-frame/commit/stats (the UI feed). |
| GET | `/health` | liveness — **200 only when the backend child is alive**; reports state/restarts/uptime/pid. |
| GET | `/admin/status` | detailed status (token-gated). |
| POST | `/admin/restart` | force a clean backend reload (token-gated). |
| POST | `/admin/stop` / `/admin/start` | stop (and keep stopped) / start the backend (token-gated). |

Admin auth: set `DG_ADMIN_TOKEN` in `.env` to require `Authorization: Bearer <token>` on
`/admin/*`. If unset, those endpoints are open (a startup warning is logged).

```bash
# force a reload
curl -X POST localhost:8084/admin/restart -H "Authorization: Bearer $DG_ADMIN_TOKEN"
# watch recovery in action
docker compose logs -f diffusiongemma
```

## Performance

Built for native **sm_89** (Ada) and runs with **`GGML_CUDA_GRAPH_OPT=1`** (concurrent Q/K/V CUDA
streams + buffer reuse). Diffusion generation saturates the tensor cores (256-token canvas
denoised in parallel) rather than being memory-bandwidth bound like autoregressive decode.

**PR #24427 — evaluated 2026-06-20 and REJECTED (don't re-chase without new info).** It has more
CUDA kernels (fused sampling, device-side denoise loop) and is ~32% faster *per denoising step*
(0.053 vs 0.078 s/step here). But end-to-end it was ~2× SLOWER: this build's adaptive sampler
early-stops at ~15 steps while #24427 ran the full 48. It also **crashes on multi-GPU** (aborts in
`ggml_cuda_diffusion_sample_topk` unless `CUDA_VISIBLE_DEVICES` pins one GPU), is an unmerged draft
with merge conflicts, and exposes a different HTTP-server interface (would require rewriting
`dg_server.py`). Net: the current #24423 build (commit `9b4dae8`) is faster and stable.

Q8_0 remains the best 8-bit GGUF for this model — there is no UD-Q8_K_XL or abliterated *diffusion*
variant as of 2026-06.

## Reasoning ("thinking") rendering

DiffusionGemma emits its reasoning in a `<|channel>thought … <channel|>` block before the answer.
`dg_server.py` handles the two consumers differently, on purpose:

- **Streaming** (`stream:true`, what Open WebUI uses): reasoning is re-wrapped as a balanced
  **`<think>…</think>`** block in `delta.content`, then the answer follows. Open WebUI parses
  `<think>` tags into a collapsible "Thinking" block on every version (more reliable than the
  `reasoning_content` delta field, which has been buggy across releases). The split is *stateful*
  so the thinking never leaks into the answer mid-stream — the earlier per-commit `split_channels`
  bug dumped the raw `<|channel>thought` markup inline before the closer appeared.
- **Non-streaming** (`stream:false`, what the email gateway uses): clean answer in
  `message.content`, reasoning in a separate `message.reasoning_content` field (raw `<|channel>`
  markup stripped). **Do not change this** — the email gateway reads `content` as the answer and
  drops `reasoning_content`.

## Consumers

- **email-mcp-gateway** reaches it via `host.docker.internal:8084` (unchanged — the container
  publishes 8084 to the host); uses **non-streaming**.
- **OpenWebUI** reaches it as `http://diffusiongemma:8084/v1` over `visionflow_runtime` (or
  `127.0.0.1:8084` if still host-networked); uses **streaming** → sees the `<think>` block.

## Rollback

If the container misbehaves, fall back to the legacy host process:
```bash
docker compose down
cd ~/githubs/llm-server && setsid bash -c './diffusiongemma-serve.sh >/tmp/diffusiongemma.log 2>&1' </dev/null &
```
