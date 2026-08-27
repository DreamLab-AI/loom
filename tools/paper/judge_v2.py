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


def _cli_prompt(question, reference, candidate):
    user = (f"QUESTION:\n{question}\n\nREFERENCE ANSWER (ground truth):\n{reference}\n\n"
            f"CANDIDATE ANSWER:\n{candidate[:4000]}\n\nGrade now.")
    return RUBRIC + "\n\n" + user


def judge_call_cli(kind, model, question, reference, candidate, retries=6):
    """CLI judges (Claude / Codex) with serial exponential backoff. Reused for
    the never-used-third-family re-judge (reviewer W1). kind in {claude, codex}."""
    import sys as _sys
    _sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from cli_judge import judge_claude, judge_codex
    prompt = _cli_prompt(question, reference, candidate)
    delay = 8
    for a in range(retries + 1):
        try:
            obj = judge_claude(prompt, model=model) if kind == "claude" else judge_codex(prompt)
            if obj and isinstance(obj.get("score"), (int, float)):
                return int(obj["score"]), obj.get("why", "")
            last = f"unparseable: {str(obj)[:80]}"
        except Exception as e:  # noqa: BLE001
            last = str(e)[:120]
            if "limit" in last.lower() or "429" in last:
                time.sleep(delay); delay = min(delay * 2, 60); continue
        if a < retries:
            time.sleep(3)
    return None, f"cli judge failed: {last}"


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
    # --judge selects the local CLI judge families (no OpenRouter): cli-claude
    # (opus-4-6, the reviewer's clean never-used family) or cli-codex (gpt-5.6).
    ap.add_argument("--judge", default="cli-claude",
                    choices=["cli-claude", "cli-codex"])
    ap.add_argument("--judge-model", default="claude-opus-4-6")
    ap.add_argument("--dir", default="uplift-results/paper-v2", type=Path)
    ap.add_argument("--out", default="judged.json",
                    help="output filename (use a distinct name to avoid clobbering the gpt-4.1 judged.json)")
    ap.add_argument("--sleep", type=float, default=0.5)
    ap.add_argument("--shard", default="0/1",
                    help="k/N: this worker judges answers where index %% N == k (swarm parallelism)")
    args = ap.parse_args(argv)
    _k, _n = (int(x) for x in args.shard.split("/"))

    kind = "claude" if args.judge == "cli-claude" else "codex"

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

    outpath = args.dir / args.out
    judged = []
    done = set()
    if outpath.exists():
        judged = json.load(open(outpath))
        done = {(j["set"], j["id"], j["arm"]) for j in judged}

    todo = [r for r in rows if (r["set"], r["id"], r["arm"]) not in done]
    # stable order so shards are disjoint and reproducible, then take this shard
    todo.sort(key=lambda r: (r["set"], r["id"], r["arm"]))
    todo = [r for idx, r in enumerate(todo) if idx % _n == _k]
    print(f"[{args.judge}/{args.judge_model}] {len(todo)} answers to judge "
          f"({len(done)} already done) -> {outpath}", file=sys.stderr, flush=True)
    for i, r in enumerate(todo):
        q = qmeta[(r["set"], r["id"])]
        score, why = judge_call_cli(kind, args.judge_model,
                                    q["question"], reference_for(q), r["content"])
        if score is None:
            print(f"  FAIL {r['set']}/{r['id']}/{r['arm']}: {why}", file=sys.stderr, flush=True)
            continue
        judged.append({"set": r["set"], "id": r["id"], "arm": r["arm"],
                       "score": score, "why": why})
        json.dump(judged, open(outpath, "w"), indent=1)  # checkpoint every item — resumable
        if (i + 1) % 10 == 0:
            print(f"  {i+1}/{len(todo)} judged", file=sys.stderr, flush=True)
        time.sleep(args.sleep)
    json.dump(judged, open(outpath, "w"), indent=1)
    print(f"complete: {len(judged)} scores → {outpath}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
