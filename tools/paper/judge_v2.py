#!/usr/bin/env python3
"""Cross-family semantic judging for the paper-v2 arms.

Reuses the exact v1 rubric (bench/quality/judge.py) for comparability. Judge is
GPT via OpenRouter — a different model family from the Qwen candidate (anti
self-preference). Blind to arm: the judge sees question/reference/candidate
only. Reference = the frozen set's reference_answer + key points.

Inputs : uplift-results/paper-v2/live-results.jsonl (loom/raw arms)
         uplift-results/paper-v2/control-results.jsonl (true/shuffled/masked/
         irrelevant arms; optional, judged when present)
Output : uplift-results/paper-v2/judged.json  [{set,id,arm,score,why}]
Resumable: existing (set,id,arm) triples in the output are skipped.

Usage:
  python3 tools/paper/judge_v2.py --judge-model openai/gpt-4.1
"""
from __future__ import annotations
import argparse, json, os, re, sys, time, urllib.error, urllib.request
from pathlib import Path

RUBRIC = (
    "You are a strict, fair evaluation judge. Grade the CANDIDATE ANSWER against the "
    "REFERENCE ANSWER for the QUESTION, on a 0-5 integer scale:\n"
    "5 = fully correct and complete vs the reference; 4 = correct, minor omission; "
    "3 = partially correct, no major error; 2 = mostly wrong or one right point; "
    "1 = on-topic but no correct content; 0 = wrong, irrelevant, or fabricated.\n"
    "Judge CONTENT match to the reference, not style or verbosity. If the candidate "
    "adds correct information beyond the reference, do not penalise it. If it "
    "contradicts the reference, penalise heavily.\n"
    "Respond with ONLY a JSON object: {\"score\": <0-5>, \"why\": \"<one sentence>\"}."
)
SETS = {
    "arcane": "uplift-results/general/arcane-questions.json",
    "thin": "uplift-results/general/thin-questions.json",
    "general": "uplift-results/general/general-questions.json",
}


def judge_call(base, model, key, question, reference, candidate, timeout=90, retries=2):
    url = base.rstrip("/") + "/chat/completions"
    user = (f"QUESTION:\n{question}\n\nREFERENCE ANSWER (ground truth):\n{reference}\n\n"
            f"CANDIDATE ANSWER:\n{candidate[:4000]}\n\nGrade now.")
    body = json.dumps({"model": model, "temperature": 0, "max_tokens": 200,
                       "messages": [{"role": "system", "content": RUBRIC},
                                    {"role": "user", "content": user}]}).encode()
    last = None
    for a in range(retries + 1):
        try:
            req = urllib.request.Request(url, data=body, method="POST",
                headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            r = json.load(urllib.request.urlopen(req, timeout=timeout))
            txt = r["choices"][0]["message"].get("content") or ""
            m = re.search(r"\{.*\}", txt, re.S)
            if m:
                obj = json.loads(m.group(0))
                if isinstance(obj.get("score"), (int, float)):
                    return int(obj["score"]), obj.get("why", "")
            g = re.search(r"[0-5]", txt)
            if g:
                return int(g.group(0)), txt[:120]
            last = f"unparseable: {txt[:80]}"
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}: {e.read().decode()[:120]}"
        except Exception as e:  # noqa: BLE001
            last = str(e)[:120]
        if a < retries:
            time.sleep(1.0 * (a + 1))
    return None, f"judge failed: {last}"


def reference_for(q: dict) -> str:
    ref = q.get("reference_answer", "")
    pts = q.get("answer_key_points") or []
    if pts:
        ref += "\nKey points: " + "; ".join(pts)
    return ref


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--judge-base-url", default="https://openrouter.ai/api/v1")
    ap.add_argument("--judge-model", default="openai/gpt-4.1")
    ap.add_argument("--judge-key-env", default="OPENROUTER_API_KEY")
    ap.add_argument("--dir", default="uplift-results/paper-v2", type=Path)
    ap.add_argument("--sleep", type=float, default=0.3)
    args = ap.parse_args(argv)

    key = os.environ.get(args.judge_key_env)
    if not key:
        sys.exit(f"judge key env {args.judge_key_env} empty")

    qmeta = {}
    for name, path in SETS.items():
        for q in json.load(open(path)):
            qmeta[(name, q["id"])] = q

    rows = []
    for fname in ("live-results.jsonl", "control-results.jsonl"):
        p = args.dir / fname
        if not p.exists():
            continue
        for line in open(p):
            r = json.loads(line)
            if "error" in r or "skipped" in r or not r.get("content"):
                continue
            rows.append(r)

    outpath = args.dir / "judged.json"
    judged = []
    done = set()
    if outpath.exists():
        judged = json.load(open(outpath))
        done = {(j["set"], j["id"], j["arm"]) for j in judged}

    todo = [r for r in rows if (r["set"], r["id"], r["arm"]) not in done]
    print(f"{len(todo)} answers to judge ({len(done)} already done)", file=sys.stderr)
    for i, r in enumerate(todo):
        q = qmeta[(r["set"], r["id"])]
        score, why = judge_call(args.judge_base_url, args.judge_model, key,
                                q["question"], reference_for(q), r["content"])
        if score is None:
            print(f"  FAIL {r['set']}/{r['id']}/{r['arm']}: {why}", file=sys.stderr)
            continue
        judged.append({"set": r["set"], "id": r["id"], "arm": r["arm"],
                       "score": score, "why": why})
        if (i + 1) % 20 == 0:
            json.dump(judged, open(outpath, "w"), indent=1)
            print(f"  {i+1}/{len(todo)} judged", file=sys.stderr)
        time.sleep(args.sleep)
    json.dump(judged, open(outpath, "w"), indent=1)
    print(f"complete: {len(judged)} scores → {outpath}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
