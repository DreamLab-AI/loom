//! The semantic-artefact contract (ADR-137 "Semantic artefact qualification").
//!
//! The estate review's finding was precise: `ruvector-core` REPLACES the
//! caller's `DbOptions` with whatever configuration the opened database has
//! stored, so a consumer that passes cosine/384 and then asserts nothing has no
//! idea what geometry it is actually querying. The probe reproduced two
//! consequences — a Euclidean artefact accepted and its distance relabelled as
//! cosine (aligned directions scoring 0.5 instead of 1.0), and a 3-dimensional
//! artefact reported READY and only failing later, at query time.
//!
//! This module is the fix's vocabulary: an [`ArtefactContract`] stating what the
//! serving node requires, an [`ArtefactQualification`] stating what the artefact
//! actually IS, and an [`ArtefactError`] enumerating the ways the two disagree.
//! It is pure data and comparison — the adapter (`loom-vector-ruvector`) reads
//! the effective stored settings and calls [`ArtefactContract::qualify`].
//!
//! Two rules the types enforce structurally:
//!
//! 1. **Readiness follows qualification, not openability.** A qualification that
//!    is not [`ArtefactQualification::is_qualified`] must make the index report
//!    `is_ready() == false`, so a wrong-width artefact is rejected BEFORE a
//!    query rather than after it.
//! 2. **A score is labelled with the metric that produced it.** The
//!    qualification carries the artefact's EFFECTIVE [`VectorMetric`], so no
//!    caller can print `score_scale: "cosine"` over a Euclidean distance.

use serde::{Deserialize, Serialize};

/// The distance metric an artefact was actually built with.
///
/// Mirrors `ruvector_core::types::DistanceMetric` without depending on it — the
/// domain crate is the hexagon's leaf and must not gain a vector-engine
/// dependency. The adapter maps the engine enum onto this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VectorMetric {
    Cosine,
    Euclidean,
    DotProduct,
    Manhattan,
    /// The engine reported a metric this domain does not model. Carried rather
    /// than collapsed into one of the four, so an unexpected artefact is
    /// rejected with the engine's own word for what it is.
    Other,
}

impl VectorMetric {
    /// The wire spelling, matching the serde rename.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cosine => "cosine",
            Self::Euclidean => "euclidean",
            Self::DotProduct => "dot-product",
            Self::Manhattan => "manhattan",
            Self::Other => "other",
        }
    }

    /// Whether a similarity in `[0,1]` can honestly be derived from this
    /// metric's distance by the adapter's `1 - d` conversion. ONLY cosine: for
    /// every other metric that arithmetic produces a number that looks like a
    /// cosine and is not one.
    #[must_use]
    pub fn yields_cosine_similarity(self) -> bool {
        matches!(self, Self::Cosine)
    }
}

/// The index geometry a serving node requires of its semantic artefact.
///
/// `model_id` is the embedding model the corpus vectors were produced with. It
/// cannot come from the vector database — RuVector stores geometry, not model
/// provenance — so it is read from the artefact's generation sidecar and
/// compared here. A changed model at equal width is the failure mode that no
/// dimension check can catch, which is why an unknown model is a rejection
/// under [`ArtefactContract::require_model_id`] rather than a warning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtefactContract {
    pub dimensions: usize,
    pub metric: VectorMetric,
    pub model_id: String,
    /// When true (the default), an artefact whose sidecar does not declare its
    /// embedding model fails qualification. Set false only for a deployment
    /// knowingly serving a pre-contract artefact.
    pub require_model_id: bool,
}

impl ArtefactContract {
    /// The locked Loom contract: bge-small-en-v1.5, 384 dimensions, cosine.
    #[must_use]
    pub fn bge_small_384() -> Self {
        Self {
            dimensions: 384,
            metric: VectorMetric::Cosine,
            model_id: "bge-small-en-v1.5".to_owned(),
            require_model_id: true,
        }
    }

    /// Relax the model-identity requirement (a pre-contract artefact whose
    /// sidecar predates the `embeddingModel` field).
    #[must_use]
    pub fn without_model_id_requirement(mut self) -> Self {
        self.require_model_id = false;
        self
    }

