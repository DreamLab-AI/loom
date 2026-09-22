# ADR-140 — Loom v2: the grounding plane becomes agent-drivable, and the canonical unit gains data grounding

Status: Proposed · 2026-09-22 · Extends ADR-135 (node boundary, model-is-a-URL), ADR-136 (tooling
allocation), ADR-138 (grounding contract), ADR-139 (per-request opt-out) · Supersedes no decision;
it **widens** the serving surface ADR-135 D1 opened and **extends** the canonical unit ADR-136 D1
fixed · Governs a change to `loom-domain` ports, adds two crates, retires one Node service

**External evidence:** Chong, Zhang, Fan & Du, *EvoOntology: A Self-Evolving Ontology Layer for Data
Agents*, arXiv:2609.15779 (14 Sep 2026), RUC-DataLab. Code: `github.com/ruc-datalab/EvoOntology`.
Cited throughout as **[Evo]**. Its numbers are its own; none are reproduced here as Loom results.

---

## THE PRIZE (unchanged, quoted verbatim from `ddd-ontology-loom-context.md`)

> The **human-scrutible markdown-with-ontology block** is the canonical, served unit: one per-IRI
> block of curated research prose (`dfull`) headed by its typed ontology relations, that a human can
> read, review and audit. RuVector (semantic index), oxigraph (SPARQL), the lexical matcher,
> Xinference embeddings — all **accelerators behind the markdown**, never replacing it as the served
> unit.

Every decision below is subordinate to it. Invariant I-P1 (every port resolves to an `Iri`
addressing a `CanonicalUnit`) is preserved verbatim and extended to the new plane.

---

## 1. Context

### 1.1 What Loom is today, stated in the external taxonomy

[Evo] divides agent–data interaction into **raw querying** (the agent explores the source itself)
and **semantic-layer interaction** (a curated layer mediates). It then splits the second into two
serving regimes and measures them against each other:

| Regime | Mechanism | [Evo] result |
|---|---|---|
| `Baseline` | ReAct, no layer | DDR-Bench Traj-Wise 69.5 |
| `Baseline + SL` | builder-constructed semantic layer **injected as a static prompt fragment** | *inconsistent*: −15.0 on Claude-Sonnet-5; BIRD EX −5.6 on GPT-5.5 while VES rose |
| `EvoOntology` | the **same content** exposed as MCP tools the agent queries per turn, plus a gated evolution loop | 89.5 (+20.0), and 42.0K vs 52.6K tokens per task |

Loom's serving path is `Baseline + SL`. `chat_completions` scaffolds the last user message, merges
the block into the first system message and delegates. The consumer never chooses what to retrieve;
the façade decides once, before the model has reasoned at all.

This is not a criticism of the corpus — Loom's corpus is reasoner-checked and human-curated, which
[Evo]'s agent-built layer is not. It is a criticism of **one-shot, pre-reasoning, unprunable
injection** as the only way that corpus reaches a model. [Evo]'s diagnosis is precise and matches
what ADR-139 already documented from production: a static fragment *"competes with the agent's other
instructions and cannot be pruned per turn."* ADR-139's incident — the class **Node** served at
confidence 42 to a consumer writing about a test script — is that failure mode exactly. We fixed it
with an opt-out flag. The opt-out is a workaround for a serving-regime limitation, not a design.

### 1.2 Loom already owns most of the missing plane

`crates/loom-facade/src/routes/mod.rs` already routes `/loom/scaffold`, `/loom/sparql`,
`/loom/search`, `/loom/search/semantic`. `ports.rs` already exposes `LexicalIndex::resolve`
(`Iri → CanonicalUnit`), `GraphStore::query` + `search_labels`, and `VectorIndex::nearest`. The
retrieval capability is built, audited and byte-parity-tested. What does not exist is a **protocol
an agent can drive it with mid-reasoning**, and a **compact index it can navigate from**.

### 1.3 Three rival doors to one ontology

**Measured 2026-09-22 — see Addendum A for the probe transcript.**

| Door | Substrate | Source actually served | Reasoned? | Gate | Generation identity | Grounding |
|---|---|---|---|---|---|---|
| `loom-facade` HTTP | Rust | built bundle, generation `2026-08-22`, **8,146** classes, 282,492 triples incl. `ontology-inferred.ttl` | **yes** (Whelk closure loaded) | `LexicalIndex::assemble` | yes (sha-addressed, 4 artefacts) | yes (ADR-138) |
| `app/ontology-mcp/index.js` | Node, 1 dep | its *own* `scaffold-index.json`, or a public website fetch | no | none | none | none; fail-open `{error}` strings |
| agentbox `ontology-bridge` | Node | **raw Logseq markdown on disk**, `visionGraph/knowledge/pages`, **8,433** classes (8,444 files) — `"backend": "local-markdown"`, `"note": "VisionClaw bypassed"`, `"_route": "local-fallback"` | **no** | none | none | none |

