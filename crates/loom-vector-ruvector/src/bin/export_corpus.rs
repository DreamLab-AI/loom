//! `export_corpus` — off-turn artifact bootstrap (pg-write channel, §11.2).
//!
//! Reads the already-verified 384-dim embeddings from the `ontology-corpus`
//! namespace in ruvector-postgres (NO re-embedding) and bulk-inserts them into a
//! fresh `ruvector_core::VectorDB` keyed by IRI, then writes the
//! `<artifact>.generation.json` sidecar. Read-only SQL — never writes to PG.
//!
//! Usage:
//!   `export_corpus` --out data/ontology-corpus.rvdb [--conninfo <pg conninfo>]
//!                 [--namespace ontology-corpus]
//!
//! Compiled only under `--features pg-write` (see `[[bin]] required-features`),
//! so the serving binary never links Postgres.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ruvector_core::types::{
    DbOptions, DistanceMetric, HnswConfig, QuantizationConfig, VectorEntry,
};
use ruvector_core::VectorDB;

const EMBEDDING_DIMENSIONS: usize = 384;
const DEFAULT_NAMESPACE: &str = "ontology-corpus";
const DEFAULT_CONNINFO: &str =
    "host=ruvector-postgres port=5432 dbname=ruvector user=ruvector password=ruvector";
const INSERT_BATCH: usize = 1000;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

struct Args {
    out: PathBuf,
    conninfo: String,
    namespace: String,
}

fn parse_args() -> Result<Args, BoxErr> {
    let mut out: Option<PathBuf> = None;
    let mut conninfo: Option<String> = None;
    let mut namespace: Option<String> = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f.to_owned(), Some(v.to_owned())),
            None => (arg.clone(), None),
        };
        let value = |it: &mut dyn Iterator<Item = String>| -> Result<String, BoxErr> {
            if let Some(v) = inline.clone() {
                Ok(v)
            } else {
                it.next()
                    .ok_or_else(|| format!("missing value for {flag}").into())
            }
        };
        match flag.as_str() {
            "--out" | "-o" => out = Some(PathBuf::from(value(&mut it)?)),
            "--conninfo" | "-c" => conninfo = Some(value(&mut it)?),
            "--namespace" | "-n" => namespace = Some(value(&mut it)?),
            "--help" | "-h" => {
                println!(
                    "export_corpus --out <path> [--conninfo <pg>] [--namespace {DEFAULT_NAMESPACE}]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    Ok(Args {
        out: out.ok_or("--out <path> is required")?,
        conninfo: conninfo
            .or_else(|| std::env::var("RUVECTOR_PG_CONNINFO").ok())
            .unwrap_or_else(|| DEFAULT_CONNINFO.to_owned()),
        namespace: namespace.unwrap_or_else(|| DEFAULT_NAMESPACE.to_owned()),
    })
}

/// Parse an `embedding::text` cell — tolerates `[..]`, `{..}`, `(..)` wrappers.
fn parse_embedding(raw: &str) -> Option<Vec<f32>> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .or_else(|| trimmed.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
        .or_else(|| trimmed.strip_prefix('(').and_then(|s| s.strip_suffix(')')))
        .unwrap_or(trimmed);

    if inner.trim().is_empty() {
        return None;
    }
    inner
        .split(',')
        .map(|tok| tok.trim().parse::<f32>().ok())
        .collect()
}

/// RFC3339 UTC stamp for `secs` since the Unix epoch (dependency-free).
/// Howard Hinnant's `civil_from_days`.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn rfc3339_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + i64::from(m <= 2);

    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Outcome of the read/parse stage.
struct FetchStats {
    fetched: usize,
    bad_parse: usize,
    bad_dim: usize,
}

/// Probe the embedding column, then read + parse every non-NULL row. Read-only.
async fn fetch_vectors(
    client: &tokio_postgres::Client,
    namespace: &str,
) -> Result<(Vec<VectorEntry>, FetchStats), BoxErr> {
    // Probe the column type/format (empirical verification of the AM output).
    let Some(row) = client
        .query_opt(
            "SELECT pg_typeof(embedding)::text AS ty, left(embedding::text, 72) AS sample \
             FROM memory_entries \
             WHERE namespace = $1 AND embedding IS NOT NULL LIMIT 1",
            &[&namespace],
        )
        .await?
    else {
        return Err(format!("no non-NULL embeddings in namespace '{namespace}'").into());
    };
    let ty: String = row.get("ty");
    let sample: String = row.get("sample");
    eprintln!("probe: embedding column type = {ty}; sample = {sample}…");

    // Read all rows; cast to text for robust, type-agnostic parsing.
    let rows = client
        .query(
            "SELECT key, embedding::text AS emb \
             FROM memory_entries \
             WHERE namespace = $1 AND embedding IS NOT NULL",
            &[&namespace],
        )
        .await?;
    eprintln!("fetched {} candidate rows", rows.len());

    let mut entries = Vec::with_capacity(rows.len());
    let mut bad_parse = 0_usize;
    let mut bad_dim = 0_usize;
    for row in &rows {
        let key: String = row.get("key");
        let emb: String = row.get("emb");
        let Some(vector) = parse_embedding(&emb) else {
            bad_parse += 1;
            continue;
        };
        if vector.len() != EMBEDDING_DIMENSIONS {
            bad_dim += 1;
            continue;
        }
        entries.push(VectorEntry {
            id: Some(key),
            vector,
            metadata: None,
        });
    }
    eprintln!("parsed {} valid 384-dim vectors", entries.len());
    Ok((
        entries,
        FetchStats {
            fetched: rows.len(),
            bad_parse,
            bad_dim,
        },
    ))
}

