//! The axum router (§9) + handlers, endpoint-parity with the Python façade
//! (`app/loom_facade.py` do_GET/do_POST), same paths, same aliases.
//!
//! Handler parity notes carried from the reference:
//! - `chat_completions` scaffolds the LAST user message, merges the block into
//!   the first system message (or inserts one at position 0), delegates via
//!   `ModelBackend` (which floors `max_tokens` and strips `stream` — the façade
//!   does NOT re-do that), and annotates the 200 JSON with the `loom:{…}` block;
//! - `scaffold` returns the served block + the audit surface (`seeds`,
//!   `fusion_path`) over Python's shape;
//! - `health` is a superset: adds `semantic` readiness/generation;
//! - `semantic_search` is the ONE endpoint that may show the raw index shape
//!   (bare IRI + cosine), because it is labelled as the index, not an answer —
//!   it never feeds `/v1/chat/completions`.

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Map, Value};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use loom_domain::{FusionPath, ScaffoldOpts};
use loom_scaffold::message_text;
use loom_scaffold::tuning::SYSTEM_PREAMBLE;

use crate::error::ApiError;
use crate::fusion::build_scaffold;
use crate::state::AppState;

/// Build the full §9 router around an `AppState`, with the tower layer stack:
/// timeout (`LOOM_TIMEOUT`, 408 on elapse), body-size cap, permissive CORS
/// (mirrors `Access-Control-Allow-Origin: *`), and HTTP tracing.
pub fn build_router(state: AppState) -> Router {
    let timeout = Duration::from_secs(state.config.timeout_secs);
    let max_body = state.config.max_body_bytes;
    Router::new()
        .route("/health", get(health))
        .route("/loom/generation", get(generation))
        .route("/generation", get(generation)) // alias (Python parity)
        .route("/loom/scaffold", post(scaffold))
        .route("/scaffold", post(scaffold)) // alias
        .route("/loom/sparql", post(sparql))
        .route("/sparql", post(sparql)) // alias
        .route("/loom/search", post(search))
        .route("/search", post(search)) // alias
        .route("/loom/search/semantic", post(semantic_search)) // NEW: HNSW debug surface (gated)
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(models))
        .with_state(state)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            timeout,
        ))
        .layer(RequestBodyLimitLayer::new(max_body))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

// --- GET /health ------------------------------------------------------------

async fn health(State(st): State<AppState>) -> Response {
    let backend_configured = !st.config.backend_url.is_empty();
    // `backend_reachable` is null when there is no backend (Python parity).
    let backend_reachable = if backend_configured {
        Value::Bool(st.backend.reachable().await)
    } else {
        Value::Null
    };
    let backend = if backend_configured {
        Value::String(st.backend.endpoint().to_owned())
    } else {
        Value::Null
    };

    let generation = to_value(&st.generation.current());
    let semantic = json!({
        "ready": st.semantic.is_ready(),
        "generation": to_value(&st.semantic.generation()),
    });
    let graph = to_value(&st.graph.status());

    Json(json!({
        "ok": true,
        "facet": "loom-facade",
        "mode": "scaffold",
        "backend": backend,
        "backend_reachable": backend_reachable,
        "index_classes": st.retriever.class_count(),
        "graph": graph,
        "semantic": semantic,
        "generation": generation,
        "deploy_profile": st.config.deploy_profile,
    }))
    .into_response()
}

// --- GET /loom/generation (+ /generation) -----------------------------------

async fn generation(State(st): State<AppState>) -> Response {
    Json(to_value(&st.generation.current())).into_response()
}

// --- POST /loom/scaffold (+ /scaffold) --------------------------------------

async fn scaffold(State(st): State<AppState>, body: Bytes) -> Response {
    let j = parse_body(&body);
    let prompt = first_str(&j, &["prompt", "query"]).unwrap_or_default();
    if prompt.is_empty() {
        return bad_request("missing prompt");
    }
    let prose = j
        .get("prose")
        .and_then(Value::as_bool)
        .unwrap_or(st.config.default_prose);
    let opts = ScaffoldOpts {
        budget_tokens: usize_field(&j, "budget_tokens", st.config.budget),
        hops: usize_field(&j, "hops", st.config.default_hops),
        prose,
        confidence_injection: st.policy.confidence_injection,
        max_seeds: usize_field(&j, "max_seeds", st.config.default_max_seeds),
        k_semantic: st.config.semantic_k,
        path: FusionPath::NoMatch,
    };

    match build_scaffold(&st, &prompt, opts).await {
        Ok(s) => Json(json!({
            "scaffold": s.block,
            "engaged": s.engaged,
            "approx_tokens": s.approx_tokens,
            "prose": prose,
            "seeds": s.seeds,
            "top_score": s.top_score,
            "effective_budget": s.effective_budget,
            "fusion_path": s.fusion_path,
            "generation": to_value(&st.generation.current()),
        }))
        .into_response(),
        Err(e) => ApiError(e).into_response(),
    }
}