Two findings, both worse than assumed when this ADR was drafted:

1. **`ontology-bridge` does not serve VisionClaw at all.** Its configured `VISIONCLAW_API_URL` is
   dead and it has silently fallen back to reading raw Logseq pages off disk. Every agent using the
   `ontology-augment` skill is therefore reading **un-reasoned, un-curated, un-gated markdown** and
   receiving no generation stamp with which to notice.
2. **The two doors disagree in both directions.** On `knowledge-graph`, the bridge returns three
   relations the served generation does not have (`requires: Graph Database`, `enables: Question
   Answering, Semantic Search`) — it is a month *fresher*. Loom returns `ancestors: Spatial
   Computing` — the reasoned closure the bridge cannot compute. The bridge also exposes ~290 pages
   the build deliberately excludes. Neither door is currently right: **Loom is authoritative but
   stale; the bridge is live but ungoverned.**

Every governance property ADR-135/136/138 bought is bypassed by the consumers who most need it, and
consolidating onto Loom as it stands would cost agents a month of corpus currency. **This is the
largest correctness defect in the current design, and it is not a retrieval problem.**

### 1.4 What [Evo] establishes that we should treat as load-bearing

Ablations, on their benchmarks, with their layer — directional evidence, not our numbers:

- **The acceptance gate is the single most important component** (−11.2 without it). An
  ungated self-improving layer degrades.
- **Attribution to a level before patching** is second (−6.3).
- **Exposure edits dominate the gain**: Tool-level 57% of cumulative gain, Content 34%, Schema 9%.
  Tool-only evolution alone recovered +13.2 of +20.0.
- **Mappings (−13.4) and Evidence (−8.7) are the load-bearing content families**; Constraints (−3.5)
  and Relations (−2.1) are not. A term that is not bound to a queryable locus, and not backed by an
  executed probe, is the weakest kind of entry.
- **Layers fitted per backbone do not transfer**: every cross-backbone store lost 6.6–10.9 points;
  accepted-term Jaccard ≤0.62 between backbones.
- **Tools cost less, not more**: manifest + on-demand fetch raised input tokens per turn 3.2K→4.6K
  but cut turns 14.6→8.4, for −20% total tokens per task.

Loom has Terms and Relations. It has **no Mappings and no Evidence** — nothing binds an IRI to a
queryable locus, and nothing records an executed probe. Against [Evo]'s ablation that is the two
strongest families missing. It is also exactly why Loom is a *prose* grounding layer and not yet a
*data* grounding layer, which is what the ecosystem asked for.

---

## 2. Decision

### D1 — Two planes, one node, one gate, one generation

Loom serves the same corpus generation through **two protocol surfaces in one binary**:

1. **Injection plane** — `POST /v1/chat/completions`, unchanged. The stable model door
   (ADR-135 D1) for consumers that are not tool-capable or not agentic: the email gateway's
   synthesis calls, single-turn lookups, the verbatim serving regime (F1).
2. **Agentic plane** — an **MCP server** over Streamable HTTP at `/mcp` on the same listener.
   The consumer drives retrieval. *(Amended 2026-09-22, ADR-141: the stdio shim binary
   `loom-mcp-stdio` shipped with P0 and was removed the same day — see Addendum B.)*

Both planes resolve through `LexicalIndex::assemble` (THE gate) and both stamp the **same**
`Grounding` object with the same six statuses (ADR-138). One binary, one `AppState`, one
generation, one telemetry shape. A consumer may mix planes in one session.

The plane is chosen by the **consumer**, never by server config. `loom_options.scaffold: false`
(ADR-139) keeps its meaning on the injection plane and needs no analogue on the agentic plane,
because on that plane nothing is injected unless asked for.

### D2 — Manifest-first exposure on the agentic plane

The only ontology content placed in an agent's context without being asked for is a **session
manifest**: generation id and content digest, corpus profile (domain families, class counts,
maturity and coverage distribution), the tool contract, and a bounded set of high-salience term
identifiers. Everything else is fetched on demand.

The property this buys is the one the injection plane cannot have: **prompt cost is O(1) in corpus
size.** Today's injected budget is O(matched blocks) and grows as the corpus does; the manifest does
not. This is the mechanism behind [Evo]'s −20% total tokens, and it is the answer to the
context-length objection that has constrained Loom's budget clamp since PRD-026.

### D3 — Retire the two rival doors

- **Delete `app/ontology-mcp/`.** Not port it. It duplicates the index, has no gate, no generation
  identity and no grounding contract, and fails open with untyped error strings. Its four tools
  (`ontology_search`, `ontology_class_get`, `ontology_neighbours`, `ontology_ask`) are re-provided
  by the agentic plane, gated and stamped.
