# The private-corpus programme, as requirements

**Status:** systems design note. Requirements and implementation status, not results.
**Supersedes:** §`sec:datalake` ("Ideal State: A Private Corporate Data Lake", D1–D7) of the v7
text, archived at `../gain-over-copy-paper/archive/v7/main-v7.tex`,
which was cut from the manuscript in v8 and has not returned.
**Date:** 21 September 2026. **Manuscript in force:** `docs/research/gain-over-copy-paper/gain-over-copy-paper.tex` (v9.1).

The v6 section was deleted because its framing did not follow from the measurements, not because the
properties it described are unwanted. Most of them are ordinary good practice for an organisation
that must answer from its own corpus without the content leaving the boundary, and they remain the
design targets for this node. This note keeps them as requirements, states how far each is built,
and pins each tested claim to the paper section that tests it, so that the programme survives
without the reasoning the evidence does not support.

## What v6 claimed that does not follow, and must not be reinstated

Three claims from that section are retracted. They are written out here so nobody reconstructs them
from the requirements below.

1. **One copy ceiling validating every seam.** v6 proposed putting "the same judge-free control at
   every seam" and treated that single move as the through-line justifying the other six properties.
   The control is defined for a task with a natural no-op extractor over shown text and a gold set
   known in advance. A choice task has no such extractor, and a baseline constructed from a ranker is
   a different instrument whose sensitivity is a property of the baseline (paper §`sec:accounting`,
   §`sec:companion`; companion note §`sec:naming`). A ceiling computed at one seam says nothing about
   another, and the ceiling is an offline benchmark measurement, not a live per-request gate.
2. **Negative gain over copy as proof that reasoning is absent or is being paid for wastefully.**
   v6 argued that "a lake that pays for synthesis over a curated corpus is paying for reasoning the
   instrument shows is not occurring". Zero or negative gain is compatible with successful reasoning:
   a question can require following two relations while its answer name is already a string in the
   context, so exposure is 1, a perfect answer scores 1, and the gain is exactly zero
   (paper §`sec:ceiling`). The accounting bounds what the evidence can credit, not what the model did.
3. **Rejecting an extraction that fails to exceed its own source's exposure.** v6's promotion gate
   rejected "an extraction whose recall does not exceed what its own source already exposes ... as
   restatement". That penalises faithful extraction, whose objective is precisely to recover what the
   source says. The gate is name-completeness of the source, not positive gain over it
   (paper §`sec:analysis`), and it is necessary and insufficient: term-stuffing inflates a ceiling
   while leaving a block wrong.

A fourth v6 line goes with them: that the system "delivers vetted knowledge faithfully and
attributably". "Delivery" is scored as recall of exposed gold target names and nothing more, and
attribution precision is unmeasured (paper §`sec:intro`, §`sec:roadmap` question 4).

## The requirements

Each item states the requirement, what is built and where, what has been tested and in which paper
section, and what remains untested. "Tested outcome" means a measurement in the released record;
everything else is untested regardless of how confident the design is.

### R1. Incoming assertions are separated from serving-approved knowledge

- **Requirement.** Extraction output lands somewhere readable and attributable without becoming
  servable. Admission to the served corpus is a separate, governed act.
- **Implemented.** Per-episode ledger pages inside the graph, written by the VisionFlow ingest
  pipeline and wikilinked to the topics they concern; ledger pages carry no ontology markup, so
  nothing enters the served corpus by default, and content acquires markup only at batched section
  regeneration after approval (paper §`sec:lifecycle`). The served side mirrors a single
  sha-addressable generation atomically (§`sec:system`).
- **Tested.** The alternative landing zone, direct enrichment of thin pages, was rejected on judged
  evidence: mean negative across twenty pairs under both rubrics, and the section-splice mechanic
  could not apply on thirteen of them (§`sec:lifecycle`, §`sec:pagejudge`).
- **Untested.** The ledger path at volume, reader behaviour on ledger pages, and whether the split
  changes the quality of what is eventually promoted.

### R2. Additions carry provenance and are reversible

- **Requirement.** Every addition is traceable to its source and can be withdrawn without data loss.
- **Implemented.** Candidates reach the graph's governed propose/approve queue as scored dossiers
  with provenance down to assertion fingerprints; promotion is idempotent by fingerprint, so the
  at-least-once delivery inherent to such pipelines cannot double-promote. The ledger is the
  append-only source of truth for extracted assertions and regenerated sections are disposable
  projections, which is why a diluting projection can be discarded (paper §`sec:lifecycle`).
  Mature curated prose predates the ledger and is never rebuilt from it.
- **Tested.** One refusal: the thin-page enrichment branch, refused on its own judged evidence
  (§`sec:pagejudge`).
- **Untested.** Reversibility at scale, fingerprint collision behaviour, and recovery time for a
  withdrawn generation.

### R3. The model is substitutable behind a stable interface

- **Requirement.** The deployed model is a URL behind a façade. Replacing it is a configuration
  change measured by evaluation, with no consumer-side change.
- **Implemented.** A hexagonal workspace in which only the domain core mints a served unit
  (`crates/loom-domain/src/ports.rs`), with one adapter per backend; generation is delegated to the
  configured backend URL through an OpenAI-compatible façade
  (`crates/loom-backend-openai`, `crates/loom-facade/src/routes`), and responses carry model identity
  (paper §`sec:system`). Retrieval, gating, scaffold serialisation and read-only SPARQL need no model
  at all (`crates/loom-scaffold`, `crates/loom-graph-oxigraph`; §`sec:system` Table 1).
