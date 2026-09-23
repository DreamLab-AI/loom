//! Building the serving HNSW artefact, and promoting a `vault build` bundle.
//!
//! Loom serves semantic search from a `ruvector-core` database file,
//! `ontology-corpus.rvdb`, beside a `<artifact>.generation.json` sidecar. Two
//! producers build that artefact and both go through [`write_serving_artifact`],
//! so the index configuration can never drift between them:
//!
//! - `export_corpus` (feature `pg-write`) reads vectors from `ruvector-postgres`;
//! - [`promote_vault_build`] reads the portable records a `vault build
//!   --with-rvdb` writes as `ontology-corpus.records.jsonl` (contract C3).
//!
//! `vault` does not write the database itself: the records file is the
//! *portable* form (vectors inline, no database format), and turning it into the
//! serving artefact is Loom's promotion step. Promotion verifies the records file
//! against the bundle's own `.generation.json`, requires every record to carry the
//! bundle's generation and a 384-dimension vector, builds the database with the
//! index law's settings, and records the two derived files in the marker so the
//! reload transition (`scripts/reload-published-generation.py`) can verify them.
//!
//! **Run promotion as its own process.** `ruvector-core` keeps every database it
//! opens in a process-wide pool for the life of the process, and the file lock
//! follows the file through a rename. A server that promoted in-process could not
//! then open the artefact it had just built. The `promote_vault_build` binary is
//! the supported entry point; its integration test proves the reader qualifies
//! the result from a separate process, which is the production shape.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use ruvector_core::types::{
    DbOptions, DistanceMetric, HnswConfig, QuantizationConfig, VectorEntry,
};
use ruvector_core::VectorDB;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::EMBEDDING_DIMENSIONS;

/// The only embedding model the serving artefact may be built from.
pub const EMBEDDING_MODEL: &str = "bge-small-en-v1.5";
/// The portable records file a `vault build --with-rvdb` writes (contract C3).
pub const RECORDS_FILE: &str = "ontology-corpus.records.jsonl";
/// The serving artefact Loom opens.
pub const ARTIFACT_FILE: &str = "ontology-corpus.rvdb";
/// The serving artefact's sidecar.
pub const SIDECAR_FILE: &str = "ontology-corpus.rvdb.generation.json";
/// Present while a bundle is being changed; the reload transition refuses to act.
pub const IN_FLIGHT: &str = ".promotion-in-flight";

const INSERT_BATCH: usize = 1000;

/// Build a fresh serving artefact at `out` from `entries` and write its sidecar.
///
/// The database is created with the index law's settings (cosine, HNSW `m=16`,
/// `ef_construction=128`, full precision) and its stored count is checked
/// against what was inserted. The sidecar declares `generation` as
/// `generatedAt`, so the served semantic generation is the bundle's generation.
///
/// # Errors
///
/// Returns an error if an entry is not 384-dimensional, if the database cannot
/// be created or written, if the stored count disagrees, or if the sidecar
/// cannot be written.
pub fn write_serving_artifact(
    out: &Path,
    entries: &[VectorEntry],
    generation: &str,
    source: &str,
) -> Result<usize, String> {
    if let Some(bad) = entries
        .iter()
        .find(|e| e.vector.len() != EMBEDDING_DIMENSIONS)
    {
        return Err(format!(
            "vector for {:?} has {} dimensions, not {EMBEDDING_DIMENSIONS}",
            bad.id,
            bad.vector.len()
        ));
    }
    let sidecar = sidecar_path(out);
    for path in [out, sidecar.as_path()] {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| format!("remove {}: {e}", path.display()))?;
        }
    }

    let opts = DbOptions {
        dimensions: EMBEDDING_DIMENSIONS,
        distance_metric: DistanceMetric::Cosine,
        storage_path: out.to_string_lossy().into_owned(),
        hnsw_config: Some(HnswConfig {
            m: 16,
            ef_construction: 128,
            ef_search: 100,
            max_elements: entries.len() + 1024,
        }),
        quantization: Some(QuantizationConfig::None),
    };
    let db = VectorDB::new(opts).map_err(|e| format!("create VectorDB failed: {e}"))?;
    let mut inserted = 0_usize;
    for chunk in entries.chunks(INSERT_BATCH) {
        let ids = db
            .insert_batch(chunk.to_vec())
            .map_err(|e| format!("insert_batch failed at {inserted}/{}: {e}", entries.len()))?;
        inserted += ids.len();
    }
    let stored = db.len().map_err(|e| format!("len() failed: {e}"))?;
    if stored != inserted {
        return Err(format!(
            "count mismatch: stored {stored} != inserted {inserted}"
        ));
    }
    drop(db);

    let sidecar_json = json!({
        "generatedAt": generation,
        "classCount": inserted,
        "source": source,
        "embeddingModel": EMBEDDING_MODEL,
        "dimensions": EMBEDDING_DIMENSIONS,
    });
    let text = serde_json::to_string_pretty(&sidecar_json).map_err(|e| e.to_string())?;
    std::fs::write(&sidecar, text).map_err(|e| format!("write {}: {e}", sidecar.display()))?;
    Ok(inserted)
}