- **Re-point agentbox `ontology-bridge` at Loom's `/mcp`**, retaining its existing tool names as a
  thin alias layer so no skill, agent definition or `.mcp-hub-servers.json` entry breaks. The
  ontology-augment skill keeps working unchanged; it simply starts receiving gated, generation-
  stamped answers.
- **The bridge's silent fallback is removed, not preserved.** A door that cannot reach its corpus
  must fail loudly with a typed error, never quietly downgrade to a different corpus. The current
  `_route: "local-fallback"` behaviour is how a month-long divergence went unnoticed.

**Precondition on D3 (added 2026-09-22 after the probe).** Re-pointing is only a net win once the
served generation is current, because the bridge is a month fresher than the bundle. The reload path
already exists (`scripts/reload-published-generation.py`, `docs/published-generation-reload.md`) and
is deliberately disabled pending a matched semantic/graph bundle. **P0 therefore ships the `/mcp`
plane and deletes `app/ontology-mcp/` unconditionally, but the `ontology-bridge` re-point is gated on
a published generation no older than the raw corpus it replaces.** Until then the bridge stays as it
is and `/health` reports the divergence, so the gap is visible rather than silent.
- Public/offline use (`ONTOLOGY_SITE`, `narrativegoldmine.com`) becomes a **corpus source
  configuration** of the one node, not a second implementation.

One corpus, one gate, one generation, one grounding contract, for models and agents alike.

### D4 — The canonical unit gains `Mapping` and `Evidence`

THE PRIZE is preserved exactly: the served unit remains one human-reviewable per-IRI markdown block.
Its typed header gains two families alongside the existing typed relations:

- **`Mapping`** — binds the IRI to a *queryable locus*: a SPARQL pattern over the Whelk-reasoned
  closure, a RuVector namespace + filter, a Logseq page / file path, or an external source
  registered in the PRD-025 connector manifest. A term with a Mapping can be *checked*; a term
  without one can only be *read*.
- **`Evidence`** — a stored, re-executable probe against a Mapping, and the digest of its last
  verified result, stamped with the generation that verified it.

Both are **optional per IRI** and their coverage is reported in the manifest and on `/health`, so
the corpus can gain them incrementally without a flag day and consumers can see how grounded a
given domain actually is. Both are corpus content under the same governance as prose, not runtime
state.

**Authoring policy (decided 2026-09-22): derive mechanically, hand-author the remainder.**

| Family | Origin | Cost |
|---|---|---|
| `Locus::Sparql` | **derived** — every IRI already has a subject position in the reasoned closure; the pattern is generated from the build, not written | zero marginal |
| `Locus::Corpus` | **derived** — `ClassEntry` already carries the source page; the mapping is the build's own join key | zero marginal |
| `Locus::Vector` | **derived** — the RuVector namespace + IRI filter is mechanical | zero marginal |
| `Locus::Connector` | **hand-authored** — binding an IRI to an external system (PRD-025 connector manifest) is a judgement about what that system means | per-connector, not per-IRI |
| `Evidence` | **hand-authored probe, mechanically re-executed** — a human writes the probe once for the IRIs that matter; the build re-runs it every generation and stores the result digest | per-IRI, prioritised by domain |

Derived Mappings therefore land for the whole corpus in one build change, and hand-authoring is
confined to connectors and to Evidence on IRIs a human chooses to underwrite. Coverage is reported
per family so "derived-only" is never mistaken for "verified". Hand-authored entries are gated by
the existing `enrich-gate.yml` in `jjohare/logseq`; derived entries are build output and are gated
by the build's own consistency check.

This is the change that turns Loom from a curated-prose grounding layer into an **ontology-and-graph
data** grounding layer. It is also, per [Evo]'s ablation, where the largest content-side gain is.

**Amendment (2026-09-22, ADR-141) — D4 adopts OKF's vocabulary, not its own.**
The decision above stands in substance; only its *names* change, because the corpus that will
carry these families is validated against Open Knowledge Format v0.2 and Loom must not hold a
private synonym for a public term. The rename is mechanical and complete:

| D4 as decided | D4 as realised (OKF) | Note |
|---|---|---|
| `Evidence { iri, probe, result_digest, verified_at }` | `AttestedComputation { runtime, parameters, computation, executor { resource, receipt }, attester { resource } }` | the executor (who ran it) and the attester (who stands behind the result) are now separate parties, which the single `verified_at` could not express |
| `Mapping { iri, locus, note }` | `Mapping { resource, locus, note }` | `locus` keeps its enum unchanged; OKF's `resource` URI *is* the address, so the second `iri` field is gone rather than duplicated |
| `CoverageReport { terms, mappings_derived, mappings_authored, evidence_verified }` | `CoverageReport { terms, attested_computations, verified_machine, verified_human }` | coverage is reported by OKF **trust tier**. The derived/authored split is superseded: who verified an entry is a stronger and checkable claim than how its locus was produced |
| `CanonicalUnit.corpus_nature: CorpusNature` | `CanonicalUnit.provenance: Provenance { generated, verified }` | `corpusNature` asserted human direction uniformly for every unit and was never checkable per entry; `Provenance` is per-unit and carries actor URIs. Populated from the scaffold index's OKF trust keys where present, else `generated: process:vault`, `verified: []` |
| `UnitKind::Evidence` | `UnitKind::AttestedComputation` (wire: `"attested-computation"`) | the MCP `kind` enum follows the domain |

Both families remain optional per IRI, their coverage is still reported honestly (including at
zero), and they are still corpus content under governance rather than runtime state. The authoring
policy table above is unchanged except that "hand-authored by `enrich-gate.yml` in `jjohare/logseq`"
now reads "gated by `vault gate` in the local vault" (ADR-141).

### D5 — `loom.probe`: executable, bounded grounding

A read-only tool that executes a `Mapping`'s locus and returns the result plus its digest, clamped
exactly as `/loom/sparql` is today (`_FORBIDDEN` / `_READ_FORM` / injected `LIMIT`, carried from
`loom_graph.py`). An agent can then *verify* a claim against the graph rather than trusting the
block, and a `Grounding` can distinguish "the corpus says so" from "the corpus says so and the graph
still agrees at this generation".

This is the single new capability that makes the ecosystem's graph data addressable to an LLM
without handing it a raw SPARQL endpoint and hoping.

### D6 — Exposure evolves automatically; semantics evolve under governance

Adopt [Evo]'s loop shape — *diagnose → attribute → patch one level → gate* — and **split it by
level**, because the two levels have different truth conditions:

| Level | Examples | Semantically inert? | Admission |
|---|---|---|---|
| **Exposure** *(confirmed 2026-09-22: self-evolving, gated, auto-reverting)* | manifest shape and salience set, tool descriptions and arity, budget clamp, ranking weights, per-backbone verbatim threshold | **yes** — changes what is *shown*, never what is *true* | automatic: paired eval on a held-out fold, margin τ, auto-revert on regression, every accepted patch recorded in the ledger |
| **Content / Schema** | adding, removing or revising a Term, Mapping, Constraint, Evidence or Relation; changing the object model | **no** | **never auto-applied.** The evolution agent emits a typed `PatchProposal` into the `AttestationLedger`; admission requires (a) Whelk EL++ consistency at build time, (b) paired-eval improvement, (c) human sign-off, then re-entry through the corpus build |

The split is the reconciliation with ADR-135/136, and it is not a compromise: [Evo] measures
**57% of its gain at the Tool level**, which is precisely the level we can safely automate. We take
the majority of the available gain with no governance exposure, and we route the remaining 34%
through a stronger gate than the paper's — theirs is a benchmark score alone, ours is entailment
plus provenance plus a human.

Rejected explicitly: an LLM agent mutating the served ontology against a validation score. That is
the ungoverned path ADR-135 exists to refuse, and [Evo]'s own −11.2 gate ablation is the argument
for why score-only admission is thin.

### D7 — Backbone-invariant knowledge, backbone-fitted exposure

ADR-135 D1.2's "the model is just a URL behind the door" is **restated, not withdrawn**:

> The corpus, the reasoned closure and every canonical unit are **backbone-invariant** — that is the
> door, and swapping the model never touches them. The **exposure profile** — manifest shape,
> salience set, budget, ranking weights, verbatim threshold, whether the agentic plane is preferred
> — is a named, versioned, per-backbone artefact selected when the model behind the door changes.

[Evo]'s 6.6–10.9-point cross-backbone transfer loss is the evidence that an exposure profile must
*exist* and be fitted. It is not evidence that knowledge should be fitted — their layer transfers
badly precisely because their content was backbone-fitted, and ours is not. Making the distinction
explicit strengthens the claim; leaving it implicit lets a reader read our headline as refuted.

Profiles are data (`exposure-profiles/<backbone>.json`), versioned with the generation, and a
missing profile falls back to a conservative default rather than failing.

### D8 — Cost and turns become first-class benchmark axes; PRD-028 gains a third arm

`bench/UPLIFT-BENCH-PROTOCOL.md` reports recall against the copy ceiling. It must also report, per
task: **input tokens, output tokens, turns, total tokens, wall-clock**. [Evo] wins on quality *and*
on 20% fewer total tokens; a protocol that cannot see that axis cannot see its own trade-off.

PRD-028 currently pits Loom against flat-text retrieval. That is now the wrong strongest baseline.
Its arms become:

1. unaided model (closed-book control);
2. strong flat-text retrieval at matched context budget;
3. **static scaffold injection** (Loom today);
4. **the same corpus, same gate, exposed on the agentic plane** (Loom v2).

Arms 3 and 4 differ *only* in serving regime — same corpus, same generation, same gate — which makes
it the clean experiment [Evo] did not run (their layer differs in construction too). The expected
result, stated in advance so it can be wrong: **injection wins single-turn closed-corpus lookup;
tools win multi-turn research over wide or heterogeneous sources.** If that holds, the routing rule
is a measured product feature, not a guess.

---

## 3. Realisation in the Rust workspace

### 3.1 New domain types (`loom-domain/src/model.rs`, `artefact.rs`)

```rust
/// The O(1)-in-corpus-size session index: the ONLY ontology content placed in an
/// agent's context unasked (D2).
pub struct Manifest {
    pub generation: Generation,
    pub content_digest: Digest,
    pub profile: CorpusProfile,        // domain families, class counts, maturity histogram
    pub coverage: CoverageReport,      // % of IRIs carrying a Mapping / Evidence (D4)
    pub salient: Vec<SalientTerm>,     // bounded, exposure-profile-selected
    pub tools: Vec<ToolDescriptor>,    // the contract, from the active ExposureProfile
}

/// Binds an IRI to something queryable (D4). The variant IS the capability claim.
pub enum Locus {
    Sparql { pattern: String },                       // over the Whelk-reasoned closure
    Vector { namespace: String, filter: Option<String> },
    Corpus { page: String },                          // Logseq page / file path
    Connector { source_id: String, selector: String },// PRD-025 connector manifest
}

pub struct Mapping { pub iri: Iri, pub locus: Locus, pub note: Option<String> }

/// An executed probe and the digest of its verified result (D4).
pub struct Evidence {
    pub iri: Iri,
    pub probe: Locus,
    pub result_digest: Digest,
    pub verified_at: Generation,
}

pub struct ProbeResult { pub rows: SparqlResult, pub digest: Digest, pub matches_evidence: bool }

/// Backbone-fitted, semantically inert (D7).
pub struct ExposureProfile {
    pub backbone: String,
    pub version: u32,
    pub manifest_salience: usize,
    pub budget_tokens: usize,
    pub verbatim_threshold: f64,
    pub prefer_plane: Plane,           // Injection | Agentic
    pub tool_descriptions: BTreeMap<String, String>,
}

/// A proposed, attributed, single-level change (D6). NEVER applied by the node.
pub struct PatchProposal {
    pub level: PatchLevel,             // Exposure | Content | Schema
    pub signature: FailureSignature,   // what in the trajectories motivated it
    pub hypothesis: String,            // the expected behavioural effect, stated before evaluation
    pub diff: PatchDiff,
    pub paired_eval: Option<PairedEvalOutcome>,
}
```

### 3.2 New ports (`loom-domain/src/ports.rs`)

```rust
/// Builds the session manifest from the loaded generation + active profile (D2).
#[async_trait]
pub trait ManifestSource: Send + Sync {
    async fn manifest(&self, profile: &ExposureProfile) -> Result<Manifest, LoomError>;
}

/// Executes a Mapping's locus, read-only and clamped (D5). Returns rows AND the
/// digest, so a caller can compare against stored Evidence without trusting prose.
#[async_trait]
pub trait ProbeExecutor: Send + Sync {
    async fn probe(&self, locus: &Locus, limit: usize) -> Result<ProbeResult, LoomError>;
}

/// Read-side of the evolution loop (D6). Trajectories are recorded by the façade;
/// the evolution agent consumes them OFF the serving path.
#[async_trait]
pub trait TrajectoryStore: Send + Sync {
    async fn record(&self, t: &Trajectory) -> Result<(), LoomError>;
    async fn signatures(&self, since: Generation) -> Result<Vec<FailureSignature>, LoomError>;
}

/// Exposure profiles are data, loaded per backbone, conservative default on miss (D7).
pub trait ExposureProfileStore: Send + Sync {
    fn for_backbone(&self, backbone: &str) -> ExposureProfile;
    fn active(&self) -> &ExposureProfile;
}
```

`LexicalIndex` gains one method and keeps `assemble` as the sole gate:

```rust
    /// Bounded, kind-filtered navigation for the agentic plane. Returns
    /// CANDIDATES, never served content — `resolve`/`assemble` remain the only
    /// paths to a CanonicalUnit, so Invariant I-P1 holds on the new plane.
    async fn browse(&self, query: &str, kind: UnitKind, n: usize)
        -> Result<Vec<ConceptMatch>, LoomError>;
```

### 3.3 Crate changes

