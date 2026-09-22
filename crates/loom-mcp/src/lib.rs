//! `loom-mcp` — the agentic plane's protocol adapter (ADR-140 D1).
//!
//! This crate is JSON-RPC framing, tool schemas and error mapping. It holds
//! **no retrieval logic whatsoever**: every tool call is delegated to a
//! [`ToolHost`], which the façade implements over the same ports, the same
//! fusion pipeline and the same gate that `/v1/chat/completions` uses. That is
//! the structural guarantee behind "two planes, one gate" — a second door
//! cannot acquire a second retrieval policy, because there is nowhere in this
//! crate to put one.
//!
//! Invariant I-P1 on the new plane: `loom.browse` returns ADDRESSES (IRI, score,
//! provenance) and `loom.resolve` is the only tool that returns corpus content,
//! via the host's `assemble`. the crate's `tests` module asserts the first half of that directly.
//!
//! Transport is out of scope here too. [`dispatch`] maps one JSON-RPC value to
//! one response value; the façade mounts it on Streamable HTTP and the stdio
//! shim pumps it over stdin/stdout.

#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use loom_domain::{Iri, LoomError, UnitKind};

pub mod schema;

#[cfg(test)]
mod tests;

/// The MCP revisions this server will speak. The client's requested version is
/// echoed when we support it; otherwise we answer with our newest and let the
/// client decide, which is what the specification asks for.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// The newest revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub const SERVER_NAME: &str = "ontology-loom";

/// One tool's result: the payload the agent reads, and the ADR-138 grounding
/// envelope that accompanies **every** answer on this plane exactly as it does
/// on the injection plane.
///
/// Keeping `grounding` a separate field (rather than letting hosts bury it in
/// `payload`) is what lets [`dispatch`] assert its presence structurally.
#[derive(Clone, Debug)]
pub struct ToolOutcome {
    pub payload: Value,
    pub grounding: Value,
}

impl ToolOutcome {
    pub fn new(payload: Value, grounding: Value) -> Self {
        Self { payload, grounding }
    }
}

/// The port this adapter drives. Implemented by the façade over `AppState`.
///
/// Every method is the agentic-plane spelling of something the HTTP plane
/// already does; none of them is a new retrieval path.
#[async_trait]
pub trait ToolHost: Send + Sync {
    /// The session index (ADR-140 D2). O(1) in corpus size.
    async fn manifest(&self, salience: usize) -> Result<ToolOutcome, LoomError>;

    /// Candidate ADDRESSES for a query. Never content.
    async fn browse(&self, query: &str, kind: UnitKind, n: usize)
        -> Result<ToolOutcome, LoomError>;

    /// The one content-returning tool: resolve addresses through the gate.
    async fn resolve(&self, iris: &[Iri]) -> Result<ToolOutcome, LoomError>;

    /// Read-only, clamped SPARQL over the reasoned closure.
    async fn sparql(&self, query: &str) -> Result<ToolOutcome, LoomError>;

    /// Typed neighbours of an IRI.
    async fn neighbours(&self, iri: &Iri, limit: usize) -> Result<ToolOutcome, LoomError>;

    /// Shortest typed paths between two IRIs.
    async fn paths(&self, from: &Iri, to: &Iri, max_hops: usize) -> Result<ToolOutcome, LoomError>;
}

/// JSON-RPC error codes, plus the one application code this server adds.
mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// A retrieval failure. Distinct from INVALID_PARAMS so a caller can tell
    /// "you asked wrongly" from "the corpus could not answer".
    pub const RETRIEVAL_ERROR: i64 = -32000;
}

/// Handle one JSON-RPC message.
///
/// Returns `None` for a notification (no `id`), which has no response by
/// definition — the transport must not write anything in that case.
pub async fn dispatch<H: ToolHost + ?Sized>(host: &H, request: &Value) -> Option<Value> {
    let obj = request.as_object()?;
    let id = obj.get("id").cloned();
    let method = obj.get("method").and_then(Value::as_str);

    // A notification: act where it matters, answer never.
    let id = id?;

    let Some(method) = method else {
        return Some(error(&id, code::INVALID_REQUEST, "missing method"));
    };
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));

    let result: Value = match method {
        "initialize" => initialize_result(&params),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": schema::tools() }),
        "tools/call" => return Some(call_tool(host, &id, &params).await),
        other => {
            return Some(error(
                &id,
                code::METHOD_NOT_FOUND,
                &format!("unknown method: {other}"),
            ))
        }
    };

    Some(success(&id, &result))
}

