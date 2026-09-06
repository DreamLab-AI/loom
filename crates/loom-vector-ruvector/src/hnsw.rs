//! `HnswIndex` — the in-process semantic read path (§11.2, EXP-008).
//!
//! # Artefact qualification (ADR-137 closeout)
//!
//! `VectorDB::new` REPLACES the caller's `DbOptions` with the configuration the
//! opened database has stored. Passing cosine/384 therefore proves nothing: the
//! probe in the estate review opened a Euclidean artefact through this adapter
//! and watched aligned directions score 0.5 as if they were cosine, and opened a
//! 3-dimensional artefact that reported READY and only failed at query time.
//!
//! So this adapter now reads the EFFECTIVE stored settings back out of
//! `VectorDB::options()` after opening, compares them against an
//! [`ArtefactContract`], and derives `is_ready()` from that comparison rather
//! than from "the file opened and is non-empty". Three consequences:
//!
//! - a wrong-metric artefact is REJECTED, never relabelled — the score scale a
//!   caller reads is the metric the artefact was built with;
//! - a wrong-width artefact is rejected at open, before any query;
//! - embedding-model identity, which no geometry check can catch, is read from
//!   the generation sidecar and required to match. A sidecar that declares no
//!   model fails the contract unless `LOOM_SEMANTIC_REQUIRE_MODEL_ID=0` says a
//!   pre-contract artefact is knowingly being served.
//!
//! The rejection is still a DEGRADE, not an error: the semantic index is an
//! accelerator, so an unqualified artefact turns the fallback off and reports
//! why in `/health.semantic.qualification`.
//!
//! Holds a `ruvector_core::VectorDB` opened on the `LOOM_HNSW_ARTIFACT` storage
//! DB. `VectorDB::new` reads the stored config and auto-rebuilds the HNSW index
//! from the persisted vectors on open (verified in `ruvector-core/vector_db.rs`).
//! `nearest()` runs a cosine ANN and maps each `SearchResult` →
//! `ConceptMatch { iri: <record id>, score: cosine_similarity, provenance:
//! SemanticHnsw }`. The IRI is the record's own primary key, so the engine can
//! never leak its row shape back as an answer (I-P1 anti-corruption).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use loom_domain::artefact::{ArtefactContract, ArtefactQualification, VectorMetric};
use loom_domain::error::LoomError;
use loom_domain::model::{
    ConceptMatch, Generation, GenerationId, GenerationSource, Iri, MatchProvenance,
};
use loom_domain::ports::VectorIndex;
use ruvector_core::types::{DbOptions, DistanceMetric, HnswConfig, SearchQuery};
use ruvector_core::VectorDB;
use serde::Deserialize;

/// The locked embedding width (bge-small-en-v1.5). A mismatch invalidates the
/// artifact, so the reader treats an off-width query as fail-open, not a 500.
pub const EMBEDDING_DIMENSIONS: usize = 384;

/// Default artifact path inside the serving container (§10 config table).
pub const DEFAULT_ARTIFACT_PATH: &str = "/app/data/ontology-corpus.rvdb";

/// The generation sidecar written by the exporter, next to the artifact as
/// `<artifact>.generation.json`. Mirror-manifest source (EXP-009).
#[derive(Debug, Clone, Deserialize)]
struct GenerationSidecar {
    #[serde(rename = "generatedAt")]
    generated_at: String,
    #[serde(rename = "classCount")]
    class_count: usize,
    #[serde(default)]
    #[allow(dead_code)]
    // Retained for provenance/round-trip; not needed to build `Generation`.
    source: Option<String>,
    /// The embedding model the corpus vectors were produced with. The ONE fact
    /// the vector database cannot carry — `RuVector` stores geometry, not model
    /// provenance — and the one whose drift no width check can detect.
    #[serde(rename = "embeddingModel", default)]
    embedding_model: Option<String>,
    /// The exporter's declared vector width. Cross-checked against the stored
    /// configuration so a sidecar that describes a different artefact than the
    /// one beside it is caught (the database-to-sidecar binding CP-01 asks for).
    #[serde(rename = "dimensions", default)]
    dimensions: Option<usize>,
}

