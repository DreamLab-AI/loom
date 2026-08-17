//! `HnswIndex` — the in-process semantic read path (§11.2, EXP-008).
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
#[derive(Debug, Deserialize)]
struct GenerationSidecar {
    #[serde(rename = "generatedAt")]
    generated_at: String,
    #[serde(rename = "classCount")]
    class_count: usize,
    #[serde(default)]
    #[allow(dead_code)] // Retained for provenance/round-trip; not needed to build `Generation`.
    source: Option<String>,
}

/// `VectorIndex` over an in-process ruvector-core HNSW.
///
/// Construction never fails: an absent or unreadable artifact yields a
/// not-ready index (`db == None`) that fails open on every query.
pub struct HnswIndex {
    /// `None` ⇒ the artifact was absent, empty, or failed to open (fail-open).
    db: Option<Arc<VectorDB>>,
    artifact_path: PathBuf,
    /// Parsed once at open from the `<artifact>.generation.json` sidecar.
    generation: Generation,
}

impl HnswIndex {
    /// Open the artifact at `LOOM_HNSW_ARTIFACT` (or [`DEFAULT_ARTIFACT_PATH`]).
    #[must_use]
    pub fn from_env() -> Self {
        let path = std::env::var("LOOM_HNSW_ARTIFACT")
            .unwrap_or_else(|_| DEFAULT_ARTIFACT_PATH.to_owned());
        Self::open(path)
    }

    /// Open the ruvector-core storage DB at `artifact_path`.
    ///
    /// Fail-open by contract: a missing artifact, an empty DB, or an open error
    /// all yield a not-ready index rather than a panic or an error return. A
    /// missing artifact is NEVER created here — the exporter owns writes.
    #[must_use]
    pub fn open(artifact_path: impl Into<PathBuf>) -> Self {
        let artifact_path = artifact_path.into();
        let generation = load_generation_sidecar(&artifact_path);

        if !artifact_path.exists() {
            tracing::warn!(
                path = %artifact_path.display(),
                "HNSW artifact absent; semantic fallback disabled (fail-open)"
            );
            return Self { db: None, artifact_path, generation };
        }

        match Self::try_open(&artifact_path) {
            Ok(Some(db)) => Self { db: Some(Arc::new(db)), artifact_path, generation },
            Ok(None) => {
                tracing::warn!(
                    path = %artifact_path.display(),
                    "HNSW artifact present but empty; semantic fallback disabled (fail-open)"
                );
                Self { db: None, artifact_path, generation }
            }
            Err(e) => {
                tracing::warn!(
                    path = %artifact_path.display(),
                    error = %e,
                    "failed to open HNSW artifact; semantic fallback disabled (fail-open)"
                );
                Self { db: None, artifact_path, generation }
            }
        }
    }

    /// Try to open the DB. `Ok(None)` means the DB opened but holds no vectors.
    fn try_open(path: &Path) -> Result<Option<VectorDB>, ruvector_core::RuvectorError> {
        // On an existing DB the stored config wins; these values only seed the
        // (equivalent) new-DB case and document intent: cosine, 384-dim, HNSW.
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
            db.search(SearchQuery { vector: query, k, filter: None, ef_search: None })
        })
        .await
        .map_err(|e| LoomError::SemanticUnready(format!("hnsw search task failed: {e}")))?
        .map_err(|e| LoomError::SemanticUnready(format!("hnsw search failed: {e}")))?;

        // ruvector cosine `score` is a DISTANCE (1 - cos_sim), sorted best-first.
        // Map to a similarity in [0,1] — the scale the injection gate reads.
        Ok(results
            .into_iter()
            .map(|r| ConceptMatch {
                iri: Iri::new(r.id),
                score: (1.0_f32 - r.score).clamp(0.0, 1.0),
                provenance: MatchProvenance::SemanticHnsw,
            })
            .collect())
    }

    fn is_ready(&self) -> bool {
        self.db.is_some()
    }

    fn generation(&self) -> Generation {
        self.generation.clone()
    }
}

