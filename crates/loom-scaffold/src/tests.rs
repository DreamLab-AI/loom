//! EXP-002 (byte-parity) + EXP-003 (gate math) + EXP-010 (perf gate).
//!
//! The whole Python `_selftest()` is ported here as discrete `#[test]`s against
//! the shared `fixture.json`, plus golden byte-identity tests pinning the exact
//! `[ONTOLOGY CONTEXT]` block against `tests/golden-python/*.txt`.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use loom_domain::{FusionPath, LexicalIndex, ScaffoldOpts};

use crate::index::{ref_to_slug, slugify, ClassEntry, RawIndex, ScaffoldIndex};
use crate::match_::match_seeds;
use crate::policy::InjectionPolicy;
use crate::tuning::{FOOTER, HEADER, SYSTEM_PREAMBLE};
use crate::{
    assemble_block, scaffold_block, scaffold_messages, LexicalRetriever, ScaffoldOutcome,
};

// --- fixtures ----------------------------------------------------------------

/// Read a workspace-relative file (base = `crates/loom-scaffold`, up two levels).
fn workspace_file(rel: &str) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = base.join("../..").join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn fixture_index() -> ScaffoldIndex {
    let json = workspace_file("tests/golden-python/fixture.json");
    ScaffoldIndex::from_json_str(&json).expect("fixture parses")
}

/// Python-default scaffold: budget 1500, max_seeds 4, hops 1, prose off, gate off.
fn scaffold(idx: &ScaffoldIndex, prompt: &str) -> String {
    scaffold_opts(idx, prompt, 1500, 1)
}

fn scaffold_opts(idx: &ScaffoldIndex, prompt: &str, budget: usize, hops: usize) -> String {
    let policy = InjectionPolicy::default();
    scaffold_block(idx, prompt, budget, 4, hops, false, None, &policy).block
}

const DEFAULT_PROMPT: &str = "Explain how a knowledge graph uses a graph database";

// --- tokeniser contract (selftest) ------------------------------------------

#[test]
fn slugify_kebab_case() {
    assert_eq!(slugify("Knowledge  Graph!"), "knowledge-graph");
}

#[test]
fn iri_ref_to_slug() {
    assert_eq!(ref_to_slug("urn:ngm:class:graph-database"), "graph-database");
}

// --- link + seed + expand + serialise (selftest) ----------------------------

#[test]
fn block_has_wrapper() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(block.starts_with(HEADER) && block.ends_with(FOOTER), "{block}");
}

#[test]
fn seed_section_present() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(block.contains("## Knowledge Graph (ai, maturity: mature)"), "{block}");
}

#[test]
fn is_a_line_present() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(block.contains("is-a: Graph; ancestors: Data Structure"), "{block}");
}

#[test]
fn relations_line_present() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(block.contains("uses: Graph Database"), "{block}");
    assert!(block.contains("hasPart: Ontology"), "{block}");
}

#[test]
fn neighbour_def_ontology_present() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(
        block.contains("- Ontology: A formal, explicit specification"),
        "{block}"
    );
}

#[test]
fn neighbour_def_vector_database_present() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(
        block.contains("- Vector Database: A database optimised for similarity search"),
        "{block}"
    );
}

#[test]
fn seed_not_repeated_as_neighbour_def() {
    let block = scaffold(&fixture_index(), DEFAULT_PROMPT);
    assert!(!block.contains("- Graph Database:"), "{block}");
}

#[test]
fn hops0_suppresses_neighbour_defs() {
    let block0 = scaffold_opts(&fixture_index(), DEFAULT_PROMPT, 1500, 0);
    assert!(!block0.contains("- Ontology:"), "{block0}");
    assert!(!block0.contains("- Vector Database:"), "{block0}");
}

#[test]
fn irrelevant_prompt_empty() {
    assert_eq!(scaffold(&fixture_index(), "best sourdough starter recipe"), "");
}

#[test]
fn empty_prompt_empty() {
    assert_eq!(scaffold(&fixture_index(), ""), "");
}

// --- budget clamp (selftest) ------------------------------------------------

#[test]
fn clamp_shrinks_output() {
    let idx = fixture_index();
    let big = scaffold_opts(&idx, DEFAULT_PROMPT, 1500, 1);
    let small = scaffold_opts(&idx, DEFAULT_PROMPT, 60, 1);
    assert!(small.is_empty() || small.len() < big.len());
}