/// `VectorIndex` over an in-process ruvector-core HNSW.
///
/// Construction never fails: an absent or unreadable artifact yields a
/// not-ready index (`db == None`) that fails open on every query.
pub struct HnswIndex {
    /// `None` ⇒ the artifact was absent, empty, failed to open, or FAILED
    /// QUALIFICATION. An unqualified artefact is never held, so no query can
    /// reach a geometry this node did not accept.
    db: Option<Arc<VectorDB>>,
    artifact_path: PathBuf,
    /// Parsed once at open from the `<artifact>.generation.json` sidecar.
    generation: Generation,
    /// What the artefact turned out to be versus what was required — captured
    /// once at open, reported verbatim in `/health`.
    qualification: ArtefactQualification,
}

impl HnswIndex {
    /// Open the artifact at `LOOM_HNSW_ARTIFACT` (or [`DEFAULT_ARTIFACT_PATH`]),
    /// under the contract from the environment.
    #[must_use]
    pub fn from_env() -> Self {
        let path = std::env::var("LOOM_HNSW_ARTIFACT")
            .unwrap_or_else(|_| DEFAULT_ARTIFACT_PATH.to_owned());
        Self::open_with_contract(path, &contract_from_env())
    }

    /// The locked Loom contract, optionally relaxed on model identity by
    /// `LOOM_SEMANTIC_REQUIRE_MODEL_ID=0`.
    ///
    /// Strict is the default deliberately. A changed embedding model at equal
    /// width produces confident, plausible, wrong neighbours — the one semantic
    /// failure that is invisible in every other check — and semantic fallback is
    /// itself default-off, so requiring the declaration costs no deployment
    /// anything until it opts in.
    #[must_use]
    pub fn contract_from_env() -> ArtefactContract {
        contract_from_env()
    }

    /// Open under the locked contract (384/cosine/bge-small-en-v1.5, strict).
    #[must_use]
    pub fn open(artifact_path: impl Into<PathBuf>) -> Self {
        Self::open_with_contract(artifact_path, &ArtefactContract::bge_small_384())
    }

    /// Open the ruvector-core storage DB at `artifact_path` and qualify it.
    ///
    /// Fail-open by contract: a missing artifact, an empty DB, an open error, or
    /// a contract rejection all yield a not-ready index rather than a panic or an
    /// error return. A missing artifact is NEVER created here — the exporter owns
    /// writes.
    #[must_use]
    pub fn open_with_contract(
        artifact_path: impl Into<PathBuf>,
        contract: &ArtefactContract,
    ) -> Self {
        let artifact_path = artifact_path.into();
        let sidecar = read_sidecar(&artifact_path);
        let generation = sidecar_to_generation(sidecar.as_ref());

        let unopened = |detail: &str| Self {
            db: None,
            artifact_path: artifact_path.clone(),
            generation: generation.clone(),
            qualification: ArtefactQualification::unopened(contract.clone(), detail),
        };

        if !artifact_path.exists() {
            tracing::warn!(
                path = %artifact_path.display(),
                "HNSW artifact absent; semantic fallback disabled (fail-open)"
            );
            return unopened("artifact absent");
        }

        let db = match Self::try_open(&artifact_path) {
            Ok(Some(db)) => db,
            Ok(None) => {
                tracing::warn!(
                    path = %artifact_path.display(),
                    "HNSW artifact present but empty; semantic fallback disabled (fail-open)"
                );
                return unopened("artifact opened but holds no vectors");
            }
            Err(e) => {
                tracing::warn!(
                    path = %artifact_path.display(),
                    error = %e,
                    "failed to open HNSW artifact; semantic fallback disabled (fail-open)"
                );
                return unopened(&format!("open failed: {e}"));
            }
        };

        // THE CHECK. `options()` is the EFFECTIVE configuration after
        // `VectorDB::new` overwrote the caller's with whatever the database
        // stored — the only honest source for what is about to be queried.
        let effective = db.options();
        let metric = map_metric(effective.distance_metric);
        let declared_model = sidecar.as_ref().and_then(|s| s.embedding_model.as_deref());
        let mut qualification = contract.qualify(effective.dimensions, metric, declared_model);

        // Database-to-sidecar binding: a sidecar that declares a width the
        // database does not have is describing a different artefact.
        if let Some(declared_dim) = sidecar.as_ref().and_then(|s| s.dimensions) {
            if declared_dim != effective.dimensions {
                qualification
                    .rejections
                    .push(loom_domain::artefact::ArtefactError::Dimension {
                        got: declared_dim,
                        want: effective.dimensions,
                    });
            }
        }

        if !qualification.is_qualified() {
            tracing::warn!(
                path = %artifact_path.display(),
                reasons = ?qualification.reasons(),
                "HNSW artifact FAILED qualification; semantic fallback disabled (fail-open)"
            );
            return Self {
                db: None,
                artifact_path,
                generation,
                qualification,
            };
        }

        tracing::info!(
            path = %artifact_path.display(),
            dimensions = effective.dimensions,
            metric = metric.as_str(),
            model = ?declared_model,
            "HNSW artifact qualified"
        );
        Self {
            db: Some(Arc::new(db)),
            artifact_path,
            generation,
            qualification,
        }
    }

