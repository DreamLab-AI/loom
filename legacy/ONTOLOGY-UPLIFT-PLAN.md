> **Status: Historical (2026-08-11 HP toolkit guide) — frozen, do not extend.** Retired to
> `legacy/` on 2026-08-17 (DOC-REENGINEERING-PLAN §2.1). It documents the `legacy/scripts/`
> bench drivers and the now-retired vendored `app/pipeline/`. Its live content — the retrieval
> recipe and the bench ask — is subsumed by [`PRD-027`](../docs/design/PRD-027-rust-loom-reengineering.md)
> and [`bench/UPLIFT-BENCH-PROTOCOL.md`](../bench/UPLIFT-BENCH-PROTOCOL.md). The `ontology_scaffold.py`
> / `ontology_proxy.py` it references were retired with the Python serving code; their behaviour
> lives in the Rust `loom-scaffold` crate and the frozen goldens in `tests/golden-python/`.

# Ontology Uplift Toolkit — Plan & Usage (for the agent running model tests on HP-Desktop)

**Deployed:** 2026-08-11 by the workspace Claude session (via `ssh hp`).
**Audience:** the agent/operator running the Muse-Glimmer ↔ Gemma comparison benches in
`loom/legacy/scripts/` (`bench-headhead.py`, `bench-headhead-hard.py`, …).
**Ask from John:** once the data lands, restart the current test matrix **with the
ontology scaffold engaged as a third axis** — raw vs scaffolded, per model.

---

## What this is

A portable, always-available bridge between the local LLM (whatever is serving on
:8084/:8085) and the DreamLab knowledge graph — 8.1k OWL classes / ~262k triples
authored in the logseq corpus, published on https://narrativegoldmine.com, and
**now including the reasoned closure** (inferred superclasses + inherited
relations, computed in CI on every corpus push). Nothing here depends on
VisionClaw or agentbox being up: the data is mirrored locally, refreshed from the
public site.

The retrieval recipe is the one proven in PRD-020 (agentbox "one brain"):
*link → seed → expand → serialise → budget-clamp → prepend*. It ran 18/18 tests
there; this is the portable re-packaging.

## Directory layout (`~/githubs/loom/ (formerly llm-server/ontology/)`)

| Path | What |
|---|---|
| `ONTOLOGY-UPLIFT-PLAN.md` | this file |
| `mirror.sh` | pulls the published artifacts into `data/` (idempotent, timestamp-aware). Run manually or via the shipped timer. |
| `data/scaffold-index.json` | compact one-file class index (title, definition, domain, quality, maturity, parents, **inferred ancestors**, relations, backlinks). The single source the tools below read. |
| `data/ontology.ttl` / `data/ontology-inferred.ttl` | full asserted graph + reasoned closure (Turtle), for SPARQL work (`pip install pyoxigraph` if wanted; nothing below requires it). |
| `ontology_scaffold.py` | **A2 — the bench-ready piece.** Stdlib-only module: `scaffold(prompt, budget_tokens)` / `scaffold_messages(messages)`. Import it in a bench script to prepend budget-clamped ontology context. `--selftest` built in. |
| `bench-integration.md` | exact wiring pattern for the bench harness (`--scaffold` flag, A/B loop). |
| `ontology_proxy.py` | **A3 — agentic option.** OpenAI-compatible proxy (default :8086 → upstream :8085). Modes: `scaffold` (inject context transparently), `tools` (gives the model `ontology_search/class_get/neighbours` and runs the tool loop), `off` (passthrough). Point any client at :8086 instead of the model port. |
| `ontology-proxy.service` | systemd unit template (ship-only; enable if wanted). |
| `ontology-mcp/` | **A4 — MCP server** (Node): `ontology_search/class_get/neighbours/ask` for MCP hosts (Claude Code, OpenWebUI, etc.). `ONTOLOGY_SITE` (public) or `ONTOLOGY_INDEX` (local) modes. |

## How to run the uplift comparison (suggested design)

1. `./mirror.sh` — confirm `data/scaffold-index.json` exists and reports ~8k classes
   (`python3 ontology_scaffold.py --stats`).
2. Add a third axis to the head-to-head: for each (model, question), run
   **raw** and **scaffolded** (see `bench-integration.md` — one import + one call:
   `messages = scaffold_messages(messages)`).
3. Keep the scaffold budget fixed (default 1500 tok) so models are compared fairly;
   record the `injected ≈ tokens` figure per question (the scaffold block length/4).
