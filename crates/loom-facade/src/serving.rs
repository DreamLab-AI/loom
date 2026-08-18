//! Findings-driven serving decisions layered on the chat path, all façade-side,
//! all config-gated, defaults preserving current behaviour exactly:
//!
//! - **F1 verbatim serving** — when a high-confidence scaffold engages, serve its
//!   canonical markdown WITHOUT calling the backend (the paper's serving-regime
//!   verdict: generative re-encoding of a high-confidence scaffold mostly risks
//!   fidelity and costs 60–170s vs ~2ms retrieval).
//! - **F2 exposure telemetry** — after an answer returns, report how many served
//!   titles it restated (the copy-fidelity deficit made observable).
//! - **F3 thinking + budget control** — optionally disable backend thinking for
//!   delivery-shaped engaged requests, and floor `max_tokens` so think-tokens do
//!   not exhaust the budget on long scaffolds.
//!
//! The gate, the matcher and the token-floor primitive all live in the pure
//! crates (`loom-scaffold`, `loom-backend-openai`); this module only sequences
//! them and shapes the OpenAI-compatible envelope. Invariant I-P1 holds: the
//! verbatim content is the scaffold's own served markdown (resolved from `Iri`s
//! by the lexical port), never a raw engine artefact.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

use loom_domain::{ExposureReport, LexicalIndex, Scaffold};
use loom_scaffold::exposure::{exposure_report, not_covered_line, DROPPED_CAP};
use loom_scaffold::tuning::{FOOTER, HEADER};

// --- F1: verbatim eligibility + envelope ------------------------------------

/// The delivery-lookup shape verbatim serving is restricted to: the LAST message
/// is from the user and there are NO prior assistant turns. A multi-turn
/// conversation (any assistant message) delegates as today — verbatim answers a
/// single lookup, not a dialogue.
#[must_use]
pub fn is_delivery_lookup_shape(messages: &[Value]) -> bool {
    let last_is_user = messages
        .last()
        .and_then(|m| m.get("role"))
        .and_then(Value::as_str)
        == Some("user");
    let has_assistant = messages
        .iter()
        .any(|m| m.get("role").and_then(Value::as_str) == Some("assistant"));
    last_is_user && !has_assistant
}

/// Per-request opt-out: `"loom_options": {"verbatim": false}`. Unknown to normal
/// OpenAI clients (they never send it); when present-and-false it forces the
/// delegate path even under `LOOM_VERBATIM_MODE`. Any other value (absent, true,
/// non-bool) leaves the mode's decision untouched.
#[must_use]
pub fn verbatim_opted_out(body: &Map<String, Value>) -> bool {
    body.get("loom_options")
        .and_then(|o| o.get("verbatim"))
        .and_then(Value::as_bool)
        == Some(false)
}

/// A streaming request (`"stream": true`). Verbatim bypasses streaming (delegate
/// as today) — a synthetic SSE stream is not worth the complexity for the
/// delivery-lookup case, and the backend strips `stream` on the delegate path.
#[must_use]
pub fn is_streaming(body: &Map<String, Value>) -> bool {
    body.get("stream").and_then(Value::as_bool) == Some(true)
}

/// Strip the Loom-private `loom_options` field so it never reaches the backend
/// (a strict OpenAI server could reject an unknown field).
pub fn strip_loom_options(body: &mut Map<String, Value>) {
    body.remove("loom_options");
}

/// Strip the `[ONTOLOGY CONTEXT]` / `[END ONTOLOGY CONTEXT]` wrapper from a
/// served block, leaving the bare per-IRI markdown. The block is produced by
/// `clamp` as exactly `"{HEADER}\n{body}\n{FOOTER}"`, so this is a prefix/suffix
/// peel — no parsing.
#[must_use]
fn strip_ontology_wrapper(block: &str) -> String {
    let s = block.strip_prefix(HEADER).unwrap_or(block);
    let s = s.strip_prefix('\n').unwrap_or(s);
    let s = s.strip_suffix(FOOTER).unwrap_or(s);
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.to_owned()
}

/// The verbatim message content: a one-line provenance header naming the
/// generation, a blank line, then the unwrapped served blocks.
#[must_use]
pub fn verbatim_content(block: &str, generation_id: &str) -> String {
    let body = strip_ontology_wrapper(block);
    format!(
        "_Served verbatim from the Ontology Loom (generation: {generation_id}); \
         no model generation was performed._\n\n{body}"
    )
}

