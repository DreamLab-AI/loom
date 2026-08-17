# Differentiation analysis — what is *singular* about the Loom paper, and the requirements that constrained it

*Author: differentiation analyst (agent). Sources, in order read: `main.tex` + `refs.bib`;
`README.md`, `LOOM-POSITIONING.md`, `ADR-135`, `ADR-136`, `RUST-ARCHITECTURE.md §0`, `research-notes.md`;
RuVector `project-state` memory (loom/prize/copy-ceiling/consolidation/recall-gate); adversarial web
checks (perplexity) against 2024–2026 RAG-eval, KG-grounding, and answer-leakage literature.*

> **Adversarial stance taken throughout:** for each candidate differentiator I assumed prior work
> *already does it* and searched until the search failed. A verdict of SINGULAR means the specific
> artefact — not the general theme — resisted that search. Where the theme is old and only the
> packaging is new, I say so and mark it down.

---

## 0. The one-line recommendation

**The paper should stake its singular claim on differentiator (a): the per-item *copy ceiling* and
the signed *gain over copy*, reported as the mandatory control for any grounding eval whose gold is
derived from the injected corpus — evidenced by the uniformly-negative gain across ten models.**
It is the only candidate where the *specific artefact* (a deterministic extractor-recall reference
point per item, plus a signed cross-model faithfulness metric read against it) survived an
adversarial literature search, and it is the only one *established empirically here* rather than
designed or asserted. Everything else is either a known mechanism (c), product positioning (d), or
an engineering virtue (b, e) — real, but not a defensible *research* contribution on its own.

---

## 1. Verdict table

| # | Differentiator | Verdict | Strongest rival (arXiv / venue) | Why it survives / fails |
|---|---|---|---|---|
| **a** | Copy ceiling + signed gain-over-copy as the standard control; uniformly-negative gain across 10 models ⇒ re-encoding a curated corpus is value-destroying on this metric | **SINGULAR** (novel packaging of a shared insight) | `2606.05633` *Answer Presence Drives RAG Rewriting Gains*; also `2605.08838` SeedRG (`Acc_gold` = "upper bound on answerability"); SePer ICLR 2025 `ixMBnOhFGd` (retrieval utility = belief change) | The *phenomenon* (answer leakage / answer-presence inflates RAG) is well established and SHARED. The *artefact* — a per-item extractor-recall ceiling `c_i` as the reference point, and a signed gain read across models as a faithfulness discriminator, reported as standard — was not found in prior work. The negative-gain-across-10-models result is genuinely new. |
| **b** | Human-scrutible canonical unit (per-IRI markdown-with-ontology) enforced *architecturally* (port return types, I-P1) vs GraphRAG/G-Retriever opaque encodings | **WEAK** (as a paper claim) / SINGULAR only as an engineering stance | G-Retriever `2402.07630` (GNN soft-prompt — the contrast class); GroundedKG-RAG `2604.04359` (node/edge grounded in source sentences, inspectable); OG-RAG (EMNLP 2025 main, `2025.emnlp-main.1674`) | "Human-readable, attributable retrieval unit vs opaque summary/embedding" is a *known virtue* and others already ground retrieval to inspectable source. The compile-time enforcement (I-P1) is real and rare, but it is a **software-engineering assertion, unmeasured and unfalsifiable in the paper** — no human-audit study, no attribution-precision number. Not a research contribution until measured. |
| **c** | Multivariate bar: in-domain fidelity AND out-of-domain non-regression measured together, confidence gate as mechanism, general-null as a first-class axis | **SHARED** | Self-RAG `2310.11511`; Adaptive-RAG `2403.14403`; Mallen `2212.10511`; GaRAGe (ACL 2025 findings, deflection-when-no-grounding) | Confidence-/complexity-gated selective injection as the anti-jaggedness mechanism is textbook post-2024. Reporting non-regression as its own axis is good practice but not novel. Novel *only in combination* with (a); cannot carry the paper alone. |
| **d** | Requirements-constrained serving regime: LAN-private, model-swappable (model-is-a-URL), curation amortised once — an enterprise/private regime the eval literature ignores | **WEAK / SHARED** | OG-RAG `2025.emnlp-main.1674`; enterprise/private-RAG line (REALM `2002.08909`, Atlas `2208.03299`, RETRO `2112.04426`) | Ontology-grounding for private domains, attribution, and amortised curation are covered by OG-RAG and the private-corpus RAG line. The *specific* "curated corpus **vs** generic web search, amortised, behind a swappable façade" comparison genuinely is under-studied — but it is **designed, not measured** here (§Multivariate Bar axis 3 is future work). Positioning, not contribution. |
| **e** | Benchmark-honesty machinery: recall gate shipped default-OFF because measured 0.816 < 0.87 floor — negative-result-driven feature gating | **WEAK** | No arXiv rival; the "quality-gate / fail-the-build-on-regression" CI pattern is standard practice (confident-ai, redis RAG-eval guides) | Shipping a feature disabled because a benchmark is red is exemplary integrity and strong *rhetoric* for the paper's credibility, but it is an engineering practice, not a differentiating research claim. Keep it as a discussion-section credibility signal, not a contribution bullet. |

