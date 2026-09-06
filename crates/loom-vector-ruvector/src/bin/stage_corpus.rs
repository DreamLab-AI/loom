//! `stage_corpus` — embed the concept records and stage idempotent upserts into
//! the `ontology-corpus` namespace (pg-write channel, §11.2).
//!
//! A port of `tools/ingest/embed_and_stage.py` (deleted 2026-09-03). The
//! embedding half was already Rust — `loom-embed-xinference` names the Python as
//! its origin — so what is ported here is the half that had no Rust home: the
//! `memory_entries` row projection and the idempotent upsert staging.
//!
//! Matches the live `memory_entries` schema and our conventions exactly:
//! - `key` = `urn:ngm:class:<slug>` — the IRI, the join key across
//!   ttl / scaffold / prose / HNSW.
//! - `id` = `loom:ontology-corpus:<key>` (PK; `ON CONFLICT (id)` ⇒ idempotent).
//! - `value` = `to_jsonb(<record text>)` — as `memory_store` stores it.
//! - `embedding` = `ruvector(384)`, cosine HNSW.
//! - `metadata` = slug / title / domain / maturity / quality / `has_prose` /
//!   **generation** — the generation stamp makes the close-the-loop step
//!   (re-embed on promotion) a cheap per-IRI diff.
//! - `source_type = loom`, `project_id = NULL`.
//!
//! The embedder is LOCKED to `bge-small-en-v1.5`/384 by `loom-embed-xinference`
//! (a different model silently invalidates the index), and every returned vector
//! is length-checked there — a mismatch is an error, not a quietly-wrong answer.
//!
//! **Emits a `.sql` file rather than writing to Postgres**, exactly as the
//! Python did: the operator streams it in with `docker exec -i … psql`, which
//! keeps this step reviewable before it touches the database and lets it run
//! without a live connection. The **HNSW rebuild is a SEPARATE step and is
//! never folded into the upsert** — and never `CREATE INDEX CONCURRENTLY`,
//! which double-inserts on the ruvector HNSW access method (verified).
//!
//! Usage:
//! ```text
//! stage_corpus --records uplift-results/ingest/concept-records.jsonl \
//!     --out uplift-results/ingest/ontology-corpus.sql \
//!     --namespace ontology-corpus --batch 96
//! ```

#![allow(clippy::doc_markdown)]

use std::fmt::Write as _;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use loom_embed_xinference::{EmbeddingProvider, XinferenceEmbedder, DIMENSIONS, MODEL_ID};
use serde::Deserialize;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

const DEFAULT_NAMESPACE: &str = "ontology-corpus";
const DEFAULT_SOURCE_TYPE: &str = "loom";
/// Records per Xinference request.
const DEFAULT_BATCH: usize = 96;
/// Rows per `INSERT` statement — keeps single statements a sane size.
const DEFAULT_ROWS_PER_INSERT: usize = 250;
/// The `memory_entries` column list, in the order the rows below render.
const COLUMNS: &str = "(id, namespace, key, value, embedding, metadata, source_type, project_id)";

/// One line of `concept-records.jsonl`, as `build-concept-records` emits it.
#[derive(Debug, Deserialize)]
struct ConceptRecord {
    /// The class slug (the record's `id` field, not the PK).
    id: String,
    text: String,
    /// Passed through to the `metadata` jsonb column verbatim.
    metadata: serde_json::Value,
}

// --- pure SQL rendering (no network, no database — unit-testable) ------------

/// A SQL single-quoted literal from an arbitrary string (Python `sql_str`).
fn sql_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The `ruvector` array literal: fixed 6 decimal places, as Python's `f"{x:.6f}"`.
fn embedding_literal(vector: &[f32]) -> String {
    let mut out = String::with_capacity(vector.len() * 10 + 2);
    out.push('[');
    for (i, x) in vector.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{x:.6}");
    }
    out.push(']');
    out
}

/// Render one `VALUES (...)` tuple for `memory_entries`.
///
/// `metadata` is rendered with compact `serde_json` rather than reproducing
/// Python's `", "` spacing: the column is `jsonb`, which Postgres canonicalises
/// on ingest (keys reordered, whitespace dropped), so the stored value is
/// identical either way.
fn render_row(
    record: &ConceptRecord,
    vector: &[f32],
    namespace: &str,
    source_type: &str,
) -> Result<String, BoxErr> {
    let key = format!("urn:ngm:class:{}", record.id);
    let row_id = format!("{source_type}:{namespace}:{key}");
    let metadata = serde_json::to_string(&record.metadata)?;
    Ok(format!(
        "({}, {}, {}, to_jsonb({}::text), {}::ruvector({DIMENSIONS}), {}::jsonb, {}, NULL)",
        sql_str(&row_id),
        sql_str(namespace),
        sql_str(&key),
        sql_str(&record.text),
        sql_str(&embedding_literal(vector)),
        sql_str(&metadata),
        sql_str(source_type),
    ))
}

