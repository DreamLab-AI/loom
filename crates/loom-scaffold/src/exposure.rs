//! Exposure telemetry matcher (F2) — a pure port of the paper's deterministic
//! title matcher (`tools/paper/decompose_exposure.py` `normalise`/`gold_hit`,
//! itself copied verbatim from `bench_ontology_uplift.py`). Semantic parity, not
//! byte parity, is the bar (the mission's instruction): same normalisation, same
//! substring-or-≥80%-word-overlap decision.
//!
//! The paper measures a *copy-fidelity deficit* — models drop ~1 in 14 exposed
//! items. This module makes that observable per request: given the titles served
//! in the scaffold and the model's answer, it reports how many titles were
//! restated and which were dropped. O(targets × answer) as required.
//!
//! Python reference:
//! ```python
//! _PUNCT_RE = re.compile(r"[^a-z0-9\s]+")
//! _WS_RE = re.compile(r"\s+")
//! def normalise(s): return _WS_RE.sub(" ", _PUNCT_RE.sub(" ", s.lower())).strip()
//! def gold_hit(title, norm_answer, answer_words):
//!     nt = normalise(title)
//!     if not nt: return False
//!     if nt in norm_answer: return True
//!     words = [w for w in nt.split() if len(w) >= 4]
//!     if not words: return False
//!     return sum(1 for w in words if w in answer_words) / len(words) >= 0.8
//! ```

use std::collections::HashSet;

use loom_domain::ExposureReport;

/// Default cap on the `dropped` list carried in telemetry (mission: ≤ 12).
pub const DROPPED_CAP: usize = 12;

/// `normalise` — lowercase, replace every run of non-`[a-z0-9\s]` with a single
/// space (Python replaces each maximal non-class run with one space, since the
/// regex is greedy with `+`), collapse whitespace, trim. ASCII-lowercasing
/// matches Python `str.lower()` for the ASCII range the titles live in; any
/// non-ASCII char is a non-`[a-z0-9\s]` byte and becomes a space either way.
#[must_use]
pub fn normalise(s: &str) -> String {
    let lowered = s.to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut pending_space = false;
    let mut started = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_space && started {
                out.push(' ');
            }
            out.push(ch);
            pending_space = false;
            started = true;
        } else {
            // Any non-`[a-z0-9]` char — whitespace (`\s`) OR other punctuation
            // (`[^a-z0-9\s]`) — is a separator; Python collapses both to one space
            // after the two chained `.sub(" ", …)` passes. Coalesce here.
            pending_space = true;
        }
    }
    out
}

/// `gold_hit` — is `title` present in the already-normalised `text`? Substring
/// hit, else ≥ 80% of the title's length-≥4 words present as whole words in
/// `text_words`. `text_words` MUST be the split of the same `norm_text`.
#[must_use]
#[allow(clippy::implicit_hasher)] // callers always pass the std hasher
pub fn title_hit(title: &str, norm_text: &str, text_words: &HashSet<&str>) -> bool {
    let nt = normalise(title);
    if nt.is_empty() {
        return false;
    }
    if norm_text.contains(&nt) {
        return true;
    }
    let words: Vec<&str> = nt.split(' ').filter(|w| w.len() >= 4).collect();
    if words.is_empty() {
        return false;
    }
    let present = words.iter().filter(|w| text_words.contains(**w)).count();
    // Python: present / len(words) >= 0.8 (float division).
    (present as f64) / (words.len() as f64) >= 0.8
}

/// Build the per-request `ExposureReport`. `candidate_titles` is the superset of
/// titles that MIGHT have been served (class titles + relation-target titles);
/// only those actually present in `block` (survived the budget clamp) count as
/// exposure `targets`, matched with the SAME matcher used against the answer — so
/// the exposure denominator is exactly "what the model saw", robustly, without
/// parsing the markdown. `delivered` are targets restated in `answer`; `dropped`
/// are targets omitted, de-duplicated, order-preserved, capped at `cap`.
#[must_use]
pub fn exposure_report(
    candidate_titles: &[String],
    block: &str,
    answer: &str,
    cap: usize,
) -> ExposureReport {
    let norm_block = normalise(block);
    let block_words: HashSet<&str> = norm_block.split(' ').filter(|w| !w.is_empty()).collect();
    let norm_answer = normalise(answer);
    let answer_words: HashSet<&str> = norm_answer.split(' ').filter(|w| !w.is_empty()).collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut targets = 0usize;
    let mut delivered = 0usize;
    let mut dropped: Vec<String> = Vec::new();

    for title in candidate_titles {
        // Normalise-dedupe so "Knowledge Graph" served once is counted once even
        // if it appears as both a class title and a relation target.
        let key = normalise(title);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        // Exposure: did this title survive into the served block?
        if !title_hit(title, &norm_block, &block_words) {
            continue;
        }
        seen.insert(key);
        targets += 1;
        if title_hit(title, &norm_answer, &answer_words) {
            delivered += 1;
        } else if dropped.len() < cap {
            dropped.push(title.clone());
        }
    }

    ExposureReport {
        targets,
        delivered,
        dropped,
    }
}

