//! EXP-010 — the lexical `match()` performance gate. Builds the 8k-class
//! synthetic index (port of the Python self-test generator) and benches
//! `match_seeds`. The <50ms bar is also asserted in a `#[test]` (see
//! `src/tests.rs::match_8k_under_50ms_p99`) so CI catches regressions without a
//! bench run; this bench gives the release-build p99 headroom number.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use indexmap::IndexMap;

use loom_scaffold::index::{slugify, ClassEntry, RawIndex, ScaffoldIndex};
use loom_scaffold::match_::match_seeds;

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

fn title_case(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn build_big_index(n: usize) -> ScaffoldIndex {
    let words = [
        "neural", "network", "graph", "vector", "agent", "protocol", "quantum", "edge", "cloud",
        "model", "data", "semantic", "spatial", "audio", "render", "mesh", "token", "stream",
        "policy", "ledger", "cipher", "fabric", "lattice", "kernel",
    ];
    let mut rng = Xor(42);
    let mut classes: IndexMap<String, ClassEntry> = IndexMap::new();
    for i in 0..n {
        let k = 1 + rng.range(3);
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
    ScaffoldIndex::from_raw(RawIndex {
        version: Some(1),
        generated: String::new(),
        classes,
    })
    .expect("big index builds")
}

fn bench_match(c: &mut Criterion) {
    let idx = build_big_index(8000);
    let query = "how does a knowledge graph relate to a neural network model";
    c.bench_function("match_8k", |b| {
        b.iter(|| {
            let seeds = match_seeds(black_box(&idx), black_box(query), 4);
            black_box(seeds);
        });
    });
}

criterion_group!(benches, bench_match);
criterion_main!(benches);