/// Build a fresh `VectorDB` at `out` from `entries` and write the sidecar.
/// Returns (`inserted`, `sidecar_path`, `generated_at`).
fn build_artifact(out: &Path, entries: &[VectorEntry]) -> Result<(usize, PathBuf, String), BoxErr> {
    if out.exists() {
        std::fs::remove_file(out)?;
    }
    let sidecar = sidecar_path(out);
    if sidecar.exists() {
        std::fs::remove_file(&sidecar)?;
    }

    let opts = DbOptions {
        dimensions: EMBEDDING_DIMENSIONS,
        distance_metric: DistanceMetric::Cosine,
        storage_path: out.to_string_lossy().into_owned(),
        // Index-law: non-concurrent rebuild, m=16, ef_construction=128 (§11.2).
        hnsw_config: Some(HnswConfig {
            m: 16,
            ef_construction: 128,
            ef_search: 100,
            max_elements: entries.len() + 1024,
        }),
        // Full precision preserves the recall floor.
        quantization: Some(QuantizationConfig::None),
    };
    let db = VectorDB::new(opts).map_err(|e| format!("create VectorDB failed: {e}"))?;

    let total = entries.len();
    let mut inserted = 0_usize;
    for chunk in entries.chunks(INSERT_BATCH) {
        let ids = db
            .insert_batch(chunk.to_vec())
            .map_err(|e| format!("insert_batch failed at {inserted}/{total}: {e}"))?;
        inserted += ids.len();
        eprintln!("  inserted {inserted}/{total}");
    }

    let stored = db.len().map_err(|e| format!("len() failed: {e}"))?;
    if stored != inserted {
        return Err(format!("count mismatch: stored {stored} != inserted {inserted}").into());
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let generated_at = rfc3339_utc(now);
    let sidecar_json = serde_json::json!({
        "generatedAt": generated_at,
        "classCount": inserted,
        "source": "ontology-corpus-export",
    });
    std::fs::write(&sidecar, serde_json::to_string_pretty(&sidecar_json)?)?;

    Ok((inserted, sidecar, generated_at))
}

#[tokio::main]
async fn main() -> Result<(), BoxErr> {
    let args = parse_args()?;
    eprintln!(
        "export_corpus: namespace='{}' → out='{}'",
        args.namespace,
        args.out.display()
    );

    let (client, connection) = tokio_postgres::connect(&args.conninfo, tokio_postgres::NoTls)
        .await
        .map_err(|e| format!("connect failed: {e}"))?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("postgres connection error: {e}");
        }
    });

    let (entries, stats) = fetch_vectors(&client, &args.namespace).await?;
    if stats.bad_parse > 0 || stats.bad_dim > 0 {
        eprintln!(
            "skipped: {} unparseable, {} wrong-dimension",
            stats.bad_parse, stats.bad_dim
        );
    }
    if entries.is_empty() {
        return Err("no valid 384-dim embeddings parsed — aborting".into());
    }

    let (inserted, sidecar, generated_at) = build_artifact(&args.out, &entries)?;

    println!("──────────────────────────────────────────────");
    println!("export_corpus SUMMARY");
    println!("  namespace     : {}", args.namespace);
    println!("  rows fetched  : {}", stats.fetched);
    println!("  rows exported : {inserted}");
    println!("  dims verified : {EMBEDDING_DIMENSIONS} (all rows)");
    println!(
        "  skipped       : {} unparseable, {} wrong-dim",
        stats.bad_parse, stats.bad_dim
    );
    println!("  artifact      : {}", args.out.display());
    println!(
        "  sidecar       : {} (generatedAt={generated_at})",
        sidecar.display()
    );
    println!("──────────────────────────────────────────────");

    Ok(())
}

fn sidecar_path(artifact: &Path) -> PathBuf {
    let mut raw = artifact.as_os_str().to_owned();
    raw.push(".generation.json");
    PathBuf::from(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_embedding_brackets() {
        assert_eq!(parse_embedding("[1.0,2,3.5]"), Some(vec![1.0, 2.0, 3.5]));
        assert_eq!(parse_embedding("  [ 0.1 , -0.2 ] "), Some(vec![0.1, -0.2]));
        assert_eq!(parse_embedding("{1,2,3}"), Some(vec![1.0, 2.0, 3.0]));
        assert_eq!(parse_embedding("1,2,3"), Some(vec![1.0, 2.0, 3.0]));
        assert_eq!(parse_embedding("[1e-3,2.5e2]"), Some(vec![0.001, 250.0]));
        assert_eq!(parse_embedding("[]"), None);
        assert_eq!(parse_embedding("[1,foo,3]"), None);
    }

    #[test]
    fn rfc3339_epoch_and_known_dates() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        // 2026-08-17T00:00:00Z = 1_786_924_800 (verified against date -u).
        assert_eq!(rfc3339_utc(1_786_924_800), "2026-08-17T00:00:00Z");
        assert_eq!(rfc3339_utc(1_786_924_800 + 3661), "2026-08-17T01:01:01Z");
    }

    #[test]
    fn sidecar_path_suffix() {
        assert_eq!(
            sidecar_path(Path::new("data/ontology-corpus.rvdb")),
            PathBuf::from("data/ontology-corpus.rvdb.generation.json")
        );
    }
}