/// Format the optional "Not covered above" line (`LOOM_EXPOSURE_APPEND`). Returns
/// `None` when nothing was dropped (never append a noise line).
#[must_use]
pub fn not_covered_line(report: &ExposureReport) -> Option<String> {
    if report.dropped.is_empty() {
        return None;
    }
    Some(format!("Not covered above: {}.", report.dropped.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_matches_python_semantics() {
        assert_eq!(normalise("Knowledge Graph"), "knowledge graph");
        // Punctuation runs collapse to a single space; trim edges.
        assert_eq!(normalise("  RGB-Protocol!!  "), "rgb protocol");
        assert_eq!(normalise("A/B  testing"), "a b testing");
        // Non-ASCII becomes a separator (matches `[^a-z0-9\s]`).
        assert_eq!(normalise("café"), "caf");
        assert_eq!(normalise(""), "");
    }

    fn words(norm: &str) -> HashSet<&str> {
        norm.split(' ').filter(|w| !w.is_empty()).collect()
    }

    #[test]
    fn title_hit_substring_and_word_overlap() {
        let norm = normalise("the knowledge graph stores typed edges");
        let w = words(&norm);
        // Exact substring.
        assert!(title_hit("Knowledge Graph", &norm, &w));
        // Word-overlap ≥ 0.8: 2/2 words present though not contiguous.
        let norm2 = normalise("graphs are typed; knowledge is stored");
        let w2 = words(&norm2);
        assert!(title_hit("Knowledge", &norm2, &w2));
        // Below 0.8: only 1 of 3 length-≥4 words present.
        let norm3 = normalise("vector similarity only");
        let w3 = words(&norm3);
        assert!(!title_hit("Graph Database System", &norm3, &w3));
    }

    #[test]
    fn title_hit_short_words_need_substring() {
        // "AI" has no length-≥4 word; only a substring hit counts.
        let norm = normalise("built with ai techniques");
        let w = words(&norm);
        assert!(title_hit("ai", &norm, &w)); // substring "ai" in "ai"
        let norm2 = normalise("machine learning models");
        let w2 = words(&norm2);
        assert!(!title_hit("AI", &norm2, &w2)); // "ai" not a substring, no long words
    }

    #[test]
    fn exposure_counts_only_served_titles() {
        let block = "[ONTOLOGY CONTEXT]\n## Knowledge Graph\nsome def\n## Graph Database\nx\n[END ONTOLOGY CONTEXT]";
        // Candidate includes a title NOT in the block (Vector Database) — filtered.
        let candidates = vec![
            "Knowledge Graph".to_owned(),
            "Graph Database".to_owned(),
            "Vector Database".to_owned(),
        ];
        let answer = "A knowledge graph organises entities.";
        let r = exposure_report(&candidates, block, answer, DROPPED_CAP);
        assert_eq!(r.targets, 2, "only Knowledge Graph + Graph Database served");
        assert_eq!(r.delivered, 1, "answer restated Knowledge Graph only");
        assert_eq!(r.dropped, vec!["Graph Database".to_owned()]);
    }

    #[test]
    fn exposure_dedupes_and_caps() {
        let block = "## Knowledge Graph ## Knowledge Graph"; // duplicated candidate
        let candidates = vec!["Knowledge Graph".to_owned(), "Knowledge Graph".to_owned()];
        let r = exposure_report(&candidates, block, "nothing here", DROPPED_CAP);
        assert_eq!(r.targets, 1, "normalise-dedupe counts once");
        assert_eq!(r.delivered, 0);
        assert_eq!(r.dropped.len(), 1);

        // Cap the dropped list.
        let many: Vec<String> = (0..20).map(|i| format!("Title{i}")).collect();
        let block2 = many
            .iter()
            .map(|t| format!("## {t}"))
            .collect::<Vec<_>>()
            .join(" ");
        let r2 = exposure_report(&many, &block2, "unrelated", 12);
        assert_eq!(r2.targets, 20);
        assert_eq!(r2.dropped.len(), 12, "dropped capped at 12");
    }

    #[test]
    fn not_covered_line_only_on_drops() {
        let mut r = ExposureReport {
            targets: 2,
            delivered: 2,
            dropped: vec![],
        };
        assert!(not_covered_line(&r).is_none());
        r.dropped = vec!["A".to_owned(), "B".to_owned()];
        assert_eq!(not_covered_line(&r).unwrap(), "Not covered above: A, B.");
    }
}