    /// Try to open the DB. `Ok(None)` means the DB opened but holds no vectors.
    ///
    /// The seed options below are exactly that — a seed. On an EXISTING database
    /// `VectorDB::new` discards them and restores the stored configuration, which
    /// is precisely why [`Self::open_with_contract`] reads `options()` back
    /// afterwards instead of trusting what it passed in.
    fn try_open(path: &Path) -> Result<Option<VectorDB>, ruvector_core::RuvectorError> {
        let opts = DbOptions {
            dimensions: EMBEDDING_DIMENSIONS,
            distance_metric: DistanceMetric::Cosine,
            storage_path: path.to_string_lossy().into_owned(),
            hnsw_config: Some(HnswConfig::default()),
            quantization: Some(ruvector_core::types::QuantizationConfig::None),
        };
        let db = VectorDB::new(opts)?;
        if db.len()? == 0 {
            return Ok(None);
        }
        Ok(Some(db))
    }

    /// The path this index was opened from (diagnostics).
    #[must_use]
    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }
}

#[async_trait]
impl VectorIndex for HnswIndex {
    async fn nearest(&self, query_vec: &[f32], k: usize) -> Result<Vec<ConceptMatch>, LoomError> {
        let Some(db) = self.db.as_ref().map(Arc::clone) else {
            return Err(LoomError::SemanticUnready(format!(
                "artifact not ready: {}",
                self.artifact_path.display()
            )));
        };

        if query_vec.len() != EMBEDDING_DIMENSIONS {
            // Off-width query invalidates the cosine geometry; degrade, never 500.
            return Err(LoomError::SemanticUnready(format!(
                "query dimension {} != {EMBEDDING_DIMENSIONS}",
                query_vec.len()
            )));
        }

        let query = query_vec.to_vec();
        let results = tokio::task::spawn_blocking(move || {
            db.search(SearchQuery {
                vector: query,
                k,
                filter: None,
                ef_search: None,
            })
        })
        .await
        .map_err(|e| LoomError::SemanticUnready(format!("hnsw search task failed: {e}")))?
        .map_err(|e| LoomError::SemanticUnready(format!("hnsw search failed: {e}")))?;

        // ruvector cosine `score` is a DISTANCE (1 - cos_sim), sorted best-first.
        // Map to a similarity in [0,1] — the scale the injection gate reads.
        //
        // This conversion is only honest for cosine, which is why it lives
        // BEHIND the qualification gate: `db` is `Some` only for an artefact
        // whose effective metric IS cosine, so `1 - d` here cannot be applied to
        // a Euclidean distance the way the probe reproduced.
        debug_assert!(
            self.qualification
                .served_metric()
                .is_some_and(VectorMetric::yields_cosine_similarity),
            "a queryable artefact must be cosine"
        );
        Ok(results
            .into_iter()
            .map(|r| ConceptMatch {
                iri: Iri::new(r.id),
                score: (1.0_f32 - r.score).clamp(0.0, 1.0),
                provenance: MatchProvenance::SemanticHnsw,
            })
            .collect())
    }

    /// Readiness IS qualification. `db` is only ever `Some` for an artefact
    /// that passed the contract, so the two can never disagree.
    fn is_ready(&self) -> bool {
        self.db.is_some() && self.qualification.is_qualified()
    }

    fn generation(&self) -> Generation {
        self.generation.clone()
    }

    fn qualification(&self) -> ArtefactQualification {
        self.qualification.clone()
    }
}

