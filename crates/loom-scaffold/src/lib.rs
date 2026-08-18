//! `loom-scaffold` — the lexical matcher + confidence-gated injection gate, a
//! 1:1 port of `app/ontology_scaffold.py`. PURE, LLM-free, network-free. The
//! serialiser here is THE PRIZE's on-the-wire form; byte-identity with Python on
//! the golden fixture is the correctness bar (EXP-002/003/010).
//!
//! Divergences from `RUST-ARCHITECTURE.md` §5, all resolved in Python's favour
//! (byte-parity is the spec):
//! - Scoring + the gate arithmetic run in `f64` (Python float), not the doc's
//!   `f32` sketch, so ordering and the `int(budget*frac)` truncation are exact.
//! - The serialiser emits relation keys VERBATIM in camelCase (`hasPart`), not
//!   the domain `RelationKind` hyphenated wire form (`has-part`).

#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
// Deliberate numeric-parity casts: scores/budgets cross f64↔usize↔f32 exactly
// where Python's float arithmetic does. The conversions ARE the spec.
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

pub mod exposure;
pub mod index;
pub mod match_;
pub mod policy;
pub mod prose;
pub mod serialise;
pub mod tuning;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use async_trait::async_trait;

use loom_domain::{
    CanonicalUnit, ConceptMatch, CorpusNature, Generation, GenerationId, GenerationSource, Iri,
    LexicalIndex, LoomError, MatchProvenance, Relation, RelationKind, Scaffold, ScaffoldOpts,
};

use crate::index::{est_tokens, ref_to_slug, ScaffoldIndex};
use crate::match_::match_seeds;
use crate::policy::InjectionPolicy;
use crate::prose::{load_prose, ProseIndex};
use crate::serialise::{clamp, section_for};
use crate::tuning::{
    DEFAULT_INDEX_PATH, DEFAULT_PROSE_PATH, ENV_INDEX_VAR, ENV_PROSE_VAR, PROSE_SEEDS,
    SYSTEM_PREAMBLE,
};

// --- the scaffold pipeline (free functions — mirror Python `scaffold`) -------

/// Telemetry mirror of Python's `meta_out`, plus the assembled block.
#[derive(Debug, Clone)]
pub struct ScaffoldOutcome {
    pub block: String,
    pub top_score: f64,
    pub seed_count: usize,
    pub effective_budget: usize,
    pub injected: bool,
}

impl ScaffoldOutcome {
    fn nothing() -> Self {
        Self {
            block: String::new(),
            top_score: 0.0,
            seed_count: 0,
            effective_budget: 0,
            injected: false,
        }
    }
}

/// Assemble the block from an EXPLICIT ordered seed list (`(slug, score)`),
/// applying the gate + clamp. This is the shared core of both `scaffold_block`
/// (lexical) and `LexicalIndex::assemble` (fusion-fed).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn assemble_block(
    idx: &ScaffoldIndex,
    seeds: &[(String, f64)],
    budget_tokens: usize,
    hops: usize,
    prose: bool,
    prose_index: Option<&ProseIndex>,
    policy: &InjectionPolicy,
) -> ScaffoldOutcome {
    if seeds.is_empty() {
        return ScaffoldOutcome::nothing();
    }
    let top_score = seeds[0].1;
    // Confidence gate skipped injection (top below MIN_INJECT_SCORE) ⇒ no block.
    let Some(effective_budget) = policy.effective_budget(top_score, budget_tokens) else {
        return ScaffoldOutcome {
            block: String::new(),
            top_score,
            seed_count: seeds.len(),
            effective_budget: 0,
            injected: false,
        };
    };

    let empty = ProseIndex::new();
    let prose_data: &ProseIndex = if prose {
        prose_index.unwrap_or(&empty)
    } else {
        &empty
    };

    let seed_slugs: HashSet<String> = seeds.iter().map(|(s, _)| s.clone()).collect();
    let sections: Vec<String> = seeds
        .iter()
        .enumerate()
        .map(|(i, (slug, _))| {
            let pe = if i < PROSE_SEEDS {
                prose_data.get(slug)
            } else {
                None
            };
            section_for(idx, slug, &seed_slugs, hops, pe)
        })
        .collect();

    let block = clamp(&sections, effective_budget);
    ScaffoldOutcome {
        block,
        top_score,
        seed_count: seeds.len(),
        effective_budget,
        injected: true,
    }
}

