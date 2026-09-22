//! The session manifest (ADR-140 D2) — the ONLY ontology content an agent
//! receives without asking for it.
//!
//! The property this type exists to hold is a cost one: on the injection plane
//! the prompt carries O(matched blocks) of corpus, and that grows as the corpus
//! does. The manifest is O(1) in corpus size — a profile, a coverage report, a
//! bounded salience set and the tool contract — and every detailed record is
//! fetched on demand through `loom.resolve`. That is what lets the agentic plane
//! stay affordable on a corpus far larger than any budget clamp would allow us
//! to inject.
//!
//! Invariant I-P1 is unaffected: nothing here is a served answer. A
//! [`SalientTerm`] is an ADDRESS (an `Iri` plus enough label to decide whether
//! to fetch it), never content. The manifest tells an agent what exists and how
//! to ask; only `resolve` returns a `CanonicalUnit`.

use crate::model::{Generation, Iri};

/// What kind of unit a [`browse`](crate::ports::LexicalIndex::browse) call is
/// looking for. Present from P0 with two variants because the tool schema must
/// be stable from the first release; the corpus gains the rest with ADR-140 D4.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitKind {
    /// Any addressable unit — the P0 default and the only kind the current
    /// corpus distinguishes.
    #[default]
    Any,
    /// A domain concept (the `Term` family).
    Term,
    /// A binding from a Term to a queryable locus (ADR-140 D4). Empty until P2.
    Mapping,
    /// An executed, re-executable computation with an executor and an attester
    /// (ADR-140 D4 in OKF vocabulary, ADR-141). Empty until P2.
    AttestedComputation,
}

impl UnitKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Term => "term",
            Self::Mapping => "mapping",
            Self::AttestedComputation => "attested-computation",
        }
    }
}

/// One domain family and how much of the corpus sits in it. The agent uses this
/// to decide where to look before it has looked anywhere.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DomainFamily {
    pub name: String,
    pub classes: usize,
    /// Mean curation quality across the family, `[0, 1]`. A low number is a
    /// warning to the agent, which is why it is exposed rather than hidden.
    pub mean_quality: f64,
}

/// The shape of the corpus, without any of its content.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CorpusProfile {
    pub classes: usize,
    pub domains: Vec<DomainFamily>,
    /// `maturity` label → count. Free-form because the corpus owns the
    /// vocabulary (`established`, `emerging`, …) and the node must not silently
    /// drop a label it does not recognise.
    pub maturity: Vec<(String, usize)>,
    pub mean_quality: f64,
}

/// How grounded the corpus actually is, in OKF vocabulary (ADR-140 D4 as
/// amended by ADR-141).
///
/// Reported honestly, including when every count except `terms` is zero: a
/// consumer must be able to tell "no attested computations exist yet" from
/// "they exist and this IRI has none". The two `verified_*` counters are OKF
/// trust tiers, not a quality score — a machine verification and a human
/// signature are different claims and are counted apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CoverageReport {
    /// Terms (classes) in the generation.
    pub terms: usize,
    /// Terms carrying at least one [`AttestedComputation`](crate::okf::AttestedComputation).
    pub attested_computations: usize,
    /// Terms verified by a `process:`/`agent:` actor.
    pub verified_machine: usize,
    /// Terms verified by a `human:` actor — the governance-signed tier.
    pub verified_human: usize,
}

impl CoverageReport {
    /// The honest report for a generation that carries terms and no OKF trust
    /// keys at all.
    #[must_use]
    pub fn terms_only(terms: usize) -> Self {
        Self {
            terms,
            ..Self::default()
        }
    }
}

/// An ADDRESS the agent may choose to resolve. Never content — `definition` is
/// a one-line hint capped by the manifest builder, not the served block.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SalientTerm {
    pub iri: Iri,
    pub title: String,
    pub domain: Option<String>,
    pub quality: Option<f64>,
}

/// One tool in the contract, as the active exposure profile describes it.
/// Descriptions are profile data (ADR-140 D7) so they can be fitted per backbone
/// without touching the corpus or recompiling the node.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
}

/// The session index (ADR-140 D2).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub generation: Generation,
    pub profile: CorpusProfile,
    pub coverage: CoverageReport,
    pub salient: Vec<SalientTerm>,
    pub tools: Vec<ToolDescriptor>,
    /// Accelerators unavailable for this session, named rather than implied —
    /// the live node currently serves with `semantic.ready == false`, and an
    /// agent that does not know that will over-trust a lexical miss
    /// (ADR-140 Addendum A, conclusion 3).
    pub degraded: Vec<String>,
}
