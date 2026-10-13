//! `loom-bench gen|run|score`: the ADR-140 P1 pilot. See `bench/p1-pilot/PREREGISTRATION.md`.
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::too_many_arguments,
    items_after_statements,
    clippy::items_after_statements
)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use loom_bench::{bootstrap_ci, mean_delta, median, recall, Paired, Question, Rng, RunRecord};
use serde_json::{json, Value};

const DEFAULT_BASE: &str = "http://192.168.2.132:8084";
const SYSTEM_ARM4: &str = "You can look up the DreamLab ontology with the provided tools. \
Use them, then answer concisely and name the concepts explicitly.";

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    arg(args, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(900))
        .build()?)
}

async fn generation(h: &reqwest::Client, base: &str) -> Result<String> {
    let v: Value = h
        .get(format!("{base}/loom/generation"))
        .send()
        .await?
        .json()
        .await?;
    v.get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("no generation id"))
}

/// Call an `/mcp` tool and return its structured result.
async fn mcp(h: &reqwest::Client, base: &str, tool: &str, arguments: Value) -> Result<Value> {
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":tool,"arguments":arguments}});
    let v: Value = h
        .post(format!("{base}/mcp"))
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let res = v.get("result").ok_or_else(|| anyhow!("mcp error: {v}"))?;
    if let Some(r) = res.pointer("/structuredContent/result") {
        return Ok(r.clone());
    }
    let text = res
        .pointer("/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    Ok(serde_json::from_str(text).unwrap_or(Value::Null))
}

fn iri(s: &str) -> String {
    if s.contains(':') {
        s.to_owned()
    } else {
        format!("urn:ngm:class:{s}")
    }
}

fn strs(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

struct Graph<'a> {
    h: &'a reqwest::Client,
    base: &'a str,
    cache: HashMap<String, Value>,
}

impl Graph<'_> {
    async fn node(&mut self, i: &str) -> Result<&Value> {
        if !self.cache.contains_key(i) {
            let v = mcp(
                self.h,
                self.base,
                "loom.neighbours",
                json!({"iri": i, "limit": 1}),
            )
            .await?;
            self.cache.insert(i.to_owned(), v);
        }
        Ok(&self.cache[i])
    }
    async fn ancestors(&mut self, i: &str) -> Result<HashSet<String>> {
        let n = self.node(i).await?;
        Ok(strs(n, "is_a")
            .into_iter()
            .chain(strs(n, "ancestors"))
            .collect())
    }
    async fn title(&mut self, i: &str) -> Result<String> {
        let n = self.node(i).await?;
        n.get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("no title for {i}"))
    }
}

