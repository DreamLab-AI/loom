#!/usr/bin/env python3
"""OpenRouter judge wrapper for the control cohorts (paper-v9 remediation).

`judge_v2.py`'s `main()` only drives the two local CLI judge families; its
HTTP judge (`judge_call`, OpenAI-compatible) is unreachable from the CLI. This
is a thin wrapper that reuses `judge_v2`'s RUBRIC, `reference_for()` and
`judge_call()` verbatim (so the rubric is byte-identical to the original
paper-v2 judging) and points them at OpenRouter.

  OPENAI_API_KEY 401s in this estate; OPENROUTER_API_KEY is the live one.

Every judged row records the judge model, base URL, temperature, the UTC
timestamp and a sha256 of the graded candidate text, so a score can always be
tied back to the exact completion it graded.

Usage:
  OPENROUTER_API_KEY=... python3 tools/paper/judge_openrouter.py \
      --rows uplift-results/paper-v2/control-results.jsonl \
             uplift-results/paper-v2/live-results.jsonl \
      --sets arcane,thin \
      --out uplift-results/controls-rejudge-2026-09-21/judged.json
"""
from __future__ import annotations
import argparse, hashlib, json, os, sys, threading, time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
REPO = HERE.parent.parent

from judge_v2 import RUBRIC, SETS, judge_call, reference_for  # noqa: E402

RUBRIC_SHA = hashlib.sha256(RUBRIC.encode()).hexdigest()[:16]


def load_questions() -> dict:
    qmeta = {}
    for name, rel in SETS.items():
        p = REPO / rel
        if not p.exists():
            continue
        for q in json.load(open(p)):
            qmeta[(name, q["id"])] = q
    return qmeta


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rows", nargs="+", required=True,
                    help="jsonl file(s) of completions to grade")
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--model", default="openai/gpt-4.1")
    ap.add_argument("--base", default="https://openrouter.ai/api/v1")
    ap.add_argument("--sets", default="arcane,thin")
    ap.add_argument("--concurrency", type=int, default=6)
    ap.add_argument("--key-env", default="OPENROUTER_API_KEY")
    args = ap.parse_args(argv)

    key = os.environ.get(args.key_env)
    if not key:
        print(f"{args.key_env} not set", file=sys.stderr)
        return 2
    keep_sets = set(args.sets.split(","))
    qmeta = load_questions()

    rows, seen = [], set()
    for path in args.rows:
        for line in open(path):
            r = json.loads(line)
            if r.get("set") not in keep_sets:
                continue
            if "error" in r or "skipped" in r:
                continue
            if not (r.get("content") or "").strip():
                continue          # empty completions are ungradeable, by design
            k = (r["set"], r["id"], r["arm"])
            if k in seen:
                continue          # first row wins; sources are de-duplicated upstream
            seen.add(k)
            rows.append(r)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    judged, done = [], set()
    if args.out.exists():
        judged = json.load(open(args.out))
        done = {(j["set"], j["id"], j["arm"]) for j in judged}
    todo = sorted((r for r in rows if (r["set"], r["id"], r["arm"]) not in done),
                  key=lambda r: (r["set"], r["id"], r["arm"]))
    print(f"[{args.model}] {len(todo)} to judge ({len(done)} already done) -> {args.out}",
          file=sys.stderr, flush=True)

    lock = threading.Lock()
    counter = [0]

    def grade(r):
        q = qmeta[(r["set"], r["id"])]
        score, why = judge_call(args.base, args.model, key,
                                q["question"], reference_for(q), r["content"],
                                timeout=120, retries=4)
        rec = {"set": r["set"], "id": r["id"], "arm": r["arm"],
               "score": score, "why": why,
               "judge_model": args.model, "judge_base": args.base,
               "judge_temperature": 0, "rubric_sha256_16": RUBRIC_SHA,
               "candidate_sha256_16": hashlib.sha256(
                   r["content"].encode()).hexdigest()[:16],
               "candidate_chars": len(r["content"]),
               "judged_at": datetime.now(timezone.utc).isoformat()}
        with lock:
            judged.append(rec)
            counter[0] += 1
            if counter[0] % 10 == 0 or counter[0] == len(todo):
                json.dump(judged, open(args.out, "w"), indent=1)
                print(f"  {counter[0]}/{len(todo)} judged", file=sys.stderr, flush=True)
        if score is None:
            print(f"  FAIL {r['set']}/{r['id']}/{r['arm']}: {why}",
                  file=sys.stderr, flush=True)
        return rec

    with ThreadPoolExecutor(max_workers=args.concurrency) as ex:
        list(ex.map(grade, todo))

    judged.sort(key=lambda j: (j["set"], j["id"], j["arm"]))
    json.dump(judged, open(args.out, "w"), indent=1)
    ok = sum(1 for j in judged if j.get("score") is not None)
    print(f"complete: {ok}/{len(judged)} scored -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