**Net:** one SINGULAR (a), one SHARED (c), three WEAK/positioning (b, d, e). The paper currently
lists five contributions; the defensible core is **(a)**, framed as a reusable methodological
standard, with (c) as the necessary second axis and (b/d/e) demoted to design/positioning/credibility.

---

## 2. Per-differentiator detail

### (a) Copy ceiling + gain over copy — **SINGULAR (novel packaging of a shared insight)**

**(i) Precise claim.** For each question compute `c_i = exposed_i / |gold_i|`, the recall a
deterministic no-op extractor of *exactly the text shown to the model* would score. The mean `c_i`
is the recall a perfect copier achieves; **gain over copy = mean scaffold recall − mean copy
ceiling** turns delivery-fidelity into a signed, model-discriminating quantity. Across ten models it
is uniformly negative (−0.067 … −0.022): no model recovers gold beyond what the exposure check
already finds, so on this lexical metric they differ only in how much surfaced gold they *drop*. The
strong corollary the paper draws: re-encoding an already-curated corpus is, on this axis,
value-destroying — the curation carries the answer, the model can only lose fidelity relative to a copy.

**(ii) Closest prior work.**
- **`2606.05633` — Answer Presence Drives RAG Rewriting Gains (2026).** The strongest rival. Uses
  controlled removal/injection of the gold answer in retrieved/rewritten context; removing gold drops
  reader F1 28–64 points, injecting it lifts F1. It *proves answer-presence drives the lift* and even
  audits single-`[MASK]` leakage probes as sentinel-fragile. But it does **not** define a per-item
  extractor-recall *ceiling* as a reference point, and does **not** report a signed gain across models
  as a faithfulness ranking. It establishes the phenomenon; it does not ship the metric.
- **`2605.08838` — SeedRG / leakage-free benchmarks (2026).** Defines `Acc_gold` (accuracy given only
  gold context) as "an upper bound on answerability" and `Acc_gold − Acc_no_ctx` as "answerability
  accuracy." This is the *closest formal cousin*: a gold-context upper bound. Difference: SeedRG's
  upper bound is the *full model given gold*, used to *filter leakage out of a benchmark*; the copy
  ceiling is a *no-op extractor of the shown text*, used to *characterise the served answer* and
  deliberately keeps the leakage as the measurement.
- **SePer, ICLR 2025 (`ixMBnOhFGd`).** Retrieval utility `U = P(a*|d) − P(a*)` — belief change from
  retrieval. Same "gain over a baseline" shape, but the baseline is the *parametric* model, not an
  *extractor of the context*; and it is a probabilistic belief measure, not a lexical extractor ceiling.
- **Kaushik & Lipton 2018 (input-only baselines); Sainz 2023 / Golchin 2024 (contamination audits).**
  Already cited. The paper correctly positions the copy ceiling as their RAG-era, graph-grounded
  generalisation — input-only baselines answer "how much competence is shortcut," contamination audits
  detect leakage post hoc; the copy ceiling makes the deliberate exposure the metric's reference point.
- **RAGAS "context recall" / "context utilization" / GraphRAG-Bench "evidence recall."** All measure
  whether the *context contains* the answer, or whether it was *used*. None is a per-item extractor
  ceiling used as the reference for a signed model-discriminating gain.

**(iii) Verdict: SINGULAR** — but honest about what kind. The *insight* (leakage/answer-presence
inflates graph-derived RAG scores) is SHARED and, post-`2606.05633`, actively studied. What is not in
the literature is (1) the copy ceiling as a *standard reported control* computed per item from the
exact shown text, and (2) the *signed gain-over-copy read across a 10-model sweep* as the axis that
separates models where raw uplift cannot. Claim the metric and the multi-model result, not the
underlying phenomenon.

**(iv) What would make it rigorous.**
1. **Negative controls** (already flagged as required in the Discussion): irrelevant, shuffled-target,
   and label-masked scaffolds. If gain-over-copy stays ~0 under a shuffled scaffold, the metric is
   measuring exposure faithfully; if it moves, the ceiling is leaking through position.
