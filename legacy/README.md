# Legacy — the retired `llm-server` host toolkit

Preserved from `~/githubs/llm-server` when that (un-versioned) repo was retired on
2026-08-15. The Loom stack (`../docker-compose.yml`) replaced it as the serving path;
these files are the fallback/rollback toolkit and the record of how the numbers in
`../docs/research/` were produced.

- **`scripts/`** — host-side serve scripts (`serve-gemma4-31b.sh`, `serve-muse-glimmer.sh`,
  `serve-gemma4-abliterated.sh`, …), `swap-model.sh`, `download-model.sh`, and the shell
  throughput benches (`bench-throughput.sh`, `bench-turboquant.sh`, `bench-gemma4-kv.sh`).
  They referenced llama.cpp builds at `~/githubs/llm-server/llama.cpp{,-mtp}` (deleted;
  rebuild — the Loom `model/Dockerfile` pins a validated commit) and the model store now
  at `~/models/`.
  > The **Python** bench drivers (`bench-headhead*.py`, `bench-bullshit.py`,
  > `bench-quality*.py`, `bench-sonnet-manual.py`, `plot-model-comparison.py`) and their
  > `run-*.sh` wrappers were **deleted on 2026-09-03** — benchmarks for retired models
  > against llama.cpp builds that no longer exist. The numbers they produced remain in
  > `MODEL-BENCHMARKS.md` and `../docs/research/evidence/`. See
  > `../bench/LEGACY-PYTHON-NOTE.md`.
- **`config/`** — per-model env files and the retired `muse-glimmer.service` systemd unit.
- **`diffusiongemma-docker/`** — **deleted 2026-09-03.** DiffusionGemma was retired at the
  Qwen3.8 cutover; the server source, Dockerfile and compose file went with it.
- **`openwebui-docker/`** — the open-webui compose referenced by
  `../docs/REMOTE-CLIENT-SETUP.md`.
- **`GEMMA4-31B-CONNECTION.md` / `GEMMA4-HYBRID.md` / `MUSE-GLIMMER-CONNECTION.md`** —
  connection docs for the fallback models (weights kept in `~/models/`; the abliterated
  models are deliberately retained as fallbacks — do not auto-delete).

Raw bench logs backing the benchmark reports live in `../docs/research/evidence/`.
