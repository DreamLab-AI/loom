//! EXP-004 integration — adapter against a real oxigraph store loaded from a
//! fixture written by the test (tempdir). Proves:
//!   * the allowlist loads ONLY ontology.ttl + ontology-inferred.ttl (a third
//!     .ttl in the same dir is never loaded — DDD BC24 I11),
//!   * a relationship-pattern SPARQL spanning both files returns typed rows,
//!   * an aggregation (COUNT) returns a non-zero count,
//!   * label search returns hits with the matched predicate.
//!
//! Plus an `#[ignore]`d smoke test over the repo's real 282k-triple generation
//! at `app/data/` when it is present.

use std::fs;
use std::path::PathBuf;

use loom_domain::GraphStore;
use loom_graph_oxigraph::OxigraphStore;

const ONTOLOGY_TTL: &str = "\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ngm: <https://narrativegoldmine.com/ns/v1#> .
@prefix ex: <urn:ngm:class:> .

ex:knowledge-graph rdfs:label \"Knowledge Graph\" ;
    ngm:title \"Knowledge Graph\" ;
    rdfs:subClassOf ex:graph .
ex:graph rdfs:label \"Graph\" ;
    skos:prefLabel \"Graph Structure\" .
ex:rgb-protocol rdfs:label \"RGB Protocol\" ;
    rdfs:subClassOf ex:protocol .
ex:protocol rdfs:label \"Protocol\" .
";

// The reasoned closure adds an inferred subClassOf edge present in NEITHER the
// asserted file NOR the forbidden working graph.
const ONTOLOGY_INFERRED_TTL: &str = "\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <urn:ngm:class:> .

ex:knowledge-graph rdfs:subClassOf ex:protocol .
";

// A third file in the same dir — the working graph. It must NEVER be loaded.
const WORKING_GRAPH_TTL: &str = "\
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <urn:ngm:class:> .

ex:secret-private-note rdfs:label \"PRIVATE WORKING NOTE\" .
";

fn fixture_store() -> (tempfile::TempDir, OxigraphStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("ontology.ttl"), ONTOLOGY_TTL).unwrap();
    fs::write(
        dir.path().join("ontology-inferred.ttl"),
        ONTOLOGY_INFERRED_TTL,
    )
    .unwrap();
    fs::write(dir.path().join("working-graph.ttl"), WORKING_GRAPH_TTL).unwrap();
    let store = OxigraphStore::load(dir.path());
    (dir, store)
}

#[tokio::test]
async fn loads_only_the_allowlist() {
    let (_dir, store) = fixture_store();
    let status = store.status();
    assert!(status.available, "fixture store should be available");
    assert_eq!(
        status.loaded_files,
        vec![
            "ontology.ttl".to_owned(),
            "ontology-inferred.ttl".to_owned()
        ],
        "only the two published-ontology artifacts load, in allowlist order"
    );
    assert!(
        !status.loaded_files.iter().any(|f| f == "working-graph.ttl"),
        "the working graph must never appear in loaded_files"
    );
}

#[tokio::test]
async fn working_graph_triples_are_absent() {
    let (_dir, store) = fixture_store();
    // The forbidden file's subject resolves to nothing — proof it never loaded.
    let ask = store
        .query("ASK { <urn:ngm:class:secret-private-note> ?p ?o }")
        .await
        .unwrap();
    assert_eq!(
        ask.boolean,
        Some(false),
        "working-graph.ttl triple must not be queryable"
    );
    let hits = store
        .search_labels("private working note", 10)
        .await
        .unwrap();
    assert!(
        hits.is_empty(),
        "working-graph label must not be searchable"
    );
}

#[tokio::test]
async fn relationship_pattern_returns_typed_rows() {
    let (_dir, store) = fixture_store();
    let res = store
        .query(
            "SELECT ?child ?parent WHERE { \
             ?child <http://www.w3.org/2000/01/rdf-schema#subClassOf> ?parent }",
        )
        .await
        .unwrap();
    assert_eq!(res.columns, vec!["child".to_owned(), "parent".to_owned()]);
    assert!(!res.rows.is_empty(), "subClassOf pattern must return rows");
    // The inferred edge (only in ontology-inferred.ttl) must be present, proving
    // both allowlisted files loaded into one store.
    let has_inferred = res
        .rows
        .iter()
        .any(|r| r[0] == "urn:ngm:class:knowledge-graph" && r[1] == "urn:ngm:class:protocol");
    assert!(
        has_inferred,
        "the reasoned-closure edge must be queryable: {:?}",
        res.rows
    );
}

#[tokio::test]
async fn aggregation_count_is_nonzero() {
    let (_dir, store) = fixture_store();
    let res = store
        .query(
            "SELECT (COUNT(?child) AS ?n) WHERE { \
             ?child <http://www.w3.org/2000/01/rdf-schema#subClassOf> ?parent }",
        )
        .await
        .unwrap();
    assert_eq!(res.rows.len(), 1, "COUNT yields a single row");
    let n: usize = res.rows[0][0].parse().expect("COUNT value parses");
    assert!(n >= 3, "expected at least the 3 subClassOf edges, got {n}");
}

#[tokio::test]
async fn label_search_returns_hits_with_predicate() {
    let (_dir, store) = fixture_store();
    let hits = store.search_labels("graph", 10).await.unwrap();
    assert!(!hits.is_empty(), "'graph' must match labels");
    // Every hit resolves to an addressable IRI and records which predicate matched.
    for hit in &hits {
        assert!(
            hit.iri.as_str().starts_with("urn:ngm:class:"),
            "{:?}",
            hit.iri
        );
        assert!(
            matches!(
                hit.predicate.as_str(),
                "http://www.w3.org/2000/01/rdf-schema#label"
                    | "http://www.w3.org/2004/02/skos/core#prefLabel"
                    | "https://narrativegoldmine.com/ns/v1#title"
            ),
            "unexpected predicate: {}",
            hit.predicate
        );
    }
    assert!(
        hits.iter()
            .any(|h| h.iri.as_str() == "urn:ngm:class:knowledge-graph"),
        "knowledge-graph should surface for 'graph'"
    );
}

/// Real-data smoke test over the repo's mirrored generation. Ignored by default
/// (it needs the ~282k-triple artifacts and takes seconds to load); run with
/// `cargo test -p loom-graph-oxigraph -- --ignored`.
#[tokio::test]
#[ignore = "requires app/data/ontology*.ttl (282k triples)"]
async fn real_data_smoke() {
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/data");
    if !data_dir.join("ontology.ttl").exists() {
        eprintln!("skipping: {} has no ontology.ttl", data_dir.display());
        return;
    }
    let store = OxigraphStore::load(&data_dir);
    let status = store.status();
    assert!(
        status.available,
        "real data should load: {:?}",
        status.error
    );
    assert!(
        status.triples > 100_000,
        "expected 282k triples, got {}",
        status.triples
    );
    let hits = store.search_labels("protocol", 20).await.unwrap();
    assert!(
        !hits.is_empty(),
        "'protocol' should return label hits over real data"
    );
}
