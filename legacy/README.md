# Legacy — the retired `llm-server` host toolkit

Preserved from `~/githubs/llm-server` when that (un-versioned) repo was retired on
2026-08-15. The Loom stack (`../docker-compose.yml`) replaced it as the serving path;
these files are the fallback/rollback toolkit and the record of how the numbers in
`../docs/research/` were produced.

- **`scripts/`** — host-side serve scripts (`serve-gemma4-31b.sh`, `serve-muse-glimmer.sh`,
  `serve-gemma4-abliterated.sh`, …), `swap-model.sh`, `download-model.sh`, and every bench
  driver (`bench-headhead*.py`, `bench-bullshit.py`, `bench-quality*.py`,
  `bench-throughput.sh`, `plot-model-comparison.py`). They referenced llama.cpp builds at
  `~/githubs/llm-server/llama.cpp{,-mtp}` (deleted; rebuild — the Loom `model/Dockerfile`
  pins a validated commit) and the model store now at `~/models/`.
- **`config/`** — per-model env files and the retired `muse-glimmer.service` systemd unit.
- **`diffusiongemma-docker/`** — sole copy of the DiffusionGemma server source
  (`dg_server.py`, Dockerfile, `llama-diffgemma-src.tar.gz`).
- **`openwebui-docker/`** — the open-webui compose referenced by
  `../docs/REMOTE-CLIENT-SETUP.md`.
- **`GEMMA4-31B-CONNECTION.md` / `GEMMA4-HYBRID.md` / `MUSE-GLIMMER-CONNECTION.md`** —
  connection docs for the fallback models (weights kept in `~/models/`; the abliterated
  models are deliberately retained as fallbacks — do not auto-delete).

Raw bench logs backing the benchmark reports live in `../docs/research/evidence/`.
