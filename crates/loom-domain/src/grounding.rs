//! `grounding` — the confidence-surfacing contract (PRD-026 FR-11).
//!
//! Everything the caller needs to judge HOW WELL an answer was grounded, and
//! WHAT the injection gate did about it, as one serialisable noun. Three facts
//! the wire form has to keep apart, because conflating them is what made the
//! old bare `top_score` unreadable:
//!
//! 1. **Which engine grounded it** (`signal`) — lexical, semantic, or nothing.
//! 2. **What scale the raw score is on** (`score_scale`) — the lexical seed
//!    score is an unbounded ADDITIVE sum (8.0 per exact-title word, 2.0 spread
//!    over title-word overlap, 0.75 per slug substring); an HNSW hit is a
//!    cosine in `[0,1]`. A bare `19.5` and a bare `0.83` are not comparable and
//!    must never be read as if they were.
//! 3. **A normalised `confidence` in `[0,1]`** — the one number a consumer can
//!    threshold on without knowing which engine answered.
//!
//! `decision` then records what the gate DID (full budget, scaled budget,
//! skipped, or served verbatim), with the `threshold` it was judged against and
//! the `effective_budget` that came out. Per-seed detail rides along in `seeds`,
//! including `injected` — whether that seed's section actually survived the
//! budget clamp, as opposed to merely having been selected.
//!
//! This module is pure data + arithmetic: no I/O, no gate policy. The gate
//! itself lives in `loom-scaffold::policy`; it reaches back here through the
//! [`GateThresholds`] trait so the domain never depends on an adapter.

use serde::{Deserialize, Serialize};

/// The gate's default `min_inject_score`, mirrored here so the domain can build
/// an honest [`Grounding::none`] without depending on `loom-scaffold`.
///
/// `loom-scaffold::tuning::MIN_INJECT_SCORE_DEFAULT` remains the authority; a
/// parity test in that crate pins the two together so they cannot drift.
pub const DEFAULT_MIN_INJECT_SCORE: f64 = 2.0;

// --- the three axes ---------------------------------------------------------

/// WHICH retrieval engine produced the grounding evidence.
///
/// Distinct from `FusionPath` (which records the ROUTE the fusion pipeline
/// took): a caller reading telemetry wants "was this lexical or semantic?"
/// without having to know the pipeline's internal branch names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroundingSignal {
    /// The inverted-index matcher scored a hit. `score_scale` is additive.
    Lexical,
    /// The HNSW fallback surfaced the candidates. `score_scale` is cosine.
    Semantic,
    /// Nothing matched; there is no evidence and `confidence` is 0.
    None,
}

impl GroundingSignal {
    /// Normalise a raw score ONTO THIS SIGNAL'S SCALE, yielding `[0,1]`.
    ///
    /// - `Lexical`: the clamped ratio against `strong_match_score` — the score
    ///   at which the gate hands over the full budget.
    /// - `Semantic`: already a cosine, so clamped through unchanged.
    /// - `None`: always 0.
    #[must_use]
    pub fn confidence_of(self, score: Option<f64>, strong_match_score: f64) -> f64 {
        match self {
            Self::Lexical => confidence_for(score, strong_match_score),
            Self::Semantic => match score {
                Some(s) if s.is_finite() => s.clamp(0.0, 1.0),
                _ => 0.0,
            },
            Self::None => 0.0,
        }
    }

    /// The score scale this signal reports on — the pairing is not a free
    /// choice, so callers get it from here rather than restating it.
    #[must_use]
    pub fn score_scale(self) -> ScoreScale {
        match self {
            Self::Semantic => ScoreScale::Cosine,
            Self::Lexical | Self::None => ScoreScale::LexicalAdditive,
        }
    }
}

/// What the confidence gate DID with the retrieved evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InjectionDecision {
    /// The whole requested budget was granted (gate off, or a strong match).
    Full,
    /// A weak-but-admissible match: the budget was scaled down proportionally.
    Scaled,
    /// Below `min_inject_score` — nothing injected, caller gets the raw prompt.
    Skipped,
    /// High confidence: the scaffold was served AS the answer, no backend call.
    /// `threshold` then carries the verbatim threshold, not `min_inject_score`.
    Verbatim,
}

/// The units `top_score` and each seed `score` are expressed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScoreScale {
    /// Unbounded additive lexical seed score (`match_`'s accumulated weights).
    LexicalAdditive,
    /// Cosine similarity in `[0,1]` (HNSW).
    Cosine,
}

// --- the gate's thresholds, as seen from the domain -------------------------

