//! `MirrorStore` — the `GenerationStore` read side (§11.6). Ports the Python
//! façade's `_generation()` best-source-first resolution and adds
//! `verify_atomicity` (re-hash every recorded artifact sha, incl. the `.rvdb`
//! sidecar), the atomicity check the mirror's `.generation.json` commit marker
//! promises (ADR-136 D4).
//!
//! SCOPE (ADR-135 closeout): this store is the **disk view** — what the data
//! directory says RIGHT NOW. It is deliberately not the serving identity any
//! more. `LoadedBundle` (see `bundle.rs`) captures the generation of the content
//! a process actually loaded, and that is what `/health`, `/loom/generation` and
//! every grounding object report. `MirrorStore` remains so the façade can show
//! the disk view BESIDE the loaded one, which is precisely how a promotion that
//! has not been activated becomes visible instead of being mistaken for a
//! serving change.
//!
//! Best source first, exactly as Python:
//!   1. `build-manifest.json` (commitSha/buildId — WS-A, when upstream ships it);
//!   2. `.generation.json` (the atomic commit marker — proof the served set is
//!      ONE verified generation, never mixed-build);
//!   3. the scaffold index's own `generated` stamp (barest pre-manifest fallback).
//!
//! # Two commit-marker shapes, one reader (ADR-141)
//!
//! `.generation.json` exists in two shapes and this reader accepts both:
//!
//! | Shape | Identity | Artefacts | Emitted by |
//! |---|---|---|---|
//! | **vault build** (C3) | `id: "visionGraph@<sha>"`, `commit`, `content_digest`, `class_count`, `page_count`, `vocabulary_version`, `stale_after` | `artifacts: [ {name, sha256, bytes} ]` | `vault build` |
//! | **mirror** (legacy) | `generation: "<ISO stamp>"`, `promoted_at`, `cluster_span_seconds` | `artifacts: { name: {sha256, bytes} }` | `app/mirror.sh` |
//!
//! This is a READER-side superset, not a served compatibility shim: the
//! grounding contract and `/loom/generation` have one shape whichever marker
//! was read. The live HP node still carries a mirror marker until the first
//! `vault build` promotion, and a reader that refused it would take the node
//! down to gain nothing.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use loom_domain::{
    ArtifactSha, Generation, GenerationId, GenerationSource, GenerationStore, LoomError,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Reads the generation identity from a mirror data directory. Cheap to clone
/// (just the dir path); every read hits the filesystem so a promote is picked up
/// without a restart.
#[derive(Debug, Clone)]
pub struct MirrorStore {
    data_dir: PathBuf,
    /// The scaffold index filename inside `data_dir` (the pre-manifest fallback
    /// source). Kept explicit so a non-default `ONTOLOGY_INDEX` still resolves.
    index_file: String,
}

impl MirrorStore {
    /// Build from the scaffold-index path (the façade's `ONTOLOGY_INDEX`); the
    /// data directory is its parent, matching Python's `dirname(INDEX)`.
    #[must_use]
    pub fn new(index_path: &str) -> Self {
        let p = Path::new(index_path);
        let data_dir = p
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let index_file = p.file_name().map_or_else(
            || "scaffold-index.json".to_owned(),
            |f| f.to_string_lossy().into_owned(),
        );
        Self {
            data_dir,
            index_file,
        }
    }

    /// The data directory this store reads (the bundle module resolves the
    /// same directory, so it is shared rather than recomputed).
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The scaffold index filename inside [`Self::data_dir`].
    #[must_use]
    pub fn index_file(&self) -> &str {
        &self.index_file
    }

    /// The `.generation.json` commit marker's artefact digests, as recorded.
    /// `None` when there is no readable marker.
    #[must_use]
    pub fn marker_artifacts(&self) -> Option<Vec<ArtifactSha>> {
        let raw = std::fs::read_to_string(self.generation_manifest_path()).ok()?;
        let m: Value = serde_json::from_str(&raw).ok()?;
        Some(parse_artifacts(&m))
    }

    /// Whether a readable `.generation.json` commit marker exists.
    #[must_use]
    pub fn has_commit_marker(&self) -> bool {
        std::fs::read_to_string(self.generation_manifest_path())
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .is_some()
    }

    /// The commit-marker path (`<data_dir>/.generation.json`).
    #[must_use]
    pub fn commit_marker_path(&self) -> PathBuf {
        self.generation_manifest_path()
    }

    fn build_manifest(&self) -> Option<Generation> {
        let raw = std::fs::read_to_string(self.data_dir.join("build-manifest.json")).ok()?;
        let m: Value = serde_json::from_str(&raw).ok()?;
        let commit_sha = str_field(&m, "commitSha");
        let build_id = str_field(&m, "buildId");
        // Identity prefers commitSha, then buildId (Python keys both).
        let id = commit_sha
            .clone()
            .or_else(|| build_id.clone())
            .unwrap_or_else(|| "build-manifest".to_owned());
        Some(Generation {
            id: GenerationId(id),
            source: GenerationSource::BuildManifest,
            generated_at: str_field(&m, "generatedAt"),
            commit_sha,
            promoted_at: None,
            cluster_span_seconds: None,
            artifacts: Vec::new(),
            verified_single_generation: true,
            class_count: None,
            content_digest: None,
            page_count: None,
            vocabulary_version: None,
            stale_after: None,
        })
    }