/// Full pipeline: link → seed → expand → serialise → clamp. Mirrors Python
/// `scaffold(prompt, budget_tokens, max_seeds, hops, index, prose, prose_index)`.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn scaffold_block(
    idx: &ScaffoldIndex,
    prompt: &str,
    budget_tokens: usize,
    max_seeds: usize,
    hops: usize,
    prose: bool,
    prose_index: Option<&ProseIndex>,
    policy: &InjectionPolicy,
) -> ScaffoldOutcome {
    let seeds = match_seeds(idx, prompt, max_seeds);
    assemble_block(idx, &seeds, budget_tokens, hops, prose, prose_index, policy)
}

// --- scaffold_messages (OpenAI chat merge) ----------------------------------

/// Extract plain text from an OpenAI message `content` (string or parts list).
#[must_use]
pub fn message_text(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_owned();
    }
    if let Some(parts) = content.as_array() {
        let texts: Vec<&str> = parts
            .iter()
            .filter(|p| p.get("type").and_then(serde_json::Value::as_str) == Some("text"))
            .map(|p| {
                p.get("text")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
            })
            .collect();
        return texts.join(" ");
    }
    String::new()
}

/// Scaffold an OpenAI chat `messages` array from its LAST user message. Returns a
/// NEW array (input untouched). Merges the block into the first system message,
/// else inserts one at position 0. Empty scaffold ⇒ messages returned unchanged.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn scaffold_messages(
    idx: &ScaffoldIndex,
    messages: &[serde_json::Value],
    budget_tokens: usize,
    max_seeds: usize,
    hops: usize,
    prose: bool,
    prose_index: Option<&ProseIndex>,
    policy: &InjectionPolicy,
) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = messages.to_vec();
    let last_user_text = out
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(serde_json::Value::as_str) == Some("user"))
        .map(|m| message_text(m.get("content").unwrap_or(&serde_json::Value::Null)));
    let Some(text) = last_user_text else {
        return out;
    };

    let outcome = scaffold_block(
        idx,
        &text,
        budget_tokens,
        max_seeds,
        hops,
        prose,
        prose_index,
        policy,
    );
    if outcome.block.is_empty() {
        return out;
    }
    let injection = format!("{SYSTEM_PREAMBLE}\n\n{}", outcome.block);

    let sys_pos = out
        .iter()
        .position(|m| m.get("role").and_then(serde_json::Value::as_str) == Some("system"));
    match sys_pos {
        Some(i)
            if out[i]
                .get("content")
                .and_then(serde_json::Value::as_str)
                .is_some() =>
        {
            let existing = out[i]["content"].as_str().unwrap().trim_end().to_owned();
            out[i]["content"] = serde_json::Value::String(format!("{existing}\n\n{injection}"));
        }
        _ => {
            out.insert(
                0,
                serde_json::json!({"role": "system", "content": injection}),
            );
        }
    }
    out
}

// --- RelationKind mapping (camelCase scaffold key → domain kind) -------------

fn relation_kind_from_camel(key: &str) -> RelationKind {
    match key {
        "hasPart" => RelationKind::HasPart,
        "requires" => RelationKind::Requires,
        "enables" => RelationKind::Enables,
        "dependsOn" => RelationKind::DependsOn,
        "implements" => RelationKind::Implements,
        "uses" => RelationKind::Uses,
        "partOf" => RelationKind::PartOf,
        "relatedTo" => RelationKind::RelatedTo,
        "bridgesTo" => RelationKind::BridgesTo,
        "supports" => RelationKind::Supports,
        "standardizedBy" => RelationKind::StandardizedBy,
        "contrastsWith" => RelationKind::ContrastsWith,
        other => RelationKind::Other(other.to_owned()),
    }
}