    /// Compare an artefact's OBSERVED settings against this contract.
    ///
    /// `observed_model_id` is the sidecar's declared embedding model, or `None`
    /// when the sidecar does not carry one. The returned qualification is
    /// always fully populated — a rejection still records what was observed, so
    /// `/health` can say *why* semantic serving is off rather than only *that*
    /// it is.
    #[must_use]
    pub fn qualify(
        &self,
        observed_dimensions: usize,
        observed_metric: VectorMetric,
        observed_model_id: Option<&str>,
    ) -> ArtefactQualification {
        let mut rejections = Vec::new();

        if observed_dimensions != self.dimensions {
            rejections.push(ArtefactError::Dimension {
                got: observed_dimensions,
                want: self.dimensions,
            });
        }
        if observed_metric != self.metric {
            rejections.push(ArtefactError::Metric {
                got: observed_metric,
                want: self.metric,
            });
        }
        match observed_model_id {
            Some(got) if got == self.model_id => {}
            Some(got) => rejections.push(ArtefactError::Model {
                got: got.to_owned(),
                want: self.model_id.clone(),
            }),
            None if self.require_model_id => rejections.push(ArtefactError::ModelUnknown {
                want: self.model_id.clone(),
            }),
            None => {}
        }

        ArtefactQualification {
            dimensions: observed_dimensions,
            metric: observed_metric,
            model_id: observed_model_id.map(std::borrow::ToOwned::to_owned),
            contract: self.clone(),
            rejections,
        }
    }
}

/// What an artefact turned out to be, and whether that satisfies the contract.
///
/// Serialised verbatim into `/health.semantic.qualification`, so an operator
/// reads the observed geometry and every rejection reason in one place.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtefactQualification {
    /// The EFFECTIVE stored dimension count (RuVector's, not the caller's).
    pub dimensions: usize,
    /// The EFFECTIVE stored metric. This is the label a score must carry.
    pub metric: VectorMetric,
    /// The sidecar's declared embedding model, when it declares one.
    pub model_id: Option<String>,
    /// What was required.
    pub contract: ArtefactContract,
    /// Every way the artefact failed the contract. Empty ⇒ qualified.
    pub rejections: Vec<ArtefactError>,
}

impl ArtefactQualification {
    /// A qualification for an artefact that could not be opened at all — absent,
    /// empty, or an engine error. Not a contract failure: there is nothing to
    /// compare, so the single rejection names the open failure.
    #[must_use]
    pub fn unopened(contract: ArtefactContract, detail: impl Into<String>) -> Self {
        Self {
            dimensions: 0,
            metric: VectorMetric::Other,
            model_id: None,
            contract,
            rejections: vec![ArtefactError::Unopened(detail.into())],
        }
    }

    /// Whether this artefact may be served from. The ONE predicate
    /// `VectorIndex::is_ready` must be derived from.
    #[must_use]
    pub fn is_qualified(&self) -> bool {
        self.rejections.is_empty()
    }

    /// The first rejection, for a one-line diagnostic. `None` when qualified.
    #[must_use]
    pub fn first_rejection(&self) -> Option<&ArtefactError> {
        self.rejections.first()
    }

    /// The score scale a caller may honestly label this artefact's scores with.
    ///
    /// A qualified (therefore cosine) artefact yields cosine similarities; an
    /// unqualified one yields nothing at all, because it is never queried.
    #[must_use]
    pub fn served_metric(&self) -> Option<VectorMetric> {
        self.is_qualified().then_some(self.metric)
    }

    /// Every rejection as a human sentence — the `/health` diagnostic list.
    #[must_use]
    pub fn reasons(&self) -> Vec<String> {
        self.rejections.iter().map(ToString::to_string).collect()
    }
}