2. **Break the lexical-artefact objection.** The uniformly-negative gain could be an artefact of
   lexical scoring under-counting paraphrase. Re-score a subset with an NLI/entailment or RAGAS-style
   faithfulness judge and show the sign survives — otherwise "value-destroying" is a scorer property.
3. **Convergent validity.** Correlate per-model gain-over-copy with an independent faithfulness metric
   (RAGAS faithfulness, context-utilization) to show it ranks models the same way — that upgrades it
   from "a lexical curio" to "a faithfulness axis."
4. **Show it discriminates where it matters.** Report gain-over-copy on the multi-hop / relational
   questions (research-notes §5 flags these as unrun) — if some model exceeds the ceiling there
   (positive gain = recovering gold the exposure check missed), that is the headline the negative
   in-domain result currently forecloses.

### (b) Architecturally-enforced human-scrutible canonical unit — **WEAK as a paper claim**

**(i) Precise claim.** The served, auditable unit is one per-IRI markdown-with-ontology block
(`dfull` prose + typed relations + `corpusNature`); every retrieval/attestation engine (lexical,
HNSW, oxigraph SPARQL, mincut, GNN, ProofGate) is an accelerator strictly *behind* it, and this is
enforced at compile time — every port method returns or resolves to an `Iri` addressing a
`CanonicalUnit` (Invariant I-P1, `RUST-ARCHITECTURE §0`; ADR-136 D1 "THE PRIZE"). The rejection is
explicit and named: the GraphRAG/G-Retriever trajectory where knowledge degrades into opaque LLM
community summaries or GNN-encoded soft-prompt subgraphs.

**(ii) Closest prior work.** G-Retriever (`2402.07630`) is the archetype of the opaque contrast
class (PCST subgraph → GAT → soft prompt). GroundedKG-RAG (`2604.04359`) already grounds each
node/edge in the source sentence and is inspectable. OG-RAG (`2025.emnlp-main.1674`) traces answers
to source document + ontological rule ("30% faster attribution"). Atlan's "lineage integrity /
context trustworthiness" framing names the same virtue for industry retrieval indexes.

**(iii) Verdict: WEAK (paper) / SINGULAR (engineering).** "Human-readable, attributable unit vs
opaque encoding" is a recognised virtue and others ground to inspectable source. The genuinely
unusual part — enforcing the invariant in the *type system* so an adapter physically cannot serve a
vector row / triple / summary in the markdown's place — has **no ML-paper equivalent**, but it is a
design assertion the paper neither measures nor could falsify. As written it is a claim about the
*system*, not a finding.

**(iv) What would make it rigorous.** A human-audit study: time-to-verify and attribution-precision
for an answer traced to the per-IRI markdown unit vs the same answer traced to (a GraphRAG community
summary / a top-k embedding hit). This is exactly the "attribution (designed)" axis the paper
already defers in §Multivariate Bar — promoting it from designed to measured is what would convert
(b) from positioning into a contribution.

### (c) The multivariate bar (in-domain fidelity + OOD non-regression, gated) — **SHARED**

**(i) Precise claim.** A deployable private-knowledge system must clear two axes at once: near-copy
in-domain delivery, and *no jaggedness* on general/novel questions, achieved by confidence-gated
injection that supplies nothing when it judges a query off-domain; the general-null (Δ intervals all
include zero across five models) is reported as a first-class axis, not a footnote.

**(ii) Closest prior work.** Self-RAG (`2310.11511`, learn-to-retrieve/critique), Adaptive-RAG
(`2403.14403`, gate on question complexity), FLARE (`2305.06983`), Mallen (`2212.10511`, adaptive
retrieval on popularity), GaRAGe (ACL 2025 findings, deflection when grounding is irrelevant), plus
the CTI systematic study (`2604.11419`, abstention + non-regression as explicit failure modes). The
"gate both halves separately, non-regression as a CI gate" practice is standard in 2026 RAG-eval guides.

**(iii) Verdict: SHARED.** The mechanism (confidence-gated selective injection to avoid displacing
parametric knowledge) and the outcome framing (measure non-regression, not just uplift) are both
established. The paper's contribution here is *execution and combination* with (a) — legitimate, but
not a standalone differentiator.

**(iv) What would make it rigorous.** The paper already concedes the OOD arm is near-ceiling and
underpowered (20/stratum, 40 gate-engaged gradings, wide intervals → "no jaggedness detected," not
proven). Power it up (more OOD items, a pre-registered equivalence margin) and add the web-search
grounding arm (axis 3) so non-regression is measured against the *right* baseline, not the bare model.