/// The contract this process serves under, from the environment.
fn contract_from_env() -> ArtefactContract {
    let base = ArtefactContract::bge_small_384();
    match std::env::var("LOOM_SEMANTIC_REQUIRE_MODEL_ID")
        .ok()
        .as_deref()
    {
        Some("0" | "false" | "no" | "") => base.without_model_id_requirement(),
        _ => base,
    }
}

/// Map the engine's metric onto the domain's, without the domain depending on
/// the engine. An engine variant this build does not know becomes `Other`, which
/// fails every contract — the safe direction.
fn map_metric(m: DistanceMetric) -> VectorMetric {
    match m {
        DistanceMetric::Cosine => VectorMetric::Cosine,
        DistanceMetric::Euclidean => VectorMetric::Euclidean,
        DistanceMetric::DotProduct => VectorMetric::DotProduct,
        DistanceMetric::Manhattan => VectorMetric::Manhattan,
    }
}

/// Compute the sidecar path `<artifact>.generation.json`.
fn sidecar_path(artifact: &Path) -> PathBuf {
    let mut raw = artifact.as_os_str().to_owned();
    raw.push(".generation.json");
    PathBuf::from(raw)
}

/// Read the generation sidecar, if it is present and parses.
fn read_sidecar(artifact: &Path) -> Option<GenerationSidecar> {
    std::fs::read_to_string(sidecar_path(artifact))
        .ok()
        .and_then(|s| serde_json::from_str::<GenerationSidecar>(&s).ok())
}

#[cfg(test)]
/// Load and map the generation sidecar. A missing/unparseable sidecar yields an
/// `Unavailable` generation (so the §6 parity guard treats it as non-matching).
fn load_generation_sidecar(artifact: &Path) -> Generation {
    sidecar_to_generation(read_sidecar(artifact).as_ref())
}

