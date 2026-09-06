//! `loom-graph-oxigraph` — `GraphStore` over a native in-memory `oxigraph::store::Store`.
//!
//! Ports `app/loom_graph.py` verbatim in semantics: the read-only SPARQL clamp
//! (`_FORBIDDEN` / `_READ_FORM` / LIMIT injection / row cap), the
//! `rdfs:label`/`skos:prefLabel`/`ngm:title` CONTAINS label search, and the
//! `_term_str` decoration-stripping — the last collapses into typed term
//! matching now that oxigraph hands back typed `QueryResults`.
//!
//! INVARIANT (DDD BC24 I11): the Loom serves the PUBLISHED ONTOLOGY ONLY. `load`
//! bulk-loads a hard-coded allowlist — `ontology.ttl` + `ontology-inferred.ttl`
//! — never a glob, so the working graph can never be loaded by accident.
//!
//! Fail-open: an absent or failed store constructs fine, reports
//! `status().available == false`, and returns `LoomError::GraphUnavailable` from
//! `query`/`search_labels` so the façade degrades to lexical — it never panics.

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::OnceLock;

use async_trait::async_trait;
use oxigraph::io::RdfFormat;
use oxigraph::model::Term;
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;
use regex::Regex;

use loom_domain::{GraphStatus, GraphStore, Iri, LabelHit, LoomError, SparqlResult};

/// The DDD BC24 I11 published-ontology allowlist. NEVER a glob.
const ALLOWLIST: [&str; 2] = ["ontology.ttl", "ontology-inferred.ttl"];

/// `LOOM_SPARQL_LIMIT` default — the LIMIT injected on an un-LIMITed SELECT.
const DEFAULT_LIMIT: usize = 10_000;
/// `LOOM_SPARQL_MAX_ROWS` default — the server-side row cap (truncation flag).
const DEFAULT_MAX_ROWS: usize = 10_000;

// -- the read-only clamp (regex semantics carried verbatim from loom_graph.py) --

fn forbidden_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(INSERT|DELETE|LOAD|CLEAR|DROP|CREATE|COPY|MOVE|ADD|SERVICE)\b").unwrap()
    })
}

fn read_form_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(SELECT|ASK|CONSTRUCT|DESCRIBE)\b").unwrap())
}

fn limit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bLIMIT\s+\d+").unwrap())
}

/// A SELECT whose first *effective* keyword is SELECT — i.e. SELECT after any
/// leading SPARQL prologue (BASE/PREFIX declarations, `#` comments, whitespace).
///
/// DELIBERATE DIVERGENCE FROM PYTHON (audit finding 3, better-than-parity): the
/// Python clamp anchors on `re.match(r"\s*SELECT")`, so a leading `PREFIX` block
/// slips a SELECT past LIMIT injection and evaluates unbounded until the row cap.
/// The LIMIT clamp is a SECURITY control, not a parity feature, so we consume the
/// prologue and inject for those SELECTs too. ASK/CONSTRUCT/DESCRIBE (whose first
/// effective keyword is not SELECT) still do not match.
fn leading_select_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?is)^\s*(?:(?:BASE\s+<[^>]*>|PREFIX\s+[^\s:]*:\s*<[^>]*>|#[^\n]*\n)\s*)*SELECT",
        )
        .unwrap()
    })
}

/// Reject writes/SERVICE and require a read form. Ports `LoomGraph.validate`;
/// both failures are `BadQuery` (→ HTTP 400 at the router edge).
pub fn validate(query: &str) -> Result<(), LoomError> {
    if forbidden_re().is_match(query) {
        return Err(LoomError::BadQuery(
            "forbidden keyword (write/SERVICE) — the Loom store is read-only".to_owned(),
        ));
    }
    if !read_form_re().is_match(query) {
        return Err(LoomError::BadQuery(
            "only SELECT/ASK/CONSTRUCT/DESCRIBE are permitted".to_owned(),
        ));
    }
    Ok(())
}