// --- POST /loom/sparql (+ /sparql) ------------------------------------------

async fn sparql(State(st): State<AppState>, body: Bytes) -> Response {
    let j = parse_body(&body);
    let query = first_str(&j, &["query"]).unwrap_or_default();
    if query.is_empty() {
        return bad_request("missing query");
    }
    match st.graph.query(&query).await {
        Ok(r) => Json(to_value(&r)).into_response(),
        Err(e) => ApiError(e).into_response(), // BadQuery→400; GraphUnavailable→200 degraded
    }
}

// --- POST /loom/search (+ /search) ------------------------------------------

async fn search(State(st): State<AppState>, body: Bytes) -> Response {
    let j = parse_body(&body);
    let needle = first_str(&j, &["q", "query"]).unwrap_or_default();
    if needle.is_empty() {
        return bad_request("missing q");
    }
    let limit = usize_field(&j, "limit", 20);
    match st.graph.search_labels(&needle, limit).await {
        Ok(hits) => Json(to_value(&hits)).into_response(),
        Err(e) => ApiError(e).into_response(),
    }
}

// --- POST /loom/search/semantic ---------------------------------------------

async fn semantic_search(State(st): State<AppState>, body: Bytes) -> Response {
    // Default-OFF (LOOM_SEMANTIC_DEBUG_SURFACE, audit finding 1): the labelled
    // index-debug surface is disabled unless explicitly turned on, so a bare
    // IRI+score shape can never be reached by default.
    if !st.config.semantic_debug_surface {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "semantic debug surface disabled" })),
        )
            .into_response();
    }
    let j = parse_body(&body);
    let query = first_str(&j, &["q", "query", "prompt"]).unwrap_or_default();
    if query.is_empty() {
        return bad_request("missing q");
    }
    // Debug/eval surface: honest about readiness, never markdown.
    if !st.semantic.is_ready() {
        return Json(json!({
            "ready": false,
            "results": [],
            "generation": to_value(&st.semantic.generation()),
        }))
        .into_response();
    }
    let k = usize_field(&j, "k", st.config.semantic_k);
    let qvec = match st.embedder.embed(&query).await {
        Ok(v) => v,
        Err(e) => return ApiError(e).into_response(), // embed error → 502 (labelled)
    };
    match st.semantic.nearest(&qvec, k).await {
        Ok(hits) => {
            // Bare IRI + cosine — the one place the raw index shape may show,
            // BECAUSE it is labelled as the index, not as an answer (I-P1 safe).
            let results: Vec<Value> = hits
                .iter()
                .map(|h| json!({ "iri": h.iri.as_str(), "score": h.score }))
                .collect();
            Json(json!({
                "ready": true,
                "results": results,
                "generation": to_value(&st.semantic.generation()),
            }))
            .into_response()
        }
        Err(e) => ApiError(e).into_response(), // SemanticUnready→200 degraded
    }
}

// --- POST /v1/chat/completions ----------------------------------------------

