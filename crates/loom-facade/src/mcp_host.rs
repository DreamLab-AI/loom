//! The façade's [`ToolHost`] implementation — the agentic plane's retrieval,
//! expressed entirely in terms of the ports the HTTP plane already uses
//! (ADR-140 D1).
//!
//! There is deliberately no retrieval *policy* here. `browse` is the lexical
//! port; `resolve` goes through `LexicalIndex::assemble`, which is THE gate;
//! `sparql` is the same clamped `GraphStore::query` the HTTP route calls. The
//! only new logic is bounded traversal (`neighbours`, `paths`), and that is
//! sequencing over `resolve` rather than a new engine — exactly the role
//! `fusion.rs` plays for the injection plane.
//!
//! Every outcome carries the ADR-138 envelope built by
//! [`crate::routes::grounding::envelope`], so an agent on this plane can make
//! the same `corpus_backed` judgement a chat consumer can.

use std::collections::{HashSet, VecDeque};

use async_trait::async_trait;
use serde_json::{json, to_value, Value};

use loom_domain::{
    CanonicalUnit, FusionPath, Grounding, GroundingStatus, Iri, LoomError, ScaffoldOpts, UnitKind,
};
use loom_mcp::{ToolHost, ToolOutcome};

use crate::fusion::build_scaffold;
use crate::routes::grounding as grounding_route;
use crate::state::AppState;

/// A traversal is bounded in breadth as well as depth: a highly connected class
/// must not turn one `loom.paths` call into a corpus-sized walk.
const MAX_VISITED: usize = 4_000;

impl AppState {
    /// The honest-zero grounding for a tool that performed no retrieval through
    /// the gate (traversal, SPARQL, manifest). It reports the gate's own
    /// threshold rather than the compiled-in default, exactly as the no-match
    /// path on the HTTP plane does.
    fn inert_grounding(&self, status: GroundingStatus) -> Value {
        let g = Grounding::none(self.policy.min_inject_score);
        grounding_route::envelope(self, &g, status)
    }

    /// The scaffold knobs a `loom.resolve` runs under: the node's configured
    /// budget and hops, with the gate's own confidence-injection setting. An
    /// explicit resolve is still budget-clamped — asking by address is a request
    /// to read, not a licence to bypass the clamp.
    fn resolve_opts(&self) -> ScaffoldOpts {
        ScaffoldOpts {
            budget_tokens: self.config.budget,
            hops: self.config.default_hops,
            prose: self.config.default_prose,
            confidence_injection: self.policy.confidence_injection,
            max_seeds: self.config.default_max_seeds,
            k_semantic: self.config.semantic_k,
            path: FusionPath::NoMatch,
        }
    }