/// Inject a `LIMIT` on a SELECT that omitted one. Strengthens `LoomGraph._clamp`
/// (audit finding 3): the injection fires when the first *effective* keyword is
/// SELECT — after any leading `BASE`/`PREFIX`/comment prologue, which the Python
/// `re.match(r"\s*SELECT")` anchoring lets bypass the clamp — and only when no
/// `LIMIT n` already appears. Non-SELECT read forms are never LIMIT-injected.
pub fn clamp(query: &str, default_limit: usize) -> String {
    if read_form_re().is_match(query)
        && leading_select_re().is_match(query)
        && !limit_re().is_match(query)
    {
        // Python: query.rstrip().rstrip(";") — trim trailing whitespace, then
        // trailing semicolons, then append the LIMIT on a fresh line.
        let trimmed = query.trim_end().trim_end_matches(';');
        format!("{trimmed}\nLIMIT {default_limit}")
    } else {
        query.to_owned()
    }
}

/// Typed replacement for Python's `_term_str` regex stripping: an IRI renders as
/// its bare address, a literal as its lexical value (datatype/lang decoration
/// dropped), a blank node as `_:id`.
fn term_str(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => n.as_str().to_owned(),
        Term::Literal(l) => l.value().to_owned(),
        // BlankNode always; Triple only under the rdf-star feature — Display
        // gives `_:id` / the N-Triples form, matching pyoxigraph's `str()`.
        other => other.to_string(),
    }
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The `rdfs:label` / `skos:prefLabel` / `ngm:title` CONTAINS query from
/// `LoomGraph.search`. `?p` is added to the projection (Python discarded it) so
/// the typed `LabelHit.predicate` can be populated; the WHERE is identical.
fn label_query(needle_lower: &str, limit: usize) -> String {
    let needle = esc(needle_lower);
    format!(
        "SELECT DISTINCT ?s ?p ?label WHERE {{\n  \
         ?s ?p ?label .\n  \
         FILTER(?p IN (<http://www.w3.org/2000/01/rdf-schema#label>,\n                \
         <http://www.w3.org/2004/02/skos/core#prefLabel>,\n                \
         <https://narrativegoldmine.com/ns/v1#title>))\n  \
         FILTER(CONTAINS(LCASE(STR(?label)), \"{needle}\"))\n\
         }} LIMIT {limit}"
    )
}

/// Ports `_esc`: backslash then double-quote, so the needle is a safe SPARQL
/// string literal.
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// In-process read-truth store loaded from the mirrored generation.
pub struct OxigraphStore {
    store: Option<Store>,
    loaded_files: Vec<String>,
    triples: usize,
    error: Option<String>,
    default_limit: usize,
    max_rows: usize,
}

impl OxigraphStore {
    /// Load the published-ontology allowlist from `data_dir` into a fresh
    /// in-memory store. Never returns `Result`, never panics: an absent dir,
    /// absent files, or a parse failure all yield an unavailable-but-honest
    /// store (`status().available == false`, reason in `status().error`).
    ///
    /// Limits are read once from `LOOM_SPARQL_LIMIT` / `LOOM_SPARQL_MAX_ROWS`.
    pub fn load(data_dir: impl AsRef<Path>) -> Self {
        Self::load_with(
            data_dir,
            env_usize("LOOM_SPARQL_LIMIT", DEFAULT_LIMIT),
            env_usize("LOOM_SPARQL_MAX_ROWS", DEFAULT_MAX_ROWS),
        )
    }

    fn load_with(data_dir: impl AsRef<Path>, default_limit: usize, max_rows: usize) -> Self {
        let dir = data_dir.as_ref();
        let store = match Store::new() {
            Ok(s) => s,
            Err(e) => {
                return Self::failed(format!("store init failed: {e}"), default_limit, max_rows)
            }
        };
        let mut loaded = Vec::new();
        for name in ALLOWLIST {
            let path = dir.join(name);
            if !path.exists() {
                continue;
            }
            let file = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    return Self::failed(format!("graph load failed: {e}"), default_limit, max_rows)
                }
            };
            if let Err(e) = store.load_from_reader(RdfFormat::Turtle, BufReader::new(file)) {
                return Self::failed(format!("graph load failed: {e}"), default_limit, max_rows);
            }
            loaded.push((*name).to_owned());
        }
        if loaded.is_empty() {
            // Divergence from Python (which reports available=true for an empty
            // store): with no allowlisted artifact present we report
            // available=false, which is the honest /health signal for fail-open.
            return Self {
                store: None,
                loaded_files: Vec::new(),
                triples: 0,
                error: Some(format!(
                    "no published-ontology artifacts (ontology.ttl / ontology-inferred.ttl) in {}",
                    dir.display()
                )),
                default_limit,
                max_rows,
            };
        }
        let triples = store.len().unwrap_or(0);
        Self {
            store: Some(store),
            loaded_files: loaded,
            triples,
            error: None,
            default_limit,
            max_rows,
        }
    }

    fn failed(error: String, default_limit: usize, max_rows: usize) -> Self {
        Self {
            store: None,
            loaded_files: Vec::new(),
            triples: 0,
            error: Some(error),
            default_limit,
            max_rows,
        }
    }

    fn run(&self, query: &str) -> Result<SparqlResult, LoomError> {
        let store = self.store.as_ref().ok_or_else(|| {
            LoomError::GraphUnavailable(
                self.error
                    .clone()
                    .unwrap_or_else(|| "store not loaded".to_owned()),
            )
        })?;
        validate(query)?;
        let clamped = clamp(query, self.default_limit);
        let results = store
            .query(clamped.as_str())
            .map_err(|e| LoomError::BadQuery(e.to_string()))?;
        to_sparql_result(results, self.max_rows)
    }
}