/// The typed ways a semantic artefact can fail its contract.
///
/// Each variant names BOTH sides of the disagreement so a receipt records what
/// was required as well as what was found — a bare "incompatible artefact"
/// tells an operator nothing about which of the three axes moved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ArtefactError {
    #[error("artefact dimension {got} != contract {want}")]
    Dimension { got: usize, want: usize },

    #[error("artefact metric {} != contract {}", got.as_str(), want.as_str())]
    Metric {
        got: VectorMetric,
        want: VectorMetric,
    },

    #[error("artefact embedding model {got:?} != contract {want:?}")]
    Model { got: String, want: String },

    #[error("artefact declares no embedding model; contract requires {want:?}")]
    ModelUnknown { want: String },

    #[error("artefact could not be opened: {0}")]
    Unopened(String),

    #[error("artefact generation {artefact:?} != corpus generation {corpus:?}")]
    GenerationMismatch { artefact: String, corpus: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_artefact_qualifies() {
        let q = ArtefactContract::bge_small_384().qualify(
            384,
            VectorMetric::Cosine,
            Some("bge-small-en-v1.5"),
        );
        assert!(q.is_qualified(), "384/cosine/bge must qualify: {:?}", q.rejections);
        assert_eq!(q.served_metric(), Some(VectorMetric::Cosine));
        assert!(q.reasons().is_empty());
    }

    /// The probe's row 2: a Euclidean artefact must NOT be relabelled cosine.
    #[test]
    fn euclidean_artefact_is_rejected_not_relabelled() {
        let q = ArtefactContract::bge_small_384().qualify(
            384,
            VectorMetric::Euclidean,
            Some("bge-small-en-v1.5"),
        );
        assert!(!q.is_qualified());
        assert_eq!(
            q.first_rejection(),
            Some(&ArtefactError::Metric {
                got: VectorMetric::Euclidean,
                want: VectorMetric::Cosine
            })
        );
        assert_eq!(q.served_metric(), None, "an unqualified artefact serves no scale");
        assert!(!VectorMetric::Euclidean.yields_cosine_similarity());
    }

    /// The probe's row 3: a wrong-width artefact is rejected at qualification,
    /// not at query time.
    #[test]
    fn wrong_width_artefact_is_rejected_before_query() {
        let q =
            ArtefactContract::bge_small_384().qualify(3, VectorMetric::Cosine, Some("bge-small-en-v1.5"));
        assert!(!q.is_qualified());
        assert_eq!(
            q.first_rejection(),
            Some(&ArtefactError::Dimension { got: 3, want: 384 })
        );
    }

    /// The failure no width check can catch: a different model at equal width.
    #[test]
    fn changed_model_at_equal_width_is_rejected() {
        let q = ArtefactContract::bge_small_384().qualify(
            384,
            VectorMetric::Cosine,
            Some("all-MiniLM-L6-v2"),
        );
        assert!(!q.is_qualified());
        assert!(matches!(
            q.first_rejection(),
            Some(ArtefactError::Model { .. })
        ));
    }

    #[test]
    fn unknown_model_rejects_under_strict_contract_and_passes_when_relaxed() {
        let strict = ArtefactContract::bge_small_384().qualify(384, VectorMetric::Cosine, None);
        assert!(!strict.is_qualified());
        assert!(matches!(
            strict.first_rejection(),
            Some(ArtefactError::ModelUnknown { .. })
        ));

        let relaxed = ArtefactContract::bge_small_384()
            .without_model_id_requirement()
            .qualify(384, VectorMetric::Cosine, None);
        assert!(relaxed.is_qualified());
    }

    #[test]
    fn every_disagreement_is_reported_not_just_the_first() {
        let q = ArtefactContract::bge_small_384().qualify(3, VectorMetric::Euclidean, None);
        assert_eq!(q.rejections.len(), 3, "dimension + metric + model: {:?}", q.rejections);
        assert_eq!(q.reasons().len(), 3);
    }

    #[test]
    fn unopened_artefact_is_unqualified_with_a_named_reason() {
        let q = ArtefactQualification::unopened(ArtefactContract::bge_small_384(), "absent");
        assert!(!q.is_qualified());
        assert!(matches!(
            q.first_rejection(),
            Some(ArtefactError::Unopened(_))
        ));
    }
}
