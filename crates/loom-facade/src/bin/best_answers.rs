//! `best-answers` — the collector for the PRD-025 distillation loop (Amendment
//! 2026-10-02, first new part).
//!
//! It selects "the Loom's best answers" from scored uplift-bench runs, using a
//! definition an existing evaluator already judges: the frozen gold-recall
//! scorer behind `uplift-results/` (see `bench/UPLIFT-BENCH-PROTOCOL.md`). A row
//! is BEST when it was served through the scaffold (`mode = scaffold`,
//! `scaffold_engaged = true`), finished normally (`finish_reason = stop`) and
//! recovered every gold item (`recall = 1.0`). Nothing here judges prose.
//!
//! Questions are split by their first class slug, so a held-out question never
//! shares a class with an exemplar: `sha256(slug)[0] % 5 == 0` is held out.
//! Only train-split rows are emitted; the held-out ids go to stderr for the A/B.
//!
//!     cargo run -q -p loom-facade --bin best-answers -- \
//!         uplift-results/questions.jsonl uplift-results/**/scores-*.jsonl > best.jsonl
//!
//! Each `scores-X.jsonl` is joined to its sibling `results-X.jsonl` for the
//! answer text. Output is one JSON object per question, ready for a RuVector
//! `memory_store` into the `loom-best-answers` namespace. `LOOM_DISTIL=0` is the
//! kill switch: the bin prints `DISTIL-OFF` and emits nothing.

use std::collections::{BTreeMap, HashMap};
use std::process::ExitCode;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Namespace the emitted rows are written to.
const NAMESPACE: &str = "loom-best-answers";

/// True when the question's first class slug falls in the held-out fifth.
fn held_out(slug: &str) -> bool {
    Sha256::digest(slug.as_bytes())[0] % 5 == 0
}

/// The BEST predicate over one score row.
fn is_best(score: &Value) -> bool {
    score["mode"] == "scaffold"
        && score["scaffold_engaged"] == true
        && score["finish_reason"] == "stop"
        && score["recall"].as_f64() == Some(1.0)
}