// --- LexicalRetriever (impls LexicalIndex) ----------------------------------

/// The lexical port: an owned `ScaffoldIndex` + prose + the env-derived gate.
#[derive(Debug)]
pub struct LexicalRetriever {
    index: ScaffoldIndex,
    prose: ProseIndex,
    policy: InjectionPolicy,
    generation: Generation,
}

impl LexicalRetriever {
    /// Build a retriever around an already-parsed index (prose empty, gate from env).
    #[must_use]
    pub fn from_index(index: ScaffoldIndex) -> Self {
        let generation = build_generation(&index);
        Self {
            index,
            prose: ProseIndex::new(),
            policy: InjectionPolicy::from_env(),
            generation,
        }
    }

    /// Parse an index from a JSON string; prose empty.
    pub fn from_json_str(s: &str) -> Result<Self, LoomError> {
        let index = ScaffoldIndex::from_json_str(s).map_err(LoomError::IndexUnavailable)?;
        Ok(Self::from_index(index))
    }

    /// Load from `path` (else `$ONTOLOGY_INDEX`, else the default). Also loads the
    /// prose index from `$ONTOLOGY_PROSE_INDEX`/default (fail-open, empty on miss).
    pub fn load(path: Option<&str>) -> Result<Self, LoomError> {
        let index_path = resolve_index_path(path);
        let text = std::fs::read_to_string(&index_path)
            .map_err(|e| LoomError::IndexUnavailable(format!("{}: {e}", index_path.display())))?;
        let index = ScaffoldIndex::from_json_str(&text).map_err(LoomError::IndexUnavailable)?;
        let prose = load_prose(&resolve_prose_path(None));
        let generation = build_generation(&index);
        Ok(Self {
            index,
            prose,
            policy: InjectionPolicy::from_env(),
            generation,
        })
    }

    /// Attach a loaded prose index (builder-style; enables prose-enriched mode).
    #[must_use]
    pub fn with_prose(mut self, prose: ProseIndex) -> Self {
        self.prose = prose;
        self
    }

    /// Direct access for tests/benches.
    #[must_use]
    pub fn index(&self) -> &ScaffoldIndex {
        &self.index
    }
}

fn build_generation(index: &ScaffoldIndex) -> Generation {
    let generated_at = if index.generated.is_empty() {
        None
    } else {
        Some(index.generated.clone())
    };
    let id = GenerationId(if index.generated.is_empty() {
        "scaffold-index".to_owned()
    } else {
        index.generated.clone()
    });
    Generation {
        id,
        source: GenerationSource::ScaffoldIndex,
        generated_at,
        commit_sha: None,
        promoted_at: None,
        cluster_span_seconds: None,
        artifacts: Vec::new(),
        verified_single_generation: false,
        class_count: Some(index.class_count()),
    }
}

fn resolve_index_path(path: Option<&str>) -> PathBuf {
    let raw = path
        .map(str::to_owned)
        .or_else(|| std::env::var(ENV_INDEX_VAR).ok())
        .unwrap_or_else(|| DEFAULT_INDEX_PATH.to_owned());
    expand_user(&raw)
}

fn resolve_prose_path(path: Option<&str>) -> PathBuf {
    let raw = path
        .map(str::to_owned)
        .or_else(|| std::env::var(ENV_PROSE_VAR).ok())
        .unwrap_or_else(|| DEFAULT_PROSE_PATH.to_owned());
    expand_user(&raw)
}

/// Minimal `os.path.expanduser` — expands a leading `~/` to `$HOME`.
fn expand_user(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(raw)
}

// --- module-level get-or-init singleton -------------------------------------

static GLOBAL: OnceLock<LexicalRetriever> = OnceLock::new();