| Crate | Change |
|---|---|
| `loom-domain` | + the types and ports above. Still leaf, still no I/O. |
| `loom-scaffold` | + `browse`, + manifest assembly from the index, + salience selection. Still network-free. |
| `loom-graph-oxigraph` | + `ProbeExecutor` for `Locus::Sparql`, reusing the existing read-only clamp verbatim. |
| `loom-vector-ruvector` | + `ProbeExecutor` for `Locus::Vector`. |
| **`loom-mcp`** *(new)* | Driving adapter: MCP tool schemas + dispatch over Streamable HTTP. Depends on `loom-domain` only; holds no retrieval logic. |
| **`loom-evolve`** *(new)* | **Pure.** Diagnose / attribute / patch-proposal types and the paired-eval admission predicate with margin τ. No I/O, no LLM client — it decides, it does not act. |
| `loom-facade` | Mounts `/mcp` on the same listener and the same `AppState`; records trajectories; selects the exposure profile. |
| `loom-attest-proofgate` | + `PatchProposal` entries alongside `GateVerdict`, same `ChainedLedger`. |

The ring is unchanged: adapters still depend only inward, and `loom-mcp` physically cannot reach a
retrieval engine except through a domain port. The accelerator boundary stays a build fact.

### 3.4 The tool contract (`/mcp`)

| Tool | Maps to | Returns |
|---|---|---|
| `loom.manifest` | `ManifestSource::manifest` | the session index (D2) |
| `loom.browse` | `LexicalIndex::browse` (+ `VectorIndex::nearest` on a lexical miss, as today) | bounded candidates: IRI, title, score, signal |
| `loom.resolve` | `LexicalIndex::resolve` → `assemble` | the served markdown blocks + `Grounding` |
| `loom.sparql` | `GraphStore::query` | clamped read-only rows |
| `loom.neighbours` | `GraphStore` | typed neighbours of an IRI |
| `loom.paths` | `GraphStore` | shortest typed paths between two IRIs |
| `loom.probe` | `ProbeExecutor` | rows + digest + `matches_evidence` (D5) |

Every response carries the ADR-138 `Grounding` envelope. `loom.browse` is the one tool that may show
index shape (IRI + score) — the same latitude `/loom/search/semantic` already has, for the same
reason: it is labelled as the index, not as an answer.

---

## 4. What is explicitly NOT adopted from [Evo]

1. **Agent-built ontology content.** Their builder agent proposes from a training workload and
   commits on probe verification. Loom's corpus is human-curated and Whelk-checked, which is the
   asset. We adopt their *probe-verification* idea as `Evidence` (D4) and reject autonomous
   commitment.
2. **Self-mutating semantics.** See D6.
3. **Backbone-fitted knowledge.** See D7.
4. **Their gate.** A validation-score margin alone is weaker than entailment + provenance + review.
   We keep τ as *one* of three admission conditions at Content level, and as the *sole* condition
   only at the semantically inert Exposure level.
5. **Their benchmarks as our targets.** DDR-Bench / InsightBench / BIRD are heterogeneous-data-agent
   benchmarks; Loom's claim is about private curated knowledge. PRD-028 remains the study that
   answers our question. [Evo]'s numbers set direction, not thresholds.

---

## 5. Phased delivery

| Phase | Scope | Gate to proceed |
|---|---|---|
| **P0** — one door | `/mcp` + `loom.manifest/browse/resolve/sparql/neighbours/paths` over the **existing** index and gate. Delete `app/ontology-mcp/`. Re-point `ontology-bridge` with name aliases. No corpus change. | existing byte-parity goldens green; `ontology-augment` skill unchanged and now generation-stamped |
| **P1** — measure | Cost + turns in the bench harness. PRD-028 arms 3 and 4 on the pilot corpus. | arm 3 vs arm 4 result recorded with CIs, whatever it says |
| **P2** — data grounding | `Mapping` + `Evidence` in the logseq build; `loom.probe`; coverage on `/health` and in the manifest. | `enrich-gate.yml` extended; coverage reported honestly, even at 0% |
| **P3** — exposure evolution | `ExposureProfile` artefacts; `loom-evolve` paired-eval admission at Exposure level only; auto-revert. | τ and the held-out fold frozen **before** the first accepted patch |
| **P4** — governed content evolution | `PatchProposal` into the ledger; human sign-off path; re-entry via the corpus build. | P3 has a clean auto-revert record |

P0 is days, not weeks, and is worth doing on its own merits: it closes the ungated-door defect
(§1.3) regardless of what P1 measures.

---

## 6. Consequences

### Positive

- One gate, one generation, one grounding contract for **every** consumer, model or agent. The
  governance ADR-135/136/138 bought stops being bypassable.
- Prompt cost becomes O(1) in corpus size on the agentic plane, removing the constraint that has
  bounded the injection budget since PRD-026.