#[test]
fn clamp_respects_budget() {
    let small = scaffold_opts(&fixture_index(), DEFAULT_PROMPT, 60, 1);
    assert!(small.is_empty() || crate::index::est_tokens(&small) <= 60);
}

#[test]
fn impossible_budget_empty() {
    assert_eq!(scaffold_opts(&fixture_index(), DEFAULT_PROMPT, 1, 1), "");
}

// --- scaffold_messages (selftest) -------------------------------------------

fn merge_messages(idx: &ScaffoldIndex, messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let policy = InjectionPolicy::default();
    scaffold_messages(idx, messages, 1500, 4, 1, false, None, &policy)
}

#[test]
fn messages_insert_system_at_zero() {
    let idx = fixture_index();
    let msgs = vec![serde_json::json!({"role": "user", "content": "what is a knowledge graph?"})];
    let out = merge_messages(&idx, &msgs);
    assert_eq!(out[0]["role"], "system");
    assert!(out[0]["content"].as_str().unwrap().contains(HEADER));
    // input not mutated (borrowed slice unchanged).
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["role"], "user");
}

#[test]
fn messages_instruction_preamble_present() {
    let idx = fixture_index();
    let msgs = vec![serde_json::json!({"role": "user", "content": "what is a knowledge graph?"})];
    let out = merge_messages(&idx, &msgs);
    let first_sentence = SYSTEM_PREAMBLE.split('.').next().unwrap();
    assert!(out[0]["content"].as_str().unwrap().contains(first_sentence));
}

#[test]
fn messages_merge_into_existing_system() {
    let idx = fixture_index();
    let msgs = vec![
        serde_json::json!({"role": "system", "content": "You are a benchmark model."}),
        serde_json::json!({"role": "user", "content": "compare a graph database and a vector database"}),
    ];
    let out = merge_messages(&idx, &msgs);
    assert_eq!(out.len(), 2);
    let sys = out[0]["content"].as_str().unwrap();
    assert!(sys.starts_with("You are a benchmark model."), "{sys}");
    assert!(sys.contains(HEADER), "{sys}");
}

#[test]
fn messages_parts_content_last_user() {
    let idx = fixture_index();
    let msgs = vec![
        serde_json::json!({"role": "user", "content": "unrelated earlier turn about weather"}),
        serde_json::json!({"role": "assistant", "content": "sure"}),
        serde_json::json!({"role": "user", "content": [{"type": "text", "text": "define ontology"}]}),
    ];
    let out = merge_messages(&idx, &msgs);
    assert_eq!(out[0]["role"], "system");
    assert!(out[0]["content"].as_str().unwrap().contains("## Ontology"));
}

#[test]
fn messages_no_match_unchanged() {
    let idx = fixture_index();
    let msgs = vec![serde_json::json!({"role": "user", "content": "sourdough hydration"})];
    let out = merge_messages(&idx, &msgs);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["role"], "user");
}

// --- EXP-002: golden byte-identity ------------------------------------------

#[test]
fn golden_default_byte_identical() {
    let block = scaffold_opts(&fixture_index(), DEFAULT_PROMPT, 1500, 1);
    let golden = workspace_file("tests/golden-python/golden_default.txt");
    assert_eq!(block, golden, "default scaffold must be byte-identical to Python");
}

#[test]
fn golden_hops0_byte_identical() {
    let block = scaffold_opts(&fixture_index(), DEFAULT_PROMPT, 1500, 0);
    let golden = workspace_file("tests/golden-python/golden_hops0.txt");
    assert_eq!(block, golden, "hops=0 scaffold must be byte-identical to Python");
}

#[test]
fn golden_budget60_empty() {
    let block = scaffold_opts(&fixture_index(), DEFAULT_PROMPT, 60, 1);
    let golden = workspace_file("tests/golden-python/golden_budget60.txt");
    assert_eq!(block, golden);
    assert!(golden.is_empty());
}

#[test]
fn golden_irrelevant_empty() {
    let block = scaffold(&fixture_index(), "best sourdough starter recipe");
    let golden = workspace_file("tests/golden-python/golden_irrelevant.txt");
    assert_eq!(block, golden);
    assert!(golden.is_empty());
}