fn parse_jsonl(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Joins scores to answers and keeps the shortest best answer per train question.
/// Returns `(emitted rows, held-out question ids)`.
fn collect(questions: &[Value], runs: &[(Vec<Value>, Vec<Value>)]) -> (Vec<Value>, Vec<String>) {
    let mut held = Vec::new();
    let mut train: HashMap<&str, &Value> = HashMap::new();
    for q in questions {
        let (Some(id), Some(slug)) = (q["id"].as_str(), q["class_slugs"][0].as_str()) else {
            continue;
        };
        if held_out(slug) {
            held.push(id.to_owned())
        } else {
            train.insert(id, q);
        }
    }
    let mut best: BTreeMap<String, (String, String, usize)> = BTreeMap::new();
    for (scores, results) in runs {
        let answers: HashMap<(&str, &str), &str> = results
            .iter()
            .filter_map(|r| {
                Some((
                    (r["id"].as_str()?, r["model"].as_str()?),
                    r["answer"].as_str()?,
                ))
            })
            .collect();
        for s in scores.iter().filter(|s| is_best(s)) {
            let (Some(id), Some(model)) = (s["id"].as_str(), s["model"].as_str()) else {
                continue;
            };
            let (true, Some(answer)) = (train.contains_key(id), answers.get(&(id, model))) else {
                continue;
            };
            let entry = best
                .entry(id.to_owned())
                .or_insert_with(|| (model.to_owned(), (*answer).to_owned(), 0));
            entry.2 += 1;
            if answer.len() < entry.1.len() {
                (entry.0, entry.1) = (model.to_owned(), (*answer).to_owned());
            }
        }
    }
    let rows = best
        .into_iter()
        .map(|(id, (model, answer, support))| {
            let q = train[id.as_str()];
            json!({
                "namespace": NAMESPACE,
                "key": format!("loom-best:{id}"),
                "value": format!("Q: {}\nA: {}", q["prompt"].as_str().unwrap_or(""), answer),
                "meta": { "id": id, "model": model, "support": support, "domain": q["domain"],
                          "template": q["template"], "class_slugs": q["class_slugs"],
                          "judge": "uplift gold-recall = 1.0, scaffold engaged" }
            })
        })
        .collect();
    (rows, held)
}

fn main() -> ExitCode {
    if std::env::var("LOOM_DISTIL").as_deref() == Ok("0") {
        println!("DISTIL-OFF");
        return ExitCode::SUCCESS;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((qpath, scores)) = args.split_first() else {
        eprintln!("usage: best-answers <questions.jsonl> <scores-*.jsonl>...");
        return ExitCode::from(2);
    };
    let read = |p: &str| {
        std::fs::read_to_string(p)
            .map_err(|e| eprintln!("{p}: {e}"))
            .ok()
    };
    let Some(questions) = read(qpath).map(|t| parse_jsonl(&t)) else {
        return ExitCode::from(2);
    };
    let mut runs = Vec::new();
    for s in scores {
        let (Some(st), Some(rt)) = (read(s), read(&s.replace("scores-", "results-"))) else {
            return ExitCode::from(2);
        };
        runs.push((parse_jsonl(&st), parse_jsonl(&rt)));
    }
    let (rows, held) = collect(&questions, &runs);
    for r in &rows {
        println!("{r}");
    }
    eprintln!(
        "BEST-ANSWERS rows={} held_out={} runs={}",
        rows.len(),
        held.len(),
        runs.len()
    );
    eprintln!("HELD-OUT {}", held.join(","));
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(id: &str, slug: &str) -> Value {
        json!({"id": id, "class_slugs": [slug], "prompt": format!("P{id}"), "domain": "ai", "template": "T-REL"})
    }
    fn score(id: &str, model: &str, recall: f64) -> Value {
        json!({"id": id, "model": model, "mode": "scaffold", "scaffold_engaged": true, "finish_reason": "stop", "recall": recall})
    }
    fn answer(id: &str, model: &str, text: &str) -> Value {
        json!({"id": id, "model": model, "answer": text})
    }
    /// A slug in each split, found by search so the test does not hard-code hashes.
    fn slugs() -> (String, String) {
        let find = |want| {
            (0..)
                .map(|i| format!("s{i}"))
                .find(|s| held_out(s) == want)
                .unwrap()
        };
        (find(false), find(true))
    }

    #[test]
    fn predicate_requires_full_recall_scaffold_and_stop() {
        assert!(is_best(&score("a", "m", 1.0)));
        assert!(!is_best(&score("a", "m", 0.8)));
        let mut raw = score("a", "m", 1.0);
        raw["mode"] = json!("raw");
        assert!(!is_best(&raw));
        let mut cut = score("a", "m", 1.0);
        cut["finish_reason"] = json!("length");
        assert!(!is_best(&cut));
    }

    #[test]
    fn keeps_shortest_train_answer_and_withholds_held_out() {
        let (train, held) = slugs();
        let qs = [q("q1", &train), q("q2", &held)];
        let runs = [
            (
                vec![score("q1", "m1", 1.0), score("q2", "m1", 1.0)],
                vec![answer("q1", "m1", "a long answer"), answer("q2", "m1", "x")],
            ),
            (
                vec![score("q1", "m2", 1.0)],
                vec![answer("q1", "m2", "short")],
            ),
        ];
        let (rows, held_ids) = collect(&qs, &runs);
        assert_eq!(held_ids, ["q2"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["key"], "loom-best:q1");
        assert_eq!(rows[0]["meta"]["model"], "m2");
        assert_eq!(rows[0]["meta"]["support"], 2);
        assert_eq!(rows[0]["value"], "Q: Pq1\nA: short");
    }
}
