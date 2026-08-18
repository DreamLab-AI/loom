# Loom — positioning: grounding performant models in private customer knowledge

*What the Loom is for, stated plainly, and the multivariate bar it has to clear. This note
frames the design docs (`PRD-025`, `ADR-135`) and the research (`docs/research/`) around the
actual product goal rather than an academic abstraction.*

> **Substrate note (2026-08-17).** The Rust re-platform (`ADR-137` / `PRD-027`) changes the
> *substrate* under the Loom — stdlib Python → a single Rust binary — not the goal below, the
> multivariate bar, or the "curated corpus **vs** a generic web search" frame. Everything in
> this note holds unchanged; only how the façade is built moved.

> **Measurement note (2026-08-18).** The copy-ceiling framing below (faithful delivery is the
> product, not a confound) is now the measured result, not a forecast:
> [`docs/research/paper-v2/main.pdf`](../research/paper-v2/main.pdf) (*The Copy Ceiling*) reports
> a 0.964 ceiling, a uniformly negative gain over copy across ten models, an exposure/recovery
> decomposition of 11,360 gold items, and a production paired study (loom−raw +0.27 pooled, +0.79
> where curation is deepest) with an out-of-domain equivalence result inside a ±0.25 margin. The
> "what this means for evaluation" plan below (axes 2–4) is partly superseded: axis 1 and the
> paired study are done; the web-search baseline (axis 3) and attribution precision (axis 4)
> remain open. Evidence index: [`docs/research/README.md`](../research/README.md).

## The goal

**Make any (swappable) model performant against large, important, *private* customer datasets,
where answers to questions are accurate because they are grounded in curated, vetted, in-domain
knowledge.** The Loom serves a retrieved slice of a reasoned ontology into a model's context at
query time. The corpus is human-directed and provenance-tracked; the value is delivering
knowledge the model *cannot* have memorised, with an authoritative source behind every answer.

The unaided-vs-grounded gap is the proof the knowledge is genuinely private: on in-domain
graph-derived questions, models score ~0.36 from parametric memory alone and ~0.94 when given
the curated context. The model does not know the customer's domain; the Loom supplies it.

## Faithful delivery is the product, not a confound

A curated ontology **contains** the answers to in-domain questions by design. Measured as a
"copy ceiling" (the recall a no-op extractor of the injected context would get), that ceiling is
~0.96 on Gemini 3.7 Flash and the model tracks just under it. Read as an academic gotcha this
looks like "the model is only copying." Read as a product it is exactly right: **the answer is
trustworthy because the source is trustworthy, and the model faithfully restates the vetted
fact.** High extractive fidelity to an authoritative curated source is the feature — the same
property that makes the output attributable and non-hallucinated.

This is *pre-researched token spend*: the expensive, rigorous work of building and reasoning over
the ontology is amortised once, then reused cheaply on every query. Compared with grounding the
same model in a generic **web search** (Brave/Google) for an in-domain question, the curated
corpus is vetted, structured, higher-precision and cheaper per query. Web search is the
*fallback for out-of-domain*, not the in-domain competitor.

## The bar is multivariate

A single "in-domain recall" number is not the target. The Loom has to clear several axes at once:

| Axis | Requirement | Mechanism |
|---|---|---|
| **Locally grounded (in-domain)** | Excellent recall; faithful, attributable delivery of the curated answer | Static scaffold injection from the deployed generation |
| **General / new questions** | **Not jagged** — ontology injection must not degrade questions the model already handles; ideally neutral-to-positive | Confidence-aware selective injection: strong on-ontology → full budget; off-ontology → skip |
| **Out-of-domain / novel** | Fall back to internet-research agents; still **inherit the ontology's domain underpinning** (vocabulary, entity resolution, framing) | Gate + downstream web-research path; the ontology frames the query even when it doesn't answer it |
| **Cost / provenance** | Amortised curation; every answer carries its corpus generation and grounding confidence | `loom` response block (`injected_tokens`, `grounding.top_score`, `generation`) |

The interference risk is real and is why selective injection exists: injected context can *displace*
a model's own correct parametric knowledge when it is weak or off-topic, degrading general
questions. The gate (skip below a confidence threshold, scale to `top_score`) is what keeps the
system from being jagged on the general set while staying strong on the local one.

## What this means for evaluation

The benchmark must be **multivariate**, and the current graph-derived suite only covers the first
axis. The honest, complete evaluation design is:

1. **In-domain recall** (have): paired raw-vs-scaffold on graph-derived questions, reported
   *against the copy ceiling* so "faithful delivery" is measured, not assumed.
2. **General-question robustness / jaggedness** (needed): an *out-of-domain* question set (general
   knowledge, off-ontology) run raw-vs-scaffold to show injection does **not** regress it — the
   confidence-injection A/B, currently the open item in `bench/UPLIFT-BENCH-PROTOCOL.md`.
3. **Web-search baseline** (needed): a third grounding arm — same model, same questions, grounded
   in live web-search results instead of the curated scaffold — to substantiate "curated beats
   generic web in-domain" and "web is the right fallback out-of-domain."
4. **Attribution / faithfulness** (needed): report that grounded answers are traceable to a corpus
   generation (provenance is already emitted), and, ideally, an attribution-precision metric.

The multi-model sweep (10 models, `bench/sweep/`) feeds axis 1 across providers; axes 2–4 are the
next experiments and are what turn the internal result into a defensible claim about a private
knowledge-grounding *system*, not a single recall number.

## One-line summary

The Loom is a **private-knowledge grounding layer**: it makes swappable models answer accurately
and attributably on a customer's in-domain corpus they could never know parametrically, delivers
that curated knowledge faithfully and cheaply (pre-researched, amortised), and — via
confidence-gated injection and a web-research fallback — does so **without going jagged** on the
general questions the model should still handle on its own.