fn to_sparql_result(results: QueryResults, max_rows: usize) -> Result<SparqlResult, LoomError> {
    match results {
        QueryResults::Boolean(b) => Ok(SparqlResult {
            columns: Vec::new(),
            rows: Vec::new(),
            boolean: Some(b),
            truncated: false,
        }),
        QueryResults::Solutions(solutions) => {
            let columns: Vec<String> = solutions
                .variables()
                .iter()
                .map(|v| v.as_str().to_owned())
                .collect();
            let mut rows = Vec::new();
            let mut truncated = false;
            for (i, sol) in solutions.enumerate() {
                if i >= max_rows {
                    truncated = true;
                    break;
                }
                let sol = sol.map_err(|e| LoomError::BadQuery(e.to_string()))?;
                let row = columns
                    .iter()
                    .map(|c| sol.get(c.as_str()).map_or_else(String::new, term_str))
                    .collect();
                rows.push(row);
            }
            Ok(SparqlResult {
                columns,
                rows,
                boolean: None,
                truncated,
            })
        }
        QueryResults::Graph(triples) => {
            // CONSTRUCT/DESCRIBE yield triples, not bindings — Python emitted a
            // single "triple" column of `str(triple)`; we keep that shape.
            let mut rows = Vec::new();
            let mut truncated = false;
            for (i, tr) in triples.enumerate() {
                if i >= max_rows {
                    truncated = true;
                    break;
                }
                let tr = tr.map_err(|e| LoomError::BadQuery(e.to_string()))?;
                rows.push(vec![tr.to_string()]);
            }
            Ok(SparqlResult {
                columns: vec!["triple".to_owned()],
                rows,
                boolean: None,
                truncated,
            })
        }
    }
}

#[async_trait]
impl GraphStore for OxigraphStore {
    async fn query(&self, sparql: &str) -> Result<SparqlResult, LoomError> {
        self.run(sparql)
    }

    async fn search_labels(&self, needle: &str, limit: usize) -> Result<Vec<LabelHit>, LoomError> {
        // Python trims + lowercases the needle; empty → error.
        let needle_lower = needle.trim().to_lowercase();
        if needle_lower.is_empty() {
            return Err(LoomError::BadQuery("empty query".to_owned()));
        }
        let result = self.run(&label_query(&needle_lower, limit))?;
        // columns are [s, p, label] by construction.
        let hits = result
            .rows
            .into_iter()
            .filter_map(|mut row| {
                if row.len() < 3 {
                    return None;
                }
                let label = row.pop().unwrap();
                let predicate = row.pop().unwrap();
                let iri = row.pop().unwrap();
                Some(LabelHit {
                    iri: Iri::new(iri),
                    label,
                    predicate,
                })
            })
            .collect();
        Ok(hits)
    }