/// The sidecar path for an artefact: `<artifact>.generation.json`.
#[must_use]
pub fn sidecar_path(artifact: &Path) -> PathBuf {
    let mut name = artifact.as_os_str().to_owned();
    name.push(".generation.json");
    PathBuf::from(name)
}

/// What a promotion did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteReport {
    /// The bundle's generation id.
    pub generation: String,
    /// Records turned into vectors.
    pub records: usize,
    /// The serving artefact's SHA-256.
    pub artifact_sha256: String,
}

/// Turn a `vault build` bundle's portable records into Loom's serving artefact.
///
/// Reads `<data>/.generation.json`, verifies [`RECORDS_FILE`] against the digest
/// and size the marker lists, builds [`ARTIFACT_FILE`] and [`SIDECAR_FILE`], and
/// rewrites the marker with those two entries added (or replaced on a re-run)
/// and a `derived` note naming their source. The marker's `content_digest` is
/// the source-corpus digest (contract C3) and is left as it is. While the
/// bundle changes, [`IN_FLIGHT`] is present, so the reload transition refuses
/// to act on a half-promoted bundle.
///
/// # Errors
///
/// Refuses, leaving the marker unchanged, if the marker is not a `vault build`
/// marker, if the records file is not listed or does not match its listed
/// digest, or if any record names a different generation or has the wrong
/// vector width.
pub fn promote_vault_build(data: &Path) -> Result<PromoteReport, String> {
    let marker_path = data.join(".generation.json");
    let raw = std::fs::read(&marker_path).map_err(|e| format!("read marker: {e}"))?;
    let mut marker: Value =
        serde_json::from_slice(&raw).map_err(|e| format!("parse marker: {e}"))?;
    let generation = marker
        .get("id")
        .and_then(Value::as_str)
        .ok_or("marker has no `id`: not a vault build marker")?
        .to_owned();
    let listed = marker
        .get("artifacts")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|x| x.get("name").and_then(Value::as_str) == Some(RECORDS_FILE))
        })
        .cloned()
        .ok_or_else(|| {
            format!("marker does not list {RECORDS_FILE}; build with `vault build --with-rvdb`")
        })?;

    let records_path = data.join(RECORDS_FILE);
    let (sha, bytes) = file_digest(&records_path)?;
    if listed.get("sha256").and_then(Value::as_str) != Some(sha.as_str())
        || listed.get("bytes").and_then(Value::as_u64) != Some(bytes)
    {
        return Err(format!(
            "{RECORDS_FILE} does not match the digest the marker lists"
        ));
    }
    let entries = read_records(&records_path, &generation)?;

    let in_flight = data.join(IN_FLIGHT);
    std::fs::write(&in_flight, b"").map_err(|e| format!("mark in flight: {e}"))?;
    let outcome = build_and_record(data, &marker_path, &mut marker, &entries, &generation);
    std::fs::remove_file(&in_flight).map_err(|e| format!("clear in-flight marker: {e}"))?;
    outcome
}