/// Byte-parity ALSO holds through the async port API (`seeds` → `assemble`),
/// not only the free functions — this is the shape the facade calls.
#[tokio::test]
async fn golden_default_via_port_api() {
    let retriever = LexicalRetriever::from_index(fixture_index());
    let seeds = retriever.seeds(DEFAULT_PROMPT, 4).await.unwrap();
    let opts = ScaffoldOpts {
        budget_tokens: 1500,
        hops: 1,
        prose: false,
        confidence_injection: false,
        max_seeds: 4,
        k_semantic: 5,
        path: FusionPath::LexicalHit,
    };
    let scaffold = retriever.assemble(DEFAULT_PROMPT, &seeds, opts).await.unwrap();
    let golden = workspace_file("tests/golden-python/golden_default.txt");
    assert_eq!(scaffold.block, golden);
    assert!(scaffold.engaged);
    assert_eq!(scaffold.fusion_path, FusionPath::LexicalHit);
    assert_eq!(scaffold.seeds.len(), 3);
    // seed order: knowledge-graph, graph-database, graph.
    assert_eq!(scaffold.seeds[0].iri.slug(), "knowledge-graph");
    assert_eq!(scaffold.seeds[1].iri.slug(), "graph-database");
    assert_eq!(scaffold.seeds[2].iri.slug(), "graph");
}

// --- EXP-003: confidence-gate math parity -----------------------------------

fn policy(ci: bool, strong: f64, min: f64, frac: f64) -> InjectionPolicy {
    InjectionPolicy {
        confidence_injection: ci,
        strong_match_score: strong,
        min_inject_score: min,
        min_inject_fraction: frac,
    }
}

#[test]
fn gate_off_returns_full_budget_regardless_of_score() {
    let p = policy(false, 8.0, 2.0, 0.4);
    // injection off ⇒ Python baseline: full budget whenever any seed matched.
    assert_eq!(p.effective_budget(0.0, 1500), Some(1500));
    assert_eq!(p.effective_budget(19.5, 1500), Some(1500));
    assert_eq!(p.effective_budget(2.0, 1500), Some(1500));
}

#[test]
fn gate_on_below_min_skips() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // top below MIN_INJECT_SCORE ⇒ skip injection entirely.
    assert_eq!(p.effective_budget(1.99, 1500), None);
    assert_eq!(p.effective_budget(0.0, 1500), None);
}

#[test]
fn gate_on_at_or_above_strong_full_budget() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // ratio >= 1.0 ⇒ frac clamped to 1.0 ⇒ full budget.
    assert_eq!(p.effective_budget(8.0, 1500), Some(1500));
    assert_eq!(p.effective_budget(19.5, 1500), Some(1500));
}

#[test]
fn gate_on_midrange_scales_budget() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // top=4.0, strong=8.0 ⇒ frac=0.5 (exact) ⇒ 1500*0.5 = 750.
    assert_eq!(p.effective_budget(4.0, 1500), Some(750));
    // top=6.0 ⇒ frac=0.75 (exact) ⇒ 1125.
    assert_eq!(p.effective_budget(6.0, 1500), Some(1125));
}

#[test]
fn gate_on_fraction_floor_clamps() {
    let p = policy(true, 8.0, 2.0, 0.4);
    // top=2.0 (== min) ⇒ ratio 0.25 < 0.4 ⇒ clamp UP to 0.4, not 0.25.
    let eff = p.effective_budget(2.0, 1500).unwrap();
    // identical f64 path to Python's int(1500*0.4); NOT the un-clamped 375.
    assert_eq!(eff, (1500.0_f64 * 0.4) as usize);
    assert_ne!(eff, (1500.0_f64 * 0.25) as usize);
}

#[test]
fn gate_on_strong_zero_avoids_div() {
    let p = policy(true, 0.0, 2.0, 0.4);
    // strong_match_score <= 0 ⇒ frac stays 1.0 (Python guards the divide).
    assert_eq!(p.effective_budget(5.0, 1500), Some(1500));
}

#[test]
fn gate_on_min_one_budget_floor() {
    let p = policy(true, 8.0, 0.1, 0.4);
    // tiny requested budget ⇒ max(1, floor) keeps at least 1 token.
    assert_eq!(p.effective_budget(1.0, 1), Some(1));
    assert_eq!(p.effective_budget(1.0, 2), Some(1)); // int(2*0.4)=0 → max(1,0)=1
}

// --- EXP-010: match() performance gate --------------------------------------

/// Tiny deterministic xorshift — index synthesis only; not a parity surface.
struct Xor(u64);
impl Xor {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, n: usize) -> usize {
        (self.next() % (n as u64)) as usize
    }
}

