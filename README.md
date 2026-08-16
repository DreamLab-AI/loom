# Loom — the Ontology Loom

**A portable node that grounds LLM responses in a formal ontology, behind a stable,
model-swappable façade.** You point a consumer (an agent, an email gateway, any
OpenAI-compatible client) at one endpoint; Loom retrieves the relevant slice of the
reasoned ontology, injects it as budget-clamped context, and delegates generation to
whatever model is deployed behind it. Swap the model — Gemma → Muse-Glimmer → Qwen3.8 → next
— and no consumer changes.

Since 2026-08-14 the reference deployment ships the model engine **inside this stack**: the
`model` service (`loom-model` container) serves **Qwen3.8-27B** (unsloth UD-Q8_K_XL, vision,
embedded-MTP speculative decoding tuned n=3, 262 K native context, thinking on at
server-default `medium` effort) via llama.cpp on `:8085`, replacing the old host systemd
unit. Docs: [`docs/QWEN3.8-CONNECTION.md`](docs/QWEN3.8-CONNECTION.md) (model reference) ·
[`docs/REMOTE-CLIENT-SETUP.md`](docs/REMOTE-CLIENT-SETUP.md) (connect a LAN machine).

Loom is the *serving* half of a neurosymbolic pair. Its sibling
[**knowledgeGraph**](https://github.com/DreamLab-AI/knowledgeGraph) (published at
[narrativegoldmine.com](https://narrativegoldmine.com)) is the corpus, the Logseq→OWL
pipeline and the method — *how the ontology gets built*. Loom is
*how that checked graph gets served to ground an LLM at runtime*: it retrieves the relevant
slice into a model's context so answers restate checked facts rather than guesses. This is
the layer the 2026 industry calls a **context graph** (a label still settling; we use its
assembly sense) — the top of the stack, building an agent's working set from everything
beneath it. Loom does not reason; the symbolic check is
Whelk's, in the sibling [VisionClaw](https://github.com/DreamLab-AI/VisionClaw) engine, which
reasons over the same corpus.

---

## Why — the measured result

Grounding an LLM in a formal ontology is not a hunch here; it is measured. On a held-out,
objective benchmark (37 questions, gold answers derived from the graph itself, paired
raw-vs-grounded scoring with bootstrap 95% confidence intervals — `bench/`), static ontology
scaffolding is a **decisive, model-agnostic win**:

| Model | Raw (parametric) | + Loom scaffold | Paired uplift (95% CI) | Latency |
|---|---|---|---|---|
| Gemma-4-31B (local) | 0.146 | **0.939** | **+0.793** [+0.680, +0.894] | 31.5 s → 5.1 s |
| Muse-Glimmer-30B (local) | 0.268 | **0.939** | **+0.671** [+0.527, +0.804] | 34.7 s → 9.8 s |
| Gemini 3.7 Flash (cloud) † | 0.359 | **0.942** | **+0.583** [+0.546, +0.618] | 2.4 s → 1.2 s |

Read that twice: **three** different models — two local, one a frontier cloud model — all
land at **~0.94** grounded, from wildly different parametric baselines, and grounding is
**faster in every case** (the model stops doing heavy open-ended recall and restates supplied
facts). The lift concentrates exactly where you'd want it — the niche domains a model doesn't
already know (spatial-computing 0.23→0.97, distributed-collaboration 0.22→0.95) — and adds
least where the model is already right. That is the signature of real grounding, not leakage.
The stronger the model, the smaller the *uplift* it needs — but the grounded ceiling is the
same. That is the whole bet of the swappable façade: the scaffold carries the recall, not the
model behind the door.

† First cloud model benched (2026-08-16, `gemini-3.7-flash`), and on a larger set — 510
questions vs 37 for the local runs. Its config differs deliberately: `temp=1.0` (Google's
recommended setting for Gemini 3.x; `temp<1.0` is off-label) vs `temp=0` local, and
`reasoning_effort=low` with `max_tokens=2048` so the model's mandatory thinking doesn't
truncate answers. The *paired* delta is a within-model comparison and stays valid; the
absolute raw recall is **not** cell-for-cell comparable to the temp=0 local runs. Full
provenance and per-domain tables: [`docs/research/report-gemini-3.7-flash.md`](docs/research/report-gemini-3.7-flash.md).

Three findings shaped Loom's defaults:

1. **Static structured scaffold is the product.** The taxonomy + typed-relation + definition
   extract carries the value. `POST /loom/scaffold` is this, and it works with no model at all.
2. **Prose adds nothing over structure** (+0.007 Muse / +0.000 Gemma). Loom ships prose as an
   optional complement, off the default grounding path — it costs budget for no recall.
3. **Agentic tool-traversal is model-dependent.** Letting the model *traverse* the graph
   itself is Gemma's best axis (0.973, beating even static injection) but Muse's worst (0.649
   — it under-calls the ancestor-walk). So Loom's default is *inject*, not *traverse*; the
   tools path stays available for models that traverse well.

The full write-up is the typeset flagship report
**[`docs/research/ontology-uplift-report.pdf`](docs/research/ontology-uplift-report.pdf)**
(*Does Grounding an LLM in a Formal Ontology Actually Work?*) — two independent studies, the
Loom scaffold uplift (Study A) and the ontology-augment A/B eval (Study B), converging on the
same conclusion. Markdown reports and honesty notes in [`docs/research/`](docs/research/). The honest frame:
scaffolded scores measure *grounded-answer capability*, raw scores measure *parametric
knowledge* — the paired delta is "uplift available from grounding," and it is large.

---

## What — the façade

One deployment-agnostic contract (VisionClaw `ADR-135` D1). The model is always a URL behind
it (`DISTILL_BACKEND_URL`), never baked into the endpoint:

| Endpoint | Purpose | Needs a model? |
|---|---|---|
| `GET  /health` | liveness + corpus **generation** stamp + backend reachability | no |
| `GET  /loom/generation` | the corpus generation identity being served | no |
| `POST /loom/scaffold` | budget-clamped ontology grounding for a prompt (the retrieval facet) | **no** |
| `POST /v1/chat/completions` | scaffold-inject the last user message → delegate to the model | yes |
| `GET  /v1/models` | model identity passthrough (probe what's behind the façade) | yes |

```bash
# grounding, no model required — works anywhere the corpus is mirrored
curl -sXPOST localhost:8084/loom/scaffold \
  -d '{"prompt":"how do zero-knowledge proofs relate to blockchain scalability?","budget_tokens":700}'

# grounded generation — scaffold-injected, then delegated to the model behind the façade
curl -sXPOST localhost:8084/v1/chat/completions \
  -d '{"model":"qwen3.8-27B","messages":[{"role":"user","content":"what is a rollup?"}],"max_tokens":1536}'
```

The response of a grounded completion carries a `loom` block (`injected_tokens`, `mode`,
`grounding`, `generation`) so consumers can account for the grounding and prove which corpus
generation produced the answer. The `grounding` sub-block reports the retrieval confidence
(`top_score`), `seed_count`, and `effective_budget` for that request.

### Confidence-aware selective injection

Grounding is only helpful when the query is actually on-ontology. Research on *contextual
interference* shows that injected context can **displace** the model's own parametric
knowledge — models over-rely on retrieved evidence even when it is weak or off-topic
([Lin et al. 2026, arXiv:2506.05154](https://arxiv.org/abs/2506.05154)), and irrelevant
retrieved context measurably degrades answers
([Yoran et al. 2024, arXiv:2310.01558](https://arxiv.org/abs/2310.01558);
[Shi et al. 2023, arXiv:2302.00093](https://arxiv.org/abs/2302.00093)) — so selective,
confidence-scaled injection beats blanket grounding. Loom uses the retrieval score that `match()` already computes as the confidence
signal: a strong exact-title hit gets the full scaffold budget; a loose match gets a
proportionally smaller one; a below-threshold match is skipped entirely.

| Env var | Default | Meaning |
|---|---|---|
| `LOOM_CONFIDENCE_INJECTION` | `0` (repo) / `1` (HP compose) | master switch; off = legacy blanket injection |
| `LOOM_STRONG_MATCH_SCORE` | `8.0` | at/above this match score → full budget |
| `LOOM_MIN_INJECT_SCORE` | `2.0` | below this top score → skip injection entirely |
| `LOOM_MIN_INJECT_FRACTION` | `0.4` | weakest kept match still gets this fraction of budget |

Default-off in the code is byte-identical to blanket injection, so it is safe to ship disabled
and enable per deployment.

**A/B testing it (on HP).** The switch is a runtime env on the *Loom* (injection is
server-side), so recycle the container with each value and run the same scaffold benchmark
against it — no rebuild needed:

```bash
python3 bench/bench_ontology_uplift.py generate --seed 7 --out bench/questions.jsonl   # one shared set

# baseline — blanket injection
LOOM_CONFIDENCE_INJECTION=0 docker compose up -d
python3 bench/bench_ontology_uplift.py run --mode scaffold --questions bench/questions.jsonl --out A.jsonl

# treatment — confidence-aware
LOOM_CONFIDENCE_INJECTION=1 docker compose up -d
python3 bench/bench_ontology_uplift.py run --mode scaffold --questions bench/questions.jsonl --out B.jsonl
```

Score both, then compare recall/quality and the per-answer `loom.grounding.top_score` /
`effective_budget`. Expectation: unchanged on strong on-ontology questions, reduced injected
tokens (and less interference) on weak / off-topic ones. Protocol detail in
[`bench/UPLIFT-BENCH-PROTOCOL.md`](bench/UPLIFT-BENCH-PROTOCOL.md); unit coverage in
[`tests/test_confidence_injection.py`](tests/test_confidence_injection.py).

**Verified live (2026-08-11, HP deployment, commit `4beba5f`).** The full standing test
system passes with the new scaffold code — toolkit suite 6/6 PASS (scaffold selftest ·
proxy integration · bench selftest · confidence unit · MCP server · pipeline pytest), repo
suite 4/4 runnable PASS — and the deployed façade (flag on, 8143-class generation) shows
the intended behaviour over HTTP:

| Live query | `top_score` | Result |
|---|---|---|
| "what is a rollup in blockchain scaling?" (on-ontology) | 10.75 | full budget — `effective_budget` 1500, 683 tokens injected |
| "best recipe for banana pancakes?" (off-ontology) | 0.0 | gate fired — `injected: false`, 0 tokens injected |

The quality A/B (blanket vs confidence-aware over the shared question set) is the remaining
step; the protocol below is ready to run as-is.

---

## How — deploy

Loom is a stdlib-Python + one-Node-dep container. Peer-clone it on any node and bring it up.

**Deployment A — colocated with the model** (the reference deployment: both facets are
containers in this compose file — the `model` service is the GPU engine, the `loom` façade
fronts it over the host network):

```bash
docker compose up --build -d           # model engine on :8085 + façade on :8084 (host network)
curl http://127.0.0.1:8085/health      # llama.cpp engine (Qwen3.8-27B)
curl http://127.0.0.1:8084/health      # façade
```

The `model` service (`model/Dockerfile`) builds llama.cpp pinned to a validated commit
(CUDA, sm_75 for this host's 2× Quadro RTX 6000) and serves
`unsloth/Qwen3.8-27B-GGUF:UD-Q8_K_XL` + vision mmproj with embedded-MTP speculative
decoding, layer-split across both GPUs, 262 K native context. Weights are mounted read-only
from the host model store, not baked. To use an external model instead (llama.cpp / vLLM /
Ollama anywhere), point `DISTILL_BACKEND_URL` at it and drop the `model` service.

**Deployment B — sidecar beside consumers** (co-located with agents/email on a container
network; delegate to a remote model URL). See VisionClaw `docker-compose.unified.yml` service
`loom` (profile `loom`, reachable as `http://loom:8080`).

Either way: **swap the model on its host and the façade is unchanged.** That is the
no-technical-debt-on-upgrade guarantee, and it is why the email gateway and agent harness
hold a Loom URL, never a raw model port.

The corpus is not baked into the image — `mirror.sh` pulls the published **generation** from
[narrativegoldmine.com](https://narrativegoldmine.com) at start (atomic, fail-open to the
mounted volume), so Loom always serves a known, sha-addressable corpus generation.

---

## Where it sits — the ecosystem

Loom is **[VisionFlow](https://github.com/DreamLab-AI)'s loop-closing ontology node** —
the runtime that returns the ecosystem's shared formal semantic layer to a model at
generation time. VisionFlow is the architecture the industry now calls *neurosymbolic*:
thin agents over a shared, reasoned ontology rather than thick agents wired to raw data.
Loom serves that layer; the pieces around it build, reason over and consume it.

```
knowledgeGraph  ──publishes──▶  a corpus GENERATION (OWL + reasoned closure + indexes)
 (corpus + pipeline + method)         │
                                      ▼  mirror
                                  ┌─────────┐   scaffold-inject     ┌────────────┐
   agents / email / any client ──▶│  LOOM   │───────────────────▶ │  the model  │
        (hold the Loom URL)       │ façade  │◀───────────────────  │ (swappable) │
                                  └─────────┘   grounded answer     └────────────┘
                                      ▲
                              VisionClaw reasons over the same corpus (OWL 2 EL)
```

- **[knowledgeGraph](https://github.com/DreamLab-AI/knowledgeGraph)** — the corpus + the
  Logseq→OWL pipeline + the explorer. *Build your own ontology.* Loom **consumes** its
  published generations; it does not duplicate them (see the separation note in
  [`docs/design/`](docs/design/)).
- **[VisionClaw](https://github.com/DreamLab-AI/VisionClaw)** — GPU graph engine + OWL 2 EL
  reasoner + governance write door. The Loom design (`PRD-025` / `ADR-135`) lives there.
- **agentbox** — the agentic harness; its "one brain" retrieval resolves through Loom
  (`ADR-051`), and its deferred-distillation tools submit long-running grounded jobs to Loom.

**Self-improvement.** Dreaming grounds its nightly research through this door: a [dream cycle](https://github.com/DreamLab-AI/dream-engine) can query the reasoned ontology so hypotheses restate checked facts, not parametric guesses — then opens a draft PR a human merges.

---

## Design & research

- [`docs/design/`](docs/design/) — the capstone design: `PRD-025` (product), `ADR-135`
  (architecture decision), the DDD bounded context, agentbox `ADR-051` (harness client +
  deferred distillation), and the corpus-build/generation-identity pipeline note. The design
  was adversarially reviewed (five-lens panel) before authoring.
- [`docs/research/ontology-uplift-report.pdf`](docs/research/ontology-uplift-report.pdf) — the
  typeset flagship report (two-study, LaTeX source in [`docs/research/latex/`](docs/research/latex/)).
- [`docs/research/`](docs/research/) — the ontology-uplift benchmark markdown: the local-model
  report, the [Gemini 3.7 Flash cloud companion](docs/research/report-gemini-3.7-flash.md),
  per-domain / per-template breakdowns, and the honesty notes.
- [`bench/`](bench/) — reproduce it: `bench_ontology_uplift.py` (objective, graph-derived
  gold, paired bootstrap CIs), `UPLIFT-BENCH-PROTOCOL.md`, and `run-gemini.sh` (the cloud driver).

## Boundary — the Loom is the ontology only

The Loom serves the **published ontology** (the reasoned generation) and nothing else. It
**never reads or mirrors the working graph** (personal/working notes, which may become
multi-user or private). Uplift *into* the ontology happens through VisionClaw's governed
propose door, the forum/ACSP surface, or direct agentic writes into the corpus — never the
Loom; the new generation is then mirrored here read-only. (DDD BC24 invariant I11.)

## Status & honesty

Loom is a research/dev system on the DreamLab estate. The corpus it serves is **AI-generated
synthetic content produced under human direction, by design** — an ontology testbed, not an
authoritative encyclopaedia. Every grounded answer is traceable to a corpus generation; that
provenance attests traceable generation, not human authorship. "Platform for any ontology
connector" is earned when a second provider lands — today Loom is one node with a
provider-plugin seam, stated plainly.

## Licence

See the sibling [knowledgeGraph](https://github.com/DreamLab-AI/knowledgeGraph) licensing for
the corpus/data terms; Loom's own code is under the repository `LICENSE`.