    fn status(&self) -> GraphStatus {
        GraphStatus {
            available: self.store.is_some(),
            triples: self.triples,
            loaded_files: self.loaded_files.clone(),
            error: self.error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a store directly from a Turtle string (bypassing the file
    /// allowlist) so the clamp/row-cap behaviour can be exercised without a
    /// tempdir. Test-only.
    fn store_from_turtle(ttl: &str, default_limit: usize, max_rows: usize) -> OxigraphStore {
        let store = Store::new().unwrap();
        store
            .load_from_reader(RdfFormat::Turtle, ttl.as_bytes())
            .unwrap();
        let triples = store.len().unwrap();
        OxigraphStore {
            store: Some(store),
            loaded_files: vec!["inline.ttl".to_owned()],
            triples,
            error: None,
            default_limit,
            max_rows,
        }
    }

    const FORBIDDEN_VERBS: [&str; 10] = [
        "INSERT", "DELETE", "LOAD", "CLEAR", "DROP", "CREATE", "COPY", "MOVE", "ADD", "SERVICE",
    ];

    #[test]
    fn each_forbidden_verb_is_rejected() {
        for verb in FORBIDDEN_VERBS {
            // Even wrapped in an otherwise-valid SELECT, the write/SERVICE verb
            // wins (forbidden is checked first).
            let q = format!("SELECT * WHERE {{ ?s ?p ?o }} ; {verb} something");
            let err = validate(&q).unwrap_err();
            assert!(
                matches!(err, LoomError::BadQuery(_)),
                "verb {verb} should be BadQuery, got {err:?}"
            );
            // And lowercase — the clamp is case-insensitive.
            assert!(validate(&verb.to_lowercase()).is_err(), "lowercase {verb}");
        }
    }

    #[test]
    fn each_read_form_is_accepted() {
        let reads = [
            "SELECT * WHERE { ?s ?p ?o }",
            "ASK { ?s ?p ?o }",
            "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
            "DESCRIBE <urn:ngm:class:knowledge-graph>",
        ];
        for q in reads {
            assert!(validate(q).is_ok(), "read form should pass: {q}");
        }
    }

    #[test]
    fn non_read_form_is_rejected() {
        // No write verb, but no read form either.
        let err = validate("PREFIX ex: <urn:x#> WHERE { ?s ?p ?o }").unwrap_err();
        assert!(matches!(err, LoomError::BadQuery(_)));
    }

    #[test]
    fn limit_injected_exactly_once_when_absent() {
        let out = clamp("SELECT ?s WHERE { ?s ?p ?o }", 10_000);
        assert!(
            out.contains("LIMIT 10000"),
            "expected injected LIMIT: {out}"
        );
        assert_eq!(
            out.matches("LIMIT").count(),
            1,
            "LIMIT must appear exactly once: {out}"
        );
    }

    #[test]
    fn limit_not_injected_when_present() {
        let q = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 5";
        assert_eq!(clamp(q, 10_000), q, "explicit LIMIT must be left untouched");
    }

    #[test]
    fn limit_not_injected_for_non_select_read_forms() {
        for q in [
            "ASK { ?s ?p ?o }",
            "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
            "DESCRIBE <urn:x>",
        ] {
            assert_eq!(clamp(q, 10_000), q, "non-SELECT must not get LIMIT: {q}");
        }
    }

    #[test]
    fn limit_injected_for_prefix_led_select() {
        // Better-than-Python (audit finding 3): a leading PREFIX no longer lets a
        // SELECT bypass the clamp — LIMIT is injected exactly once.
        let q = "PREFIX ex: <urn:x#>\nSELECT ?s WHERE { ?s ?p ?o }";
        let out = clamp(q, 10_000);
        assert!(
            out.contains("LIMIT 10000"),
            "expected injected LIMIT: {out}"
        );
        assert_eq!(out.matches("LIMIT").count(), 1, "exactly once: {out}");
    }

    #[test]
    fn limit_not_injected_for_prefix_led_select_with_existing_limit() {
        let q = "PREFIX ex: <urn:x#>\nSELECT ?s WHERE { ?s ?p ?o } LIMIT 5";
        assert_eq!(clamp(q, 10_000), q, "explicit LIMIT must be left untouched");
    }

    #[test]
    fn limit_injected_for_base_and_prefix_chain() {
        // A BASE + multiple PREFIX + comment prologue is consumed; the effective
        // leading keyword is SELECT, so the clamp fires.
        let q = "BASE <urn:base#>\n# a comment\nPREFIX ex: <urn:x#>\nPREFIX : <urn:y#>\nSELECT ?s WHERE { ?s ?p ?o }";
        let out = clamp(q, 10_000);
        assert!(
            out.contains("LIMIT 10000"),
            "expected injected LIMIT: {out}"
        );
        assert_eq!(out.matches("LIMIT").count(), 1, "exactly once: {out}");
    }

    #[test]
    fn limit_not_injected_for_prefix_led_non_select() {
        // ASK/CONSTRUCT/DESCRIBE keep their no-LIMIT behaviour even behind a
        // PREFIX prologue (their first effective keyword is not SELECT).
        for q in [
            "PREFIX ex: <urn:x#>\nASK { ?s ?p ?o }",
            "PREFIX ex: <urn:x#>\nCONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
            "PREFIX ex: <urn:x#>\nDESCRIBE <urn:x>",
        ] {
            assert_eq!(clamp(q, 10_000), q, "non-SELECT must not get LIMIT: {q}");
        }
    }

    #[test]
    fn limit_injection_strips_trailing_semicolon() {
        let out = clamp("SELECT ?s WHERE { ?s ?p ?o } ;", 10_000);
        assert!(out.trim_end().ends_with("LIMIT 10000"), "{out}");
        assert!(
            !out.contains(';'),
            "trailing semicolon must be stripped: {out}"
        );
    }

    #[tokio::test]
    async fn row_cap_sets_truncated() {
        let ttl = "@prefix ex: <urn:ex:> .\n\
                   ex:a ex:p ex:1 .\nex:a ex:p ex:2 .\nex:a ex:p ex:3 .\n\
                   ex:a ex:p ex:4 .\nex:a ex:p ex:5 .\n";
        let store = store_from_turtle(ttl, 10_000, 2);
        let res = store.query("SELECT ?o WHERE { ?s ?p ?o }").await.unwrap();
        assert_eq!(res.rows.len(), 2, "row cap keeps exactly max_rows");
        assert!(res.truncated, "over-cap result must set truncated");
    }

    #[tokio::test]
    async fn injected_limit_actually_caps_results() {
        let ttl = "@prefix ex: <urn:ex:> .\n\
                   ex:a ex:p ex:1 .\nex:a ex:p ex:2 .\nex:a ex:p ex:3 .\n\
                   ex:a ex:p ex:4 .\nex:a ex:p ex:5 .\n";
        // default_limit=3, high max_rows: the injected LIMIT (not the row cap)
        // bounds the result and truncated stays false.
        let store = store_from_turtle(ttl, 3, 10_000);
        let res = store.query("SELECT ?o WHERE { ?s ?p ?o }").await.unwrap();
        assert_eq!(res.rows.len(), 3, "injected LIMIT caps the result");
        assert!(!res.truncated, "a LIMIT-bounded result is not truncated");
    }

    #[tokio::test]
    async fn ask_returns_boolean() {
        let ttl = "@prefix ex: <urn:ex:> .\nex:a ex:p ex:b .\n";
        let store = store_from_turtle(ttl, 10_000, 10_000);
        let yes = store.query("ASK { ?s ?p ?o }").await.unwrap();
        assert_eq!(yes.boolean, Some(true));
        let no = store.query("ASK { <urn:ex:absent> ?p ?o }").await.unwrap();
        assert_eq!(no.boolean, Some(false));
    }

    #[tokio::test]
    async fn write_query_is_bad_query_not_executed() {
        let store = store_from_turtle(
            "@prefix ex: <urn:ex:> .\nex:a ex:p ex:b .\n",
            10_000,
            10_000,
        );
        let err = store
            .query("INSERT DATA { <urn:ex:x> <urn:ex:p> <urn:ex:y> }")
            .await
            .unwrap_err();
        assert!(matches!(err, LoomError::BadQuery(_)));
    }

    #[tokio::test]
    async fn unavailable_store_fails_open() {
        let store = OxigraphStore::load("/nonexistent-loom-data-dir");
        assert!(!store.status().available);
        let err = store
            .query("SELECT * WHERE { ?s ?p ?o }")
            .await
            .unwrap_err();
        assert!(matches!(err, LoomError::GraphUnavailable(_)));
    }

    #[test]
    fn term_str_strips_decoration() {
        use oxigraph::model::{Literal, NamedNode};
        let iri: Term = NamedNode::new("urn:ngm:class:kg").unwrap().into();
        assert_eq!(term_str(&iri), "urn:ngm:class:kg");
        let plain: Term = Literal::new_simple_literal("Knowledge Graph").into();
        assert_eq!(term_str(&plain), "Knowledge Graph");
        let typed: Term = Literal::new_typed_literal(
            "42",
            NamedNode::new("http://www.w3.org/2001/XMLSchema#integer").unwrap(),
        )
        .into();
        assert_eq!(term_str(&typed), "42");
        let lang: Term = Literal::new_language_tagged_literal("Graphe", "fr")
            .unwrap()
            .into();
        assert_eq!(term_str(&lang), "Graphe");
    }
}
