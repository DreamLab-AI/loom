# Loom — the Ontology Loom node

A portable ontology node with a stable, **model-swappable façade**. It grounds LLM
responses in the DreamLab knowledge-graph corpus (retrieval/scaffold), and delegates
generation to whatever model is deployed behind it — swap the model with zero consumer
change. See VisionClaw `PRD-025` / `ADR-135`, agentbox `ADR-051`.

## Deploy (colocated with the model — Deployment A)
```bash
docker compose up --build -d        # façade on http://127.0.0.1:8088 (host network)
curl http://127.0.0.1:8088/health   # ok + generation stamp + backend reachability
```
`DISTILL_BACKEND_URL` (default `http://127.0.0.1:8085/v1`) is the model-swap seam — point
it at the host llama.cpp server. The model changes on the host; the façade does not.

## Façade contract
- `GET  /health` — liveness, corpus generation, backend reachability
- `GET  /loom/generation` — the corpus generation identity served
- `POST /loom/scaffold` `{prompt,budget_tokens,prose}` — budget-clamped ontology grounding
  (NO model needed — retrieval facet, testable anywhere)
- `POST /v1/chat/completions` — scaffold-inject the last user message → delegate to the model
- `GET  /v1/models` — model identity passthrough

## Portability
Peer-clone this repo on any mesh node and `docker compose up`. Two topologies:
- **A (this compose)** — colocated with the model, host network, GPU-local.
- **B (sidecar)** — on a container network beside consumers; delegate to a remote model
  URL (see VisionClaw `docker-compose.unified.yml` service `loom`).

## Layout
- `app/` — the façade (`loom_facade.py`) + retrieval (`ontology_scaffold.py`),
  OpenAI proxy, `ontology-mcp/` (MCP consumers), `mirror.sh` (corpus generation).
- `bench/` — objective ontology-uplift benchmark + protocol.
- `docs/` — deployment plan.
