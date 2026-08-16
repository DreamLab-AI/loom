#!/usr/bin/env python3
"""Semantic judge: grade model answers against an INDEPENDENT frontier gold.

This is the quality axis that breaks the graph-as-oracle circularity. Gold is
authored by a frontier model with deep web research and NO ontology scaffold
(uplift-results/quality/pilot-gold.json); candidate answers are re-used from the
sweep runs (no models re-run). A judge model -- a DIFFERENT family from the gold
author (the author is Opus; judge with GPT via OpenRouter) -- grades each
candidate against the reference on a 0-5 rubric, reference-guided and blind to
which arm produced it. Self-preference (Panickssery/Zheng): never let the judge
be the same family as the candidate; we flag family matches.

For questions where the frontier ABSTAINED (private-only stratum), the public
reference does not exist; those are graded against the ontology's own gold titles
(the curated truth) and reported separately -- that is exactly the stratum where a
grounded cheap/local model should win and frontier+web cannot.

Usage:
  python3 bench/quality/judge.py \
      --gold uplift-results/quality/pilot-gold.json \
      --questions uplift-results/quality/pilot-questions.json \
      --answers-dir uplift-results/sweep \
      --models gemini-2.5-flash-lite,gpt-4.1-mini,... \
      --judge-base-url https://openrouter.ai/api/v1 \
      --judge-model openai/gpt-4.1 --judge-key-env OPENROUTER_API_KEY \
      --out uplift-results/quality/pilot-judged.json
"""
from __future__ import annotations
import argparse, json, os, re, sys, time, urllib.request, urllib.error

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
        except Exception as e:
            last = str(e)[:120]
        if a < retries:
            time.sleep(1.0 * (a + 1))
    return None, f"judge failed: {last}"


def load_answers(answers_dir, model, mode, ids):
    path = os.path.join(answers_dir, f"results-{model}-{mode}.jsonl")
    if not os.path.exists(path):
        return {}
    out = {}
    for line in open(path, encoding="utf-8"):
        if not line.strip():
            continue
        r = json.loads(line)
        if r.get("id") in ids and not r.get("error"):
            out[r["id"]] = r.get("answer") or ""
    return out


def family(model_label):
    m = model_label.lower()
    for fam in ("gemini", "gpt", "claude", "haiku", "llama", "mistral", "qwen", "glm", "deepseek"):
        if fam in m:
            return "gpt" if fam == "gpt" else fam
    return "?"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gold", required=True)
    ap.add_argument("--questions", required=True)
    ap.add_argument("--answers-dir", required=True)
    ap.add_argument("--models", required=True, help="comma-separated sweep labels")
    ap.add_argument("--judge-base-url", required=True)
    ap.add_argument("--judge-model", required=True)
    ap.add_argument("--judge-key-env", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--web-answers", default=None,
                    help="optional web-arm answers json ({id: {answer}}) judged as model 'web-perplexity'")
    ap.add_argument("--modes", default="raw,scaffold",
                    help="comma-separated condition names to judge (e.g. 'bare,harness' for the general experiment)")
    ap.add_argument("--sleep", type=float, default=0.3)
    args = ap.parse_args()

    key = os.environ.get(args.judge_key_env)
    if not key:
        raise SystemExit(f"judge key env {args.judge_key_env} empty")
    gold = {g["id"]: g for g in json.load(open(args.gold))}
    qmeta = {q["id"]: q for q in json.load(open(args.questions))}
    ids = set(gold) & set(qmeta)
    judge_fam = family(args.judge_model)
    models = [m.strip() for m in args.models.split(",") if m.strip()]

    # Build the arms to judge: each model's raw+scaffold, plus the web arm.
    arms = []  # (model_label, mode, answers_dict)
    mode_names = [m.strip() for m in args.modes.split(",") if m.strip()]
    for model in models:
        for mode in mode_names:
            arms.append((model, mode, load_answers(args.answers_dir, model, mode, ids)))
    if args.web_answers and os.path.exists(args.web_answers):
        web = {k: v.get("answer", "") for k, v in json.load(open(args.web_answers)).items()
               if not v.get("error")}
        arms.append(("web-perplexity", "web", web))

    def reference_for(g, q):
        # public → the independent web-researched answer; private → the
        # corpus-authoritative answer the mesh authored (fallback to titles).
        ref = g.get("answer") or ""
        if ref.strip():
            return ref
        return ("[Authoritative answer per the curated corpus: "
                + ", ".join(q.get("gold_titles", [])) + "]")

    rows = []
    for model, mode, ans in arms:
        fam_match = family(model) == judge_fam
        for qid in sorted(ids):
            g = gold[qid]; q = qmeta[qid]
            cand = ans.get(qid)
            if cand is None:
                continue
            reference = reference_for(g, q)
            if not reference.strip():
                continue
            score, why = judge_call(args.judge_base_url, args.judge_model, key,
                                    q["question_natural"], reference, cand)
            rows.append({"model": model, "mode": mode, "id": qid,
                         "domain": q["domain"], "template": q["template"],
                         "stratum": g.get("stratum") or ("private" if g.get("abstained") else "public"),
                         "findability": g.get("web_findability"),
                         "reference_source": g.get("reference_source"),
                         "score": score, "why": why,
                         "judge_family_match": fam_match})
            if args.sleep:
                time.sleep(args.sleep)
        done = sum(1 for r in rows if r["model"] == model and r["mode"] == mode and r["score"] is not None)
        print(f"judged {model}/{mode}: {done} gradings", file=sys.stderr)

    json.dump(rows, open(args.out, "w"), indent=2)

    # ---- summary ----
    def mean(xs):
        xs = [x for x in xs if isinstance(x, (int, float))]
        return sum(xs) / len(xs) if xs else None
    print("\n# Quality judging — proximity to frontier gold (0-5)\n")
    print(f"judge: {args.judge_model}  |  gold: independent frontier (web-researched)\n")
    print("| model | stratum | raw (0-5) | scaffold (0-5) | Δ | fam-match |")
    print("|---|---|---|---|---|---|")
    for model in models:
        for stratum in ("public", "private"):
            raw = mean([r["score"] for r in rows if r["model"] == model and r["mode"] == "raw" and r["stratum"] == stratum])
            sca = mean([r["score"] for r in rows if r["model"] == model and r["mode"] == "scaffold" and r["stratum"] == stratum])
            if raw is None and sca is None:
                continue
            fm = any(r["judge_family_match"] for r in rows if r["model"] == model)
            d = None if raw is None or sca is None else round(sca - raw, 2)
            print(f"| {model} | {stratum} | {'-' if raw is None else round(raw,2)} "
                  f"| {'-' if sca is None else round(sca,2)} | {'-' if d is None else d} "
                  f"| {'YES' if fm else ''} |")


if __name__ == "__main__":
    raise SystemExit(main())