fn build_and_record(
    data: &Path,
    marker_path: &Path,
    marker: &mut Value,
    entries: &[VectorEntry],
    generation: &str,
) -> Result<PromoteReport, String> {
    // Build under a name no earlier call in this process has opened
    // (ruvector-core keeps a process-wide registry of databases by path), then
    // rename both files into place, so the swap is atomic and repeatable.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let staging = data.join(format!(
        ".{ARTIFACT_FILE}.{}-{nonce}.tmp",
        std::process::id()
    ));
    let records = write_serving_artifact(&staging, entries, generation, "vault-portable-records")?;
    std::fs::rename(&staging, data.join(ARTIFACT_FILE))
        .map_err(|e| format!("place {ARTIFACT_FILE}: {e}"))?;
    std::fs::rename(sidecar_path(&staging), data.join(SIDECAR_FILE))
        .map_err(|e| format!("place {SIDECAR_FILE}: {e}"))?;

    let mut added = Vec::new();
    for name in [ARTIFACT_FILE, SIDECAR_FILE] {
        let (sha, bytes) = file_digest(&data.join(name))?;
        added.push(json!({ "name": name, "sha256": sha, "bytes": bytes }));
    }
    let artifact_sha256 = added[0]["sha256"].as_str().unwrap_or_default().to_owned();

    let list = marker
        .get_mut("artifacts")
        .and_then(Value::as_array_mut)
        .ok_or("marker artifacts vanished")?;
    list.retain(|a| {
        !matches!(
            a.get("name").and_then(Value::as_str),
            Some(ARTIFACT_FILE | SIDECAR_FILE)
        )
    });
    list.extend(added);
    list.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    let mut derived = BTreeMap::new();
    for name in [ARTIFACT_FILE, SIDECAR_FILE] {
        derived.insert(
            name,
            json!({ "from": RECORDS_FILE, "by": "loom promote_vault_build" }),
        );
    }
    marker["derived"] = json!(derived);

    let tmp = marker_path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(marker).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| format!("write marker: {e}"))?;
    std::fs::rename(&tmp, marker_path).map_err(|e| format!("replace marker: {e}"))?;
    Ok(PromoteReport {
        generation: generation.to_owned(),
        records,
        artifact_sha256,
    })
}

fn read_records(path: &Path, generation: &str) -> Result<Vec<VectorEntry>, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut entries = Vec::new();
    for (n, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| format!("read line {}: {e}", n + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: Value =
            serde_json::from_str(&line).map_err(|e| format!("record {}: {e}", n + 1))?;
        let key = record
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("record {} has no key", n + 1))?;
        let record_generation = record
            .pointer("/metadata/generation")
            .and_then(Value::as_str);
        if record_generation != Some(generation) {
            return Err(format!(
                "record {key} names generation {record_generation:?}, not {generation}"
            ));
        }
        let vector: Vec<f32> = record
            .get("embedding")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("record {key} has no embedding"))?
            .iter()
            .map(|v| {
                v.as_f64()
                    // JSON numbers parse as f64; the embedder produced f32, so
                    // narrowing back is exact for every value it can emit.
                    .map(
                        #[allow(clippy::cast_possible_truncation)]
                        |f| f as f32,
                    )
                    .ok_or_else(|| format!("record {key}: non-numeric"))
            })
            .collect::<Result<_, _>>()?;
        if vector.len() != EMBEDDING_DIMENSIONS {
            return Err(format!(
                "record {key} has {} dimensions, not {EMBEDDING_DIMENSIONS}",
                vector.len()
            ));
        }
        entries.push(VectorEntry {
            id: Some(key.to_owned()),
            vector,
            metadata: None,
        });
    }
    if entries.is_empty() {
        return Err(format!("{} holds no records", path.display()));
    }
    Ok(entries)
}