/// Seconds since the Unix epoch for the `created` field (0 on a clock error —
/// honest rather than panicking a serve).
#[must_use]
fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Build a valid OpenAI-shaped chat completion carrying the verbatim content, a
/// `finish_reason` of `stop`, honest zero `usage`, a `model` of `loom-verbatim`,
/// and the `loom` telemetry (with `served_mode: "verbatim"` and the exposure
/// block). `loom_block` carries the shared telemetry (grounding, fusion_path,
/// generation, injected_tokens, served_mode, exposure) assembled by the caller.
#[must_use]
pub fn verbatim_completion(content: &str, loom_block: &Value) -> Value {
    let created = unix_secs();
    json!({
        "id": format!("loom-verbatim-{created}"),
        "object": "chat.completion",
        "created": created,
        "model": "loom-verbatim",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop",
        }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
        "loom": loom_block,
    })
}

// --- F2: exposure orchestration ---------------------------------------------

/// Collect the candidate served titles for a scaffold: each injected seed's class
/// title plus its serialised relation-target titles, resolved via the lexical
/// port (`Iri` → `CanonicalUnit`). `exposure_report` then filters this superset
/// to those actually present in the served block, so over-collecting is safe.
#[must_use]
pub fn collect_served_titles(retriever: &dyn LexicalIndex, scaffold: &Scaffold) -> Vec<String> {
    let mut titles: Vec<String> = Vec::new();
    for seed in &scaffold.seeds {
        let Some(unit) = retriever.resolve(&seed.iri) else {
            continue;
        };
        titles.push(unit.title.clone());
        for rel in &unit.relations {
            for target in &rel.targets {
                if let Some(tu) = retriever.resolve(target) {
                    titles.push(tu.title);
                }
            }
        }
    }
    titles
}

/// Compute the exposure report for an engaged scaffold against `answer`.
#[must_use]
pub fn compute_exposure(
    retriever: &dyn LexicalIndex,
    scaffold: &Scaffold,
    answer: &str,
) -> ExposureReport {
    let titles = collect_served_titles(retriever, scaffold);
    exposure_report(&titles, &scaffold.block, answer, DROPPED_CAP)
}

