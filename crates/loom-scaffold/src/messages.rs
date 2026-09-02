//! `scaffold_messages` — the OpenAI chat-array merge. Split out of `lib.rs` so
//! neither file crosses the 500-line ceiling; the public path is unchanged
//! (`loom_scaffold::scaffold_messages`), re-exported from the crate root.

use crate::index::ScaffoldIndex;
use crate::policy::GatePolicy;
use crate::prose::ProseIndex;
use crate::scaffold_block;
use crate::tuning::SYSTEM_PREAMBLE;

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
    policy: &GatePolicy,
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