fn initialize_result(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = match requested {
        Some(v) if SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
        _ => PROTOCOL_VERSION,
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        "instructions": schema::INSTRUCTIONS,
    })
}

async fn call_tool<H: ToolHost + ?Sized>(host: &H, id: &Value, params: &Value) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error(id, code::INVALID_PARAMS, "missing tool name");
    };
    let args = params
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let outcome = match name {
        "loom.manifest" => {
            host.manifest(usize_arg(&args, "salience", schema::DEFAULT_SALIENCE))
                .await
        }
        "loom.browse" => match str_arg(&args, "query") {
            Err(e) => return error(id, code::INVALID_PARAMS, &e),
            Ok(query) => {
                let kind = kind_arg(&args);
                host.browse(
                    &query,
                    kind,
                    usize_arg(&args, "n", schema::DEFAULT_BROWSE_N),
                )
                .await
            }
        },
        "loom.resolve" => match iris_arg(&args) {
            Err(e) => return error(id, code::INVALID_PARAMS, &e),
            Ok(iris) => host.resolve(&iris).await,
        },
        "loom.sparql" => match str_arg(&args, "query") {
            Err(e) => return error(id, code::INVALID_PARAMS, &e),
            Ok(query) => host.sparql(&query).await,
        },
        "loom.neighbours" => match str_arg(&args, "iri") {
            Err(e) => return error(id, code::INVALID_PARAMS, &e),
            Ok(iri) => {
                host.neighbours(
                    &Iri::new(iri),
                    usize_arg(&args, "limit", schema::DEFAULT_NEIGHBOUR_LIMIT),
                )
                .await
            }
        },
        "loom.paths" => match (str_arg(&args, "from"), str_arg(&args, "to")) {
            (Err(e), _) | (_, Err(e)) => return error(id, code::INVALID_PARAMS, &e),
            (Ok(from), Ok(to)) => {
                host.paths(
                    &Iri::new(from),
                    &Iri::new(to),
                    usize_arg(&args, "max_hops", schema::DEFAULT_MAX_HOPS),
                )
                .await
            }
        },
        other => {
            return error(
                id,
                code::METHOD_NOT_FOUND,
                &format!("unknown tool: {other}"),
            )
        }
    };

    match outcome {
        Ok(outcome) => success(id, &tool_result(&outcome)),
        // A retrieval failure is reported as a JSON-RPC error rather than an
        // `isError` tool result: the distinction the ADR-138 contract cares about
        // is "the corpus had nothing" (a successful result with
        // `grounding.status: no-match`) versus "the node could not look", and
        // only the latter belongs here.
        Err(e) => error(id, code::RETRIEVAL_ERROR, &e.to_string()),
    }
}

/// The MCP `CallToolResult`. The text block is what a model without structured
/// output support reads; `structuredContent` is the same data typed, with the
/// grounding envelope always present.
fn tool_result(outcome: &ToolOutcome) -> Value {
    let text = serde_json::to_string_pretty(&outcome.payload)
        .unwrap_or_else(|_| outcome.payload.to_string());
    json!({
        "content": [ { "type": "text", "text": text } ],
        "structuredContent": {
            "result": outcome.payload,
            "grounding": outcome.grounding,
        },
        "isError": false,
    })
}

fn success(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A parse failure on the transport boundary, with a null id per JSON-RPC.
pub fn parse_error(message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": { "code": code::PARSE_ERROR, "message": message },
    })
}

// --- argument coercion ------------------------------------------------------

fn str_arg(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing or empty string argument: {key}"))
}

/// Clamped to the schema's maximum, never trusted from the wire — an agent that
/// asks for 10,000 salient terms gets the cap, not a manifest that defeats its
/// own purpose.
fn usize_arg(args: &Map<String, Value>, key: &str, default: usize) -> usize {
    let raw = args
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(default);
    raw.clamp(1, schema::max_for(key))
}

fn kind_arg(args: &Map<String, Value>) -> UnitKind {
    match args.get("kind").and_then(Value::as_str) {
        Some("term") => UnitKind::Term,
        Some("mapping") => UnitKind::Mapping,
        Some("attested-computation") => UnitKind::AttestedComputation,
        _ => UnitKind::Any,
    }
}

fn iris_arg(args: &Map<String, Value>) -> Result<Vec<Iri>, String> {
    let raw = args
        .get("iris")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing array argument: iris".to_owned())?;
    let iris: Vec<Iri> = raw
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .take(schema::MAX_RESOLVE_IRIS)
        .map(Iri::new)
        .collect();
    if iris.is_empty() {
        return Err("iris must contain at least one non-empty IRI or slug".to_owned());
    }
    Ok(iris)
}
