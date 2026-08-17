#!/usr/bin/env python3
"""Paired live-ecosystem harness for the paper-v2 experiments.

Isolates THE ecosystem feature — ontology-grounded scaffold serving — by holding
the model constant (Qwen3.8-27B on HP GPUs) and varying only the serving path:

  loom arm : http://192.168.2.132:8084/v1/chat/completions
             (production Rust loom; confidence-gated scaffold injection;
             per-answer telemetry in the response `loom` block)
  raw arm  : http://127.0.0.1:18085/v1/chat/completions
             (SSH tunnel to the same model, no scaffold — the bare backend)

Same question, same params, paired by id. Runs the three frozen question sets
(arcane / thin / general) used by the v1 sweep so results are judge-comparable.

Usage:
  ssh -f -N -L 18085:127.0.0.1:8085 john@10.10.10.1     # once, for the raw arm
  python3 tools/paper/live_harness.py --out uplift-results/paper-v2
"""
from __future__ import annotations
import argparse, json, sys, time, urllib.request, urllib.error
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

LOOM = "http://192.168.2.132:8084/v1/chat/completions"
RAW = "http://127.0.0.1:18085/v1/chat/completions"
SETS = {
    "arcane": "uplift-results/general/arcane-questions.json",
    "thin": "uplift-results/general/thin-questions.json",
    "general": "uplift-results/general/general-questions.json",
}
MAX_TOKENS = 1536  # protocol: reasoning backends truncate to empty below this


def ask(url: str, question: str, timeout: int = 300) -> dict:
    body = {
        "model": "loom",
        "messages": [{"role": "user", "content": question}],
        "max_tokens": MAX_TOKENS,
        "temperature": 0.0,
    }
    req = urllib.request.Request(url, data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"}, method="POST")
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=timeout) as r:
        d = json.load(r)
    latency = time.time() - t0
    content = (d.get("choices") or [{}])[0].get("message", {}).get("content", "")
    return {
        "content": content,
        "latency_s": round(latency, 2),
        "completion_tokens": d.get("usage", {}).get("completion_tokens"),
        "model": d.get("model"),
        "loom": d.get("loom"),  # None on the raw arm
    }


def run_one(setname: str, q: dict, arm: str, url: str, retries: int = 3) -> dict:
    last = None
    for attempt in range(retries):
        try:
            r = ask(url, q["question"])
            return {"set": setname, "id": q["id"], "arm": arm, "question": q["question"],
                    "category": q.get("category"), "difficulty": q.get("difficulty"),
                    "attempt": attempt, **r}
        except Exception as e:  # noqa: BLE001 — record and retry transport errors
            last = str(e)
            time.sleep(5 * (attempt + 1))
    return {"set": setname, "id": q["id"], "arm": arm, "error": last}


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="uplift-results/paper-v2", type=Path)
    ap.add_argument("--concurrency", type=int, default=2)
    ap.add_argument("--sets", default="arcane,thin,general")
    args = ap.parse_args(argv)
    args.out.mkdir(parents=True, exist_ok=True)

    # raw-arm tunnel preflight
    try:
        ask(RAW, "Reply with exactly: OK")
    except Exception as e:  # noqa: BLE001
        sys.exit(f"raw-arm tunnel not up ({e}) — run: ssh -f -N -L 18085:127.0.0.1:8085 john@10.10.10.1")

    jobs = []
    for name in args.sets.split(","):
        qs = json.load(open(SETS[name]))
        for q in qs:
            jobs.append((name, q, "loom", LOOM))
            jobs.append((name, q, "raw", RAW))
    print(f"{len(jobs)} calls ({len(jobs)//2} paired questions), concurrency {args.concurrency}",
          file=sys.stderr)

    outpath = args.out / "live-results.jsonl"
    done = set()
    if outpath.exists():  # resumable: skip completed (set,id,arm) triples
        for line in open(outpath):
            r = json.loads(line)
            if "error" not in r:
                done.add((r["set"], r["id"], r["arm"]))
        print(f"resuming: {len(done)} already complete", file=sys.stderr)
    jobs = [j for j in jobs if (j[0], j[1]["id"], j[2]) not in done]

    with open(outpath, "a") as f, ThreadPoolExecutor(max_workers=args.concurrency) as ex:
        n = 0
        for res in ex.map(lambda j: run_one(*j), jobs):
            f.write(json.dumps(res, ensure_ascii=False) + "\n")
            f.flush()
            n += 1
            if n % 10 == 0:
                print(f"  {n}/{len(jobs)} done", file=sys.stderr)
    print(f"complete → {outpath}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