/// The `ON CONFLICT (id)` clause that makes the whole file idempotent.
const ON_CONFLICT: &str = "ON CONFLICT (id) DO UPDATE SET value=EXCLUDED.value, \
     embedding=EXCLUDED.embedding, metadata=EXCLUDED.metadata, \
     source_type=EXCLUDED.source_type, updated_at=CURRENT_TIMESTAMP;";

/// Write the whole staged transaction. Returns the number of rows written.
fn write_sql<W: Write>(
    w: &mut W,
    records: &[ConceptRecord],
    vectors: &[Vec<f32>],
    args: &Args,
) -> Result<usize, BoxErr> {
    writeln!(w, "BEGIN;")?;
    writeln!(
        w,
        "-- ontology-corpus concept index: {} classes, {MODEL_ID}/{DIMENSIONS}",
        records.len()
    )?;

    let mut written = 0_usize;
    for chunk_start in (0..records.len()).step_by(args.rows_per_insert) {
        let end = (chunk_start + args.rows_per_insert).min(records.len());
        writeln!(w, "INSERT INTO memory_entries {COLUMNS} VALUES")?;
        let rows: Vec<String> = (chunk_start..end)
            .map(|i| render_row(&records[i], &vectors[i], &args.namespace, &args.source_type))
            .collect::<Result<_, _>>()?;
        written += rows.len();
        write!(w, "{}", rows.join(",\n"))?;
        writeln!(w, "\n{ON_CONFLICT}")?;
    }

    writeln!(w, "COMMIT;")?;
    Ok(written)
}

// --- CLI ---------------------------------------------------------------------

struct Args {
    records: PathBuf,
    out: PathBuf,
    namespace: String,
    source_type: String,
    batch: usize,
    rows_per_insert: usize,
}

fn parse_args() -> Result<Args, BoxErr> {
    let mut args = Args {
        records: PathBuf::from("uplift-results/ingest/concept-records.jsonl"),
        out: PathBuf::from("uplift-results/ingest/ontology-corpus.sql"),
        namespace: DEFAULT_NAMESPACE.to_owned(),
        source_type: DEFAULT_SOURCE_TYPE.to_owned(),
        batch: DEFAULT_BATCH,
        rows_per_insert: DEFAULT_ROWS_PER_INSERT,
    };

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f.to_owned(), Some(v.to_owned())),
            None => (arg.clone(), None),
        };
        if flag == "--help" || flag == "-h" {
            println!(
                "stage_corpus [--records <records.jsonl>] [--out <corpus.sql>] \
                 [--namespace {DEFAULT_NAMESPACE}] [--source-type {DEFAULT_SOURCE_TYPE}] \
                 [--batch {DEFAULT_BATCH}] [--rows-per-insert {DEFAULT_ROWS_PER_INSERT}]"
            );
            std::process::exit(0);
        }
        let value = match inline {
            Some(v) => v,
            None => it
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?,
        };
        match flag.as_str() {
            "--records" => args.records = PathBuf::from(value),
            "--out" => args.out = PathBuf::from(value),
            "--namespace" => args.namespace = value,
            "--source-type" => args.source_type = value,
            "--batch" => args.batch = value.parse()?,
            "--rows-per-insert" => args.rows_per_insert = value.parse()?,
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    if args.batch == 0 || args.rows_per_insert == 0 {
        return Err("--batch and --rows-per-insert must be non-zero".into());
    }
    Ok(args)
}

fn read_records(path: &PathBuf) -> Result<Vec<ConceptRecord>, BoxErr> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read records {}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("parse record: {e}").into()))
        .collect()
}

