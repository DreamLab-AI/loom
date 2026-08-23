#!/usr/bin/env python3
"""Semantic (paraphrase-tolerant) re-score of the ten-model sweep's scaffold
arm, mirroring tools/paper/decompose_exposure.py's lexical matcher exactly in
structure but swapping gold_hit() for a bge-small-en-v1.5 cosine matcher.

Preserves matcher symmetry: BOTH the model answer and the copy-arm (scaffold
system context) are scored with the identical semantic matcher.
"""
from __future__ import annotations
import json, math, os, random, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from embed_lib import EmbedCache, cosine, extract_candidates, normalise_title  # noqa: E402

REPO = "/home/devuser/workspace/loom"
QUESTIONS = os.path.join(REPO, "uplift-results", "questions.jsonl")
SWEEP = os.path.join(REPO, "uplift-results", "sweep")
COPY_CONTEXTS = os.path.join(HERE, "copy_contexts.json")
CACHE_PATH = os.path.join(HERE, "embed_cache.json")

THRESHOLDS = [0.80, 0.85, 0.90]
SEED = 42
RESAMPLES = 10_000

MODELS = [
    ("gemini-3.7-flash-t0", "gemini-3.7-flash"),
    ("gemini-3.5-flash-lite", "gemini-3.5-flash-lite"),
    ("gemini-2.5-flash-lite", "gemini-2.5-flash-lite"),
    ("claude-haiku-4.5", "claude-haiku-4.5"),
    ("gpt-4.1-mini", "gpt-4.1-mini"),
    ("deepseek-chat", "deepseek-chat"),
    ("glm-4.6", "glm-4.6"),
    ("qwen-2.5-72b", "qwen-2.5-72b"),
    ("llama-3.3-70b", "llama-3.3-70b"),
    ("mistral-small-24b", "mistral-small-24b"),
]

# --- lexical matcher, copied verbatim from decompose_exposure.py, used only
# for the agreement sanity-check -------------------------------------------
_PUNCT_RE = re.compile(r"[^a-z0-9\s]+")
_WS_RE = re.compile(r"\s+")


def lex_normalise(s: str) -> str:
    return _WS_RE.sub(" ", _PUNCT_RE.sub(" ", s.lower())).strip()


def lex_gold_hit(title: str, norm_text: str, text_words: set) -> bool:
    nt = lex_normalise(title)
    if not nt:
        return False
    if nt in norm_text:
        return True
    words = [w for w in nt.split() if len(w) >= 4]
    if not words:
        return False
    return sum(1 for w in words if w in text_words) / len(words) >= 0.8


def read_jsonl(path):
    out = []
    with open(path) as fh:
        for line in fh:
            line = line.strip()
            if line:
                out.append(json.loads(line))
    return out


def bootstrap_ci(values, resamples=RESAMPLES, seed=SEED):
    rng = random.Random(seed)
    n = len(values)
    if n == 0:
        return (float("nan"), float("nan"))
    means = sorted(sum(values[rng.randrange(n)] for _ in range(n)) / n
                   for _ in range(resamples))
    lo = means[max(0, int(0.025 * resamples))]
    hi = means[min(resamples - 1, int(0.975 * resamples))]
    return lo, hi