/// Build an 8k-class synthetic index + the fixture classes (port of the Python
/// self-test generator; shared with the criterion bench).
pub fn build_big_index(n: usize) -> ScaffoldIndex {
    let words = [
        "neural", "network", "graph", "vector", "agent", "protocol", "quantum", "edge", "cloud",
        "model", "data", "semantic", "spatial", "audio", "render", "mesh", "token", "stream",
        "policy", "ledger", "cipher", "fabric", "lattice", "kernel",
    ];
    let mut rng = Xor(42);
    let mut classes: IndexMap<String, ClassEntry> = IndexMap::new();
    for i in 0..n {
        let k = 1 + rng.range(3); // 1..=3 words
        let mut picked: Vec<&str> = Vec::new();
        while picked.len() < k {
            let w = words[rng.range(words.len())];
            if !picked.contains(&w) {
                picked.push(w);
            }
        }
        let titled: Vec<String> = picked.iter().map(|w| title_case(w)).collect();
        let title = format!("{} {i}", titled.join(" "));
        let slug = slugify(&title);
        classes.insert(
            slug,
            ClassEntry {
                t: Some(title),
                d: Some("Synthetic definition ".repeat(5)),
                dom: Some("bench".to_owned()),
                q: Some((rng.range(1000) as f64) / 1000.0),
                m: Some("draft".to_owned()),
                sup: Vec::new(),
                isup: Vec::new(),
                rel: IndexMap::new(),
                bl: Vec::new(),
            },
        );
    }
    // Layer the fixture classes on top (Python: big_classes.update(_FIXTURE)).
    let fixture = fixture_index();
    for (slug, entry) in &fixture.classes {
        classes.insert(slug.clone(), entry.clone());
    }
    ScaffoldIndex::from_raw(RawIndex {
        version: Some(1),
        generated: String::new(),
        classes,
    })
    .expect("big index builds")
}

fn title_case(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

const PERF_QUERY: &str = "how does a knowledge graph relate to a neural network model";

#[test]
fn match_8k_under_50ms_p99() {
    let idx = build_big_index(8000);
    // warm-up (touch caches / branch predictor).
    let _ = match_seeds(&idx, PERF_QUERY, 4);
    let iters = 200;
    let mut samples: Vec<u128> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = std::time::Instant::now();
        let seeds = match_seeds(&idx, PERF_QUERY, 4);
        samples.push(t0.elapsed().as_micros());
        assert!(!seeds.is_empty(), "knowledge-graph must still be surfaced");
    }
    samples.sort_unstable();
    let p99 = samples[(iters * 99) / 100];
    // 50 ms == 50_000 µs. Debug build; the criterion bench is the release gate.
    assert!(p99 < 50_000, "match p99 = {p99} µs (>= 50 ms)");
    eprintln!(
        "match_8k p99 = {p99} µs, median = {} µs (debug build; n={iters})",
        samples[iters / 2]
    );
}

// --- real-index smoke (runs only when data/scaffold-index.json is present) ---

fn data_index_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("data/scaffold-index.json")
}

#[test]
fn real_index_smoke_when_present() {
    let path = data_index_path();
    if !path.exists() {
        eprintln!("skip real-index smoke: {} absent", path.display());
        return;
    }
    let text = std::fs::read_to_string(&path).expect("read real index");
    let idx = ScaffoldIndex::from_json_str(&text).expect("real index parses");
    assert_eq!(idx.class_count(), 8146, "expected the 8,146-class corpus");
    let policy = InjectionPolicy::default();
    let ScaffoldOutcome { block, .. } =
        scaffold_block(&idx, "knowledge graph", 1500, 4, 1, false, None, &policy);
    assert!(!block.is_empty(), "'knowledge graph' must engage the scaffold");
    assert!(block.starts_with(HEADER) && block.ends_with(FOOTER));
}

// --- assemble_block direct (gate + telemetry) -------------------------------

#[test]
fn assemble_block_reports_telemetry() {
    let idx = fixture_index();
    let seeds = match_seeds(&idx, DEFAULT_PROMPT, 4);
    let policy = InjectionPolicy::default();
    let outcome = assemble_block(&idx, &seeds, 1500, 1, false, None, &policy);
    assert!(outcome.injected);
    assert_eq!(outcome.seed_count, 3);
    assert_eq!(outcome.effective_budget, 1500);
    assert!(outcome.top_score > 8.0); // strong exact-title hit
}