/// Map a parsed sidecar onto a `Generation`.
fn sidecar_to_generation(parsed: Option<&GenerationSidecar>) -> Generation {
    match parsed {
        Some(g) => Generation {
            id: GenerationId(g.generated_at.clone()),
            source: GenerationSource::MirrorManifest,
            generated_at: Some(g.generated_at.clone()),
            commit_sha: None,
            promoted_at: None,
            cluster_span_seconds: None,
            artifacts: Vec::new(),
            verified_single_generation: true,
            class_count: Some(g.class_count),
        },
        None => Generation {
            id: GenerationId("unavailable".to_owned()),
            source: GenerationSource::Unavailable,
            generated_at: None,
            commit_sha: None,
            promoted_at: None,
            cluster_span_seconds: None,
            artifacts: Vec::new(),
            verified_single_generation: false,
            class_count: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_domain::artefact::ArtefactError;
    use ruvector_core::types::{QuantizationConfig, VectorEntry};
    use tempfile::tempdir;

    /// A 384-dim unit-ish vector: `1.0` at `axis`, small fill elsewhere.
    fn synth_vec(axis: usize) -> Vec<f32> {
        let mut v = vec![0.01_f32; EMBEDDING_DIMENSIONS];
        v[axis % EMBEDDING_DIMENSIONS] = 1.0;
        v
    }

    fn synth_vec_dim(axis: usize, dim: usize) -> Vec<f32> {
        let mut v = vec![0.01_f32; dim];
        v[axis % dim] = 1.0;
        v
    }

    /// Build a real ruvector-core DB with a chosen geometry — the fixture the
    /// probe used, now a first-class test helper so every contract axis has a
    /// real artefact behind it rather than a mocked one.
    fn seed_db_with(path: &Path, dimensions: usize, metric: DistanceMetric) {
        let opts = DbOptions {
            dimensions,
            distance_metric: metric,
            storage_path: path.to_string_lossy().into_owned(),
            hnsw_config: Some(HnswConfig {
                m: 16,
                ef_construction: 128,
                ef_search: 100,
                max_elements: 1024,
            }),
            quantization: Some(QuantizationConfig::None),
        };
        let db = VectorDB::new(opts).expect("create seed db");
        db.insert_batch(
            ["alpha", "beta", "gamma"]
                .iter()
                .enumerate()
                .map(|(i, slug)| VectorEntry {
                    id: Some(format!("urn:ngm:class:{slug}")),
                    vector: synth_vec_dim(i, dimensions),
                    metadata: None,
                })
                .collect(),
        )
        .expect("insert seed batch");
        assert_eq!(db.len().unwrap(), 3);
    }

    /// The contract-satisfying artefact: 384, cosine.
    fn seed_db(path: &Path) {
        seed_db_with(path, EMBEDDING_DIMENSIONS, DistanceMetric::Cosine);
    }

    /// Write a generation sidecar beside `artifact`.
    fn write_sidecar(artifact: &Path, model: Option<&str>, dimensions: Option<usize>) {
        let mut fields = vec![
            "\"generatedAt\":\"2026-08-17T09:30:00Z\"".to_owned(),
            "\"classCount\":3".to_owned(),
            "\"source\":\"ontology-corpus-export\"".to_owned(),
        ];
        if let Some(m) = model {
            fields.push(format!("\"embeddingModel\":\"{m}\""));
        }
        if let Some(d) = dimensions {
            fields.push(format!("\"dimensions\":{d}"));
        }
        std::fs::write(sidecar_path(artifact), format!("{{{}}}", fields.join(","))).unwrap();
    }

    /// A fully-conforming artefact: cosine/384 vectors plus a sidecar that
    /// declares the locked model. The baseline every rejection test perturbs.
    fn qualified_artefact(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        seed_db(&path);
        write_sidecar(&path, Some("bge-small-en-v1.5"), Some(EMBEDDING_DIMENSIONS));
        path
    }

    #[tokio::test]
    async fn absent_artifact_is_not_ready_and_fails_open() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist.rvdb");
        let idx = HnswIndex::open(&missing);

        assert!(!idx.is_ready(), "absent artifact must not be ready");
        assert!(!missing.exists(), "open() must not create the artifact");
        assert!(matches!(
            idx.qualification().first_rejection(),
            Some(ArtefactError::Unopened(_))
        ));

        let err = idx.nearest(&synth_vec(0), 5).await.unwrap_err();
        assert!(
            matches!(err, LoomError::SemanticUnready(_)),
            "absent artifact must fail open with SemanticUnready, got {err:?}"
        );
    }

    #[tokio::test]
    async fn qualified_artefact_round_trips_with_cosine_ordering_and_iri() {
        let dir = tempdir().unwrap();
        let path = qualified_artefact(dir.path(), "roundtrip.rvdb");

        let idx = HnswIndex::open(&path);
        assert!(
            idx.is_ready(),
            "qualified artefact must be ready: {:?}",
            idx.qualification().reasons()
        );
        assert_eq!(
            idx.qualification().served_metric(),
            Some(VectorMetric::Cosine)
        );

        let hits = idx.nearest(&synth_vec(0), 3).await.unwrap();
        assert!(!hits.is_empty(), "expected hits from a seeded index");
        assert_eq!(
            hits[0].iri,
            Iri::new("urn:ngm:class:alpha"),
            "nearest to axis-0 query must be alpha; id round-trips as Iri"
        );
        assert_eq!(hits[0].iri.slug(), "alpha");
        assert_eq!(hits[0].provenance, MatchProvenance::SemanticHnsw);

        for h in &hits {
            assert!(
                (0.0..=1.0).contains(&h.score),
                "similarity must be in [0,1], got {}",
                h.score
            );
        }
        assert!(
            hits[0].score > 0.9,
            "aligned query should score ~1.0, got {}",
            hits[0].score
        );
        if hits.len() > 1 {
            assert!(
                hits[0].score >= hits[1].score,
                "results must be sorted by descending similarity: {} < {}",
                hits[0].score,
                hits[1].score
            );
        }
    }

    // --- the three rows the estate-review probe reproduced -------------------

    /// Probe row 2. Previously: ready, and aligned directions scored 0.5 through
    /// the cosine conversion. Now: rejected, with the metric named on both sides.
    #[tokio::test]
    async fn euclidean_artefact_is_rejected_not_served_as_cosine() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("euclidean.rvdb");
        seed_db_with(&path, EMBEDDING_DIMENSIONS, DistanceMetric::Euclidean);
        write_sidecar(&path, Some("bge-small-en-v1.5"), Some(EMBEDDING_DIMENSIONS));

        let idx = HnswIndex::open(&path);
        assert!(!idx.is_ready(), "a Euclidean artefact must not be ready");
        assert_eq!(
            idx.qualification().first_rejection(),
            Some(&ArtefactError::Metric {
                got: VectorMetric::Euclidean,
                want: VectorMetric::Cosine
            })
        );
        assert_eq!(
            idx.qualification().served_metric(),
            None,
            "an unqualified artefact must label no score scale at all"
        );
        // And it is genuinely unqueryable, so no 0.5-as-cosine can be produced.
        assert!(matches!(
            idx.nearest(&synth_vec(0), 3).await.unwrap_err(),
            LoomError::SemanticUnready(_)
        ));
    }

    /// Probe row 3. Previously: ready, failing only later at query time. Now the
    /// rejection happens at open, so readiness never over-promises.
    #[tokio::test]
    async fn wrong_width_artefact_is_rejected_at_open_not_at_query() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("narrow.rvdb");
        seed_db_with(&path, 3, DistanceMetric::Cosine);
        write_sidecar(&path, Some("bge-small-en-v1.5"), Some(3));

        let idx = HnswIndex::open(&path);
        assert!(
            !idx.is_ready(),
            "a 3-dimensional artefact must not report ready"
        );
        assert!(idx
            .qualification()
            .rejections
            .contains(&ArtefactError::Dimension { got: 3, want: 384 }));
    }

    /// The failure no geometry check catches: same width, different model.
    #[test]
    fn changed_model_at_equal_width_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("othermodel.rvdb");
        seed_db(&path);
        write_sidecar(&path, Some("all-MiniLM-L6-v2"), Some(EMBEDDING_DIMENSIONS));

        let idx = HnswIndex::open(&path);
        assert!(!idx.is_ready());
        assert!(matches!(
            idx.qualification().first_rejection(),
            Some(ArtefactError::Model { .. })
        ));
    }

    /// A pre-contract artefact (no model declared) fails strict qualification,
    /// and passes only when a deployment explicitly relaxes the requirement.
    #[test]
    fn undeclared_model_is_strict_by_default_and_relaxable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nomodel.rvdb");
        seed_db(&path);
        write_sidecar(&path, None, None);

        let strict = HnswIndex::open(&path);
        assert!(
            !strict.is_ready(),
            "undeclared model must fail the strict contract"
        );
        assert!(matches!(
            strict.qualification().first_rejection(),
            Some(ArtefactError::ModelUnknown { .. })
        ));

        let relaxed = HnswIndex::open_with_contract(
            &path,
            &ArtefactContract::bge_small_384().without_model_id_requirement(),
        );
        assert!(
            relaxed.is_ready(),
            "relaxed contract must accept it: {:?}",
            relaxed.qualification().reasons()
        );
    }

    /// Database-to-sidecar binding: a sidecar describing a different width than
    /// the database beside it is describing a different artefact.
    #[test]
    fn sidecar_that_contradicts_the_database_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mismatched-sidecar.rvdb");
        seed_db(&path); // real width 384
        write_sidecar(&path, Some("bge-small-en-v1.5"), Some(768));

        let idx = HnswIndex::open(&path);
        assert!(!idx.is_ready());
        assert!(
            idx.qualification().rejections.iter().any(|r| matches!(
                r,
                ArtefactError::Dimension {
                    got: 768,
                    want: 384
                }
            )),
            "expected a sidecar/database width disagreement: {:?}",
            idx.qualification().reasons()
        );
    }

    // --- restart fixtures ----------------------------------------------------

    /// Reopening the same artefact must produce the same verdict. This is the
    /// restart case the closeout asks for: qualification is a property of the
    /// stored bytes, not of the process that happened to open them first.
    #[tokio::test]
    async fn qualification_is_stable_across_reopen() {
        let dir = tempdir().unwrap();
        let path = qualified_artefact(dir.path(), "restart.rvdb");

        let first = HnswIndex::open(&path);
        let first_hits = first.nearest(&synth_vec(1), 3).await.unwrap();
        drop(first);

        let second = HnswIndex::open(&path);
        assert!(second.is_ready(), "reopen must qualify identically");
        let second_hits = second.nearest(&synth_vec(1), 3).await.unwrap();
        assert_eq!(
            first_hits[0].iri, second_hits[0].iri,
            "reopen must rank identically"
        );
        assert_eq!(
            second.qualification().metric,
            VectorMetric::Cosine,
            "effective metric survives the restart"
        );
    }

    /// An artefact that was fine and is then re-exported wrong must be REJECTED
    /// on the next start, not inherited as ready from the previous run.
    #[test]
    fn restart_onto_a_degraded_artefact_rejects_it() {
        let dir = tempdir().unwrap();
        let path = qualified_artefact(dir.path(), "regress.rvdb");
        assert!(HnswIndex::open(&path).is_ready());

        // The exporter is re-run against a different model at the same width.
        write_sidecar(&path, Some("gte-small"), Some(EMBEDDING_DIMENSIONS));
        let after = HnswIndex::open(&path);
        assert!(
            !after.is_ready(),
            "a changed model must not survive a restart"
        );
        assert!(matches!(
            after.qualification().first_rejection(),
            Some(ArtefactError::Model { .. })
        ));
    }

    #[tokio::test]
    async fn off_width_query_fails_open() {
        let dir = tempdir().unwrap();
        let path = qualified_artefact(dir.path(), "width.rvdb");
        let idx = HnswIndex::open(&path);

        let err = idx.nearest(&[0.1_f32, 0.2, 0.3], 5).await.unwrap_err();
        assert!(matches!(err, LoomError::SemanticUnready(_)));
    }

    // --- sidecar mapping ------------------------------------------------------

    #[test]
    fn generation_sidecar_parses_to_mirror_manifest() {
        let dir = tempdir().unwrap();
        let artifact = dir.path().join("gen.rvdb");
        let sidecar = sidecar_path(&artifact);
        std::fs::write(
            &sidecar,
            r#"{"generatedAt":"2026-08-17T09:30:00Z","classCount":8146,"source":"ontology-corpus-export"}"#,
        )
        .unwrap();

        let gen = load_generation_sidecar(&artifact);
        assert_eq!(gen.source, GenerationSource::MirrorManifest);
        assert_eq!(gen.id, GenerationId("2026-08-17T09:30:00Z".to_owned()));
        assert_eq!(gen.generated_at.as_deref(), Some("2026-08-17T09:30:00Z"));
        assert_eq!(gen.class_count, Some(8146));
        assert!(gen.verified_single_generation);
    }

    /// The extended sidecar stays backwards-compatible: the new fields default
    /// to absent, so an existing deployed sidecar still parses.
    #[test]
    fn sidecar_without_model_fields_still_parses() {
        let dir = tempdir().unwrap();
        let artifact = dir.path().join("legacy.rvdb");
        std::fs::write(
            sidecar_path(&artifact),
            r#"{"generatedAt":"2026-08-17T14:54:45Z","classCount":8146,"source":"ontology-corpus-export"}"#,
        )
        .unwrap();
        let s = read_sidecar(&artifact).expect("legacy sidecar parses");
        assert_eq!(s.embedding_model, None);
        assert_eq!(s.dimensions, None);
        assert_eq!(s.class_count, 8146);
    }

    #[test]
    fn missing_sidecar_is_unavailable_generation() {
        let dir = tempdir().unwrap();
        let artifact = dir.path().join("no-sidecar.rvdb");
        let gen = load_generation_sidecar(&artifact);
        assert_eq!(gen.source, GenerationSource::Unavailable);
        assert!(!gen.verified_single_generation);
        assert_eq!(gen.class_count, None);
    }

    #[test]
    fn sidecar_path_appends_suffix() {
        let p = sidecar_path(Path::new("/app/data/ontology-corpus.rvdb"));
        assert_eq!(
            p,
            PathBuf::from("/app/data/ontology-corpus.rvdb.generation.json")
        );
    }

    #[test]
    fn every_engine_metric_maps_to_a_domain_metric() {
        assert_eq!(map_metric(DistanceMetric::Cosine), VectorMetric::Cosine);
        assert_eq!(
            map_metric(DistanceMetric::Euclidean),
            VectorMetric::Euclidean
        );
        assert_eq!(
            map_metric(DistanceMetric::DotProduct),
            VectorMetric::DotProduct
        );
        assert_eq!(
            map_metric(DistanceMetric::Manhattan),
            VectorMetric::Manhattan
        );
    }
}
