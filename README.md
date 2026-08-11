# Loom — the Ontology Loom

**A portable node that grounds LLM responses in a formal ontology, behind a stable,
model-swappable façade.** You point a consumer (an agent, an email gateway, any
OpenAI-compatible client) at one endpoint; Loom retrieves the relevant slice of a
knowledge-graph corpus, injects it as budget-clamped context, and delegates generation to
whatever model is deployed behind it. Swap the model — Gemma → Muse-Glimmer → next — and no
consumer changes.

Loom is the *serving* half of a neurosymbolic pair. Its sibling
[**knowledgeGraph**](https://github.com/DreamLab-AI/knowledgeGraph) (published at
[narrativegoldmine.com](https://narrativegoldmine.com)) is the corpus, the Logseq→OWL
pipeline, and the method — *how you build an ontology*. Loom is *how you serve that ontology
to ground an LLM at runtime*. It also reasons over the corpus alongside the
[VisionClaw](https://github.com/DreamLab-AI/VisionClaw) engine.

---

## Why — the measured result

Grounding an LLM in a formal ontology is not a hunch here; it is measured. On a held-out,
objective benchmark (37 questions, gold answers derived from the graph itself, paired
raw-vs-grounded scoring with bootstrap 95% confidence intervals — `bench/`), static ontology
scaffolding is a **decisive, model-agnostic win**:

| Model | Raw (parametric) | + Loom scaffold | Paired uplift (95% CI) | Latency |
|---|---|---|---|---|
| Gemma-4-31B | 0.146 | **0.939** | **+0.793** [+0.680, +0.894] | 31.5 s → 5.1 s |
| Muse-Glimmer-30B | 0.268 | **0.939** | **+0.671** [+0.527, +0.804] | 34.7 s → 9.8 s |

Read that twice: two different models, both land at **0.94** grounded, from wildly different
parametric baselines — and grounding is **~3–6× faster** (the model stops doing heavy
open-ended recall and restates supplied facts). The lift concentrates exactly where you'd
want it — the niche domains a model doesn't already know: blockchain 0.11→1.0, robotics and
standards 0→1.0 — and adds nothing where the model is already right. That is the signature of
real grounding, not leakage.

Three findings shaped Loom's defaults:

1. **Static structured scaffold is the product.** The taxonomy + typed-relation + definition
   extract carries the value. `POST /loom/scaffold` is this, and it works with no model at all.
2. **Prose adds nothing over structure** (+0.007 Muse / +0.000 Gemma). Loom ships prose as an
   optional complement, off the default grounding path — it costs budget for no recall.
3. **Agentic tool-traversal is model-dependent.** Letting the model *traverse* the graph
   itself is Gemma's best axis (0.973, beating even static injection) but Muse's worst (0.649
   — it under-calls the ancestor-walk). So Loom's default is *inject*, not *traverse*; the
   tools path stays available for models that traverse well.

Full report and honesty notes in [`docs/research/`](docs/research/). The honest frame:
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
  -d '{"model":"muse-glimmer-30B","messages":[{"role":"user","content":"what is a rollup?"}],"max_tokens":1536}'
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

**Deployment A — colocated with the model** (the reference deployment: Loom + GPU model on
one host; the façade on the host network reaches the local model server):

```bash
docker compose up --build -d           # façade on :8084 (host network)
export DISTILL_BACKEND_URL=http://127.0.0.1:8085/v1   # your local llama.cpp / vLLM / Ollama
curl http://127.0.0.1:8084/health
```

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

---

## Design & research

- [`docs/design/`](docs/design/) — the capstone design: `PRD-025` (product), `ADR-135`
  (architecture decision), the DDD bounded context, agentbox `ADR-051` (harness client +
  deferred distillation), and the corpus-build/generation-identity pipeline note. The design
  was adversarially reviewed (five-lens panel) before authoring.
- [`docs/research/`](docs/research/) — the ontology-uplift benchmark: combined model report,
  per-domain / per-template breakdowns, the model comparison, and the honesty notes.
- [`bench/`](bench/) — reproduce it: `bench_ontology_uplift.py` (objective, graph-derived
  gold, paired bootstrap CIs) + `UPLIFT-BENCH-PROTOCOL.md`.

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
