//! `match_` — the linker. Exact n-gram title/slug match + inverted title-word
//! overlap + slug-substring bonus, then the `(-score, -quality, slug)` tri-key
//! sort. Ported branch-for-branch from `ScaffoldIndex.match`. Scores accumulate
//! in `f64` (Python float) so the ordering is bit-identical.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::index::{find_words, ScaffoldIndex};
use crate::tuning::{
    EXACT_TITLE_WEIGHT, MAX_NGRAM, MIN_SEED_SCORE, OVERLAP_WEIGHT, STOPWORDS, SUBSTRING_MIN_LEN,
    SUBSTRING_WEIGHT,
};

fn is_stopword(w: &str) -> bool {
    STOPWORDS.contains(&w)
}

/// Score classes against the prompt; return `(slug, score)` seeds above the
/// gate, sorted score desc / quality desc / slug asc, truncated to `max_seeds`.
#[must_use]
pub fn match_seeds(idx: &ScaffoldIndex, prompt: &str, max_seeds: usize) -> Vec<(String, f64)> {
    let raw_words = find_words(prompt);
    if raw_words.is_empty() {
        return Vec::new();
    }
    let terms: Vec<&str> = raw_words
        .iter()
        .map(String::as_str)
        .filter(|w| !is_stopword(w) && w.len() >= 2)
        .collect();

    let mut scores: HashMap<String, f64> = HashMap::new();

    // 1. Exact title / slug match on n-grams of the raw word sequence.
    let n_words = raw_words.len();
    let max_n = MAX_NGRAM.min(n_words);
    for n in (1..=max_n).rev() {
        for i in 0..=(n_words - n) {
            let gram = &raw_words[i..i + n];
            if n == 1 && (is_stopword(&gram[0]) || gram[0].len() < 2) {
                continue;
            }
            let gs = gram.join("-");
            let slug: Option<String> = match idx.by_title(&gs) {
                Some(s) => Some(s.to_owned()),
                None if idx.contains(&gs) => Some(gs.clone()),
                None => None,
            };
            if let Some(slug) = slug {
                *scores.entry(slug).or_insert(0.0) += EXACT_TITLE_WEIGHT * (n as f64);
            }
        }
    }

    // 2. Title-word overlap via the inverted index (dedup terms).
    let term_set: HashSet<&str> = terms.iter().copied().collect();
    for w in &term_set {
        if let Some(slugset) = idx.inverted(w) {
            for slug in slugset {
                let inc = OVERLAP_WEIGHT / (idx.title_len(slug) as f64);
                *scores.entry(slug.clone()).or_insert(0.0) += inc;
            }
        }
    }

    // 3. Slug-substring bonus for longer terms (dedup, len >= SUBSTRING_MIN_LEN).
    let long_terms: HashSet<&str> = terms
        .iter()
        .copied()
        .filter(|t| t.len() >= SUBSTRING_MIN_LEN)
        .collect();
    for w in &long_terms {
        for slug in idx.slugs() {
            if slug.contains(w) {
                *scores.entry(slug.clone()).or_insert(0.0) += SUBSTRING_WEIGHT;
            }
        }
    }

    // Gate + tri-key sort.
    let mut seeds: Vec<(String, f64)> = scores
        .into_iter()
        .filter(|&(_, sc)| sc >= MIN_SEED_SCORE)
        .collect();
    seeds.sort_by(|a, b| {
        let qa = idx.get(&a.0).and_then(|e| e.q).unwrap_or(0.0);
        let qb = idx.get(&b.0).and_then(|e| e.q).unwrap_or(0.0);
        // -score (desc), then -quality (desc), then slug asc.
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| qb.partial_cmp(&qa).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.0.cmp(&b.0))
    });
    seeds.truncate(max_seeds);
    seeds
}