fn file_digest(path: &Path) -> Result<(String, u64), String> {
    let mut file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 1 << 16];
    let mut bytes = 0_u64;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    let digest = hasher.finalize();
    let hex = digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    });
    Ok((hex, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const GEN: &str = "visionGraph@abc1234";

    fn unit(seed: usize) -> Vec<f64> {
        let mut v = vec![0.0_f64; EMBEDDING_DIMENSIONS];
        v[seed % EMBEDDING_DIMENSIONS] = 1.0;
        v[(seed * 7 + 3) % EMBEDDING_DIMENSIONS] = 0.5;
        v
    }

    /// A vault-build bundle with `n` records and a marker that lists them.
    fn bundle(dir: &Path, n: usize, record_generation: &str) {
        let mut text = String::new();
        for i in 0..n {
            let rec = json!({
                "key": format!("urn:ngm:class:c{i}"),
                "metadata": { "generation": record_generation },
                "embedding": unit(i),
            });
            text.push_str(&rec.to_string());
            text.push('\n');
        }
        std::fs::write(dir.join(RECORDS_FILE), &text).unwrap();
        let (sha, bytes) = file_digest(&dir.join(RECORDS_FILE)).unwrap();
        let marker = json!({
            "id": GEN, "commit": "abc1234", "content_digest": "c".repeat(64),
            "class_count": n, "page_count": n, "vocabulary_version": 1,
            "artifacts": [ { "name": RECORDS_FILE, "sha256": sha, "bytes": bytes } ],
        });
        std::fs::write(dir.join(".generation.json"), marker.to_string()).unwrap();
    }

    fn marker(dir: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(dir.join(".generation.json")).unwrap()).unwrap()
    }

    #[test]
    fn sidecar_path_suffix() {
        assert_eq!(
            sidecar_path(Path::new("data/ontology-corpus.rvdb")),
            PathBuf::from("data/ontology-corpus.rvdb.generation.json")
        );
    }

    #[test]
    fn promotion_lists_both_derived_files_with_their_real_hashes() {
        let dir = tempdir().unwrap();
        bundle(dir.path(), 4, GEN);
        promote_vault_build(dir.path()).unwrap();
        let m = marker(dir.path());
        for name in [ARTIFACT_FILE, SIDECAR_FILE] {
            let entry = m["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["name"] == name)
                .unwrap();
            let (sha, bytes) = file_digest(&dir.path().join(name)).unwrap();
            assert_eq!(entry["sha256"], sha);
            assert_eq!(entry["bytes"], bytes);
        }
        assert_eq!(
            m["content_digest"],
            "c".repeat(64),
            "the corpus digest is left as it is"
        );
        assert!(!dir.path().join(IN_FLIGHT).exists());
    }

    #[test]
    fn re_promotion_replaces_rather_than_duplicates_entries() {
        let dir = tempdir().unwrap();
        bundle(dir.path(), 4, GEN);
        promote_vault_build(dir.path()).unwrap();
        promote_vault_build(dir.path()).unwrap();
        let names: Vec<_> = marker(dir.path())["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            names.iter().filter(|n| n.as_str() == ARTIFACT_FILE).count(),
            1
        );
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn a_tampered_records_file_is_refused_and_nothing_changes() {
        let dir = tempdir().unwrap();
        bundle(dir.path(), 4, GEN);
        let before = std::fs::read(dir.path().join(".generation.json")).unwrap();
        std::fs::write(dir.path().join(RECORDS_FILE), "{}\n").unwrap();
        assert!(promote_vault_build(dir.path()).is_err());
        assert_eq!(
            std::fs::read(dir.path().join(".generation.json")).unwrap(),
            before
        );
        assert!(!dir.path().join(ARTIFACT_FILE).exists());
    }

    #[test]
    fn a_record_from_another_generation_is_refused() {
        let dir = tempdir().unwrap();
        bundle(dir.path(), 4, "visionGraph@other");
        let err = promote_vault_build(dir.path()).unwrap_err();
        assert!(err.contains("names generation"), "{err}");
        assert!(!dir.path().join(ARTIFACT_FILE).exists());
    }

    #[test]
    fn a_bundle_without_records_is_refused_with_the_build_flag_named() {
        let dir = tempdir().unwrap();
        bundle(dir.path(), 2, GEN);
        let mut m = marker(dir.path());
        m["artifacts"] = json!([]);
        std::fs::write(dir.path().join(".generation.json"), m.to_string()).unwrap();
        let err = promote_vault_build(dir.path()).unwrap_err();
        assert!(err.contains("--with-rvdb"), "{err}");
    }
}
