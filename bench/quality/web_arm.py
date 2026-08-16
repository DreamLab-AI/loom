#!/usr/bin/env python3
"""Web-search-grounded baseline arm: answer each question with a web-grounded
LLM (Perplexity Sonar) — the "internet research agent" the operator described as
the real competitor to a curated private ontology for OUT-of-domain questions
(and the baseline the curated corpus must beat IN-domain).

One answer per question (model-independent); judged against the same frontier
gold as the ontology-grounded and raw arms, so the three grounding sources
(none / curated ontology / open web) are compared on identical questions.

Usage:
  python3 bench/quality/web_arm.py \
      --questions uplift-results/quality/cand-all.json,uplift-results/quality/pilot-questions.json \
      --model sonar-pro --out uplift-results/quality/web-answers.json
"""
from __future__ import annotations
import argparse, json, os, sys, time, urllib.request, urllib.error


def ask(model, key, question, timeout=60, retries=2):
    body = json.dumps({"model": model, "temperature": 0, "max_tokens": 400,
                       "messages": [{"role": "user", "content": question}]}).encode()
    last = None
    for a in range(retries + 1):
        try:
            req = urllib.request.Request("https://api.perplexity.ai/chat/completions",
                data=body, method="POST",
                headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            d = json.load(urllib.request.urlopen(req, timeout=timeout))
            msg = d["choices"][0]["message"].get("content") or ""
            cites = d.get("citations") or d["choices"][0].get("citations") or []
            return msg, cites
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}: {e.read().decode()[:120]}"
            if e.code == 429:
                time.sleep(3.0 * (a + 1))
        except Exception as e:
            last = str(e)[:120]
        if a < retries:
            time.sleep(1.5 * (a + 1))
    return None, last


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--questions", required=True, help="comma-separated json files (arrays of {id,question_natural})")
    ap.add_argument("--model", default="sonar-pro")
    ap.add_argument("--out", required=True)
    ap.add_argument("--sleep", type=float, default=0.5)
    args = ap.parse_args()
    key = os.environ.get("PERPLEXITY_API_KEY")
    if not key:
        raise SystemExit("PERPLEXITY_API_KEY empty")
    qs = {}
    for path in args.questions.split(","):
        for q in json.load(open(path.strip())):
            qs[q["id"]] = q["question_natural"]
    out = {}
    n_err = 0
    for i, (qid, question) in enumerate(sorted(qs.items()), 1):
        ans, cites = ask(args.model, key, question)
        if ans is None:
            n_err += 1
            out[qid] = {"answer": "", "error": cites, "model": args.model}
        else:
            out[qid] = {"answer": ans, "citations": cites, "model": args.model}
        if i % 10 == 0:
            print(f"web-arm: {i}/{len(qs)} ({n_err} errors)", file=sys.stderr)
        json.dump(out, open(args.out, "w"), indent=2)
        if args.sleep:
            time.sleep(args.sleep)
    print(f"web-arm: wrote {args.out} ({len(out)} answers, {n_err} errors)")


if __name__ == "__main__":
    raise SystemExit(main())