- The ontology becomes *checkable*, not merely readable: `Mapping` + `Evidence` + `loom.probe` are
  the ecosystem's ask ("ontology and graph data support") made concrete.
- The model-swap claim gets sharper, and survives contact with the published cross-backbone result.
- Loom stops being the arm that a published paper measures as inconsistent, and becomes the node
  that serves **both** regimes from one governed corpus — which is a stronger position than either.

### What breaks, deliberately

- `app/ontology-mcp/` is deleted. Anything pointing at it must move to `/mcp`.
- `ontology-bridge`'s backend changes from VisionClaw to Loom. Tool names are preserved; answer
  *provenance* changes, and callers now receive a `Grounding` they previously did not.
- `bench/UPLIFT-BENCH-PROTOCOL.md` gains required cost columns; historical runs lack them and must
  be marked as such rather than back-filled.

### Negative / honest caveats

- Two planes is more surface than one. Mitigated by one binary, one `AppState`, one gate — the
  duplication is protocol-shaped, not logic-shaped, and `loom-mcp` holds no retrieval code.
- Agentic-plane quality depends on the backbone's tool-calling competence. Our own bench already saw
  this ("agentic tool-traversal is model-dependent; Gemma strong, Muse weak"). D7's
  `prefer_plane` exists for exactly that, and the injection plane remains fully supported — this is
  a *widening*, not a migration.
- `Mapping` / `Evidence` authoring is real corpus work in the logseq pipeline. D4 makes them
  optional per IRI and reports coverage so the cost is visible and incremental.
- P3 introduces automated change to a served system. It is confined to semantically inert artefacts,
  is ledger-recorded and auto-reverts. If the revert record is not clean, P4 does not start.

### Neutral

- ADR-139's `loom_options.scaffold: false` is unaffected and still correct for the injection plane.
- THE PRIZE, Invariant I-P1 and the accelerator boundary are unchanged in letter and in enforcement.

---

## 7. Verification

1. `cargo test -p loom-mcp` asserts every tool response carries the full `REQUIRED_GROUNDING_FIELDS`
   set, on all six statuses, as the HTTP plane's tests already do.
2. A negative test asserts `loom.browse` cannot return unit *content* — only candidates — so I-P1 is
   compiler- and test-enforced on the new plane.
3. `/health` reports both planes' readiness, the active `ExposureProfile` name and version, and
   `Mapping`/`Evidence` coverage.
4. The existing `tests/golden-python/` byte-parity goldens must stay green through P0 — the
   injection plane is not permitted to drift while the agentic plane is added.
5. P3 acceptance requires a recorded auto-revert of at least one deliberately regressive patch.

---

## 8. References

- ADR-135 (node boundary, model-is-a-URL), ADR-136 (tooling allocation), ADR-138 (grounding
  contract), ADR-139 (per-request opt-out), PRD-025/026/027/028, `RUST-ARCHITECTURE.md`,
  `ddd-ontology-loom-context.md`, `LOOM-POSITIONING.md`.
- **[Evo]** Chong, Zhang, Fan & Du. *EvoOntology: A Self-Evolving Ontology Layer for Data Agents.*
  arXiv:2609.15779, 14 Sep 2026. RUC-DataLab. `github.com/ruc-datalab/EvoOntology`.

---

## Addendum A — door-divergence probe, 2026-09-22

Run before D3 was treated as settled. Raw transcripts, not summaries.

**Loom façade** — `GET http://192.168.2.132:8084/health`:

```
index_classes      8146
graph              available, 282492 triples, [ontology.ttl, ontology-inferred.ttl]
generation         2026-08-22T08:19:43Z, 4 artefacts sha256-stamped,
                   verified_single_generation = true
semantic.ready     false  ("artefact declares no embedding model;
                            contract requires bge-small-en-v1.5")
```

**agentbox `ontology-bridge`** — `ontology_health`:

```json
{ "status": "ok", "backend": "local-markdown",
  "source": "/home/devuser/workspace/visionGraph/knowledge/pages",
  "classCount": 8433,
  "note": "Served from the raw Logseq corpus on disk (VisionClaw bypassed).",
  "_route": "local-fallback" }
```

`ls visionGraph/knowledge/pages | wc -l` → **8444**.

**Same class, both doors** (`knowledge-graph`):

| | Loom `/loom/scaffold` (generation 2026-08-22) | bridge `ontology_class_get` (disk, today) |
|---|---|---|
| `requires` | Ontology, Schema Definition, Triple Store | + **Graph Database** |
| `enables` | Reasoning, Knowledge Discovery, Recommendation System | + **Question Answering, Semantic Search** |
| ancestors | **`ancestors: Spatial Computing`** (reasoned) | absent — `subClassOf` only |
| generation | `2026-08-22T08:19:43Z`, digest `887d2f68…` | none |
| grounding | full ADR-138 object, `confidence 1.0`, `decision "full"` | none |