    /// The `.generation.json` commit marker in either shape — the `vault build`
    /// generation (C3) or the legacy mirror marker. See the module docs.
    fn generation_marker(&self) -> Option<Generation> {
        let raw = std::fs::read_to_string(self.generation_manifest_path()).ok()?;
        let m: Value = serde_json::from_str(&raw).ok()?;
        if let Some(id) = str_field(&m, "id") {
            return Some(vault_build_generation(&m, id));
        }
        let generated_at = str_field(&m, "generation");
        let id = generated_at.clone().unwrap_or_else(|| "mirror".to_owned());
        Some(Generation {
            id: GenerationId(id),
            source: GenerationSource::MirrorManifest,
            generated_at,
            commit_sha: None,
            promoted_at: str_field(&m, "promoted_at"),
            cluster_span_seconds: m.get("cluster_span_seconds").and_then(Value::as_f64),
            artifacts: parse_artifacts(&m),
            verified_single_generation: true,
            class_count: None,
            content_digest: None,
            page_count: None,
            vocabulary_version: None,
            stale_after: None,
        })
    }

    fn scaffold_index_stamp(&self) -> Generation {
        let path = self.data_dir.join(&self.index_file);
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        {
            Some(d) => {
                let generated = str_field(&d, "generated");
                let class_count = d
                    .get("counts")
                    .and_then(|c| c.get("classes"))
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok());
                Generation {
                    id: GenerationId(
                        generated
                            .clone()
                            .unwrap_or_else(|| "scaffold-index".to_owned()),
                    ),
                    source: GenerationSource::ScaffoldIndex,
                    generated_at: generated,
                    commit_sha: None,
                    promoted_at: None,
                    cluster_span_seconds: None,
                    artifacts: Vec::new(),
                    verified_single_generation: false,
                    class_count,
                    content_digest: None,
                    page_count: None,
                    vocabulary_version: None,
                    stale_after: None,
                }
            }
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
                content_digest: None,
                page_count: None,
                vocabulary_version: None,
                stale_after: None,
            },
        }
    }

    fn generation_manifest_path(&self) -> PathBuf {
        self.data_dir.join(".generation.json")
    }
}

#[async_trait]
impl GenerationStore for MirrorStore {
    fn current(&self) -> Generation {
        self.build_manifest()
            .or_else(|| self.generation_marker())
            .unwrap_or_else(|| self.scaffold_index_stamp())
    }

    async fn verify_atomicity(&self) -> Result<(), LoomError> {
        // Only the commit marker carries per-artifact shas to re-verify. With
        // no marker there is nothing promoted to check — fail-open (Ok).
        let Some(gen) = self.generation_marker() else {
            return Ok(());
        };
        if gen.artifacts.is_empty() {
            return Ok(());
        }
        for art in &gen.artifacts {
            let path = self.data_dir.join(&art.name);
            let bytes = std::fs::read(&path)
                .map_err(|e| LoomError::GenerationDrift(format!("{}: {e}", path.display())))?;
            let got = hex_sha256(&bytes);
            if got != art.sha256 {
                return Err(LoomError::GenerationDrift(format!(
                    "{} sha256 mismatch: recorded {}, recomputed {got}",
                    art.name, art.sha256
                )));
            }
        }
        Ok(())
    }
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(std::borrow::ToOwned::to_owned)
}

/// The `vault build` marker (C3) as a [`Generation`]. `id` is
/// `visionGraph@<sha>` and is the identity every surface reports; `commit` is
/// the same sha unqualified, kept separately because `corpusSha` admission
/// compares against the bare sha.
fn vault_build_generation(m: &Value, id: String) -> Generation {
    Generation {
        id: GenerationId(id),
        source: GenerationSource::VaultBuild,
        generated_at: str_field(m, "generated_at"),
        commit_sha: str_field(m, "commit"),
        promoted_at: str_field(m, "promoted_at"),
        cluster_span_seconds: None,
        artifacts: parse_artifacts(m),
        verified_single_generation: true,
        class_count: usize_field(m, "class_count"),
        content_digest: str_field(m, "content_digest"),
        page_count: usize_field(m, "page_count"),
        vocabulary_version: m
            .get("vocabulary_version")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        stale_after: str_field(m, "stale_after"),
    }
}

fn usize_field(v: &Value, key: &str) -> Option<usize> {
    v.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
}

/// Parse the `.generation.json` `artifacts` collection in EITHER shape: the
/// vault build's list `[ {name, sha256, bytes} ]` or the mirror's map
/// `{ name: {sha256, bytes} }`. Both record the `.rvdb` sidecar alongside the
/// JSON indices, so re-hashing the result covers the HNSW artifact (§11.6).
fn parse_artifacts(m: &Value) -> Vec<ArtifactSha> {
    match m.get("artifacts") {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|meta| {
                Some(ArtifactSha {
                    name: meta.get("name").and_then(Value::as_str)?.to_owned(),
                    sha256: meta.get("sha256").and_then(Value::as_str)?.to_owned(),
                    bytes: meta.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                })
            })
            .collect(),
        Some(Value::Object(obj)) => obj
            .iter()
            .filter_map(|(name, meta)| {
                let sha256 = meta.get("sha256").and_then(Value::as_str)?.to_owned();
                let bytes = meta.get("bytes").and_then(Value::as_u64).unwrap_or(0);
                Some(ArtifactSha {
                    name: name.clone(),
                    sha256,
                    bytes,
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Lower-case hex SHA-256 of `bytes`. Shared with the bundle activator so the
/// loaded-content digest and the marker verification cannot drift apart.
#[must_use]
pub fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}
