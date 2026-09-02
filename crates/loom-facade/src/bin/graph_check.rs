//! `graph-check` — the dream-cycle graph evaluator: assert the facade's graph
//! store is loaded and queryable. Rust port of the retired
//! `tests/graph_check.py`.
//!
//!     curl -s --max-time 10 http://127.0.0.1:8084/health \
//!         | cargo run -q -p loom-facade --bin graph-check
//!
//! Reads `/health` on stdin rather than taking a URL so the `dream.config.json`
//! entrypoint needs no inline quoting — inline `python3 -c "…"` lost its inner
//! quotes crossing the annexe ssh `bash -lc` boundary (witnessed 2026-08-28,
//! `SyntaxError` every night it ran). The stdin contract is preserved verbatim.
//!
//! Drift fixed in the port: the Python printed `engine:` from `graph.engine`,
//! which `GraphStatus` (loom-domain `model.rs`) has never emitted — it printed
//! `None` every night. The port asserts the four fields `GraphStatus` actually
//! serialises: `available`, `triples`, `loaded_files`, `error`.
//!
//! Typed-struct note: `loom_facade::routes::health::HealthResponse` types the
//! `/health` body, but deliberately leaves `graph` as a `Value` — it is a
//! re-serialised `GraphStatus`, which is `Serialize`-only in `loom-domain`, so
//! there is no `Deserialize` to borrow. This bin therefore reads the block
//! untyped, and `confidence-check` owns the typed shape gate for the whole
//! payload.

use std::io::Read;
use std::process::ExitCode;

use serde_json::Value;

const OK: &str = "GRAPH-SCAN-OK";
const FAIL: &str = "GRAPH-SCAN-FAIL";

fn main() -> ExitCode {
    let mut raw = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
        eprintln!("{FAIL}: unreadable stdin: {e}");
        return ExitCode::FAILURE;
    }
    let health: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{FAIL}: /health is not JSON: {e}");
            eprintln!("{FAIL}: first 200 bytes: {}", &raw[..raw.len().min(200)]);
            return ExitCode::FAILURE;
        }
    };

    match scan(&health) {
        Ok(summary) => {
            println!("{OK} {summary}");
            ExitCode::SUCCESS
        }
        Err(findings) => {
            for f in &findings {
                eprintln!("{FAIL}: {f}");
            }
            ExitCode::FAILURE
        }
    }
}

fn scan(health: &Value) -> Result<String, Vec<String>> {
    let mut f: Vec<String> = Vec::new();

    let Some(graph) = health.get("graph").and_then(Value::as_object) else {
        return Err(vec![
            "/health.graph is missing or not an object — the facade is not \
             reporting a graph adapter at all"
                .to_owned(),
        ]);
    };

    let available = graph.get("available").and_then(Value::as_bool);
    if available.is_none() {
        f.push(format!(
            "graph.available is {} — expected a boolean",
            graph.get("available").unwrap_or(&Value::Null)
        ));
    }

    let triples = graph.get("triples").and_then(Value::as_u64);
    if triples.is_none() {
        f.push(format!(
            "graph.triples is {} — expected a non-negative integer",
            graph.get("triples").unwrap_or(&Value::Null)
        ));
    }

    let loaded: Vec<String> = match graph.get("loaded_files") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().unwrap_or("<non-string>").to_owned())
            .collect(),
        other => {
            f.push(format!(
                "graph.loaded_files is {} — expected an array of file names",
                other.unwrap_or(&Value::Null)
            ));
            Vec::new()
        }
    };

    // `error` is `Option<String>`: null on a healthy store, a message otherwise.
    let error = match graph.get("error") {
        None => {
            f.push("graph.error is MISSING — expected null or a message".to_owned());
            None
        }
        Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(v) => {
            f.push(format!("graph.error is {v} — expected null or a string"));
            None
        }
    };

    // The evaluator's actual verdict, byte-parity with the Python exit rule:
    // available AND triples > 0. The error surface is an added assertion.
    if available == Some(false) {
        f.push("graph.available is false — the store did not load".to_owned());
    }
    if triples == Some(0) {
        f.push(
            "graph.triples is 0 — the store loaded nothing (check the mounted \
             generation dir; see README 'empty floor trap')"
                .to_owned(),
        );
    }
    if let Some(msg) = &error {
        f.push(format!("graph.error is set: {msg}"));
    }

    if f.is_empty() {
        Ok(format!(
            "triples:{} available:{} loaded:[{}]",
            triples.unwrap_or(0),
            available.unwrap_or(false),
            loaded.join(", "),
        ))
    } else {
        Err(f)
    }
}