- **Tested.** Ten models were swept behind a fixed offline scaffold, holding grounding constant
  (§`sec:tenmodel`), and the production study holds the serving path constant while varying the path
  (§`sec:live`). That is model substitution exercised as an evaluation variable.
- **Untested.** Substitution as an operational procedure: no rollback drill, no measurement of
  consumer impact during a swap, and the production study fixes one backend (§`sec:limits`).

### R4. Decisions at operational boundaries are explicit, and abstention is available

- **Requirement.** Where the system chooses whether to inject, which source to use, or whether to
  answer at all, the decision is a single named authority rather than an emergent effect, and
  declining is a first-class outcome rather than a competitor option.
- **Implemented.** A single injection authority: the lexical matcher and confidence gate form the
  sole path by which any candidate reaches the model, scaling the scaffold budget to the retrieval
  score and injecting nothing below a threshold (`crates/loom-scaffold/src/policy.rs`;
  paper §`sec:system`, §`sec:ood`). An absence-keyed fallback exists for the no-match case
  (§`sec:paraphrase`). The typed-decision seam with an independent-scoring judge and a threshold
  decline rule is implemented in the agent runtime, not in this node (companion note).
- **Tested.** The gate skipped 60 of 100 off-domain gradings, making harnessed and bare answers
  identical, and detected degradation was confined to over-firing (§`sec:ood`). The absence-keyed
  fallback fired on 2 of 506 paraphrased items (§`sec:paraphrase`). On the routing seam, the judge's
  advantage over a repaired ranker baseline sits on boundary and late-clause turns and is nil on
  ordinary picking (companion note §`sec:decomp`).
- **Untested.** Whether the gate *causes* the out-of-domain non-regression: there is no matched
  always-inject ablation (§`sec:ood`). Abstention behaviour on in-domain questions the corpus does
  not cover is unmeasured, and grounding suppresses recovery on scaffold-omitted items rather than
  producing abstention (§`sec:suppression`).

### R5. Evaluation is a recurring activity, not a launch event

- **Requirement.** Served paths are re-measured on a schedule with the accounting, the signed gain,
  the population breakdown and the power stated; disagreements between a cheap procedure and a
  judged one are queued for label audit rather than counted silently against either.
- **Implemented.** The exposure matcher is ported into the node as per-request telemetry
  (`crates/loom-scaffold/src/exposure.rs`, a semantic-parity port of the paper's matcher), so
  restated and dropped titles are observable at serving time. The offline harness, the sweep rows,
  the audit sample and the manifest are released (paper §`sec:runnable`, `MANIFEST.md`).
- **Tested.** The instrument itself: the stratified audit prices what the deterministic matcher gives
  up (§`sec:audit`), and the ranker–judge disagreement audit found a corpus labelling error its
  authors had already reviewed (companion note §`sec:audit`).
- **Untested.** Standing operation. Nothing here runs on a schedule, no regression threshold is
  defined against a previous generation, and the per-request telemetry has no gold set behind it, so
  it reports restatement against what was served and is not a live copy ceiling.

### R6. Control over source, version and route to authority

- **Requirement.** The organisation controls which corpus is consulted, which version of it is
  current, and by what route an addition becomes authoritative.
- **Implemented.** Knowledge enters through a separate CI-gated builder that runs consistency tests
  and an OWL 2 EL reasoner before publishing each versioned generation; the served node mirrors one
  generation atomically so it never serves a mixed build; automated extraction enters through a
  preview-first import that refuses overwrites and emits review candidates (paper §`sec:ecosystem`).
  The unit of service and review is a markdown block scoped to one IRI, which is what makes
  single-entity human audit possible (§`sec:system`).
- **Tested.** Nothing about the governance route is measured. The corpus figures (8,146 classes,
  282,492 closure triples) are properties of the generation used, not test outcomes.
- **Untested.** Whether governed admission improves served quality, how long the propose-to-serve
  loop takes, and whether reviewers agree with the pre-filter's scoring.

### R7. Stated limits

- **Requirement.** The system's marketing states what it is for and what it is not for.
- **What is supportable.** Versioned curated material is served through a replaceable model
  interface; the scaffold supplies the gold target names for the large majority of in-domain
  questions (§`sec:indomain`); each answer carries source metadata naming the generation
  (§`sec:system`).
- **What is not supportable, and must not be claimed.** That the node is cheaper than an alternative
  at matched quality; that ontology structure beats simpler retrieval over the same corpus at equal
  budget; that attribution metadata is accurate at answer level; that answers are free of
  fabrication; that answers inherit the trustworthiness of the source. All four open questions are in
  paper §`sec:roadmap`, and none is closed.
- **Measured caution.** On gold targets the scaffold did not expose, lexical recovery falls from
  0.121 bare to 0.004 grounded (§`sec:suppression`), so out-of-corpus questions should be routed away
  from the scaffold rather than through it. Under rephrasing out of the graph's title vocabulary,
  exposure collapses from 0.964 to 0.328 (§`sec:paraphrase`), so the headline in-domain figure is the
  high end of a distribution and not a single operating point.

## What composing these would require

The seven requirements have never been run together as one deployment, and this note claims no
aggregate property from their conjunction. Composing them would need, at minimum: the three
retrieval baselines of paper §`sec:roadmap` question 3 at equal budgets, an attribution-precision
measurement for question 4, a matched always-inject ablation to attribute the out-of-domain
behaviour to the gate, and a scheduled re-measurement with a defined regression threshold. Until
those exist, this is a requirements document.
