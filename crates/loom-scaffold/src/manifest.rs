//! Session-manifest assembly (ADR-140 D2) over the already-loaded
//! `ScaffoldIndex`. Pure: one pass over the class entries, no I/O, no network.
//!
//! Everything here is a statement ABOUT the corpus, never a piece OF it. The
//! salience list carries addresses (`Iri` + title + domain + quality) so an
//! agent can decide what to fetch; the content still comes only from `resolve`
//! through the gate. That is Invariant I-P1 on the agentic plane.
//!
//! Cost note: the profile pass is O(classes) and runs per manifest request. On
//! the live corpus (8,146 classes) that is a few hundred microseconds — cheaper
//! than the retrieval it precedes, and it keeps the manifest honest about a
//! hot-reloaded generation rather than caching a stale summary.

use std::collections::BTreeMap;

use loom_domain::{
    CorpusProfile, CoverageReport, DomainFamily, Iri, Manifest, SalientTerm, ToolDescriptor,
};

use crate::index::ScaffoldIndex;

/// The P0 tool contract. Descriptions become exposure-profile data under
/// ADR-140 D7; until a profile store exists they live here as the conservative
/// default the manifest reports.
#[must_use]
pub fn default_tools() -> Vec<ToolDescriptor> {
    [
        ("loom.manifest", "The session index: corpus profile, grounding coverage, salient addresses and this tool contract. Call once at session start; it is O(1) in corpus size."),
        ("loom.browse", "Find candidate addresses for a query. Returns IRIs and scores, never content — follow with loom.resolve to read anything."),
        ("loom.resolve", "Read one or more IRIs as their canonical, human-reviewed markdown blocks, through the same confidence gate the injection plane uses. This is the only tool that returns corpus content."),
        ("loom.sparql", "Read-only SPARQL over the Whelk-reasoned closure. Clamped: read forms only, an enforced LIMIT."),
        ("loom.neighbours", "Typed neighbours of an IRI in the reasoned graph."),
        ("loom.paths", "Shortest typed paths between two IRIs."),
    ]
    .into_iter()
    .map(|(name, description)| ToolDescriptor {
        name: name.to_owned(),
        description: description.to_owned(),
    })
    .collect()
}

/// One pass over the index: domain histogram, maturity histogram, mean quality.
#[must_use]
pub fn profile(index: &ScaffoldIndex) -> CorpusProfile {
    let mut by_domain: BTreeMap<String, (usize, f64, usize)> = BTreeMap::new();
    let mut maturity: BTreeMap<String, usize> = BTreeMap::new();
    let mut quality_total = 0.0_f64;
    let mut quality_n = 0_usize;

    for entry in index.classes.values() {
        let domain = entry
            .dom
            .clone()
            .unwrap_or_else(|| "unclassified".to_owned());
        let slot = by_domain.entry(domain).or_insert((0, 0.0, 0));
        slot.0 += 1;
        if let Some(q) = entry.q {
            slot.1 += q;
            slot.2 += 1;
            quality_total += q;
            quality_n += 1;
        }
        if let Some(m) = &entry.m {
            *maturity.entry(m.clone()).or_insert(0) += 1;
        }
    }

    let mut domains: Vec<DomainFamily> = by_domain
        .into_iter()
        .map(|(name, (classes, q_total, q_n))| DomainFamily {
            name,
            classes,
            mean_quality: mean(q_total, q_n),
        })
        .collect();
    // Largest first: the agent reads this top-down to decide where to look.
    domains.sort_by(|a, b| b.classes.cmp(&a.classes).then_with(|| a.name.cmp(&b.name)));

    CorpusProfile {
        classes: index.class_count(),
        domains,
        maturity: maturity.into_iter().collect(),
        mean_quality: mean(quality_total, quality_n),
    }
}

/// The bounded address list: the highest-quality entries, spread across domains
/// so a single well-curated family cannot monopolise the manifest.
///
/// Spreading matters more than raw ranking here. A pure top-`n`-by-quality list
/// would tell an agent that the corpus is entirely about whatever domain was
/// curated hardest, which is exactly the wrong first impression to give.
#[must_use]
pub fn salient(index: &ScaffoldIndex, limit: usize) -> Vec<SalientTerm> {
    if limit == 0 {
        return Vec::new();
    }
    let mut by_domain: BTreeMap<&str, Vec<SalientTerm>> = BTreeMap::new();
    for (slug, entry) in &index.classes {
        let domain = entry.dom.as_deref().unwrap_or("unclassified");
        by_domain.entry(domain).or_default().push(SalientTerm {
            iri: Iri::from_slug(slug),
            title: entry.t.clone().unwrap_or_else(|| slug.clone()),
            domain: entry.dom.clone(),
            quality: entry.q,
        });
    }
    for terms in by_domain.values_mut() {
        terms.sort_by(|a, b| {
            b.quality
                .unwrap_or(0.0)
                .partial_cmp(&a.quality.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
        });
    }

    // Round-robin across domains until the budget is spent.
    let mut out = Vec::with_capacity(limit);
    let mut round = 0_usize;
    loop {
        let mut took_any = false;
        for terms in by_domain.values() {
            if let Some(t) = terms.get(round) {
                out.push(t.clone());
                took_any = true;
                if out.len() == limit {
                    return out;
                }
            }
        }
        if !took_any {
            return out;
        }
        round += 1;
    }
}

/// Grounding coverage in OKF vocabulary (ADR-141): how many terms carry an
/// attested computation, and how many have been verified at each trust tier.
///
/// The two verification tiers are counted independently, not as a ranking: a
/// term verified by both a build process and a human counts in both, because
/// "a machine checked it" and "a human signed it" are separate facts and a
/// consumer may care about either.
///
/// Zeroes are reported rather than hidden — a consumer must be able to
/// distinguish "no attested computations exist yet" from "this IRI has none".
#[must_use]
pub fn coverage(index: &ScaffoldIndex) -> CoverageReport {
    let mut report = CoverageReport::terms_only(index.class_count());
    for entry in index.classes.values() {
        if !entry.ac.is_empty() {
            report.attested_computations += 1;
        }
        if entry.verified.iter().any(|a| a.by.is_machine()) {
            report.verified_machine += 1;
        }
        if entry.verified.iter().any(|a| a.by.is_human()) {
            report.verified_human += 1;
        }
    }
    report
}

/// Assemble the whole manifest.
#[must_use]
pub fn build(
    index: &ScaffoldIndex,
    generation: loom_domain::Generation,
    salience: usize,
    degraded: &[String],
) -> Manifest {
    Manifest {
        generation,
        profile: profile(index),
        coverage: coverage(index),
        salient: salient(index, salience),
        tools: default_tools(),
        degraded: degraded.to_vec(),
    }
}

fn mean(total: f64, n: usize) -> f64 {
    if n == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        {
            total / n as f64
        }
    }
}
