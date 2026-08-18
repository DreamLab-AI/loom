# Loom — the Ontology Loom

**A single-binary Rust node that grounds LLM responses in a formal ontology, behind a
stable, model-swappable façade.** Point any OpenAI-compatible consumer (an agent, an email
gateway, any `/v1` client) at one endpoint; Loom retrieves the relevant slice of the reasoned
ontology as **human-scrutible per-IRI markdown-with-ontology blocks**, injects them as
budget-clamped, confidence-gated context, and delegates generation to whatever model is
deployed behind it (`DISTILL_BACKEND_URL`). Swap the model — Gemma → Muse-Glimmer → Qwen3.8 →
next — and no consumer changes.

Retrieval fuses signals over one canonical unit — a lexical inverted index and oxigraph SPARQL
over the Whelk-reasoned closure today, with an in-process RuVector HNSW semantic fallback wired
for out-of-vocabulary queries (gated off, see [Status](#status--honesty)) — but the served,
reviewable unit is **always the markdown**; the indexes only find and rank it. That invariant —
one reviewable markdown block per IRI as the single served, auditable unit — is **THE PRIZE**,
and it is enforced by the crate architecture, not by convention (see
[`docs/design/RUST-ARCHITECTURE.md`](docs/design/RUST-ARCHITECTURE.md) §0).

**What it is *for*: making swappable models performant against large, important, private
customer datasets** — answering accurately and attributably on an in-domain corpus the model
could never know parametrically, and delivering that curated, vetted knowledge faithfully and
cheaply (the rigorous curation is amortised once, reused per query). The bar is
**multivariate**: excellent recall on the locally-grounded questions, *without going jagged* on
the general or novel ones (confidence-gated injection avoids displacing the model's own
knowledge; genuinely new questions fall back to internet-research agents while still inheriting
the ontology's domain framing). The right comparison for in-domain answers is a curated corpus
**versus a generic web search**, not the bare model. Full framing:
[`docs/design/LOOM-POSITIONING.md`](docs/design/LOOM-POSITIONING.md).

Since 2026-08-14 the reference deployment ships the model engine **inside this stack**: the
`loom-model` container serves **Qwen3.8-27B** via llama.cpp on `:8085` (262 K native context,
q8_0 KV, embedded-MTP speculative decoding tuned n=3). As of 2026-08-18 the deployed variant is
the **Heretic abliterated Qwen3.8-27B Q8_0**
([`0bserverx/Qwen3.8-27B-Heretic-Abliterated-Uncensored-GGUF`](https://huggingface.co/0bserverx/Qwen3.8-27B-Heretic-Abliterated-Uncensored-GGUF),
~19.5 tok/s); the base unsloth weights are retained on disk as rollback. That GGUF ships with no
embedded chat template, so llama.cpp is told to serve it with the base Qwen3.8 template and
DeepSeek-style reasoning parsing (see [`docs/QWEN3.8-CONNECTION.md`](docs/QWEN3.8-CONNECTION.md)).
The model is a URL behind the façade; swapping it changes no consumer. Docs:
[`docs/QWEN3.8-CONNECTION.md`](docs/QWEN3.8-CONNECTION.md) (model reference),
[`docs/REMOTE-CLIENT-SETUP.md`](docs/REMOTE-CLIENT-SETUP.md) (connect a LAN machine).

Loom is the *serving* half of a neurosymbolic pair. Its sibling
[**knowledgeGraph**](https://github.com/DreamLab-AI/knowledgeGraph) (published at
[narrativegoldmine.com](https://narrativegoldmine.com), built by the canonical
[`jjohare/logseq`](https://github.com/jjohare/logseq) pipeline) is the corpus, the Logseq→OWL
pipeline and the method — *how the ontology gets built*. Loom is *how that checked graph gets
served to ground an LLM at runtime*: it retrieves the relevant slice into a model's context so
answers restate checked facts rather than guesses. This is the layer the 2026 industry calls a
**context graph** — the top of the stack, building an agent's working set from everything
beneath it. Loom does not reason; the symbolic check is Whelk's, in the sibling
[VisionClaw](https://github.com/DreamLab-AI/VisionClaw) engine, which reasons over the same
corpus at build time.

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
least where the model is already right. The stronger the model, the smaller the *uplift* it
needs, but the grounded ceiling is the same. That is the whole bet of the swappable façade:
the scaffold carries the recall, not the model behind the door.

**What that uplift *is*, measured.** A later study went further and measured what the lift is
made of, and the finding is worth stating plainly because it changes how you read the numbers
above. The paper introduces a *copy ceiling* (the recall a verbatim copy of the injected
context would already score) and reports the signed *gain over copy* beside generation recall.
On this node the copy ceiling is 0.964, and across ten models from five providers the gain over
copy is uniformly negative (−0.067 to −0.022); an item-level exposure/recovery decomposition of
11,360 gold items recovers three unexposed items in total, with per-model context utilisation
0.900–0.955. The reading is measured rather than inferred: the model adds **faithful delivery of
the exposed facts, not reasoning over the injected structure**, and that is exactly the product
for private-knowledge grounding, where the answer is trustworthy because the curated source is.
Models differ almost entirely in how much surfaced gold they drop (about one exposed item in
fourteen). This is consistent with a serving regime: verbatim retrieval wins, and heavier
machinery would have to be measured *above* the ceiling, not above the bare model. The full
argument, its softened caveats and the production paired study are in
[`docs/research/paper-v2/main.pdf`](docs/research/paper-v2/main.pdf) (*The Copy Ceiling*); the
evidence index is [`docs/research/README.md`](docs/research/README.md).

† First cloud model benched (2026-08-16, `gemini-3.7-flash`), and on a larger set — 510
questions vs 37 for the local runs. Its config differs deliberately: `temp=1.0` (Google's
recommended setting for Gemini 3.x) vs `temp=0` local, and `reasoning_effort=low` with
`max_tokens=2048` so the model's mandatory thinking doesn't truncate answers. The *paired*
delta is a within-model comparison and stays valid; the absolute raw recall is **not**
cell-for-cell comparable to the temp=0 local runs. Full provenance and per-domain tables:
[`docs/research/report-gemini-3.7-flash.md`](docs/research/report-gemini-3.7-flash.md).

Three findings shaped Loom's defaults:

1. **Static structured scaffold is the product.** The taxonomy + typed-relation + definition
   extract carries the value. `POST /loom/scaffold` is this, and it works with no model at all.
2. **Prose adds nothing over structure** (+0.007 Muse / +0.000 Gemma). Loom ships prose as an
   optional complement, off the default grounding path — it costs budget for no recall.
3. **Agentic tool-traversal is model-dependent.** Letting the model *traverse* the graph
   itself is Gemma's best axis (0.973, beating even static injection) but Muse's worst (0.649
   — it under-calls the ancestor-walk). So Loom's default is *inject*, not *traverse*; the
   tools path stays available for models that traverse well.

The node's primary evidence base is the paper
**[`docs/research/paper-v2/main.pdf`](docs/research/paper-v2/main.pdf)** (*The Copy Ceiling*),
which measures what the uplift is (delivery, not reasoning) and adds the production paired study
above. The earlier typeset report
[`docs/research/ontology-uplift-report.pdf`](docs/research/ontology-uplift-report.pdf) (*Does
Grounding an LLM in a Formal Ontology Actually Work?*, two studies, Study A the scaffold uplift
and Study B the ontology-augment A/B) is the precursor the paper re-centres against a copy
ceiling. The honest frame the paper settles: scaffolded scores measure faithful *delivery* of
exposed facts, raw scores measure *parametric knowledge*, and the paired delta is the grounding
value available, large and attributable, but delivery rather than reasoning over structure. The
full evidence index (data, harness, paper-v3) is
[`docs/research/README.md`](docs/research/README.md).

---

## What — the façade

One deployment-agnostic contract (`ADR-135` D1). The model is always a URL behind it
(`DISTILL_BACKEND_URL`), never baked into the endpoint:

| Endpoint | Purpose | Needs a model? |
|---|---|---|
| `GET  /health` | liveness + corpus **generation** stamp + backend/graph/index readiness | no |
| `GET  /loom/generation` | the corpus generation identity being served | no |
| `POST /loom/scaffold` | budget-clamped ontology grounding for a prompt (the retrieval facet) | **no** |
| `POST /loom/sparql` | read-only, clamped SPARQL over the reasoned closure | no |
| `POST /loom/search` | label/substring search over the store | no |
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
`grounding`, `fusion_path`, `generation`) so consumers can account for the grounding and prove
which corpus generation produced the answer. The `grounding` sub-block reports the retrieval
confidence (`top_score`), `seed_count`, and `effective_budget` for that request; on an injected
answer it also carries an `exposure` object (targets / delivered / dropped) so delivery fidelity
is visible per answer (see
[findings-driven serving controls](#findings-driven-serving-controls)).

### Confidence-aware selective injection

Grounding is only helpful when the query is actually on-ontology. Research on *contextual
interference* shows that injected context can **displace** the model's own parametric
knowledge — models over-rely on retrieved evidence even when it is weak or off-topic
([Lin et al. 2026, arXiv:2506.05154](https://arxiv.org/abs/2506.05154)), and irrelevant
retrieved context measurably degrades answers
([Yoran et al. 2024, arXiv:2310.01558](https://arxiv.org/abs/2310.01558);
[Shi et al. 2023, arXiv:2302.00093](https://arxiv.org/abs/2302.00093)) — so selective,
confidence-scaled injection beats blanket grounding. Loom uses the retrieval score that
`match()` already computes as the confidence signal: a strong exact-title hit gets the full
scaffold budget; a loose match gets a proportionally smaller one; a below-threshold match is
skipped entirely. This policy is the **single injection authority** — every candidate, lexical
or (gated) semantic, flows through it; nothing bypasses the gate.

| Env var | Default | Meaning |
|---|---|---|
| `LOOM_CONFIDENCE_INJECTION` | `0` (repo) / `1` (HP compose) | master switch; off = legacy blanket injection |
| `LOOM_STRONG_MATCH_SCORE` | `8.0` | at/above this match score → full budget |
| `LOOM_MIN_INJECT_SCORE` | `2.0` | below this top score → skip injection entirely |
| `LOOM_MIN_INJECT_FRACTION` | `0.4` | weakest kept match still gets this fraction of budget |

Default-off in the code is byte-identical to blanket injection, so it is safe to ship disabled
and enable per deployment. Protocol detail in
[`bench/UPLIFT-BENCH-PROTOCOL.md`](bench/UPLIFT-BENCH-PROTOCOL.md).

### Findings-driven serving controls

The measurement in [*The Copy Ceiling*](docs/research/paper-v2/main.pdf) has three operational
consequences, each shipped as an opt-in control. They follow the finding, not a preference: the
node is in a serving regime (faithful delivery, not reasoning), so where the gate is confident
the fastest faithful path is best.

**Verbatim serving** (`LOOM_VERBATIM_MODE`). When the confidence gate engages on a
high-confidence lookup, Loom can return the canonical per-IRI markdown block directly, with its
provenance, and skip the model call entirely. This is the serving-regime finding made
operational: if the answer is a faithful restatement of one exposed block, delivering that block
verbatim is both faithful and cheap. Multi-turn and streaming requests bypass verbatim and take
the normal delegated path; a caller can opt out per request with
`"loom_options": {"verbatim": false}`.

| Env var | Default | Meaning |
|---|---|---|
| `LOOM_VERBATIM_MODE` | off | serve the canonical markdown block directly on gate-engaged high-confidence lookups, no model call |
| `LOOM_VERBATIM_THRESHOLD` | `8.0` | minimum lexical `top_score` for the verbatim short-circuit; the 8.0 default (the exact-title weight) means "serve verbatim only on a full exact-title match", so paraphrase/overlap hits still delegate |

**Exposure telemetry** (`LOOM_EXPOSURE_APPEND`). The copy-fidelity deficit (a model drops about
one exposed item in fourteen) came from the paper's deterministic exposure matcher; that
instrument is now a runtime guardrail. On an injected answer the `loom` block carries a
per-answer `exposure` object reporting the injected `targets`, how many were `delivered` and how
many `dropped`, so a consumer can see delivery fidelity per request rather than infer it. The
telemetry object is emitted regardless; `LOOM_EXPOSURE_APPEND` is the opt-in that *additionally*
appends a plain `Not covered above: …` line to the answer content when served titles were dropped.

| Env var | Default | Meaning |
|---|---|---|
| `LOOM_EXPOSURE_APPEND` | off | also append a `Not covered above: …` line to the answer content when titles were dropped (the `exposure` object is in the `loom` block regardless) |

**Thinking and budget control** (`LOOM_BACKEND_NO_THINK`, `LOOM_THINK_TOKEN_FLOOR`). The paper
found a scaffold-length × reasoning-budget interaction: think-tokens can exhaust the answer
budget on long scaffolds and return empty. For delivery-shaped grounded requests the reasoning
pass mostly costs latency for no quality, so Loom can disable thinking on engaged requests
(setting `chat_template_kwargs.enable_thinking:false` on the backend call) and hold a floor under
`max_tokens` so an answer always has room.

| Env var | Default | Meaning |
|---|---|---|
| `LOOM_BACKEND_NO_THINK` | off | set `enable_thinking:false` on the backend call for gate-engaged requests |
| `LOOM_THINK_TOKEN_FLOOR` | `1536` | floor under `max_tokens` so reasoning cannot starve the answer (the paper's empty-budget threshold) |

All three ship default-off and are set per deployment in the compose env; see
[Status](#status--honesty) for the shipped-versus-wired split.

### Retrieval fusion — the semantic fallback (wired, gated OFF)

The lexical matcher structurally misses out-of-vocabulary / paraphrase queries (it scores over
class *titles*). The fix, wired but **default-off**, is an in-process
[RuVector](https://github.com/ruvnet/ruvector) HNSW semantic fallback over the ontology-corpus
namespace (8,146 IRI-keyed `bge-small-en-v1.5`/384 records, cosine). It fires **only** on a
lexical miss, embeds the query, ANN-searches the in-process index, and hands the hits **back
into the same confidence gate as candidate seeds** — it is a candidate *source*, never a gate
bypass, and the served unit is still the resolved markdown. It stays off until it clears a
**multivariate benchmark** (in-domain recall **and** general-question non-jaggedness **and** OOV
recovery); the standing regression guard is our own naive over-retrieval result
(Δ = −0.40 [−0.58, −0.22], n=285, worst on the weakest model). It is not on today — see
[Status](#status--honesty) for the honest recall number. Design: `ADR-136` D3, `ADR-137` D5,
[`docs/design/RUST-ARCHITECTURE.md`](docs/design/RUST-ARCHITECTURE.md) §6.

---

## How — build & deploy

Loom is a **single static Rust binary** — no interpreter, no wheel. The workspace is an
eight-crate hexagonal ring (`ADR-090`): a pure `loom-domain` core with port traits, five
adapters, the `loom-scaffold` policy crate, and a thin `loom-facade` axum binary. The crate
graph is the enforcement mechanism for THE PRIZE: an adapter physically cannot return its own
row/triple/vector shape as the served unit, because only `loom-domain` mints a `CanonicalUnit`.

**Build & test** — via the `justfile` (mirrors the `RUST-ARCHITECTURE` §14 CI gates):

```bash
just            # list recipes
just build      # gate 1 — compiles on BOTH feature planes (all-features + no-default-features)
just test       # gate 2 — full suite: byte-golden scaffold parity, SPARQL clamp, router oneshot
just clippy     # gate 3 — clippy pedantic, warnings-as-errors
just deny       # gate 4 — licence + advisory gate (deny.toml)
just ci         # the full green bar (gates 1–4)
```

**Deploy** — one binary, **two compose profiles** (`ADR-137` D8; files under `deploy/`):

```bash
just docker-build     # multi-stage static musl build (from the parent context; see deploy/)
just docker-run-a     # Profile A — host-colocated on HP (network_mode: host, :8084)
just docker-run-b     # Profile B — sidecar on visionclaw_network (:8080)
```

- **Profile A — host-colocated on HP (reference).** GPU-colocated with the model
  (`loom-model` Qwen3.8-27B on `:8085`); the augmentation hot path is fully in-process
  (lexical + in-process HNSW + in-context oxigraph SPARQL), so A serves **even with no
  docker-network access**. `hp-nat.service` DNATs `:8084` onto the LAN.
- **Profile B — sidecar on `visionclaw_network` (`http://loom:8080`).** GPU-free, delegates the
  model by URL; it is the consumer-facing door (the email gateway binds
  `REASONER_BASE_URL=http://loom:8080/v1`) and the in-network home for the build/off-turn
  RuVector write channel.

Both profiles serve **byte-identical generations** for the same `commitSha` (a CI/health
assertion). The corpus is not baked: `app/mirror.sh` pulls the published, sha-addressable
**generation** from [narrativegoldmine.com](https://narrativegoldmine.com) atomically
(mixed-build sets are rejected, fail-open to the mounted volume), so Loom always serves a known
generation. Either way: **swap the model on its host and the façade is unchanged** — the
no-technical-debt-on-upgrade guarantee, and why consumers hold a Loom URL, never a raw model
port.

---

## Where it sits — the ecosystem

Loom is **[VisionFlow](https://github.com/DreamLab-AI)'s loop-closing ontology node** — the
runtime that returns the ecosystem's shared formal semantic layer to a model at generation
time. VisionFlow is the architecture the industry now calls *neurosymbolic*: thin agents over a
shared, reasoned ontology rather than thick agents wired to raw data. Loom serves that layer;
the pieces around it build, reason over and consume it.

```
knowledgeGraph  ──publishes──▶  a corpus GENERATION (OWL + reasoned closure + indexes)
 (corpus + pipeline + method)         │
   built by jjohare/logseq            ▼  mirror
                                  ┌─────────┐   scaffold-inject     ┌────────────┐
   agents / email / any client ──▶│  LOOM   │───────────────────▶ │  the model  │
        (hold the Loom URL)       │ façade  │◀───────────────────  │ (swappable) │
                                  └─────────┘   grounded answer     └────────────┘
                                      ▲
                              VisionClaw reasons over the same corpus (OWL 2 EL, Whelk-rs)
```

- **[knowledgeGraph](https://github.com/DreamLab-AI/knowledgeGraph)** — the corpus + the
  Logseq→OWL pipeline + the explorer. *Build your own ontology.* The canonical builder is
  [`jjohare/logseq`](https://github.com/jjohare/logseq) (`publish.yml` runs
  `pytest pipeline/tests` + `pipeline.validate` before deploy; `enrich-gate.yml` gates
  enrichment PRs). Loom **consumes** its published generations; it does not build or re-gate
  them — the vendored builder copy is retired (see [Status](#status--honesty)).
- **[VisionClaw](https://github.com/DreamLab-AI/VisionClaw)** — GPU graph engine + OWL 2 EL
  Whelk-rs reasoner (`ADR-099`) + governance write door; hexagonal ring law (`ADR-090`). The
  Loom design (`PRD-025`/`ADR-135`) lives there; the Whelk closure is build-time authority only.
- **[RuVector](https://github.com/ruvnet/ruvector)** — the HNSW production index (`ADR-001`)
  behind the markdown; `ProofGate<T>`/`MutationLedger` (`ADR-047`) are the attestation design
  target (see the erratum in `RUST-ARCHITECTURE` §11.5 for where they actually live).
- **agentbox** — the agentic harness; its "one brain" retrieval resolves through Loom
  (`ADR-051`), and its deferred-distillation tools submit long-running grounded jobs to Loom.

**Self-improvement.** Dreaming grounds its nightly research through this door: a
[dream cycle](https://github.com/DreamLab-AI/dream-engine) can query the reasoned ontology so
hypotheses restate checked facts, not parametric guesses — then opens a draft PR a human merges.

---

## Boundary — the Loom is the ontology only

The Loom serves the **published ontology** (the reasoned generation) and nothing else. It
**never reads or mirrors the working graph** (personal/working notes, which may become
multi-user or private). Uplift *into* the ontology happens through VisionClaw's governed
propose door, the forum/ACSP surface, or direct agentic writes into the corpus — never the
Loom; the new generation is then mirrored here read-only. (DDD BC24 invariant I11.)

---

## Status & honesty

Loom is a research/dev system on the DreamLab estate. This section is the honest split between
what is **built and audited**, what is **gated off**, and where the evidence lives.

**The substrate is the Rust node.** The eight-crate hexagonal Rust workspace *is* the
implementation. It is built, gate-green (`just ci`), and was adversarially audited by a
different model family (gpt-5.4) with all five findings remediated —
[`.claude/evidence/AUDIT-gpt54.md`](.claude/evidence/AUDIT-gpt54.md). The retired stdlib-Python
serving code (`app/loom_facade.py` et al.) lives in git history (pre-`eb678a0`); its behaviour
is preserved by the Rust port and pinned by frozen byte-parity goldens in
[`tests/golden-python/`](tests/golden-python/).

**Shipped (built + audited):**
- **Lexical retrieval + confidence-gated injection** — ported constant-for-constant; the
  served markdown block is **byte-identical** to the Python original on the golden fixture.
- **Native oxigraph SPARQL** over the reasoned closure — the `pyoxigraph` FFI is gone; the
  read-only clamp is *stronger* than Python's (PREFIX/BASE-prologue-aware LIMIT injection).
- **The façade** — all `/v1/*` and `/loom/*` endpoints, the `max_tokens` floor (integer-only,
  Python-parity), the atomic generation-verified mirror, both compose profiles.
- **The corpus generation** — 8,146 concept classes, ~282k triples in the reasoned closure,
  served as one sha-addressable generation.

**Gated off (honest numbers):**
- **The HNSW semantic fallback is default-OFF because its recall gate is RED.** Measured
  `rgb-protocol 0.816` in the current document-embedding regime — **below** the `0.87` design
  floor. So `LOOM_SEMANTIC_FALLBACK` stays off; turning it on requires a query-shaped embedding
  (or a bench-justified floor) that clears the gate, **not** a threshold fudge. The wiring is
  done and tested; the *default* does not change until the multivariate bench passes.
  (`.claude/evidence/EXP-008.evidence.md`.)
- **The two-profile generation-parity live assertion** and the cold-start timing gate are the
  operational tail — the code implements both profiles; the live A≡B health assertion runs at
  deployment cutover.

**Findings-driven serving controls (this release, default-off).** The three controls above
(verbatim serving `LOOM_VERBATIM_MODE`, exposure telemetry `LOOM_EXPOSURE_APPEND`, thinking and
budget control `LOOM_BACKEND_NO_THINK` / `LOOM_THINK_TOKEN_FLOOR`) land as a consequence of the
paper's serving-regime, copy-fidelity and budget-interaction findings. Each ships **off by
default**, so enabling one is a per-deployment choice and the default path is unchanged. They are
reported here as new wiring, not as separately re-benchmarked claims: the evidence for *why* they
exist is the paper; the evidence that each *is on* is the compose env of a given deployment.

**Provenance.** The corpus it serves is **AI-generated synthetic content produced under human
direction, by design** — an ontology testbed, not an authoritative encyclopaedia. Every
grounded answer is traceable to a corpus generation; that provenance attests traceable
generation, not human authorship. "Platform for any ontology connector" is earned when a second
provider lands — today Loom is one node with a provider-plugin seam, stated plainly.

Evidence & audit: [`.claude/evidence/`](.claude/evidence/) (per-expectation executed evidence),
[`.claude/evidence/AUDIT-gpt54.md`](.claude/evidence/AUDIT-gpt54.md) (the adversarial audit +
remediations).

---

## Design & research

Read the docs map first: **[`docs/README.md`](docs/README.md)** — the navigation + authority chain.

- [`docs/design/`](docs/design/) — the governance set: `PRD-025` (product capstone), `PRD-026`
  (consolidation), `PRD-027` (Rust re-engineering requirements), `ADR-135` (keystone node
  boundary), `ADR-136` (tooling allocation), `ADR-137` (Rust re-platform + both profiles),
  `RUST-ARCHITECTURE.md` (the build blueprint), the `ddd-ontology-loom-context.md` bounded
  context (source-of-record for THE PRIZE), and agentbox `ADR-051` (harness client). The design
  was adversarially reviewed (five-lens panel) before authoring.
- [`docs/research/README.md`](docs/research/README.md): the evidence index (the paper, its data
  and harness, and the precursor uplift reports).
- [`docs/research/paper-v2/main.pdf`](docs/research/paper-v2/main.pdf): *The Copy Ceiling*, the
  node's primary evidence base (the frozen preprint; LaTeX source alongside the PDF).
- [`docs/research/ontology-uplift-report.pdf`](docs/research/ontology-uplift-report.pdf): the
  earlier typeset two-study report (precursor).
- [`docs/research/`](docs/research/) — the ontology-uplift benchmark markdown: the local-model
  report, the [Gemini 3.7 Flash cloud companion](docs/research/report-gemini-3.7-flash.md),
  per-domain / per-template breakdowns, and the honesty notes.
- [`bench/`](bench/) — reproduce it: `bench_ontology_uplift.py` (objective, graph-derived gold,
  paired bootstrap CIs), `UPLIFT-BENCH-PROTOCOL.md` (+ the WS-O multivariate fusion gate).

## Licence

See the sibling [knowledgeGraph](https://github.com/DreamLab-AI/knowledgeGraph) licensing for
the corpus/data terms; Loom's own code is under the repository `LICENSE`.