async fn run() -> Result<(), BoxErr> {
    let args = parse_args()?;
    let records = read_records(&args.records)?;
    if records.is_empty() {
        return Err(format!("no records in {}", args.records.display()).into());
    }

    let embedder = XinferenceEmbedder::from_env();
    eprintln!(
        "embedding {} records via {MODEL_ID} at {} (batch {})...",
        records.len(),
        embedder.endpoint(),
        args.batch
    );

    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(records.len());
    for (n, chunk) in records.chunks(args.batch).enumerate() {
        let texts: Vec<String> = chunk.iter().map(|r| r.text.clone()).collect();
        // The embedder length-checks every vector against 384 and errors on a
        // mismatch, so no dimension assertion is needed here.
        vectors.extend(embedder.embed_batch(&texts).await?);
        if n % 10 == 0 {
            eprintln!("  embedded {}/{}", vectors.len(), records.len());
        }
    }
    debug_assert_eq!(vectors.len(), records.len());

    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = std::fs::File::create(&args.out)
        .map_err(|e| format!("create {}: {e}", args.out.display()))?;
    let mut w = BufWriter::new(file);
    let written = write_sql(&mut w, &records, &vectors, &args)?;
    w.flush()?;
    drop(w);

    let kb = std::fs::metadata(&args.out)?.len() / 1024;
    eprintln!(
        "staged {written} upserts -> {} ({kb} KB)",
        args.out.display()
    );
    eprintln!(
        "NEXT: stream it in, then rebuild the HNSW index NON-concurrently \
         (m=16, ef_construction=128) as a separate step."
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("stage_corpus: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, text: &str) -> ConceptRecord {
        ConceptRecord {
            id: id.to_owned(),
            text: text.to_owned(),
            metadata: serde_json::json!({ "slug": id, "quality": 0.6 }),
        }
    }

    fn args() -> Args {
        Args {
            records: PathBuf::new(),
            out: PathBuf::new(),
            namespace: DEFAULT_NAMESPACE.to_owned(),
            source_type: DEFAULT_SOURCE_TYPE.to_owned(),
            batch: DEFAULT_BATCH,
            rows_per_insert: 2,
        }
    }

    #[test]
    fn sql_literals_double_single_quotes() {
        assert_eq!(sql_str("plain"), "'plain'");
        assert_eq!(sql_str("it's"), "'it''s'");
        // The classic injection shape is neutralised, not stripped.
        assert_eq!(sql_str("'); DROP TABLE x; --"), "'''); DROP TABLE x; --'");
    }

    #[test]
    fn embedding_literal_is_six_decimal_places() {
        assert_eq!(embedding_literal(&[0.5, -0.25]), "[0.500000,-0.250000]");
        assert_eq!(embedding_literal(&[]), "[]");
    }

    #[test]
    fn row_uses_the_iri_key_and_prefixed_primary_key() {
        let row = render_row(&record("rgb", "RGB."), &[0.5], "ontology-corpus", "loom").unwrap();
        assert!(row.starts_with("('loom:ontology-corpus:urn:ngm:class:rgb', 'ontology-corpus', 'urn:ngm:class:rgb', "), "{row}");
        assert!(row.contains("to_jsonb('RGB.'::text)"), "{row}");
        assert!(row.contains("'[0.500000]'::ruvector(384)"), "{row}");
        assert!(row.ends_with("'loom', NULL)"), "{row}");
    }

    #[test]
    fn transaction_batches_rows_and_is_idempotent() {
        let records = [record("a", "A."), record("b", "B."), record("c", "C.")];
        let vectors = vec![vec![0.1], vec![0.2], vec![0.3]];
        let mut buf = Vec::new();
        let written = write_sql(&mut buf, &records, &vectors, &args()).unwrap();
        let sql = String::from_utf8(buf).unwrap();

        assert_eq!(written, 3);
        assert!(sql.starts_with("BEGIN;\n"), "{sql}");
        assert!(sql.trim_end().ends_with("COMMIT;"), "{sql}");
        // rows_per_insert = 2 over 3 records ⇒ two INSERT statements.
        assert_eq!(sql.matches("INSERT INTO memory_entries").count(), 2);
        assert_eq!(sql.matches("ON CONFLICT (id) DO UPDATE").count(), 2);
        // The HNSW rebuild is never folded into the upsert (index-law).
        assert!(!sql.contains("CREATE INDEX"), "{sql}");
    }

    #[test]
    fn records_parse_from_the_builder_jsonl() {
        let line = r#"{"id": "rgb", "namespace": "ontology-corpus", "text": "RGB.", "metadata": {"slug": "rgb"}}"#;
        let r: ConceptRecord = serde_json::from_str(line).unwrap();
        assert_eq!(r.id, "rgb");
        assert_eq!(r.text, "RGB.");
    }
}