/// Lazily load and cache the default index (get-or-init). Errors on the FIRST
/// call are returned; a poisoned first init leaves the cell empty for a retry.
pub fn global_index() -> Result<&'static LexicalRetriever, LoomError> {
    if let Some(r) = GLOBAL.get() {
        return Ok(r);
    }
    let retriever = LexicalRetriever::load(None)?;
    Ok(GLOBAL.get_or_init(|| retriever))
}

#[async_trait]
impl LexicalIndex for LexicalRetriever {
    async fn seeds(&self, query: &str, max_seeds: usize) -> Result<Vec<ConceptMatch>, LoomError> {
        let raw = match_seeds(&self.index, query, max_seeds);
        Ok(raw
            .into_iter()
            .map(|(slug, score)| ConceptMatch {
                iri: Iri::from_slug(&slug),
                #[allow(clippy::cast_possible_truncation)]
                score: score as f32,
                provenance: MatchProvenance::Lexical,
            })
            .collect())
    }

    async fn assemble(
        &self,
        _query: &str,
        candidates: &[ConceptMatch],
        opts: ScaffoldOpts,
    ) -> Result<Scaffold, LoomError> {
        let seeds: Vec<(String, f64)> = candidates
            .iter()
            .map(|c| (c.iri.slug().to_owned(), f64::from(c.score)))
            .collect();
        let outcome = assemble_block(
            &self.index,
            &seeds,
            opts.budget_tokens,
            opts.hops,
            opts.prose,
            Some(&self.prose),
            &self.policy,
        );
        let engaged = !outcome.block.is_empty();
        let approx_tokens = if engaged {
            est_tokens(&outcome.block)
        } else {
            0
        };
        #[allow(clippy::cast_possible_truncation)]
        let top_score = outcome.top_score as f32;
        Ok(Scaffold {
            block: outcome.block,
            engaged,
            approx_tokens,
            seeds: candidates.to_vec(),
            top_score,
            effective_budget: outcome.effective_budget,
            fusion_path: opts.path,
            generation: self.generation.id.clone(),
        })
    }

    fn resolve(&self, iri: &Iri) -> Option<CanonicalUnit> {
        let slug = iri.slug();
        let e = self.index.get(slug)?;
        let prose_entry = self.prose.get(slug);
        let relations: Vec<Relation> = e
            .rel
            .iter()
            .filter(|(_, targets)| !targets.is_empty())
            .map(|(k, targets)| Relation {
                predicate: relation_kind_from_camel(k),
                targets: targets
                    .iter()
                    .map(|t| Iri::from_slug(&ref_to_slug(t)))
                    .collect(),
            })
            .collect();
        Some(CanonicalUnit {
            iri: Iri::from_slug(slug),
            title: self.index.title_of(slug),
            definition: e.d.clone().unwrap_or_default(),
            dfull: prose_entry.and_then(|p| p.dfull.clone()),
            landscape: prose_entry.and_then(|p| p.cl.clone()),
            domain: e.dom.clone().filter(|s| !s.is_empty()),
            maturity: e.m.clone().filter(|s| !s.is_empty()),
            #[allow(clippy::cast_possible_truncation)]
            quality: e.q.map(|q| q as f32),
            is_a: e
                .sup
                .iter()
                .map(|r| Iri::from_slug(&ref_to_slug(r)))
                .collect(),
            ancestors: e
                .isup
                .iter()
                .map(|r| Iri::from_slug(&ref_to_slug(r)))
                .collect(),
            relations,
            backlinks: e
                .bl
                .iter()
                .map(|r| Iri::from_slug(&ref_to_slug(r)))
                .collect(),
            corpus_nature: CorpusNature::SyntheticAiGeneratedHumanDirected,
            generation: self.generation.id.clone(),
        })
    }

    fn generation(&self) -> Generation {
        self.generation.clone()
    }

    fn class_count(&self) -> usize {
        self.index.class_count()
    }
}

#[cfg(test)]
mod tests;