    fn unit_json(unit: &CanonicalUnit) -> Value {
        json!({
            "iri": unit.iri.as_str(),
            "title": unit.title,
            "is_a": unit.is_a.iter().map(Iri::as_str).collect::<Vec<_>>(),
            "ancestors": unit.ancestors.iter().map(Iri::as_str).collect::<Vec<_>>(),
            "relations": unit.relations.iter().map(|r| json!({
                "predicate": r.predicate.as_predicate(),
                "targets": r.targets.iter().map(Iri::as_str).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "backlinks": unit.backlinks.iter().map(Iri::as_str).collect::<Vec<_>>(),
        })
    }

    /// Every IRI one hop from `iri`, in the reasoned projection the scaffold
    /// index carries (asserted + inferred superclasses, typed relations and
    /// backlinks).
    fn adjacent(&self, iri: &Iri) -> Vec<Iri> {
        let Some(unit) = self.retriever.resolve(iri) else {
            return Vec::new();
        };
        let mut out: Vec<Iri> = Vec::new();
        out.extend(unit.is_a.iter().cloned());
        out.extend(unit.ancestors.iter().cloned());
        for relation in &unit.relations {
            out.extend(relation.targets.iter().cloned());
        }
        out.extend(unit.backlinks.iter().cloned());
        out
    }
}

#[async_trait]
impl ToolHost for AppState {
    async fn manifest(&self, salience: usize) -> Result<ToolOutcome, LoomError> {
        // The manifest must name what is unavailable rather than let an agent
        // over-trust a lexical miss (ADR-140 Addendum A, conclusion 3).
        let mut degraded = Vec::new();
        if !self.semantic.is_ready() {
            degraded.push("semantic-hnsw".to_owned());
        }
        if !self.graph.status().available {
            degraded.push("graph".to_owned());
        }
        // A stale generation is still served (ADR-141): the manifest names it
        // so an agent can weigh an answer's age, exactly as it weighs a missing
        // accelerator. The grounding envelope is unchanged — staleness is a
        // property of the corpus, not of a retrieval's status.
        if crate::bundle::generation_is_stale(&self.generation.identity().generation) {
            degraded.push("generation-stale".to_owned());
        }
        let manifest = self.manifest.manifest(salience, &degraded).await?;
        Ok(ToolOutcome::new(
            to_value(&manifest).unwrap_or(Value::Null),
            self.inert_grounding(GroundingStatus::Navigated),
        ))
    }

    async fn browse(
        &self,
        query: &str,
        kind: UnitKind,
        n: usize,
    ) -> Result<ToolOutcome, LoomError> {
        let candidates = self.retriever.browse(query, kind, n).await?;
        // ADDRESSES ONLY (Invariant I-P1). Titles are carried because an agent
        // cannot choose between bare slugs, and a title is the address's label,
        // not the unit's content.
        let rows: Vec<Value> = candidates
            .iter()
            .map(|c| {
                let title = self.retriever.resolve(&c.iri).map(|u| u.title);
                json!({
                    "iri": c.iri.as_str(),
                    "title": title,
                    "score": c.score,
                    "provenance": c.provenance.as_str(),
                })
            })
            .collect();
        Ok(ToolOutcome::new(
            json!({ "query": query, "kind": kind.as_str(), "candidates": rows }),
            self.inert_grounding(GroundingStatus::Navigated),
        ))
    }

    async fn resolve(&self, iris: &[Iri]) -> Result<ToolOutcome, LoomError> {
        // THE GATE. Addresses are turned into seeds at a confidence the gate
        // will honour, then assembled and budget-clamped exactly as the chat
        // plane's scaffold is — an explicit resolve is a request to read these
        // units, not a licence to bypass the clamp.
        let query = iris
            .iter()
            .map(|i| i.slug().replace('-', " "))
            .collect::<Vec<_>>()
            .join(" ");
        let scaffold = build_scaffold(self, &query, self.resolve_opts()).await?;

        let requested: Vec<Value> = iris
            .iter()
            .map(|iri| match self.retriever.resolve(iri) {
                Some(unit) => json!({ "iri": iri.as_str(), "found": true, "title": unit.title }),
                None => json!({ "iri": iri.as_str(), "found": false, "title": Value::Null }),
            })
            .collect();

        // A resolve serves the corpus AS the answer with no backend call, which
        // is exactly what `Verbatim` names on the injection plane. Reusing it
        // keeps one vocabulary across both planes rather than minting a synonym.
        let status = if scaffold.engaged {
            GroundingStatus::Verbatim
        } else {
            GroundingStatus::NoMatch
        };
        let grounding = grounding_route::envelope(self, &scaffold.grounding, status);
        self.confidence
            .record(scaffold.grounding.decision, scaffold.grounding.confidence);

        Ok(ToolOutcome::new(
            json!({
                "requested": requested,
                "block": scaffold.block,
                "engaged": scaffold.engaged,
                "approx_tokens": scaffold.approx_tokens,
                "effective_budget": scaffold.effective_budget,
                "fusion_path": format!("{:?}", scaffold.fusion_path),
            }),
            grounding,
        ))
    }

    async fn sparql(&self, query: &str) -> Result<ToolOutcome, LoomError> {
        let result = self.graph.query(query).await?;
        Ok(ToolOutcome::new(
            to_value(&result).unwrap_or(Value::Null),
            self.inert_grounding(GroundingStatus::Navigated),
        ))
    }

    async fn neighbours(&self, iri: &Iri, limit: usize) -> Result<ToolOutcome, LoomError> {
        let Some(unit) = self.retriever.resolve(iri) else {
            return Ok(ToolOutcome::new(
                json!({ "iri": iri.as_str(), "found": false }),
                self.inert_grounding(GroundingStatus::Navigated),
            ));
        };
        let mut payload = Self::unit_json(&unit);
        payload["found"] = json!(true);
        payload["limit"] = json!(limit);
        // Trim each edge list to the caller's bound, largest families first
        // truncated rather than the whole object refused.
        for key in ["is_a", "ancestors", "backlinks"] {
            if let Some(arr) = payload[key].as_array_mut() {
                arr.truncate(limit);
            }
        }
        Ok(ToolOutcome::new(
            payload,
            self.inert_grounding(GroundingStatus::Navigated),
        ))
    }

    async fn paths(&self, from: &Iri, to: &Iri, max_hops: usize) -> Result<ToolOutcome, LoomError> {
        let path = self.shortest_path(from, to, max_hops);
        Ok(ToolOutcome::new(
            json!({
                "from": from.as_str(),
                "to": to.as_str(),
                "max_hops": max_hops,
                "found": path.is_some(),
                "hops": path.as_ref().map(Vec::len).map(|n| n.saturating_sub(1)),
                "path": path.map(|p| p.iter().map(Iri::as_str).map(str::to_owned).collect::<Vec<_>>()),
            }),
            self.inert_grounding(GroundingStatus::Navigated),
        ))
    }
}

impl AppState {
    /// Breadth-first shortest path over the reasoned projection, bounded in both
    /// depth (`max_hops`) and breadth ([`MAX_VISITED`]).
    ///
    /// BFS rather than anything cleverer because the bound is small and the
    /// guarantee matters: the first path found is a shortest one, so an agent
    /// can reason about distance. Unreachable-within-bounds returns `None`
    /// rather than a longest-effort path, because a wrong path is worse than no
    /// path for a grounding tool.
    fn shortest_path(&self, from: &Iri, to: &Iri, max_hops: usize) -> Option<Vec<Iri>> {
        if from.slug() == to.slug() {
            return Some(vec![from.clone()]);
        }
        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(from.slug().to_owned());
        let mut queue: VecDeque<Vec<Iri>> = VecDeque::from([vec![from.clone()]]);

        while let Some(path) = queue.pop_front() {
            if path.len() > max_hops {
                continue;
            }
            let tail = path.last()?;
            for next in self.adjacent(tail) {
                if !seen.insert(next.slug().to_owned()) {
                    continue;
                }
                let mut extended = path.clone();
                extended.push(next.clone());
                if next.slug() == to.slug() {
                    return Some(extended);
                }
                if seen.len() >= MAX_VISITED {
                    return None;
                }
                queue.push_back(extended);
            }
        }
        None
    }
}