/// The two gate numbers a [`Grounding`] needs in order to describe itself.
///
/// Implemented by `loom-scaffold::policy::GatePolicy`. The trait exists so the
/// domain stays the leaf of the hexagon: the gate depends on the domain, never
/// the reverse.
pub trait GateThresholds {
    /// The score at or above which a match earns the FULL budget — the
    /// denominator that turns an additive lexical score into a confidence.
    fn strong_match_score(&self) -> f64;
    /// The score below which injection is skipped entirely.
    fn min_inject_score(&self) -> f64;
}

/// Normalise an additive lexical score into `[0,1]`.
///
/// Returns 0 for a missing score, and for a non-positive or non-finite
/// `strong_match_score` (the divide the gate itself guards against).
#[must_use]
pub fn confidence_for(top_score: Option<f64>, strong_match_score: f64) -> f64 {
    let Some(top) = top_score else {
        return 0.0;
    };
    if !top.is_finite() || !strong_match_score.is_finite() || strong_match_score <= 0.0 {
        return 0.0;
    }
    (top / strong_match_score).clamp(0.0, 1.0)
}

// --- the nouns --------------------------------------------------------------

/// Per-seed grounding detail: what was found, how good it was, and whether it
/// actually made it into the served block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedGrounding {
    /// The seed's IRI, as a plain string (the domain `Iri` wire form).
    pub iri: String,
    /// The raw retriever score, on the parent [`Grounding`]'s `score_scale`.
    pub score: f64,
    /// The seed's own score normalised to `[0,1]`.
    pub confidence: f64,
    /// The index's curated quality for this class (`q`), when it has one.
    pub quality: Option<f64>,
    /// Which engine surfaced this seed: `"lexical"` or `"semantic-hnsw"`.
    pub provenance: String,
    /// Whether this seed's section SURVIVED the budget clamp. A selected seed
    /// whose section was trimmed off the end reports `false` — the distinction
    /// between "we found it" and "we served it".
    pub injected: bool,
}

impl SeedGrounding {
    /// Build a seed grounding from the retriever's own `ConceptMatch`, which is
    /// the shape the fusion layer actually holds.
    ///
    /// Does the two narrowing conversions the wire form needs — `Iri` to its
    /// string form, `MatchProvenance` to its lower-case spelling — and widens
    /// the `f32` scores the retrieval ports carry to the `f64` this contract
    /// reports in, so a caller never has to spell either one out.
    #[must_use]
    pub fn from_match(
        candidate: &crate::model::ConceptMatch,
        signal: GroundingSignal,
        strong_match_score: f64,
        quality: Option<f32>,
        injected: bool,
    ) -> Self {
        let score = f64::from(candidate.score);
        Self {
            iri: candidate.iri.as_str().to_owned(),
            score,
            confidence: signal.confidence_of(Some(score), strong_match_score),
            quality: quality.map(f64::from),
            provenance: candidate.provenance.as_str().to_owned(),
            injected,
        }
    }
}

/// The whole grounding story for one assembly. Present on EVERY scaffold,
/// including the no-match case, so a consumer never has to infer absence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grounding {
    pub signal: GroundingSignal,
    /// The best raw score, on `score_scale`. `None` when nothing matched.
    pub top_score: Option<f64>,
    pub score_scale: ScoreScale,
    /// `top_score` normalised to `[0,1]`: the engine-agnostic number to
    /// threshold on. Lexical ⇒ `clamp(top / strong_match_score)`; semantic ⇒
    /// the cosine itself; no match ⇒ 0.
    pub confidence: f64,
    pub decision: InjectionDecision,
    /// The score the decision was judged against: `min_inject_score`, or the
    /// verbatim threshold when `decision` is [`InjectionDecision::Verbatim`].
    pub threshold: f64,
    /// The token budget the gate actually granted; `None` when skipped.
    pub effective_budget: Option<usize>,
    /// Whether a non-empty block was ultimately served.
    pub engaged: bool,
    pub seeds: Vec<SeedGrounding>,
}

impl Grounding {
    /// The "nothing matched" grounding — an honest zero rather than an absent
    /// field. `threshold` is the gate's `min_inject_score` so the reader can
    /// see what the miss was measured against.
    #[must_use]
    pub fn none(threshold: f64) -> Self {
        Self {
            signal: GroundingSignal::None,
            top_score: None,
            score_scale: ScoreScale::LexicalAdditive,
            confidence: 0.0,
            decision: InjectionDecision::Skipped,
            threshold,
            effective_budget: None,
            engaged: false,
            seeds: Vec::new(),
        }
    }