**Conclusions carried into the decision.**

1. D3 is a **correctness fix**, not a consolidation tidy-up: the agent door serves a different,
   un-reasoned corpus and says nothing about it.
2. D3 acquires a currency precondition (see D3), because the ungoverned door is the *fresher* one.
3. The semantic plane is not ready on the live node (`semantic.ready: false`), so P0's `loom.browse`
   is lexical-only in practice — which matches `fusion.rs`'s existing short-circuit and needs no new
   handling, but must be stated in the manifest's `degraded` list rather than left implicit.
4. Nothing here changes D1, D2, D5, D7 or D8.

---

## Addendum B — P0 delivered, 2026-09-22

Status of §5's first phase: **shipped**. Workspace green — 359 tests pass, `cargo clippy
--workspace --all-targets` clean, `cargo fmt --check` clean, and the frozen
`tests/golden-python/` byte-parity goldens are untouched (verification item 4).

| Item | Where |
|---|---|
| Manifest types (`Manifest`, `CorpusProfile`, `CoverageReport`, `SalientTerm`, `ToolDescriptor`, `UnitKind`) | `loom-domain/src/manifest.rs` |
| `LexicalIndex::browse` (default = `seeds`; kind-aware override) and the `ManifestSource` port | `loom-domain/src/ports.rs` |
| `GroundingStatus::Navigated` — a navigation is neither an answer nor a failed lookup | `loom-domain/src/grounding.rs` |
| Manifest assembly: domain/maturity histograms, domain-spread salience, honest zero coverage | `loom-scaffold/src/manifest.rs` |
| MCP framing, six tool schemas, argument clamping, error mapping (**no retrieval logic**) | `crates/loom-mcp/` |
| `ToolHost` over `AppState`; bounded BFS for `neighbours`/`paths` via `resolve` | `loom-facade/src/mcp_host.rs` |
| `POST /mcp` (+ honest 405 on GET), batch support | `loom-facade/src/routes/mod.rs` |
| `app/ontology-mcp/` deleted; `scripts/dream-build.sh` npm step removed | — |
| 10 integration tests + 9 adapter tests | `tests/adr140_mcp_plane.rs`, `loom-mcp/src/tests.rs` |

**Decisions taken during implementation, recorded rather than left implicit.**

1. **`loom.resolve` reports `status: "verbatim"`.** It serves the corpus as the answer with no
   backend call, which is exactly what `Verbatim` already names on the injection plane. Minting a
   synonym would have split one vocabulary across two planes.
2. **`GroundingStatus::Navigated` was added rather than reusing `NoMatch`.** A browse that returns
   candidates is a success; calling it a no-match would make `corpus_backed: false` unreadable.
   `may_be_corpus_backed()` returns false for it, so no consumer branch changes meaning.
3. **`neighbours`/`paths` traverse via `LexicalIndex::resolve`, not SPARQL.** The scaffold index
   already carries the reasoned projection (`ancestors` = inferred closure), so the traversal needs
   no graph round-trip and works on a node whose graph store is degraded. Bounded at `max_hops` and
   4,000 visited nodes; BFS so the first path found is a shortest one.
4. **`AppState` gained a `manifest: Arc<dyn ManifestSource>` port** rather than downcasting the
   retriever. In the composition root it is the same object behind a second trait — an index must
   not be able to describe a corpus it is not serving — but the seam is where D7's
   exposure-profile-aware manifest source will land without touching retrieval.
5. **The manifest names its degradations.** The live node serves with `semantic.ready: false`; the
   manifest reports `degraded: ["semantic-hnsw"]` so an agent does not over-trust a lexical miss.

**Retracted the same day: `loom-mcp-stdio` (2026-09-22, ADR-141).** P0 shipped a stdio shim
binary beside `/mcp`. It was deleted hours later, unreleased, when the sovereign-corpus decision
(PRD Q10) settled agent access estate-wide: **no MCP inside the estate.** Agents use the `vault`
CLI and `loom-client`; `/mcp` is kept solely as the door for hosts *outside* the estate, which are
by definition remote and therefore speak HTTP. A stdio transport exists for an agent running
beside the corpus — precisely the case that now uses the CLI — so the binary had no remaining
consumer, and an unused second transport is a second surface to keep honest for nothing. `/mcp`,
`loom-mcp` and the tool contract are unaffected.

**Not yet done, and deliberately so.** The `ontology-bridge` re-point remains gated on the D3
currency precondition — the served generation (2026-08-22) is still older than the raw corpus that
door reads, so re-pointing today would trade governance for a month of staleness. That is P0's one
open item and it is an operations task, not a code one.
