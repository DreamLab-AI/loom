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

use loom_domain::{FusionPath, Scaffold, ScaffoldOpts, ServedMode};
use loom_scaffold::message_text;
use loom_scaffold::tuning::SYSTEM_PREAMBLE;

use crate::error::ApiError;
use crate::fusion::build_scaffold;
use crate::serving;
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

    // F1 preconditions read from the ORIGINAL request, before any rewrite:
    // per-request opt-out, streaming, and the delivery-lookup shape (last message
    // user, no assistant turns). Then strip the Loom-private field so the backend
    // never sees it.
    let opted_out = serving::verbatim_opted_out(&body_obj);
    let streaming = serving::is_streaming(&body_obj);
    let delivery_shape = serving::is_delivery_lookup_shape(&messages);
    serving::strip_loom_options(&mut body_obj);

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
    let mut scaffold: Option<Scaffold> = None;

    if let Some(text) = last_user_text {
        match build_scaffold(&st, &text, opts).await {
            Ok(s) => scaffold = Some(s),
            Err(e) => {
                // Parity with Python: a scaffold failure skips injection and
                // still delegates the raw prompt (never 500s the chat path).
                tracing::warn!(error = %e, "scaffold skip on chat path");
            }
        }
    }

    let engaged = scaffold.as_ref().is_some_and(|s| !s.block.is_empty());
    let fusion_path = scaffold
        .as_ref()
        .map_or(FusionPath::NoMatch, |s| s.fusion_path);
    let grounding = grounding_value(scaffold.as_ref());

    // --- F1: VERBATIM SERVING — high-confidence scaffold, no backend call. -----
    if engaged && st.config.verbatim_mode && !opted_out && !streaming && delivery_shape {
        let s = scaffold.as_ref().unwrap();
        if f64::from(s.top_score) >= st.config.verbatim_threshold {
            return verbatim_response(&st, s, &grounding, fusion_path);
        }
    }

    // --- DELEGATE PATH ---------------------------------------------------------
    if engaged {
        merge_scaffold(&mut new_msgs, &scaffold.as_ref().unwrap().block);
    }
    let after = content_sum(&new_msgs);
    let injected = after.saturating_sub(before).div_ceil(4);
    body_obj.insert("messages".to_owned(), Value::Array(new_msgs));

    // F3: thinking + budget control — only for an ENGAGED delegation. Mutates the
    // body in place (chat_template_kwargs and/or the think-token floor); never
    // touches a passthrough request. `stream` stripping and the general
    // `max_tokens` floor still happen in the backend adapter (single-sourced).
    if engaged {
        serving::apply_thinking_controls(
            &mut body_obj,
            st.config.backend_no_think,
            st.config.think_token_floor,
        );
    }
    let delegated = Value::Object(body_obj);

    match st.backend.chat(delegated).await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK);
            let mut out = resp.body;
            // Annotate the 200 JSON with the fail-labelled honesty block (incl. F2).
            if resp.status == 200 {
                annotate_delegated(
                    &st,
                    &mut out,
                    scaffold.as_ref(),
                    &grounding,
                    fusion_path,
                    injected,
                );
            }
            (status, Json(out)).into_response()
        }
        Err(e) => ApiError(e).into_response(), // NoBackend→503; unreachable/http→502
    }
}

/// The `grounding` telemetry object for an engaged scaffold (else `null`).
fn grounding_value(scaffold: Option<&Scaffold>) -> Value {
    match scaffold {
        Some(s) if !s.block.is_empty() => json!({
            "seeds": s.seeds,
            "top_score": s.top_score,
            "effective_budget": s.effective_budget,
            "engaged": true,
        }),
        _ => Value::Null,
    }
}

/// Build the F1 verbatim 200 response: the scaffold served as the answer, with the
/// `served_mode: verbatim` + exposure telemetry. No backend is called.
fn verbatim_response(
    st: &AppState,
    s: &Scaffold,
    grounding: &Value,
    fusion_path: FusionPath,
) -> Response {
    let gen = st.generation.current();
    let content = serving::verbatim_content(&s.block, &gen.id.0);
    // F2: the served content IS the answer, so exposure is honest here too.
    let exposure = serving::compute_exposure(st.retriever.as_ref(), s, &content);
    let loom_block = json!({
        "mode": "scaffold",
        "served_mode": ServedMode::Verbatim,
        "injected_tokens": est_block_tokens(&s.block),
        "grounding": grounding,
        "fusion_path": fusion_path,
        "exposure": exposure,
        "generation": to_value(&gen),
    });
    let out = serving::verbatim_completion(&content, &loom_block);
    (StatusCode::OK, Json(out)).into_response()
}

/// Attach the `loom` telemetry block to a delegated 200 response, computing F2
/// exposure (and optionally appending the "Not covered" line) when engaged.
fn annotate_delegated(
    st: &AppState,
    out: &mut Value,
    scaffold: Option<&Scaffold>,
    grounding: &Value,
    fusion_path: FusionPath,
    injected: usize,
) {
    let exposure = match scaffold {
        Some(s) if !s.block.is_empty() => {
            let answer = serving::answer_text(out);
            let report = serving::compute_exposure(st.retriever.as_ref(), s, &answer);
            // LOOM_EXPOSURE_APPEND: append a single "Not covered" line on drops.
            if st.config.exposure_append {
                serving::append_not_covered(out, &report);
            }
            to_value(&report)
        }
        _ => Value::Null,
    };
    if let Value::Object(ref mut map) = out {
        map.insert(
            "loom".to_owned(),
            json!({
                "mode": "scaffold",
                "served_mode": ServedMode::Delegated,
                "injected_tokens": injected,
                "grounding": grounding,
                "fusion_path": fusion_path,
                "exposure": exposure,
                "generation": to_value(&st.generation.current()),
            }),
        );
    }
}

/// Estimate token count of a served block for the `injected_tokens` telemetry on
/// the verbatim path (no message-merge delta is available there). Mirrors the
/// façade's `chars/4` heuristic used on the delegate path.
fn est_block_tokens(block: &str) -> usize {
    block.chars().count().div_ceil(4)
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