def main():
    questions = {q["id"]: q for q in read_jsonl(QUESTIONS)}
    copy_contexts = json.load(open(COPY_CONTEXTS))

    # ---- Phase A: gather every string needing an embedding -----------------
    print("Phase A: extracting candidate spans / gold titles ...")
    cache = EmbedCache(CACHE_PATH)

    gold_titles = set()
    for q in questions.values():
        for g in (q.get("gold") or []):
            gold_titles.add(normalise_title(g["title"]))

    copy_candidates = {}
    for qid, ctx in copy_contexts.items():
        copy_candidates[qid] = extract_candidates(ctx)

    print(f"  gold titles: {len(gold_titles)}")
    print(f"  copy-context questions: {len(copy_candidates)}, "
          f"total copy candidate strings (with dup across qs): "
          f"{sum(len(v) for v in copy_candidates.values())}")

    # embed gold titles + all copy-context candidates first (model-independent)
    to_embed = set(gold_titles)
    for s in copy_candidates.values():
        to_embed |= s
    print(f"  unique strings to embed (gold+copy, before model answers): {len(to_embed)}")
    cache.ensure(list(to_embed))

    # ---- Phase B: per-model answer candidates -------------------------------
    model_answers = {}   # label -> {qid: answer_text}
    model_candidates = {}  # label -> {qid: set}
    for label, disp in MODELS:
        rows = read_jsonl(os.path.join(SWEEP, f"results-{label}-scaffold.jsonl"))
        ans = {r["id"]: (r.get("answer") or "") for r in rows if "error" not in r}
        model_answers[label] = ans
        cands = {qid: extract_candidates(txt) for qid, txt in ans.items()}
        model_candidates[label] = cands
        to_embed = set()
        for s in cands.values():
            to_embed |= s
        print(f"  {label}: {len(ans)} answers, {len(to_embed)} unique candidate strings")
        cache.ensure(list(to_embed))

    print(f"Total cached vectors: {len(cache.map)}")

    # ---- Phase C: sanity check — semantic vs lexical agreement --------------
    print("\nPhase C: sanity-checking semantic matcher against lexical hits ...")
    sample_qids = list(questions.keys())[:60]
    lex_hits_sample = []
    for qid in sample_qids:
        q = questions[qid]
        ans = model_answers["gemini-3.7-flash-t0"].get(qid, "")
        norm = lex_normalise(ans)
        words = set(norm.split())
        for g in (q.get("gold") or []):
            if lex_gold_hit(g["title"], norm, words):
                lex_hits_sample.append((qid, g["title"]))

    def sem_hit(title, cand_set, threshold):
        nt = normalise_title(title)
        if nt not in cache.map:
            return False, 0.0
        tv = cache.get(nt)
        best = 0.0
        for c in cand_set:
            v = cache.map.get(c)
            if v is None:
                continue
            sim = cosine(tv, v)
            if sim > best:
                best = sim
        return best >= threshold, best

    agree = {t: 0 for t in THRESHOLDS}
    for qid, title in lex_hits_sample:
        cset = model_candidates["gemini-3.7-flash-t0"].get(qid, set())
        for t in THRESHOLDS:
            hit, _ = sem_hit(title, cset, t)
            if hit:
                agree[t] += 1
    n_sample = len(lex_hits_sample)
    print(f"  lexical-hit sample size: {n_sample}")
    for t in THRESHOLDS:
        rate = agree[t] / n_sample if n_sample else float("nan")
        print(f"  threshold {t}: semantic agrees on {agree[t]}/{n_sample} "
              f"({rate*100:.1f}%) of lexical hits")

    # ---- Phase D: full scoring per model per threshold ----------------------
    print("\nPhase D: scoring ...")

    # per-question copy ceiling flags/values are model-independent
    ceiling_flags = {}  # qid -> {threshold: [bool per gold item]}
    for qid, q in questions.items():
        gold = q.get("gold") or []
        cset = copy_candidates.get(qid, set())
        ceiling_flags[qid] = {}
        for t in THRESHOLDS:
            flags = [sem_hit(g["title"], cset, t)[0] for g in gold]
            ceiling_flags[qid][t] = flags

    results = {"thresholds": THRESHOLDS, "sanity": {
        "lexical_hit_sample_n": n_sample,
        "agreement_by_threshold": {str(t): round(agree[t] / n_sample, 4) if n_sample else None
                                    for t in THRESHOLDS},
    }, "per_model": {}}

    for label, disp in MODELS:
        ans_map = model_answers[label]
        cand_map = model_candidates[label]
        per_thresh = {}
        for t in THRESHOLDS:
            recalls = []
            ceilings = []
            gains = []
            n11 = n10 = n01 = n00 = 0
            for qid, ans in ans_map.items():
                q = questions[qid]
                gold = q.get("gold") or []
                if not gold:
                    continue
                cset = cand_map.get(qid, set())
                rec_flags = [sem_hit(g["title"], cset, t)[0] for g in gold]
                exp_flags = ceiling_flags[qid][t]
                if q.get("gold_type") == "any":
                    recall = 1.0 if any(rec_flags) else 0.0
                    ceil_v = 1.0 if any(exp_flags) else 0.0
                else:
                    recall = sum(rec_flags) / len(gold)
                    ceil_v = sum(exp_flags) / len(gold)
                recalls.append(recall)
                ceilings.append(ceil_v)
                gains.append(recall - ceil_v)
                for e, v in zip(exp_flags, rec_flags):
                    if e and v:
                        n11 += 1
                    elif e and not v:
                        n10 += 1
                    elif (not e) and v:
                        n01 += 1
                    else:
                        n00 += 1
            glo, ghi = bootstrap_ci(gains)
            n_items = n11 + n10 + n01 + n00
            per_thresh[str(t)] = {
                "n_questions": len(recalls),
                "semantic_recall": round(sum(recalls) / len(recalls), 4),
                "semantic_ceiling": round(sum(ceilings) / len(ceilings), 4),
                "semantic_gain": round(sum(gains) / len(gains), 4),
                "gain_ci95": [round(glo, 4), round(ghi, 4)],
                "n11_exposed_recovered": n11,
                "n10_exposed_omitted": n10,
                "n01_unexposed_recovered": n01,
                "n00_unexposed_omitted": n00,
                "n_gold_items": n_items,
            }
        # lexical numbers from the stored summary, for the side-by-side table
        summ = json.load(open(os.path.join(SWEEP, f"scores-{label}-scaffold-summary.json")))
        results["per_model"][label] = {
            "display": disp,
            "lexical_recall": round(summ["mean_recall"], 4),
            "lexical_ceiling": round(summ["mean_gold_exposed_recall"], 4),
            "lexical_gain": round(summ["recall_gain_over_exposure"], 4),
            "semantic": per_thresh,
        }
        print(f"  {disp}: lexical gain {summ['recall_gain_over_exposure']:+.4f} | "
              + " | ".join(f"sem@{t} gain {per_thresh[str(t)]['semantic_gain']:+.4f}"
                           f" n01={per_thresh[str(t)]['n01_unexposed_recovered']}"
                           for t in THRESHOLDS))

    # pooled n01 at 0.85 across models (the reasoning-evidence number)
    pooled_085 = {"n01": 0, "n10": 0, "n11": 0, "n00": 0, "n_items": 0}
    for label, _ in MODELS:
        d = results["per_model"][label]["semantic"]["0.85"]
        pooled_085["n01"] += d["n01_unexposed_recovered"]
        pooled_085["n10"] += d["n10_exposed_omitted"]
        pooled_085["n11"] += d["n11_exposed_recovered"]
        pooled_085["n00"] += d["n00_unexposed_omitted"]
        pooled_085["n_items"] += d["n_gold_items"]
    results["pooled_n01_at_0.85"] = pooled_085

    with open(os.path.join(HERE, "rescore_results.json"), "w") as fh:
        json.dump(results, fh, indent=2)
    print("\nWrote rescore_results.json")
    print(json.dumps({"sanity": results["sanity"], "pooled_n01_at_0.85": pooled_085}, indent=2))


if __name__ == "__main__":
    main()
