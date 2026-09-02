//! The serialiser — produces THE PRIZE on the wire. `_rel_items`, `_section_for`
//! and `_clamp`, ported line-for-line. The output string IS the `CanonicalUnit`'s
//! served markdown; it must never emit an encoding the Python serialiser would
//! not. Relation-type keys are emitted VERBATIM (camelCase `hasPart`, not the
//! domain's hyphenated `has-part`) — byte-parity beats the domain wire form.

use std::collections::HashSet;

use crate::index::{est_tokens, ref_to_slug, truncate, ClassEntry, ScaffoldIndex};
use crate::prose::ProseEntry;
use crate::tuning::{
    FOOTER, HEADER, ISUP_CAP, NEIGHBOUR_DEFS, NEIGHBOUR_DEF_CHARS, PROSE_CL_CHARS, PROSE_DEF_CHARS,
    REL_CAP, REL_ORDER,
};

/// `_rel_items`: REL_ORDER first (present + non-empty), then the remaining keys
/// sorted, non-empty. Yields `(predicate, targets)` in serialisation order.
fn rel_items(entry: &ClassEntry) -> Vec<(&str, &Vec<String>)> {
    let mut out: Vec<(&str, &Vec<String>)> = Vec::new();
    for rt in REL_ORDER {
        if let Some(targets) = entry.rel.get(rt) {
            if !targets.is_empty() {
                out.push((rt, targets));
            }
        }
    }
    let mut extras: Vec<&String> = entry
        .rel
        .keys()
        .filter(|k| !REL_ORDER.contains(&k.as_str()))
        .collect();
    extras.sort();
    for rt in extras {
        let targets = &entry.rel[rt];
        if !targets.is_empty() {
            out.push((rt.as_str(), targets));
        }
    }
    out
}

/// `_section_for` — one seed's markdown section.
#[must_use]
#[allow(clippy::implicit_hasher)] // callers always pass the std hasher
pub fn section_for(
    idx: &ScaffoldIndex,
    slug: &str,
    seeds: &HashSet<String>,
    hops: usize,
    prose_entry: Option<&ProseEntry>,
) -> String {
    let e = &idx.classes[slug];
    let mut lines: Vec<String> = Vec::new();

    // Head: "## Title" + optional " (dom, maturity: m)".
    let mut meta: Vec<String> = Vec::new();
    if let Some(dom) = e.dom.as_ref().filter(|s| !s.is_empty()) {
        meta.push(dom.clone());
    }
    if let Some(m) = e.m.as_ref().filter(|s| !s.is_empty()) {
        meta.push(format!("maturity: {m}"));
    }
    let title = idx.title_of(slug);
    let head = if meta.is_empty() {
        format!("## {title}")
    } else {
        format!("## {title} ({})", meta.join(", "))
    };
    lines.push(head);

    // Definition: prose dfull (truncated) preferred, else structural d.
    let dfull = prose_entry
        .and_then(|p| p.dfull.as_ref())
        .filter(|s| !s.is_empty());
    if let Some(dfull) = dfull {
        lines.push(truncate(dfull.trim(), PROSE_DEF_CHARS));
    } else if let Some(d) = e.d.as_ref().filter(|s| !s.is_empty()) {
        lines.push(d.trim().to_owned());
    }

    // is-a / ancestors.
    let parents: Vec<String> = e
        .sup
        .iter()
        .map(|r| idx.title_of(&ref_to_slug(r)))
        .collect();
    let ancestors: Vec<String> = e
        .isup
        .iter()
        .take(ISUP_CAP)
        .map(|r| idx.title_of(&ref_to_slug(r)))
        .collect();
    let mut isa_bits: Vec<String> = Vec::new();
    if !parents.is_empty() {
        isa_bits.push(format!("is-a: {}", parents.join(", ")));
    }
    if !ancestors.is_empty() {
        isa_bits.push(format!("ancestors: {}", ancestors.join(", ")));
    }
    if !isa_bits.is_empty() {
        lines.push(isa_bits.join("; "));
    }

    // relations.
    let mut rel_bits: Vec<String> = Vec::new();
    let mut neighbour_order: Vec<String> = Vec::new();
    for (rt, targets) in rel_items(e) {
        let tslugs: Vec<String> = targets
            .iter()
            .take(REL_CAP)
            .map(|t| ref_to_slug(t))
            .collect();
        let titles: Vec<String> = tslugs.iter().map(|t| idx.title_of(t)).collect();
        rel_bits.push(format!("{rt}: {}", titles.join(", ")));
        neighbour_order.extend(tslugs);
    }
    if !rel_bits.is_empty() {
        lines.push(format!("relations: {}", rel_bits.join("; ")));
    }

    // 1-hop neighbour definitions for the top relation targets.
    if hops >= 1 {
        let mut added: HashSet<String> = HashSet::new();
        for n in &neighbour_order {
            if added.len() >= NEIGHBOUR_DEFS {
                break;
            }
            if added.contains(n) || seeds.contains(n) || !idx.contains(n) {
                continue;
            }
            let nd = idx.classes[n].d.as_ref().filter(|s| !s.is_empty());
            let Some(nd) = nd else { continue };
            lines.push(format!(
                "- {}: {}",
                idx.title_of(n),
                truncate(nd.trim(), NEIGHBOUR_DEF_CHARS)
            ));
            added.insert(n.clone());
        }
    }

    // Landscape prose last, so the end-trimming clamp keeps structural facts.
    if let Some(cl) = prose_entry
        .and_then(|p| p.cl.as_ref())
        .filter(|s| !s.is_empty())
    {
        lines.push(format!(
            "landscape: {}",
            truncate(cl.trim(), PROSE_CL_CHARS)
        ));
    }

    lines.join("\n")
}

/// What survived `_clamp`: the block, and HOW MANY leading sections made it.
///
/// The count is what lets the grounding contract report `injected` per seed as
/// a fact rather than an assumption — sections are built one-per-seed in seed
/// order, and the clamp only ever trims from the END, so seeds `0..kept` are
/// exactly the ones that were served.
#[derive(Debug, Clone)]
pub struct Clamped {
    pub text: String,
    pub kept: usize,
}

/// `_clamp` — trim whole sections from the end until the block fits the budget.
/// The returned `text` is byte-identical to the pre-contract `clamp`; only the
/// survivor count is new.
#[must_use]
pub fn clamp(sections: &[String], budget_tokens: usize) -> Clamped {
    let mut kept = sections.len();
    while kept > 0 {
        let body = sections[..kept].join("\n\n");
        let text = format!("{HEADER}\n{body}\n{FOOTER}");
        if est_tokens(&text) <= budget_tokens {
            return Clamped { text, kept };
        }
        kept -= 1;
    }
    Clamped {
        text: String::new(),
        kept: 0,
    }
}
