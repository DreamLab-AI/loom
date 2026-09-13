//! The reasoning-model token floor.
//!
//! A reasoning model spends its budget in `reasoning_content` before it writes
//! a word of visible answer. Give it too small a `max_tokens` and it returns
//! HTTP 200 with **empty** content — not a short answer, no answer, and no
//! error to notice. Measured against Qwen3.8-27B: 400 tokens yields empty,
//! 1536 is safe.
//!
//! So the floor only ever *raises* an ask. Lowering one would silently shrink
//! a caller's deliberate budget, which is the same failure in the other
//! direction.

use serde_json::{Map, Value};

/// Default floor. Reasoning backends emit empty content below roughly this
/// budget; `0` disables flooring entirely.
pub const DEFAULT_MIN_MAX_TOKENS: u64 = 1536;

/// Raise any sub-`floor` **integer** `max_tokens`/`max_completion_tokens` in
/// `map` to `floor`, in place. Returns whether either key was present, at any
/// type — which is the caller's signal for whether to *insert* a budget.
///
/// Only JSON integers are floored. Strings, floats, `null` and numbers beyond
/// `u64` pass through exactly as sent: a caller who wrote `"max_tokens": "8000"`
/// meant something, and coercing it to the floor would *lower* it. Negative
/// integers do raise, matching `max(v, floor)`.
///
/// This mirrors the façade's own pre-delegation rewrite, so a request floored
/// here and a request floored there come out identical.
///
/// ```
/// # use loom_client::raise_integer_token_floor;
/// # use serde_json::json;
/// let mut body = json!({ "max_tokens": 400 }).as_object().unwrap().clone();
/// assert!(raise_integer_token_floor(&mut body, 1536));
/// assert_eq!(body["max_tokens"], 1536);
///
/// // A larger ask is left alone.
/// let mut body = json!({ "max_tokens": 8000 }).as_object().unwrap().clone();
/// raise_integer_token_floor(&mut body, 1536);
/// assert_eq!(body["max_tokens"], 8000);
/// ```
#[must_use]
pub fn raise_integer_token_floor(map: &mut Map<String, Value>, floor: u64) -> bool {
    let floor_i = i128::from(floor);
    let mut saw_any = false;
    for field in ["max_tokens", "max_completion_tokens"] {
        let Some(slot) = map.get_mut(field) else {
            continue;
        };
        saw_any = true;
        let current = slot
            .as_i64()
            .map(i128::from)
            .or_else(|| slot.as_u64().map(i128::from));
        if let Some(current) = current {
            if current < floor_i {
                *slot = Value::from(floor);
            }
        }
    }
    saw_any
}

/// Apply `floor` to `map`, inserting `max_tokens = floor` when **neither**
/// token key is present. A `floor` of `0` is a no-op.
///
/// Insertion is guarded on key *presence*, not on the value being usable: a
/// caller who explicitly sent `"max_tokens": null` has said something, and
/// adding a second budget key next to it would be a surprise.
pub fn apply_token_floor(map: &mut Map<String, Value>, floor: u64) {
    if floor == 0 {
        return;
    }
    if !raise_integer_token_floor(map, floor) {
        map.insert("max_tokens".to_owned(), Value::from(floor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: &Value) -> Map<String, Value> {
        v.as_object().expect("test value is an object").clone()
    }

    #[test]
    fn raises_a_sub_floor_integer() {
        let mut m = obj(&json!({ "max_tokens": 400 }));
        assert!(raise_integer_token_floor(&mut m, 1536));
        assert_eq!(m["max_tokens"], json!(1536));
    }

    #[test]
    fn never_lowers_a_larger_ask() {
        let mut m = obj(&json!({ "max_tokens": 12288 }));
        let _ = raise_integer_token_floor(&mut m, 1536);
        assert_eq!(m["max_tokens"], json!(12288));
    }

    #[test]
    fn raises_a_negative_like_python_max() {
        let mut m = obj(&json!({ "max_tokens": -1 }));
        let _ = raise_integer_token_floor(&mut m, 1536);
        assert_eq!(m["max_tokens"], json!(1536));
    }

    #[test]
    fn leaves_non_integers_exactly_as_sent() {
        // The regression this guards: coercing a string ask to the floor would
        // LOWER a deliberate 8000-token budget to 1536.
        for sent in [json!("8000"), json!(2048.5), json!(null)] {
            let mut m = obj(&json!({ "max_tokens": sent }));
            let before = m["max_tokens"].clone();
            let _ = raise_integer_token_floor(&mut m, 1536);
            assert_eq!(m["max_tokens"], before, "mutated a non-integer ask");
        }
    }

    #[test]
    fn floors_max_completion_tokens_too() {
        let mut m = obj(&json!({ "max_completion_tokens": 10 }));
        let _ = raise_integer_token_floor(&mut m, 1536);
        assert_eq!(m["max_completion_tokens"], json!(1536));
    }

    #[test]
    fn inserts_only_when_both_keys_are_absent() {
        let mut m = obj(&json!({ "model": "x" }));
        apply_token_floor(&mut m, 1536);
        assert_eq!(m["max_tokens"], json!(1536));

        // Present-but-null counts as present: no second key appears.
        let mut m = obj(&json!({ "max_completion_tokens": null }));
        apply_token_floor(&mut m, 1536);
        assert!(!m.contains_key("max_tokens"), "inserted alongside an explicit key");
    }

    #[test]
    fn a_zero_floor_disables_everything() {
        let mut m = obj(&json!({ "max_tokens": 1 }));
        apply_token_floor(&mut m, 0);
        assert_eq!(m["max_tokens"], json!(1));

        let mut m = obj(&json!({}));
        apply_token_floor(&mut m, 0);
        assert!(m.is_empty());
    }
}