### (d) Requirements-constrained private/swappable serving regime — **WEAK / SHARED**

**(i) Precise claim.** A serving regime the eval literature mostly ignores: LAN-private (no cloud
calls on the hot path), model-swappable (model-is-a-URL; Gemma→Muse→Qwen3.8 with zero consumer
change), curation amortised once and reused per query — with the honest comparison being curated
corpus vs generic web search, not the bare model.

**(ii) Closest prior work.** OG-RAG (`2025.emnlp-main.1674`) covers ontology-grounded private-domain
QA with attribution gains; the private-corpus RAG line (REALM `2002.08909`, Atlas `2208.03299`, RETRO
`2112.04426`) covers grounding on non-parametric private stores. Web-search-grounding-is-noisy is
covered by the refs already in `refs.bib` (Liu verifiability `2304.09848`, Cuconasu power-of-noise
`2401.14887`).

**(iii) Verdict: WEAK / SHARED.** These are real deployment constraints and good positioning, but
each element exists in the literature or is product engineering. The one genuinely under-studied
piece — a head-to-head *curated-corpus vs live-web-search* grounding comparison with amortised-cost
accounting — is **designed, not run** (the README and positioning both name it as the missing arm).
Until measured it is a hypothesis, not a differentiator.

**(iv) What would make it rigorous.** Run axis 3: same models, same in-domain questions, three
grounding arms {bare, web-search-grounded, ontology-scaffold}, reporting correctness *and* cost/latency
per query so "amortised curation beats per-query web retrieval in-domain" becomes a measured claim.

### (e) Negative-result-driven feature gating (recall gate default-off) — **WEAK**

**(i) Precise claim.** The HNSW semantic fallback ships **disabled** because its measured recall
(rgb-protocol 0.816) is below the 0.87 design floor; enabling it requires clearing a multivariate
bench, not a threshold fudge. The standing regression guard is the project's own naive over-retrieval
result (Δ = −0.40 [−0.58, −0.22], n=285, worst on the weakest model).

**(ii) Closest prior work.** No arXiv rival — this is an open-science/engineering-integrity practice.
The "quality gate / fail-the-build-on-metric-regression" pattern is standard CI/CD-for-RAG (confident-ai,
redis, slavadubrov 2026 guides all describe metric-gated merges).

**(iii) Verdict: WEAK.** It is exemplary and it materially strengthens the paper's *credibility*
(the authors gate their own features on their own red benchmarks), but it is a practice, not a
research contribution. Keep it in Discussion/Limitations as a trust signal; do not list it as a contribution.

**(iv) What would make it rigorous.** It already is, as engineering. To make it *say* something,
report the gate's decision curve: at what measured recall does enabling the fallback stop degrading
the multivariate bar? That turns "we kept it off" into "here is the floor a semantic fallback must
clear to help," which is a transferable finding.

---

## 3. The requirements narrative (the background arc)

The minimal ordered chain of real requirements that *constrained* the design, each with its design
consequence and ADR source. Read as a story: the business need forces curation, curation forces
auditability, auditability forces the accelerator boundary, privacy forces local delegation,
locality forces model-swap, swap forces an LLM-free bounded retrieval path, injection risk forces the
gate, and trust forces provenance. This is the arc the paper's background should walk.

1. **Private-knowledge need ⇒ curated, amortised corpus.** The model must answer over a large,
   private, in-domain corpus it *cannot* have memorised (raw ≈0.26 proves the privacy). *Consequence:*
   the answer is grounded in a human-directed, provenance-tracked ontology whose expensive curation is
   paid **once** and reused per query; corpus-lifecycle ownership + sha-addressable *generation* mirror
   (not baked in), single-source-of-truth build so every index is a projection of one authoritative
   copy. The copy ceiling is the eval-time expression of "the curation, not the model, carries the
   answer." *(ADR-135 D2; ADR-136 D4; LOOM-POSITIONING.)*

2. **Trust need ⇒ human auditability at single-entity granularity (THE PRIZE / I-P1).** A grounded
   answer is only trustworthy if a human can read, review and audit the served knowledge end-to-end.
   *Consequence:* the canonical served unit is one per-IRI markdown-with-ontology block; every port
   method returns or resolves to an `Iri` addressing a `CanonicalUnit`; the GraphRAG/G-Retriever
   opaque-encoding trajectory is explicitly rejected. *(RUST-ARCHITECTURE §0; ADR-136 D1.)*