    /// Assemble a grounding from the parts the gate and the retriever hold.
    ///
    /// `confidence` is derived (never passed in) so the `[0,1]` invariant is a
    /// property of this constructor rather than of every call site. `engaged`
    /// is provisionally `decision != Skipped`; a caller that knows the block
    /// came back empty after the clamp corrects it with [`Self::with_engaged`].
    #[must_use]
    pub fn from_parts(
        signal: GroundingSignal,
        top_score: Option<f64>,
        score_scale: ScoreScale,
        gate: &impl GateThresholds,
        decision: InjectionDecision,
        effective_budget: Option<usize>,
        seeds: Vec<SeedGrounding>,
    ) -> Self {
        Self {
            signal,
            top_score,
            score_scale,
            confidence: signal.confidence_of(top_score, gate.strong_match_score()),
            decision,
            threshold: gate.min_inject_score(),
            effective_budget,
            engaged: decision != InjectionDecision::Skipped,
            seeds,
        }
    }

    /// Correct `engaged` once the served block is known (an empty block after
    /// the clamp is NOT an engagement, whatever the gate decided).
    #[must_use]
    pub fn with_engaged(mut self, engaged: bool) -> Self {
        self.engaged = engaged;
        self
    }

    /// Override the threshold — for [`InjectionDecision::Verbatim`], where the
    /// decision was judged against the verbatim threshold, not the gate's
    /// `min_inject_score`.
    #[must_use]
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Restate this grounding on a different signal, re-deriving every
    /// confidence (top-level and per-seed) on the new scale.
    ///
    /// The semantic-fallback path uses this: the scaffold assembles on the
    /// lexical scale, then the fusion layer — which is the only place that
    /// knows the candidates came from HNSW — restamps it as cosine.
    #[must_use]
    pub fn with_signal(mut self, signal: GroundingSignal, strong_match_score: f64) -> Self {
        self.signal = signal;
        self.score_scale = signal.score_scale();
        self.confidence = signal.confidence_of(self.top_score, strong_match_score);
        for seed in &mut self.seeds {
            seed.confidence = signal.confidence_of(Some(seed.score), strong_match_score);
        }
        self
    }
}

// --- the per-status contract (ADR-138 closeout) ------------------------------

/// WHICH answer path produced the response the grounding is attached to.
///
/// The review's finding was that grounding objects existed for `/loom/scaffold`
/// and for a successful chat, and that "non-200 backend paths lack the grounding
/// contract". A consumer that cannot distinguish *the corpus had nothing* from
/// *the model was unreachable* will read both as an ungrounded answer and treat
/// them the same, which is exactly the failure the contract exists to prevent.
///
/// So every response — success, degrade and failure — carries a status, and the
/// six variants below are the complete set the closeout enumerates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroundingStatus {
    /// Retrieval ran and nothing cleared the gate. The corpus was consulted.
    NoMatch,
    /// The scaffold was injected and delegated, and the caller declined the
    /// verbatim serve for this request (`loom_options.verbatim = false`).
    /// Distinct from `Delegated` because the node WOULD have served verbatim:
    /// the decision was the caller's, and a benchmark that cannot see that will
    /// mis-attribute the latency.
    OptOut,
    /// The lexical gate missed and the HNSW fallback supplied the seeds. The
    /// score scale is cosine, not lexical-additive.
    SemanticFallback,
    /// The scaffold was served AS the answer; no backend was called.
    Verbatim,
    /// The scaffold was injected (or not) and the backend answered 200.
    Delegated,
    /// The backend did not answer: unreachable, non-2xx, or not configured.
    /// There is no answer to be grounded, and `corpus_backed` is false however
    /// good the retrieval was.
    BackendFailure,
}

impl GroundingStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoMatch => "no-match",
            Self::OptOut => "opt-out",
            Self::SemanticFallback => "semantic-fallback",
            Self::Verbatim => "verbatim",
            Self::Delegated => "delegated",
            Self::BackendFailure => "backend-failure",
        }
    }

    /// Whether an answer delivered on this path may be treated as corpus-backed,
    /// GIVEN that the scaffold actually engaged.
    ///
    /// The two `false` cases are the ones a consumer most needs: a no-match has
    /// no evidence, and a backend failure has no answer. Everything else is
    /// corpus-backed exactly when the scaffold engaged.
    #[must_use]
    pub fn may_be_corpus_backed(self) -> bool {
        !matches!(self, Self::NoMatch | Self::BackendFailure)
    }
}

/// The keys every grounding object must carry, on every response status.
///
/// Named here rather than left implicit in the serialiser so a contract test can
/// assert the set directly, and so a future field addition is a deliberate edit
/// to a stated contract rather than a silent shape change.
pub const REQUIRED_GROUNDING_FIELDS: &[&str] = &[
    "signal",
    "top_score",
    "score_scale",
    "confidence",
    "decision",
    "threshold",
    "effective_budget",
    "engaged",
    "seeds",
    "status",
    "corpus_backed",
    "generation",
    "content_digest",
    "degraded",
];
