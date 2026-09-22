//! The tool contract as JSON Schema, plus the bounds every argument is clamped
//! to (ADR-140 D1 §3.4).
//!
//! Descriptions here are the conservative default. Under ADR-140 D7 they become
//! exposure-profile data so they can be fitted per backbone without touching the
//! corpus; this module is what a profile overrides, not a competitor to it.

use serde_json::{json, Value};

pub const DEFAULT_SALIENCE: usize = 40;
pub const MAX_SALIENCE: usize = 200;
pub const DEFAULT_BROWSE_N: usize = 8;
pub const MAX_BROWSE_N: usize = 50;
pub const DEFAULT_NEIGHBOUR_LIMIT: usize = 25;
pub const MAX_NEIGHBOUR_LIMIT: usize = 200;
pub const DEFAULT_MAX_HOPS: usize = 4;
pub const MAX_HOPS_CEILING: usize = 8;
/// A resolve is a budgeted injection under the hood, so the address list is
/// capped well below what the gate would clamp anyway.
pub const MAX_RESOLVE_IRIS: usize = 12;

/// The ceiling for a numeric argument, by name. Unknown names get a small,
/// safe bound rather than `usize::MAX`: a new argument must opt into its limit.
#[must_use]
pub fn max_for(key: &str) -> usize {
    match key {
        "salience" => MAX_SALIENCE,
        "n" => MAX_BROWSE_N,
        "limit" => MAX_NEIGHBOUR_LIMIT,
        "max_hops" => MAX_HOPS_CEILING,
        _ => 50,
    }
}

/// Shown once at `initialize`. It states the plane's discipline, because an
/// agent that calls `resolve` on every browse hit has reproduced the injection
/// plane's cost profile without its simplicity.
pub const INSTRUCTIONS: &str = "\
The Ontology Loom serves a curated, reasoner-checked ontology. Call loom.manifest once at the \
start of a session to learn what the corpus covers and how well it is grounded. Use loom.browse \
to find addresses and loom.resolve to read only the ones you actually need — browse returns no \
content by design. loom.sparql, loom.neighbours and loom.paths query the reasoned closure \
directly. Every answer carries a `grounding` object naming the corpus generation it came from; \
when `corpus_backed` is false the corpus did not answer and you should say so rather than infer.";

/// The six P0 tools.
#[must_use]
pub fn tools() -> Vec<Value> {
    vec![
        tool(
            "loom.manifest",
            "The session index: corpus profile, grounding coverage, salient addresses and the tool contract. O(1) in corpus size — call it once at session start.",
            &json!({
                "type": "object",
                "properties": {
                    "salience": {
                        "type": "integer",
                        "description": format!("How many salient addresses to list (default {DEFAULT_SALIENCE}, max {MAX_SALIENCE})."),
                        "minimum": 1, "maximum": MAX_SALIENCE
                    }
                },
                "additionalProperties": false
            }),
        ),
        tool(
            "loom.browse",
            "Find candidate addresses for a query. Returns IRIs, scores and provenance — never corpus content. Follow with loom.resolve to read anything.",
            &json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural-language or keyword query." },
                    "kind": {
                        "type": "string",
                        "enum": ["any", "term", "mapping", "attested-computation"],
                        "description": "Which OKF unit family to search. 'mapping' and 'attested-computation' return only terms that carry that family, which is empty until the corpus does."
                    },
                    "n": { "type": "integer", "minimum": 1, "maximum": MAX_BROWSE_N,
                           "description": format!("Maximum candidates (default {DEFAULT_BROWSE_N}).") }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        tool(
            "loom.resolve",
            "Read one or more addresses as their canonical, human-reviewed markdown blocks, through the same confidence gate the chat plane uses. The only tool that returns corpus content.",
            &json!({
                "type": "object",
                "properties": {
                    "iris": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1,
                        "maxItems": MAX_RESOLVE_IRIS,
                        "description": "Full IRIs (urn:ngm:class:<slug>) or bare slugs, as returned by loom.browse."
                    }
                },
                "required": ["iris"],
                "additionalProperties": false
            }),
        ),
        tool(
            "loom.sparql",
            "Read-only SPARQL over the Whelk-reasoned closure. Read forms only; a LIMIT is enforced.",
            &json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "A SELECT/ASK/CONSTRUCT/DESCRIBE query." } },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        tool(
            "loom.neighbours",
            "Typed neighbours of an IRI in the reasoned graph: superclasses, inferred ancestors, relations and backlinks.",
            &json!({
                "type": "object",
                "properties": {
                    "iri": { "type": "string", "description": "Full IRI or bare slug." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": MAX_NEIGHBOUR_LIMIT,
                               "description": format!("Maximum neighbours (default {DEFAULT_NEIGHBOUR_LIMIT}).") }
                },
                "required": ["iri"],
                "additionalProperties": false
            }),
        ),
        tool(
            "loom.paths",
            "Shortest typed paths between two IRIs in the reasoned graph.",
            &json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string" },
                    "to": { "type": "string" },
                    "max_hops": { "type": "integer", "minimum": 1, "maximum": MAX_HOPS_CEILING,
                                  "description": format!("Search depth (default {DEFAULT_MAX_HOPS}).") }
                },
                "required": ["from", "to"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: &Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}