async fn cmd_gen(args: &[String]) -> Result<()> {
    let base = arg(args, "--base").unwrap_or_else(|| DEFAULT_BASE.to_owned());
    let out = arg(args, "--out").context("--out")?;
    let src = arg(args, "--lookup-from").context("--lookup-from bench/questions.jsonl")?;
    let seed: u64 = num(args, "--seed", 42);
    let (n_lookup, n_cross, n_path): (usize, usize, usize) = (
        num(args, "--lookup", 20),
        num(args, "--cross", 10),
        num(args, "--path", 10),
    );
    let h = http()?;
    let mut rng = Rng::new(seed);
    let gen_id = generation(&h, &base).await?;
    let mut qs: Vec<Question> = Vec::new();

    let mut lookup: Vec<Value> = fs::read_to_string(&src)?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    rng.shuffle(&mut lookup);
    for q in lookup.iter().take(n_lookup) {
        let gold: Vec<String> = q["gold"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| g["title"].as_str())
            .map(str::to_owned)
            .collect();
        qs.push(Question {
            id: String::new(),
            stratum: "lookup".into(),
            domain: q["domain"].as_str().unwrap_or("?").into(),
            prompt: q["prompt"].as_str().unwrap_or_default().into(),
            gold,
        });
    }

    let manifest = mcp(&h, &base, "loom.manifest", json!({"salience": 200})).await?;
    let pool: Vec<(String, String, String)> = manifest["salient"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s["iri"].as_str()?.to_owned(),
                s["title"].as_str()?.to_owned(),
                s["domain"].as_str()?.to_owned(),
            ))
        })
        .collect();
    if pool.len() < 40 {
        bail!("salient pool too small: {}", pool.len());
    }
    let mut g = Graph {
        h: &h,
        base: &base,
        cache: HashMap::new(),
    };
    let mut anc: HashMap<String, HashSet<String>> = HashMap::new();
    let mut freq: HashMap<String, usize> = HashMap::new();
    for (i, _, _) in &pool {
        let a = g.ancestors(i).await?;
        for x in &a {
            *freq.entry(x.clone()).or_default() += 1;
        }
        anc.insert(i.clone(), a);
    }
    let generic: HashSet<&String> = freq
        .iter()
        .filter(|(_, c)| **c * 4 > pool.len())
        .map(|(k, _)| k)
        .collect();

    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for a in 0..pool.len() {
        for b in a + 1..pool.len() {
            if pool[a].2 != pool[b].2 {
                pairs.push((a, b));
            }
        }
    }
    rng.shuffle(&mut pairs);
    let (mut used, mut cross, mut path): (HashSet<usize>, Vec<Question>, Vec<Question>) =
        (HashSet::new(), vec![], vec![]);
    for (a, b) in pairs.iter().copied() {
        if cross.len() >= n_cross && path.len() >= n_path {
            break;
        }
        if used.contains(&a) || used.contains(&b) {
            continue;
        }
        if cross.len() < n_cross {
            let shared: Vec<&String> = anc[&pool[a].0]
                .intersection(&anc[&pool[b].0])
                .filter(|x| !generic.contains(x))
                .collect();
            if (1..=5).contains(&shared.len()) {
                let mut gold = Vec::new();
                for s in shared {
                    gold.push(g.title(s).await?);
                }
                cross.push(Question { id: String::new(), stratum: "cross".into(), domain: pool[a].2.clone(), gold,
                    prompt: format!("In the DreamLab knowledge graph, name the broader concepts that both {} and {} fall under.", pool[a].1, pool[b].1) });
                used.insert(a);
                used.insert(b);
                continue;
            }
        }
        if path.len() < n_path && path.len() + cross.len() < 400 {
            let p = mcp(
                &h,
                &base,
                "loom.paths",
                json!({"from": pool[a].0, "to": pool[b].0, "max_hops": 4}),
            )
            .await?;
            let hops = p["hops"].as_u64().unwrap_or(0);
            if p["found"].as_bool() == Some(true) && (2..=3).contains(&hops) {
                let nodes = strs(&p, "path");
                let mut gold = Vec::new();
                for m in &nodes[1..nodes.len() - 1] {
                    gold.push(g.title(&iri(m)).await?);
                }
                path.push(Question { id: String::new(), stratum: "path".into(), domain: pool[a].2.clone(), gold,
                    prompt: format!("In the DreamLab knowledge graph, which concepts lie on the shortest chain of relationships linking {} to {}?", pool[a].1, pool[b].1) });
                used.insert(a);
                used.insert(b);
            }
        }
    }
    eprintln!(
        "generated lookup={} cross={} path={} on {gen_id}",
        qs.len(),
        cross.len(),
        path.len()
    );
    qs.extend(cross);
    qs.extend(path);
    rng.shuffle(&mut qs);
    let body: String = qs
        .iter_mut()
        .enumerate()
        .map(|(i, q)| {
            q.id = format!("q{:03}", i + 1);
            serde_json::to_string(q).map(|s| s + "\n")
        })
        .collect::<Result<_, _>>()?;
    fs::write(out, body)?;
    Ok(())
}

struct ArmOut {
    answer: String,
    tool_calls: u32,
    turns: u32,
    pt: u64,
    ct: u64,
    injected: u64,
    capped: bool,
    served: String,
    error: Option<String>,
}

