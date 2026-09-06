//! `build-concept-records` — the CONCEPT-INDEX record builder (ADR-136 D3).
//!
//! A 1:1 port of `tools/ingest/build_concept_records.py` (deleted 2026-09-03).
//! One record per ontology class: the seed-finding surface a semantic query
//! matches against. The embedded text is the human-scrutible unit's *header* —
//! title + definition + taxonomy + verbalised typed relations + the `dfull`
//! prose summary — so a vector query lands on the right IRI.
//!
//! **PURE + deterministic.** Reads the build-derived projections
//! (`scaffold-index.json` + `prose-index.json`, themselves single-source per
//! ADR-136 D4) and emits JSONL. No network, no embedding, no infra write. The
//! writer half is `stage-corpus` in `loom-vector-ruvector`, kept separate so
//! this half stays testable and reviewable.
//!
//! **Byte-parity is the spec.** The output is byte-identical to the Python's
//! `json.dumps(record, ensure_ascii=False)`: Python's `", "` / `": "`
//! separators, key order fixed by struct declaration order, and non-ASCII
//! emitted raw. `tests/golden-python/golden_concept_records.jsonl` pins it.
//!
//! Usage:
//! ```text
//! build-concept-records --scaffold app/data/scaffold-index.json \
//!     --prose app/data/prose-index.json \
//!     --out uplift-results/ingest/concept-records.jsonl
//! ```

#![allow(clippy::doc_markdown)]

use std::collections::HashSet;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Serialize;

use loom_scaffold::index::{ClassEntry, RawIndex};
use loom_scaffold::prose::ProseIndex;

/// Relation types in the order they read most naturally when verbalised.
/// Verbatim from Python `REL_ORDER`; types absent from a class are omitted.
const REL_ORDER: [&str; 12] = [
    "hasPart",
    "requires",
    "enables",
    "dependsOn",
    "implements",
    "uses",
    "partOf",
    "relatedTo",
    "bridgesTo",
    "supports",
    "standardizedBy",
    "contrastsWith",
];

/// The RuVector namespace every record is keyed into.
const NAMESPACE: &str = "ontology-corpus";

/// Cap per relation type: enough signal, no dilution (Python `MAX_REL_TARGETS`).
const MAX_REL_TARGETS: usize = 8;

/// Cap on direct parents verbalised into "Is a kind of:" (Python `cap=6`).
const MAX_PARENTS: usize = 6;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

// --- the record shape --------------------------------------------------------
// Field order IS the wire order: serde emits struct fields in declaration
// order, and byte-parity with Python's dict-insertion order depends on it.
// Do not reorder.

#[derive(Serialize)]
struct Record<'a> {
    id: &'a str,
    namespace: &'static str,
    text: String,
    metadata: Metadata<'a>,
}

#[derive(Serialize)]
struct Metadata<'a> {
    slug: &'a str,
    title: String,
    /// Python `cls.get("dom") or None` — an empty string becomes `null`.
    domain: Option<&'a str>,
    /// Python `cls.get("m") or None` — an empty string becomes `null`.
    maturity: Option<&'a str>,
    /// Python `cls.get("q")` — absent stays `null` (no truthiness collapse, so
    /// a genuine `0.0` survives as `0.0`).
    quality: Option<f64>,
    has_prose: bool,
    n_parents: usize,
    n_relations: usize,
    generation: &'a str,
}

// --- Python-compatible JSON formatting --------------------------------------

/// `serde_json` formatter reproducing Python's `json.dumps` defaults: `", "`
/// between items and `": "` after a key. Everything else (string escaping,
/// shortest-roundtrip floats) already matches — Python with
/// `ensure_ascii=False` and `serde_json` escape exactly the same set (`"`,
/// `\`, and C0 controls, with `\b \t \n \f \r` shortcuts, else lowercase
/// `\u00xx`) and both print floats via a shortest-roundtrip algorithm.
struct PythonFormatter;

impl serde_json::ser::Formatter for PythonFormatter {
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        writer.write_all(b": ")
    }
}

