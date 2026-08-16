#!/usr/bin/env python3
"""Run lower-tier models on GENERAL-knowledge questions, BARE vs UNDER THE
ONTOLOGY HARNESS, to test whether the confidence-gated harness helps a cheap
model on general questions or makes it jagged.

The harness context is the LIVE Loom's confidence-gated scaffold: for each
question we POST /loom/scaffold; the gate returns real structure for
in-domain-adjacent questions and (by design) nothing for off-domain ones. We
inject exactly what a deployed consumer would receive, and record the `engaged`
flag so non-jaggedness (gate correctly skips) is measurable.

Conditions per model: 'bare' (question only) and 'harness' (question + gated
scaffold). Answers are judged later against the general-knowledge gold.

Usage:
  PYTHONPATH=app python3 bench/quality/harness_arm.py \
      --questions uplift-results/general/general-questions.json \
      --loom http://192.168.2.132:8084 --outdir uplift-results/general \
      --models gemini-2.5-flash-lite,gpt-4.1-mini,claude-haiku-4.5,mistral-small-24b,deepseek-chat
"""
from __future__ import annotations
import argparse, json, os, sys, time, urllib.request, urllib.error

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import bench_ontology_uplift as bu  # chat_request  # noqa

GEM = "https://generativelanguage.googleapis.com/v1beta/openai/"
OR = "https://openrouter.ai/api/v1"
DS = "https://api.deepseek.com/v1"
# label -> (base, model_id, key_env, reasoning_effort)
MODELS = {
    "gemini-2.5-flash-lite": (GEM, "gemini-2.5-flash-lite", "GOOGLE_API_KEY", None),
    "gemini-3.5-flash-lite": (GEM, "gemini-3.5-flash-lite", "GOOGLE_API_KEY", "low"),
    "gpt-4.1-mini": (OR, "openai/gpt-4.1-mini", "OPENROUTER_API_KEY", None),
    "claude-haiku-4.5": (OR, "anthropic/claude-haiku-4.5", "OPENROUTER_API_KEY", None),
    "mistral-small-24b": (OR, "mistralai/mistral-small-24b-instruct-2501", "OPENROUTER_API_KEY", None),
    "llama-3.3-70b": (OR, "meta-llama/llama-3.3-70b-instruct", "OPENROUTER_API_KEY", None),
    "qwen-2.5-72b": (OR, "qwen/qwen-2.5-72b-instruct", "OPENROUTER_API_KEY", None),
    "deepseek-chat": (DS, "deepseek-chat", "DEEPSEEK_API_KEY", None),
    # local Qwen3.8-27B behind the Loom on HP — the actual deployed production
    # model. Raw endpoint :8085 (no auth); harness = raw + client-injected gated
    # scaffold, same method as the cloud models. Thinking is ON, so it needs more
    # token headroom (pass --max-tokens 2048).
    "qwen3.8-local": ("http://192.168.2.132:8085/v1", "qwen3.8-27B", None, None),
}

HARNESS_PREAMBLE = (
    "The following ontology context was retrieved from a curated knowledge graph. "
    "Where relevant to the question, treat it as ground truth for definitions and "
    "relationships; otherwise answer from your own knowledge.\n\n[ONTOLOGY CONTEXT]\n")


def loom_scaffold(loom, prompt, budget=700, timeout=20):
    body = json.dumps({"prompt": prompt, "budget_tokens": budget}).encode()
    req = urllib.request.Request(loom.rstrip("/") + "/loom/scaffold", data=body,
                                 method="POST", headers={"Content-Type": "application/json"})
    try:
        d = json.load(urllib.request.urlopen(req, timeout=timeout))
        return (d.get("scaffold") or ""), bool(d.get("engaged")), d.get("approx_tokens")
    except Exception as e:
        return "", False, f"err:{str(e)[:60]}"


def run_model(label, question, scaffold, engaged, max_tokens=800):
    base, model, keyenv, effort = MODELS[label]
    key = os.environ.get(keyenv) if keyenv else None
    results = {}
    for cond in ("bare", "harness"):
        if cond == "harness" and engaged and scaffold:
            content = HARNESS_PREAMBLE + scaffold + "\n\n[QUESTION]\n" + question
        else:
            content = question  # bare, or harness-but-gate-skipped (== bare, the non-jagged case)
        payload = {"model": model, "messages": [{"role": "user", "content": content}],
                   "temperature": 0, "max_tokens": max_tokens}
        if effort:
            payload["reasoning_effort"] = effort
        stats = {}
        try:
            t0 = time.perf_counter()
            resp = bu.chat_request(base, payload, 120, 3, auth_bearer=key, stats=stats)
            lat = round((time.perf_counter() - t0) * 1000)
            ans = resp["choices"][0]["message"].get("content") or ""
            results[cond] = {"answer": ans, "latency_ms": lat, "attempts": stats.get("attempts"),
                             "harness_engaged": bool(cond == "harness" and engaged and scaffold),
                             "finish_reason": resp["choices"][0].get("finish_reason")}
        except Exception as e:
            results[cond] = {"error": str(e)[:150], "harness_engaged": bool(engaged)}
    return results


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--questions", required=True)
    ap.add_argument("--loom", default="http://192.168.2.132:8084")
    ap.add_argument("--outdir", required=True)
    ap.add_argument("--models", required=True)
    ap.add_argument("--max-tokens", type=int, default=800,
                    help="answer budget; use 2048 for thinking models like qwen3.8-local")
    ap.add_argument("--sleep", type=float, default=0.25)
    args = ap.parse_args()
    os.makedirs(args.outdir, exist_ok=True)
    qs = json.load(open(args.questions))
    models = [m.strip() for m in args.models.split(",") if m.strip() in MODELS]

    # 1. fetch the gated harness scaffold once per question (shared across models)
    scaf = {}
    n_eng = 0
    for q in qs:
        s, eng, tok = loom_scaffold(args.loom, q["question"])
        scaf[q["id"]] = {"scaffold": s, "engaged": eng, "approx_tokens": tok}
        n_eng += int(eng)
    json.dump(scaf, open(os.path.join(args.outdir, "harness-scaffolds.json"), "w"), indent=2)
    print(f"harness: {n_eng}/{len(qs)} questions engaged the confidence gate", file=sys.stderr)

    # 2. run each model bare + harness
    for label in models:
        out = {"bare": [], "harness": []}
        for i, q in enumerate(qs, 1):
            sc = scaf[q["id"]]
            r = run_model(label, q["question"], sc["scaffold"], sc["engaged"], max_tokens=args.max_tokens)
            for cond in ("bare", "harness"):
                out[cond].append({"id": q["id"], "model": label, "category": q.get("category"),
                                  **r[cond]})
            if i % 15 == 0:
                print(f"  {label}: {i}/{len(qs)}", file=sys.stderr)
            if args.sleep:
                time.sleep(args.sleep)
        for cond in ("bare", "harness"):
            path = os.path.join(args.outdir, f"results-{label}-{cond}.jsonl")
            with open(path, "w") as fh:
                for row in out[cond]:
                    fh.write(json.dumps(row, ensure_ascii=False) + "\n")
        print(f"wrote {label} bare+harness ({len(qs)} each)", file=sys.stderr)
    print(f"done: {len(models)} models, {n_eng}/{len(qs)} gate-engaged")


if __name__ == "__main__":
    raise SystemExit(main())