fn usage(v: &Value) -> (u64, u64) {
    (
        v.pointer("/usage/prompt_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        v.pointer("/usage/completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    )
}

async fn chat(h: &reqwest::Client, base: &str, body: &Value) -> Result<Value> {
    let r = h
        .post(format!("{base}/v1/chat/completions"))
        .json(body)
        .send()
        .await?;
    if !r.status().is_success() {
        bail!(
            "chat {}: {}",
            r.status(),
            r.text().await.unwrap_or_default()
        );
    }
    Ok(r.json().await?)
}

async fn arm3(
    h: &reqwest::Client,
    base: &str,
    model: &str,
    prompt: &str,
    max_tokens: u64,
) -> ArmOut {
    let body = json!({"model": model, "messages":[{"role":"user","content":prompt}], "max_tokens": max_tokens, "temperature": 0, "loom_options":{"verbatim": false}});
    match chat(h, base, &body).await {
        Ok(v) => {
            let (pt, ct) = usage(&v);
            ArmOut {
                answer: v
                    .pointer("/choices/0/message/content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                tool_calls: 0,
                turns: 1,
                pt,
                ct,
                injected: v
                    .pointer("/loom/injected_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                capped: false,
                served: v
                    .pointer("/loom/served_mode")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_owned(),
                error: None,
            }
        }
        Err(e) => ArmOut {
            answer: String::new(),
            tool_calls: 0,
            turns: 0,
            pt: 0,
            ct: 0,
            injected: 0,
            capped: false,
            served: "?".into(),
            error: Some(e.to_string()),
        },
    }
}

async fn arm4(
    h: &reqwest::Client,
    base: &str,
    model: &str,
    prompt: &str,
    max_tokens: u64,
    cap: u32,
    tools: &Value,
) -> ArmOut {
    let mut o = ArmOut {
        answer: String::new(),
        tool_calls: 0,
        turns: 0,
        pt: 0,
        ct: 0,
        injected: 0,
        capped: false,
        served: "agentic".into(),
        error: None,
    };
    let mut messages = vec![
        json!({"role":"system","content":SYSTEM_ARM4}),
        json!({"role":"user","content":prompt}),
    ];
    loop {
        let body = json!({"model": model, "messages": messages, "tools": tools, "max_tokens": max_tokens, "temperature": 0, "loom_options":{"scaffold": false}});
        let v = match chat(h, base, &body).await {
            Ok(v) => v,
            Err(e) => {
                o.error = Some(e.to_string());
                return o;
            }
        };
        o.turns += 1;
        let (pt, ct) = usage(&v);
        o.pt += pt;
        o.ct += ct;
        let msg = v
            .pointer("/choices/0/message")
            .cloned()
            .unwrap_or(Value::Null);
        o.answer = msg
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let calls = msg
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if calls.is_empty() {
            return o;
        }
        if o.tool_calls >= cap {
            o.capped = true;
            return o;
        }
        messages.push(msg.clone());
        for c in calls {
            o.tool_calls += 1;
            let name = c
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .replacen('_', ".", 1);
            let args: Value = c
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| json!({}));
            let text = match mcp(h, base, &name, args).await {
                Ok(r) => r.to_string(),
                Err(e) => format!("{{\"error\":\"{e}\"}}"),
            };
            messages.push(json!({"role":"tool","tool_call_id": c.get("id").cloned().unwrap_or(Value::Null),"content": text}));
        }
    }
}

async fn cmd_run(args: &[String]) -> Result<()> {
    let base = arg(args, "--base").unwrap_or_else(|| DEFAULT_BASE.to_owned());
    let qpath = arg(args, "--questions").context("--questions")?;
    let out = arg(args, "--out").context("--out")?;
    let model = arg(args, "--model").unwrap_or_else(|| "qwen3.8-27b-heretic-q8_0".to_owned());
    let (seed, cap, max_tokens, limit): (u64, u32, u64, usize) = (
        num(args, "--seed", 42),
        num(args, "--max-tool-calls", 8),
        num(args, "--max-tokens", 2048),
        num(args, "--limit", usize::MAX),
    );
    let h = http()?;
    let qs: Vec<Question> = fs::read_to_string(&qpath)?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let done: HashSet<(String, String)> = fs::read_to_string(&out)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<RunRecord>(l).ok())
        .map(|r| (r.id, r.arm))
        .collect();
    let listed: Value = h
        .post(format!("{base}/mcp"))
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
        .send()
        .await?
        .json()
        .await?;
    let tools: Value = listed.pointer("/result/tools").and_then(Value::as_array).context("tools/list")?.iter()
        .map(|t| json!({"type":"function","function":{"name": t["name"].as_str().unwrap_or_default().replacen('.', "_", 1),"description": t["description"],"parameters": t["inputSchema"]}})).collect();
    let pinned = generation(&h, &base).await?;
    eprintln!("pinned generation {pinned}");
    let mut rng = Rng::new(seed);
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out)?;
    for q in qs.iter().take(limit) {
        let arm4_first = rng.next_u64() & 1 == 1; // drawn for every question so resuming keeps the schedule
        for step in 0..2u8 {
            let is4 = arm4_first == (step == 0);
            let name = if is4 { "arm4" } else { "arm3" };
            if done.contains(&(q.id.clone(), name.to_owned())) {
                continue;
            }
            let now = generation(&h, &base).await?;
            if now != pinned {
                bail!("generation changed mid-run: {pinned} -> {now}; stopping");
            }
            let t = Instant::now();
            let r = if is4 {
                arm4(&h, &base, &model, &q.prompt, max_tokens, cap, &tools).await
            } else {
                arm3(&h, &base, &model, &q.prompt, max_tokens).await
            };
            let rec = RunRecord {
                id: q.id.clone(),
                stratum: q.stratum.clone(),
                arm: name.into(),
                order: step,
                generation: now,
                answer: r.answer,
                tool_calls: r.tool_calls,
                turns: r.turns,
                prompt_tokens: r.pt,
                completion_tokens: r.ct,
                injected_tokens: r.injected,
                wall_ms: u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX),
                capped: r.capped,
                served_mode: r.served,
                error: r.error,
            };
            use std::io::Write;
            writeln!(f, "{}", serde_json::to_string(&rec)?)?;
            eprintln!(
                "{} {name} turns={} tools={} tok={} {}ms{}",
                q.id,
                rec.turns,
                rec.tool_calls,
                rec.prompt_tokens + rec.completion_tokens,
                rec.wall_ms,
                rec.error
                    .as_deref()
                    .map(|e| format!(" ERR {e}"))
                    .unwrap_or_default()
            );
        }
    }
    if generation(&h, &base).await? != pinned {
        bail!("generation changed during the run");
    }
    Ok(())
}