/// Concatenate the answer text across all response choices (for matching).
#[must_use]
pub fn answer_text(out: &Value) -> String {
    out.get("choices")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c.pointer("/message/content").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Append a plain `Not covered above: …` line to the FIRST choice's message
/// content (`LOOM_EXPOSURE_APPEND`). No-op when there is no string content to
/// append to. Returns whether it appended.
pub fn append_not_covered(out: &mut Value, report: &ExposureReport) -> bool {
    let Some(line) = not_covered_line(report) else {
        return false;
    };
    let Some(content) = out.pointer_mut("/choices/0/message/content") else {
        return false;
    };
    let Some(existing) = content.as_str() else {
        return false;
    };
    *content = Value::String(format!("{existing}\n\n{line}"));
    true
}

// --- F3: thinking + budget control ------------------------------------------

/// Apply the F3 thinking + budget controls to a body about to be delegated for an
/// ENGAGED (scaffold-injected) request. Returns whether thinking is left ACTIVE
/// (for telemetry / test assertions). Non-engaged requests must NOT call this.
///
/// - `LOOM_BACKEND_NO_THINK` on + client did not set `chat_template_kwargs`
///   ⇒ inject `chat_template_kwargs: {"enable_thinking": false}` (thinking OFF).
/// - otherwise thinking stays ACTIVE (no-think off, or the client overrode with
///   their own `chat_template_kwargs`), and — when a `think_floor > 0` is set —
///   any sub-floor INTEGER `max_tokens` the client sent is raised to the floor,
///   reusing the backend adapter's audited integer-only floor primitive.
pub fn apply_thinking_controls(
    body: &mut Map<String, Value>,
    no_think: bool,
    think_floor: u64,
) -> bool {
    let client_set_ctk = body.contains_key("chat_template_kwargs");
    if no_think && !client_set_ctk {
        body.insert(
            "chat_template_kwargs".to_owned(),
            json!({ "enable_thinking": false }),
        );
        return false; // thinking disabled — no need to floor for think-budget
    }
    // Thinking active: floor the ask so think-tokens do not truncate the answer.
    if think_floor > 0 {
        // Return value (key-present) is irrelevant here — F3 only ever RAISES an
        // ask the client actually sent; it never inserts a missing key.
        let _ = loom_backend_openai::raise_integer_token_floor(body, think_floor);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_lookup_shape_gating() {
        let user = json!({ "role": "user", "content": "q" });
        let assistant = json!({ "role": "assistant", "content": "a" });
        let system = json!({ "role": "system", "content": "s" });
        // last user, no assistant → eligible
        assert!(is_delivery_lookup_shape(&[system.clone(), user.clone()]));
        assert!(is_delivery_lookup_shape(std::slice::from_ref(&user)));
        // a prior assistant turn → NOT eligible
        assert!(!is_delivery_lookup_shape(&[
            user.clone(),
            assistant,
            user.clone()
        ]));
        // last is not user → NOT eligible
        assert!(!is_delivery_lookup_shape(&[user, system]));
    }

    #[test]
    fn opt_out_and_streaming() {
        let mut m = Map::new();
        assert!(!verbatim_opted_out(&m));
        assert!(!is_streaming(&m));
        m.insert("loom_options".to_owned(), json!({ "verbatim": false }));
        assert!(verbatim_opted_out(&m));
        m.insert("loom_options".to_owned(), json!({ "verbatim": true }));
        assert!(!verbatim_opted_out(&m));
        m.insert("stream".to_owned(), json!(true));
        assert!(is_streaming(&m));
        strip_loom_options(&mut m);
        assert!(!m.contains_key("loom_options"));
    }

    #[test]
    fn strip_wrapper_and_content() {
        let block = "[ONTOLOGY CONTEXT]\n## Knowledge Graph\ndef here\n[END ONTOLOGY CONTEXT]";
        assert_eq!(
            strip_ontology_wrapper(block),
            "## Knowledge Graph\ndef here"
        );
        let content = verbatim_content(block, "gen-1");
        assert!(content.contains("generation: gen-1"));
        assert!(content.contains("## Knowledge Graph"));
        assert!(!content.contains("[ONTOLOGY CONTEXT]"));
    }

    #[test]
    fn verbatim_completion_is_openai_shaped() {
        let v = verbatim_completion("hello", &json!({ "served_mode": "verbatim" }));
        assert_eq!(v["object"], "chat.completion");
        assert_eq!(v["model"], "loom-verbatim");
        assert_eq!(v["choices"][0]["message"]["content"], "hello");
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["total_tokens"], 0);
        assert_eq!(v["loom"]["served_mode"], "verbatim");
    }

    #[test]
    fn answer_text_joins_choices() {
        let out = json!({
            "choices": [
                { "message": { "content": "one" } },
                { "message": { "content": "two" } },
            ]
        });
        assert_eq!(answer_text(&out), "one\ntwo");
    }

    #[test]
    fn append_not_covered_mutates_first_choice() {
        let mut out =
            json!({ "choices": [{ "message": { "role": "assistant", "content": "body" } }] });
        let report = ExposureReport {
            targets: 2,
            delivered: 1,
            dropped: vec!["Graph Database".to_owned()],
        };
        assert!(append_not_covered(&mut out, &report));
        let c = out["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(c.starts_with("body"));
        assert!(c.contains("Not covered above: Graph Database."));
        // No drops ⇒ no-op.
        let mut out2 = json!({ "choices": [{ "message": { "content": "x" } }] });
        let empty = ExposureReport::default();
        assert!(!append_not_covered(&mut out2, &empty));
    }

    #[test]
    fn thinking_controls_no_think_injects_kwargs() {
        let mut body = Map::new();
        body.insert("max_tokens".to_owned(), json!(256));
        // no-think ON, client did not set ctk → inject, thinking OFF, no floor.
        let active = apply_thinking_controls(&mut body, true, 1536);
        assert!(!active);
        assert_eq!(
            body["chat_template_kwargs"]["enable_thinking"],
            json!(false)
        );
        assert_eq!(body["max_tokens"], json!(256), "no floor when thinking off");
    }

    #[test]
    fn thinking_controls_client_override_keeps_thinking_and_floors() {
        let mut body = Map::new();
        body.insert(
            "chat_template_kwargs".to_owned(),
            json!({ "enable_thinking": true }),
        );
        body.insert("max_tokens".to_owned(), json!(256));
        // no-think ON but client set ctk (override) → thinking active + floor.
        let active = apply_thinking_controls(&mut body, true, 1536);
        assert!(active);
        assert_eq!(
            body["chat_template_kwargs"]["enable_thinking"],
            json!(true),
            "client kwargs untouched"
        );
        assert_eq!(body["max_tokens"], json!(1536), "sub-floor ask raised");
    }

    #[test]
    fn thinking_controls_no_think_off_floors_only() {
        let mut body = Map::new();
        body.insert("max_tokens".to_owned(), json!(4096));
        // no-think OFF → thinking active, higher ask left untouched.
        let active = apply_thinking_controls(&mut body, false, 1536);
        assert!(active);
        assert!(!body.contains_key("chat_template_kwargs"));
        assert_eq!(body["max_tokens"], json!(4096), "higher ask untouched");
        // floor disabled (0) → no change even on a sub-floor ask.
        let mut body2 = Map::new();
        body2.insert("max_tokens".to_owned(), json!(10));
        apply_thinking_controls(&mut body2, false, 0);
        assert_eq!(body2["max_tokens"], json!(10));
    }
}