/// Serialise one record exactly as Python's `json.dumps(r, ensure_ascii=False)`.
fn to_python_json<T: Serialize>(value: &T) -> Result<String, BoxErr> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, PythonFormatter);
    value.serialize(&mut ser)?;
    Ok(String::from_utf8(buf)?)
}

// --- verbalisation (1:1 with Python) ----------------------------------------

/// Python `or`-truthiness for an optional string: `None` and `""` are falsy.
fn truthy(s: Option<&String>) -> Option<&str> {
    s.map(String::as_str).filter(|v| !v.is_empty())
}

/// Python `_titles`: map target slugs to human titles (fallback: the slug with
/// `-` turned into spaces), de-duped by title, capped.
///
/// Targets are used **verbatim** — an IRI target such as
/// `urn:ngm:class:graph-database` is NOT resolved to its slug here, exactly as
/// in Python, and so falls through to the `replace("-", " ")` fallback.
fn titles<'a>(slugs: &'a [String], title_of: &'a IndexMap<&str, &str>, cap: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for s in slugs {
        let t = title_of
            .get(s.as_str())
            .map_or_else(|| s.replace('-', " "), |v| (*v).to_owned());
        if seen.insert(t.clone()) {
            out.push(t);
        }
        // Python checks the cap after every iteration, dedup hit or not.
        if out.len() >= cap {
            break;
        }
    }
    out
}

/// The embedded text: title · definition · taxonomy · relations · dfull.
/// Readable prose, so the vector points at exactly the legible unit.
fn verbalise(slug: &str, cls: &ClassEntry, dfull: &str, title_of: &IndexMap<&str, &str>) -> String {
    let title = truthy(cls.t.as_ref()).map_or_else(|| slug.replace('-', " "), ToOwned::to_owned);
    let mut parts: Vec<String> = vec![format!("{title}.")];

    if let Some(d) = truthy(cls.d.as_ref()) {
        parts.push(d.trim().to_owned());
    }

    // Taxonomy (direct parents first — the strongest structural signal).
    let parents = titles(&cls.sup, title_of, MAX_PARENTS);
    if !parents.is_empty() {
        parts.push(format!("Is a kind of: {}.", parents.join(", ")));
    }

    // Typed relations, in reading order, empty types omitted.
    let mut rel_phrases: Vec<String> = Vec::new();
    for key in REL_ORDER {
        let Some(raw) = cls.rel.get(key) else {
            continue;
        };
        let tgts = titles(raw, title_of, MAX_REL_TARGETS);
        if !tgts.is_empty() {
            rel_phrases.push(format!("{key}: {}", tgts.join(", ")));
        }
    }
    if !rel_phrases.is_empty() {
        parts.push(format!("Relations — {}.", rel_phrases.join("; ")));
    }

    // The dfull research-prose summary (when the class has one).
    if !dfull.is_empty() {
        parts.push(dfull.trim().to_owned());
    }

    parts.join(" ")
}

// --- CLI ---------------------------------------------------------------------

struct Args {
    scaffold: PathBuf,
    prose: PathBuf,
    out: PathBuf,
    sample: usize,
}