fn cmd_score(args: &[String]) -> Result<()> {
    let qs: Vec<Question> = fs::read_to_string(arg(args, "--questions").context("--questions")?)?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let recs: Vec<RunRecord> = fs::read_to_string(arg(args, "--runs").context("--runs")?)?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let get = |id: &str, arm: &str| recs.iter().find(|r| r.id == id && r.arm == arm);
    // `prompt_tokens` already includes the injected scaffold (verified on a dry run: ~1,050 against a ~27-token bare prompt).
    let tot = |r: &RunRecord| r.prompt_tokens + r.completion_tokens;
    println!("| stratum | n | arm3 recall | arm4 recall | delta | naive 95% CI | clustered 95% CI | tokens a3 / a4 (median) | turns a4 (median) | wall s a3 / a4 (median) |\n|---|---|---|---|---|---|---|---|---|---|");
    let mut strata: Vec<&str> = vec!["lookup", "cross", "path"];
    strata.push("ALL");
    for s in strata {
        let mut paired = Vec::new();
        let (mut r3, mut r4, mut t3, mut t4, mut tu, mut w3, mut w4) =
            (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
        for q in qs.iter().filter(|q| s == "ALL" || q.stratum == s) {
            let (Some(a), Some(b)) = (get(&q.id, "arm3"), get(&q.id, "arm4")) else {
                continue;
            };
            let (x, y) = (recall(&a.answer, &q.gold), recall(&b.answer, &q.gold));
            paired.push(Paired {
                cluster: q.domain.clone(),
                delta: y - x,
            });
            r3.push(x);
            r4.push(y);
            #[allow(clippy::cast_precision_loss)]
            {
                t3.push(tot(a) as f64);
                t4.push(tot(b) as f64);
                tu.push(f64::from(b.turns));
                w3.push(a.wall_ms as f64 / 1000.0);
                w4.push(b.wall_ms as f64 / 1000.0);
            }
        }
        if paired.is_empty() {
            continue;
        }
        let (n1, n2) = (
            bootstrap_ci(&paired, false, 10_000, 7),
            bootstrap_ci(&paired, true, 10_000, 7),
        );
        #[allow(clippy::cast_precision_loss)]
        let m = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        println!("| {s} | {} | {:.2} | {:.2} | {:+.2} | [{:+.2}, {:+.2}] | [{:+.2}, {:+.2}] | {:.0} / {:.0} | {:.0} | {:.0} / {:.0} |", paired.len(), m(&r3), m(&r4), mean_delta(&paired), n1.0, n1.1, n2.0, n2.1, median(&t3), median(&t4), median(&tu), median(&w3), median(&w4));
    }
    let gens: HashSet<&str> = recs.iter().map(|r| r.generation.as_str()).collect();
    println!("\nruns={} generations={gens:?}", recs.len());
    println!(
        "arm3 served verbatim (model never ran): {}",
        recs.iter()
            .filter(|r| r.arm == "arm3" && r.served_mode == "verbatim")
            .count()
    );
    #[allow(clippy::cast_precision_loss)]
    let inj: Vec<f64> = recs
        .iter()
        .filter(|r| r.arm == "arm3")
        .map(|r| r.injected_tokens as f64)
        .collect();
    println!(
        "arm3 median injected_tokens (façade estimate, already inside prompt_tokens): {:.0}",
        median(&inj)
    );
    println!(
        "arm4 capped: {}  errors: {}",
        recs.iter().filter(|r| r.arm == "arm4" && r.capped).count(),
        recs.iter().filter(|r| r.error.is_some()).count()
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gen") => cmd_gen(&args).await,
        Some("run") => cmd_run(&args).await,
        Some("score") => cmd_score(&args),
        _ => bail!("usage: loom-bench gen|run|score --questions F [--runs F --out F --base URL --model M --seed N --max-tool-calls N --limit N --lookup-from F]"),
    }
}