/// Compute the sidecar path `<artifact>.generation.json`.
fn sidecar_path(artifact: &Path) -> PathBuf {
    let mut raw = artifact.as_os_str().to_owned();
    raw.push(".generation.json");
    PathBuf::from(raw)
}

/// Load and map the generation sidecar. A missing/unparseable sidecar yields an
/// `Unavailable` generation (so the §6 parity guard treats it as non-matching).
fn load_generation_sidecar(artifact: &Path) -> Generation {
    let path = sidecar_path(artifact);
    let parsed = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<GenerationSidecar>(&s).ok());

    match parsed {
        Some(g) => Generation {
            id: GenerationId(g.generated_at.clone()),
            source: GenerationSource::MirrorManifest,
            generated_at: Some(g.generated_at),
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
    use ruvector_core::types::{QuantizationConfig, VectorEntry};
    use tempfile::tempdir;

    /// A 384-dim unit-ish vector: `1.0` at `axis`, small fill elsewhere.
    fn synth_vec(axis: usize) -> Vec<f32> {
        let mut v = vec![0.01_f32; EMBEDDING_DIMENSIONS];
        v[axis % EMBEDDING_DIMENSIONS] = 1.0;
        v
    }

    /// Build a real ruvector-core DB at `path` with three synthetic vectors.
    fn seed_db(path: &Path) {
        let opts = DbOptions {
            dimensions: EMBEDDING_DIMENSIONS,
            distance_metric: DistanceMetric::Cosine,
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
        db.insert_batch(vec![
            VectorEntry {
                id: Some("urn:ngm:class:alpha".to_owned()),
                vector: synth_vec(0),
                metadata: None,
            },
            VectorEntry {
                id: Some("urn:ngm:class:beta".to_owned()),
                vector: synth_vec(1),
                metadata: None,
            },
            VectorEntry {
                id: Some("urn:ngm:class:gamma".to_owned()),
                vector: synth_vec(2),
                metadata: None,
            },
        ])
        .expect("insert seed batch");
        assert_eq!(db.len().unwrap(), 3);
    }

    #[tokio::test]
    async fn absent_artifact_is_not_ready_and_fails_open() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist.rvdb");
        let idx = HnswIndex::open(&missing);

        assert!(!idx.is_ready(), "absent artifact must not be ready");
        // And no stray DB file was created by opening.
        assert!(!missing.exists(), "open() must not create the artifact");

        let err = idx.nearest(&synth_vec(0), 5).await.unwrap_err();
        assert!(
            matches!(err, LoomError::SemanticUnready(_)),
            "absent artifact must fail open with SemanticUnready, got {err:?}"
        );
    }

    #[tokio::test]
    async fn roundtrip_insert_search_cosine_ordering_and_iri() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("roundtrip.rvdb");
        seed_db(&path);

        let idx = HnswIndex::open(&path);
        assert!(idx.is_ready(), "seeded artifact must be ready");

        // Query aligned with `alpha` (axis 0): it must rank first, as an Iri.
        let hits = idx.nearest(&synth_vec(0), 3).await.unwrap();
        assert!(!hits.is_empty(), "expected hits from a seeded index");
        assert_eq!(
            hits[0].iri,
            Iri::new("urn:ngm:class:alpha"),
            "nearest to axis-0 query must be alpha; id round-trips as Iri"
        );
        assert_eq!(hits[0].iri.slug(), "alpha");
        assert_eq!(hits[0].provenance, MatchProvenance::SemanticHnsw);

        // Cosine similarity is in [0,1] and the top hit dominates the rest.
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

    #[tokio::test]
    async fn off_width_query_fails_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("width.rvdb");
        seed_db(&path);
        let idx = HnswIndex::open(&path);

        let err = idx.nearest(&[0.1_f32, 0.2, 0.3], 5).await.unwrap_err();
        assert!(matches!(err, LoomError::SemanticUnready(_)));
    }

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
}