3. **Auditability ⇒ accelerators strictly behind the markdown.** Nothing a machine indexes may become
   the thing a human must trust instead of the markdown. *Consequence:* lexical index, HNSW, oxigraph
   SPARQL, mincut, GNN and ProofGate are projections/adapters that find, rank or attest the unit and
   resolve back to its IRI — enforced by the crate ring (an adapter physically cannot reach the router),
   not by reviewer vigilance. *(ADR-136 D1–D3; RUST-ARCHITECTURE §0–1.)*

4. **Privacy / LAN-confinement ⇒ local model delegation, no cloud on the hot path.** Content and answers
   must stay on the LAN (the email-privacy requirement). *Consequence:* generation is delegated to a
   LAN/local backend by URL (`DISTILL_BACKEND_URL`); the augmentation hot path is fully in-process
   (Profile A serves even with no docker-network access). *(ADR-135 D1; ADR-136 §3; README; workspace
   env facts.)*

5. **Local-model churn ⇒ model swap without consumer change (model-is-a-URL).** Local/LAN models change
   often (Gemma → Muse-Glimmer → Qwen3.8-27B → next). *Consequence:* a stable OpenAI-compatible façade;
   the backend is one config line; model identity rides in the *result*, never the endpoint; swapping
   the model touches zero consumers — the "no technical debt on upgrade" guarantee, and precisely what
   lets the study hold grounding constant while varying the model under test. *(ADR-135 D1/D1.2; README.)*

6. **Bounded latency + no-LLM retrieval ⇒ grounding must work with no model, fast.** Retrieval cannot
   depend on a model call and must be cheap on the hot path. *Consequence:* `/loom/scaffold`, `/loom/
   sparql`, `/loom/search` need no model; the lexical inverted index is <50 ms over 8,146 titles; HNSW
   is an in-process *index read*, not a model call; consistent with one-brain / no-hot-path-LLM.
   *(README endpoint table; ADR-136 §3; ADR-135 D-reconciliation with ADR-112.)*

7. **Injection interference ⇒ confidence-gated, benchmark-first non-jaggedness.** Injected context can
   *displace* correct parametric knowledge on off-ontology queries. *Consequence:* a single injection
   authority scales budget to the retrieval score and skips below threshold; features that could add
   noise are benchmark-gated (semantic fallback default-off at 0.816 < 0.87; the −0.40 over-retrieval
   result is the standing regression guard). *(ADR-136 D3; README confidence section; LOOM-POSITIONING
   axis 2.)*

8. **Never-mixed provenance ⇒ attestation + boundary discipline.** Every grounded answer must be
   traceable to a named, versioned source, and a bad write must be blocked before it reaches the
   corpus. *Consequence:* each response carries the `loom` block (`injected_tokens`, `grounding`,
   `generation`, `fusion_path`) plus `corpusNature` honesty metadata; write-time admission control with
   local domain predicates + tamper-evident ledger (ProofGate/MutationLedger substrate); the Loom serves
   only the published ontology, never the working graph. *(ADR-135 D6/D7; ADR-136 D5; README boundary /
   DDD BC24 I11.)*

---

## 4. Honest closing note for the team lead

- The paper's abstract already leads with (a) and treats (c) as the second axis — that ordering is
  correct. My only structural recommendation is to **stop listing (b), (d), (e) as peer
  "contributions"** (contributions 2/4 of the current five over-claim): recast them as *design
  requirements* (§3 arc) and *credibility signals*, so a reviewer cannot pick them off as "known."
- The single most dangerous reviewer attack on (a) is the **lexical-scorer artefact** objection: that
  uniformly-negative gain is an artefact of lexical matching under-counting paraphrase, not evidence
  about faithfulness. The rigour items in §2(a)(iv) — semantic re-scoring + negative controls +
  convergent validity — are not optional polish; they are what stands between SINGULAR and "a lexical
  curio." Prioritise them in the v2 experiment harness (tasks #27/#28).
- `2606.05633` (Answer Presence Drives RAG Rewriting Gains) **must be cited and distinguished** — it is
  the nearest neighbour and its absence would read as a gap. Frame: it proves answer-presence drives
  the lift via interventions; we make the exposure a *standing per-item control* and read a signed
  cross-model faithfulness axis off it. Also add SeedRG (`2605.08838`) and SePer (ICLR 2025) as the
  "upper-bound / retrieval-utility" cousins, and OG-RAG (`2025.emnlp-main.1674`) + GroundedKG-RAG
  (`2604.04359`) as the ontology-grounding / inspectable-unit rivals.