fn parse_args() -> Result<Args, BoxErr> {
    let mut args = Args {
        scaffold: PathBuf::from("app/data/scaffold-index.json"),
        prose: PathBuf::from("app/data/prose-index.json"),
        out: PathBuf::from("uplift-results/ingest/concept-records.jsonl"),
        sample: 1,
    };

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        // Accept both `--flag value` and `--flag=value`.
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f.to_owned(), Some(v.to_owned())),
            None => (arg.clone(), None),
        };
        if flag == "--help" || flag == "-h" {
            println!(
                "build-concept-records [--scaffold <scaffold-index.json>] \
                 [--prose <prose-index.json>] [--out <records.jsonl>] [--sample N]"
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
            "--scaffold" => args.scaffold = PathBuf::from(value),
            "--prose" => args.prose = PathBuf::from(value),
            "--out" => args.out = PathBuf::from(value),
            "--sample" => args.sample = value.parse()?,
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    Ok(args)
}

/// Load the prose index. Unlike the serving path's fail-open `load_prose`, a
/// build tool fails LOUDLY: a missing or malformed prose index would silently
/// produce a corpus with no `dfull` text, which is the expensive kind of
/// wrong. Python raised here too.
fn load_prose_strict(path: &Path) -> Result<ProseIndex, BoxErr> {
    #[derive(serde::Deserialize)]
    struct ProseFile {
        #[serde(default)]
        pages: ProseIndex,
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read prose index {}: {e}", path.display()))?;
    let file: ProseFile = serde_json::from_str(&text)
        .map_err(|e| format!("parse prose index {}: {e}", path.display()))?;
    Ok(file.pages)
}

/// Build every record, in Python's `sorted(classes)` (code-point) order.
fn build_records<'a>(index: &'a RawIndex, prose: &'a ProseIndex) -> Vec<Record<'a>> {
    // slug -> title, Python `c.get("t") or slug` (note: the slug RAW, not
    // de-hyphenated — that fallback only applies to unresolved relation targets).
    let title_of: IndexMap<&str, &str> = index
        .classes
        .iter()
        .map(|(slug, c)| (slug.as_str(), truthy(c.t.as_ref()).unwrap_or(slug.as_str())))
        .collect();

    let mut slugs: Vec<&String> = index.classes.keys().collect();
    slugs.sort(); // Rust byte order over UTF-8 == Python code-point order.

    slugs
        .into_iter()
        .map(|slug| {
            let cls = &index.classes[slug.as_str()];
            let dfull = prose
                .get(slug)
                .and_then(|p| p.dfull.as_deref())
                .unwrap_or_default();
            let text = verbalise(slug, cls, dfull, &title_of);
            Record {
                id: slug,
                namespace: NAMESPACE,
                text,
                metadata: Metadata {
                    slug,
                    title: truthy(cls.t.as_ref()).unwrap_or(slug.as_str()).to_owned(),
                    domain: truthy(cls.dom.as_ref()),
                    maturity: truthy(cls.m.as_ref()),
                    quality: cls.q,
                    has_prose: !dfull.is_empty(),
                    n_parents: cls.sup.len(),
                    n_relations: cls.rel.values().map(Vec::len).sum(),
                    generation: &index.generated,
                },
            }
        })
        .collect()
}

fn run() -> Result<(), BoxErr> {
    let args = parse_args()?;

    let raw = std::fs::read_to_string(&args.scaffold)
        .map_err(|e| format!("read scaffold index {}: {e}", args.scaffold.display()))?;
    let index: RawIndex = serde_json::from_str(&raw)
        .map_err(|e| format!("parse scaffold index {}: {e}", args.scaffold.display()))?;
    let prose = load_prose_strict(&args.prose)?;

    let records = build_records(&index, &prose);

    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let file = std::fs::File::create(&args.out)
        .map_err(|e| format!("create {}: {e}", args.out.display()))?;
    let mut w = BufWriter::new(file);
    for r in &records {
        w.write_all(to_python_json(r)?.as_bytes())?;
        w.write_all(b"\n")?;
    }
    w.flush()?;

    report(&records, &index.generated, &args);
    Ok(())
}

/// The Python script's stderr summary (chars are Unicode code points, as in
/// Python's `len`).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn report(records: &[Record<'_>], generation: &str, args: &Args) {
    let n = records.len();
    let with_prose = records.iter().filter(|r| r.metadata.has_prose).count();
    let mut lens: Vec<usize> = records.iter().map(|r| r.text.chars().count()).collect();
    lens.sort_unstable();

    eprintln!("built {n} concept records (generation={generation})");
    eprintln!("  with dfull prose: {with_prose}/{n}");
    if let (Some(min), Some(max)) = (lens.first(), lens.last()) {
        let p95 = lens[(n as f64 * 0.95) as usize];
        eprintln!(
            "  text chars: min={min} median={} p95={p95} max={max}",
            lens[n / 2]
        );
    }
    eprintln!("  → {}", args.out.display());

    for r in records.iter().take(args.sample) {
        let meta = to_python_json(&r.metadata).unwrap_or_default();
        let head: String = r.text.chars().take(600).collect();
        eprintln!("\n--- sample: {} (meta: {meta}) ---\n{head}", r.id);
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("build-concept-records: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_file(rel: &str) -> String {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    /// EXP-002-style byte-parity: the Rust builder reproduces the Python's
    /// `concept-records.jsonl` byte-for-byte on the shared golden fixture.
    #[test]
    fn golden_byte_parity_with_python() {
        let index: RawIndex =
            serde_json::from_str(&workspace_file("tests/golden-python/fixture.json")).unwrap();
        let prose = load_prose_strict(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("tests/golden-python/fixture-prose.json"),
        )
        .unwrap();

        let mut got = String::new();
        for r in &build_records(&index, &prose) {
            got.push_str(&to_python_json(r).unwrap());
            got.push('\n');
        }

        let want = workspace_file("tests/golden-python/golden_concept_records.jsonl");
        assert_eq!(got, want, "byte-parity with the Python golden");
    }

    /// Python's `json.dumps` separators, verbatim.
    #[test]
    fn python_separators_and_raw_unicode() {
        #[derive(Serialize)]
        struct T {
            a: u8,
            b: Vec<u8>,
            c: Option<u8>,
            d: &'static str,
        }
        assert_eq!(
            to_python_json(&T {
                a: 1,
                b: vec![1, 2],
                c: None,
                d: "em — dash \"q\""
            })
            .unwrap(),
            r#"{"a": 1, "b": [1, 2], "c": null, "d": "em — dash \"q\""}"#
        );
    }

    /// An empty `dom`/`m` collapses to null (Python `or None`), but a genuine
    /// `0.0` quality survives — `q` has no truthiness collapse in Python.
    #[test]
    fn empty_strings_collapse_to_null_but_zero_quality_survives() {
        let json = r#"{"generated":"G","classes":{"a":{"t":"A","dom":"","m":"","q":0.0}}}"#;
        let index: RawIndex = serde_json::from_str(json).unwrap();
        let empty = ProseIndex::new();
        let recs = build_records(&index, &empty);
        let line = to_python_json(&recs[0]).unwrap();
        assert!(
            line.contains(r#""domain": null, "maturity": null, "quality": 0.0"#),
            "{line}"
        );
    }

    /// Relation targets are used verbatim: an IRI is NOT resolved to its slug,
    /// it falls through to the `-`→space fallback (matches Python `_titles`).
    #[test]
    fn iri_relation_target_is_not_resolved() {
        let json = r#"{"generated":"G","classes":{
            "a":{"t":"A","rel":{"uses":["urn:ngm:class:graph-database"]}},
            "graph-database":{"t":"Graph Database"}}}"#;
        let index: RawIndex = serde_json::from_str(json).unwrap();
        let empty = ProseIndex::new();
        let recs = build_records(&index, &empty);
        assert!(
            recs[0].text.contains("uses: urn:ngm:class:graph database"),
            "{}",
            recs[0].text
        );
    }

    /// Dedup is by TITLE and the cap is checked every iteration, dedup or not.
    #[test]
    fn titles_dedup_by_title_and_cap() {
        let map: IndexMap<&str, &str> = [("x", "Same"), ("y", "Same"), ("z", "Other")]
            .into_iter()
            .collect();
        let slugs: Vec<String> = ["x", "y", "z"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(titles(&slugs, &map, 8), vec!["Same", "Other"]);
        assert_eq!(titles(&slugs, &map, 1), vec!["Same"]);
    }

    /// Records come out in code-point order regardless of file order.
    #[test]
    fn records_are_sorted_by_slug() {
        let json = r#"{"generated":"G","classes":{"zeta":{"t":"Z"},"alpha":{"t":"A"}}}"#;
        let index: RawIndex = serde_json::from_str(json).unwrap();
        let empty = ProseIndex::new();
        let recs = build_records(&index, &empty);
        assert_eq!(
            recs.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
    }
}