4. Questions where the scaffold returns `''` (no ontology match) are automatically
   raw-vs-raw — report them separately, they measure nothing about uplift.
5. Optional second experiment: same questions through `ontology_proxy.py` in
   `tools` mode — this measures *agentic traversal* ability (tool-call quality
   differs per model; scaffold mode isolates pure knowledge uplift, tools mode
   adds the agency variable). Do not mix the two in one table.

The interesting domains to sample questions from (dense, well-related regions of
the graph): `artificial-intelligence`, `blockchain`, `distributed-systems`,
`security`, `metaverse/spatial-computing`. `data/scaffold-index.json` carries a
`dom` field per class if you want to generate domain-stratified question sets.

## Data freshness / the publishing loop (Part B, so you know where data comes from)

- Corpus lives in the `jjohare/logseq` repo (`mainKnowledgeGraph/pages`, JSON-LD
  blocks per page). Every push runs the pipeline in GitHub CI: validate →
  conflicts gate → build → **reason (transitive-subclass closure, inherited
  relations)** → deploy to narrativegoldmine.com (gh-pages).
- So the published site — and your local mirror after `mirror.sh` — always tracks
  the *reasoned optimal state* of the corpus. No LAN services involved.
- If narrativegoldmine is unreachable, everything keeps working from the last
  mirrored `data/` — the tools never need the network at inference time.

## Guard rails

- Nothing here touches the model services (`muse-glimmer.service`,
  `gemma4-31b.service`) or the llama.cpp build. The proxy is a separate,
  optional process on :8086.
- `ontology_scaffold.py` is import-only + stdlib-only — safe to import from any
  bench script with zero side effects (index loads lazily on first call, ~1s).
- If a scaffolded run looks *worse*, that is a real result — report it, don't
  tune the scaffold per-model. Budget and seed count are the only intended knobs
  (`scaffold(prompt, budget_tokens=..., max_seeds=...)`).

## Provenance

- Retrieval recipe: PRD-020 / ADR-112 (agentbox), tests 18/18, 2026-06-14.
- Reasoned closure: logseq `pipeline/reason.py` (this deployment's sibling
  commit), published as `data/ontology-inferred.ttl` + folded into
  `api/pages/<slug>.json` (`inferredSuperClasses`, `inheritedRelations`) and
  `data/scaffold-index.json` (`isup`).
- Questions → workspace Claude session (RuVector memory key
  `hp-ontology-uplift-deployment-2026-08-11` has the deployment record).

## Test system (added 2026-08-11)

The full testing system used to build this toolkit is included. One command:

    ./run-all-tests.sh    # scaffold selftest · proxy tests · MCP tests · pipeline suite

Current status on this box: all four suites PASS (scaffold 23 assertions,
proxy 28 checks, MCP 8 stdio tests, pipeline 32 pytest tests incl. the
reasoner). `pipeline/` is the same package that runs in narrativegoldmine CI —
you can validate/reason/build corpus data locally with
`./.venv/bin/python -m pipeline.build <pages_dir> <out_dir>` if you ever pull
the corpus here. Run `./run-all-tests.sh` after any `mirror.sh` refresh or
edit to the toolkit before relying on results.

## Uplift benchmark (added 2026-08-11, second drop)

`bench_ontology_uplift.py` — objective uplift measurement: graph-derived gold
answers (no LLM judge needed), paired raw-vs-scaffold scoring, bootstrap 95% CIs.
A 505-question set is pre-generated at `questions.jsonl` (seed 42, 15 domains,
T-REL/T-TAX/T-COMMON). **Full protocol: `UPLIFT-BENCH-PROTOCOL.md`** — run one
per-model session per port (verify /v1/models first; :8085 muse / :8084 gemma
are usually mutually exclusive), then a single report over all four score files.

## Prose-enriched mode (added 2026-08-11, third drop)

The scaffold now has a FOURTH axis: `scaffold-prose`. It enriches the top 2
seed sections with the prose layer the structural index truncates — full
authored definitions and the research-dated "Current Landscape" sections
(747 classes carry one) — from `data/prose-index.json` (mirrored; optional,
degrades silently to structural when absent).

- Module: `scaffold(prompt, prose=True)` / `scaffold_messages(msgs, prose=True)`
- CLI: `python3 ontology_scaffold.py "<prompt>" --prose`
- Bench: `--mode scaffold-prose` (same paired-delta machinery; compare
  raw vs scaffold vs scaffold-prose per model — keep the budget FIXED across
  modes so the comparison stays fair; prose competes for the same tokens).