async fn chat_completions(State(st): State<AppState>, body: Bytes) -> Response {
    let mut body_obj: Map<String, Value> = match parse_body(&body) {
        Value::Object(m) => m,
        _ => Map::new(),
    };

    let messages: Vec<Value> = body_obj
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Scaffold knobs for the chat path (Python `ontology_budget`/`ontology_prose`).
    let budget = body_obj
        .get("ontology_budget")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(st.config.budget);
    let prose = body_obj
        .get("ontology_prose")
        .and_then(Value::as_bool)
        .unwrap_or(st.config.default_prose);
    let opts = ScaffoldOpts {
        budget_tokens: budget,
        hops: st.config.default_hops,
        prose,
        confidence_injection: st.policy.confidence_injection,
        max_seeds: st.config.default_max_seeds,
        k_semantic: st.config.semantic_k,
        path: FusionPath::NoMatch,
    };

    // Scaffold from the LAST user message; merge into the messages array.
    let last_user_text = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(Value::as_str) == Some("user"))
        .map(|m| message_text(m.get("content").unwrap_or(&Value::Null)));

    let before = content_sum(&messages);
    let mut new_msgs = messages;
    let mut fusion_path = FusionPath::NoMatch;
    let mut grounding = Value::Null;

    if let Some(text) = last_user_text {
        match build_scaffold(&st, &text, opts).await {
            Ok(s) => {
                fusion_path = s.fusion_path;
                if !s.block.is_empty() {
                    merge_scaffold(&mut new_msgs, &s.block);
                    grounding = json!({
                        "seeds": s.seeds,
                        "top_score": s.top_score,
                        "effective_budget": s.effective_budget,
                        "engaged": true,
                    });
                }
            }
            Err(e) => {
                // Parity with Python: a scaffold failure skips injection and
                // still delegates the raw prompt (never 500s the chat path).
                tracing::warn!(error = %e, "scaffold skip on chat path");
            }
        }
    }

    let after = content_sum(&new_msgs);
    let injected = after.saturating_sub(before).div_ceil(4);

    body_obj.insert("messages".to_owned(), Value::Array(new_msgs));
    // NB: `stream` stripping and the `max_tokens` floor live in the backend
    // adapter (do NOT re-do here — the floor logic is single-sourced there).
    let delegated = Value::Object(body_obj);

    match st.backend.chat(delegated).await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK);
            let mut out = resp.body;
            // Annotate the 200 JSON with the fail-labelled honesty block.
            if resp.status == 200 {
                if let Value::Object(ref mut map) = out {
                    map.insert(
                        "loom".to_owned(),
                        json!({
                            "mode": "scaffold",
                            "injected_tokens": injected,
                            "grounding": grounding,
                            "fusion_path": fusion_path,
                            "generation": to_value(&st.generation.current()),
                        }),
                    );
                }
            }
            (status, Json(out)).into_response()
        }
        Err(e) => ApiError(e).into_response(), // NoBackend→503; unreachable/http→502
    }
}

// --- GET /v1/models ---------------------------------------------------------

async fn models(State(st): State<AppState>) -> Response {
    match st.backend.models().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => ApiError(e).into_response(),
    }
}

// --- helpers ----------------------------------------------------------------

/// Merge the scaffold block into the messages array, byte-identically to
/// `loom_scaffold::scaffold_messages`: append to the first string-content system
/// message (trim-end + blank line), else insert a fresh system message at 0.
fn merge_scaffold(msgs: &mut Vec<Value>, block: &str) {
    let injection = format!("{SYSTEM_PREAMBLE}\n\n{block}");
    let sys_pos = msgs
        .iter()
        .position(|m| m.get("role").and_then(Value::as_str) == Some("system"));
    match sys_pos {
        Some(i) if msgs[i].get("content").and_then(Value::as_str).is_some() => {
            let existing = msgs[i]["content"].as_str().unwrap().trim_end().to_owned();
            msgs[i]["content"] = Value::String(format!("{existing}\n\n{injection}"));
        }
        _ => {
            msgs.insert(0, json!({ "role": "system", "content": injection }));
        }
    }
}

/// Sum of message-content character counts — Python `sum(len(str(content)))`.
/// A string content counts its chars; a non-string counts its JSON text length
/// (the string case is what the chat clients send, and what parity needs exact).
fn content_sum(msgs: &[Value]) -> usize {
    msgs.iter()
        .map(|m| match m.get("content") {
            Some(Value::String(s)) => s.chars().count(),
            Some(other) => other.to_string().len(),
            None => 0,
        })
        .sum()
}

/// First present, non-empty string among `keys`.
fn first_str(j: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = j.get(*k).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_owned());
            }
        }
    }
    None
}

fn usize_field(j: &Value, key: &str, default: usize) -> usize {
    j.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(default)
}

fn parse_body(bytes: &Bytes) -> Value {
    if bytes.is_empty() {
        return Value::Object(Map::new());
    }
    serde_json::from_slice(bytes).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn bad_request(detail: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": detail }))).into_response()
}

/// Serialise a domain value, falling back to `null` on the (impossible for these
/// types) serialise error rather than panicking a handler.
fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}
