#!/usr/bin/env python3
"""Fair copy ceiling: the judge-free baselines get a DECLINE mechanism too.

The first control conceded `none` to the judge by construction — a copy procedure
has no rubric for `none`, so it could never select it, and that asymmetry supplied
the entire headline gain. This version gives each judge-free ranker the same
affordance the façade gives openjev: a threshold below which it answers `none`.

Deliberately biased AGAINST the judge, so the result is a floor on its advantage:
  * the copy threshold is swept and the BEST value on THIS corpus is reported, i.e.
    the baseline is oracle-tuned on the same data it is evaluated on;
  * the judge's threshold is the deployed 0.5, not its own best (0.25-0.30).
If the judge still wins under that handicap, the margin is real.
"""
import json, math, re, sys, urllib.request
from collections import Counter

EMB = "http://192.168.2.132:9997/v1/embeddings"
STOP = set("a an the of to for and or in on with without is are be this that it its as at by from into over under when "
           "use used using not never only your you we our their they them there here what which who how why do does "
           "did can could should would may might will shall must if then than else also more most less least very".split())

def toks(s):
    return [t for t in re.findall(r"[a-z0-9]+", s.lower()) if t not in STOP and len(t) > 2]

def bm25_scores(query, docs, k1=1.5, b=0.75):
    N = len(docs); dl = [len(d) for d in docs]; avgdl = sum(dl) / N
    df = Counter()
    for d in docs:
        for t in set(d):
            df[t] += 1
    out = []
    for i, d in enumerate(docs):
        tf = Counter(d); s = 0.0
        for t in set(query):
            if t not in tf:
                continue
            idf = math.log(1 + (N - df[t] + 0.5) / (df[t] + 0.5))
            s += idf * (tf[t] * (k1 + 1)) / (tf[t] + k1 * (1 - b + b * dl[i] / avgdl))
        out.append(s)
    return out

def embed(texts):
    out = []
    for i, t in enumerate(texts):
        req = urllib.request.Request(EMB, data=json.dumps(
            {"model": "bge-small-en-v1.5", "input": [t[:2000]]}).encode(),
            headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            out.append(json.load(r)["data"][0]["embedding"])
    return out

def main():
    cases = json.load(open("tests/system-one/routing-cases.json"))["cases"]
    cand = json.load(open("/tmp/candidates.json"))
    run = {c["index"]: c for c in json.load(open("/tmp/openjev-run.json"))["cases"]}
    names = sorted(cand)
    rubrics = [f"{n}: {cand[n]}" for n in names]
    doc_toks = [toks(r) for r in rubrics]

    print("embedding rubrics + prompts (sequential)…", file=sys.stderr)
    rub_emb = embed(rubrics)
    pr_emb = embed([c["prompt"] for c in cases])

    # Per-case ranked scores for each judge-free ranker.
    per = []
    for i, c in enumerate(cases):
        lex = bm25_scores(toks(c["prompt"]), doc_toks)
        emb = [sum(x * y for x, y in zip(pr_emb[i], r)) for r in rub_emb]
        best_l = max(range(len(names)), key=lambda j: lex[j])
        best_e = max(range(len(names)), key=lambda j: emb[j])
        top3_l = sorted(range(len(names)), key=lambda j: -lex[j])[:3]
        top3_e = sorted(range(len(names)), key=lambda j: -emb[j])[:3]
        per.append({
            "expected": c["expected_skill"], "is_none": c["expected_skill"] == "none",
            "judge_top1": bool(run.get(i, {}).get("correct")),
            "lex_pick": names[best_l], "lex_score": lex[best_l],
            "lex_top3": [names[j] for j in top3_l],
            "emb_pick": names[best_e], "emb_score": emb[best_e],
            "emb_top3": [names[j] for j in top3_e],
        })

    def score_at(key_pick, key_score, key_top3, thr):
        """Accuracy when the ranker declines below `thr`."""
        t1 = t3 = 0
        for r in per:
            declined = r[key_score] < thr
            pick = "none" if declined else r[key_pick]
            top3 = ["none"] + r[key_top3][:2] if declined else r[key_top3]
            t1 += (pick == r["expected"])
            t3 += (r["expected"] in top3)
        return 100.0 * t1 / len(per), 100.0 * t3 / len(per)

    print("\n" + "=" * 84)
    print("FAIR COPY CEILING — judge-free rankers WITH a decline threshold")
    print("copy thresholds oracle-tuned on this corpus; judge fixed at deployed 0.5")
    print("=" * 84)

    results = {}
    for label, kp, ks, kt in (("lexical (BM25)", "lex_pick", "lex_score", "lex_top3"),
                              ("embedding (bge)", "emb_pick", "emb_score", "emb_top3")):
        vals = sorted(r[ks] for r in per)
        lo, hi = vals[0], vals[-1]
        grid = [lo + (hi - lo) * i / 40 for i in range(41)]
        best = max(((score_at(kp, ks, kt, t), t) for t in grid), key=lambda x: x[0][0])
        (acc, soft), thr = best
        results[label] = (acc, soft, thr)
        no_decline = score_at(kp, ks, kt, float("-inf"))
        print(f"\n{label}")
        print(f"  without decline : top1 {no_decline[0]:5.1f}%   top3 {no_decline[1]:5.1f}%")
        print(f"  BEST w/ decline : top1 {acc:5.1f}%   top3 {soft:5.1f}%   (threshold {thr:.4f}, oracle-tuned)")

    j1 = 100.0 * sum(r["judge_top1"] for r in per) / len(per)
    j3 = 98.8
    print(f"\njudge (openjev @0.5) : top1 {j1:5.1f}%   top3 {j3:5.1f}%")
    print("\n" + "-" * 84)
    print("GAIN OVER COPY, against the oracle-tuned copy baseline:")
    for label, (acc, soft, thr) in results.items():
        print(f"  vs {label:<18} top1 {j1-acc:+6.1f} pts     top3 {j3-soft:+6.1f} pts")

    # Paired per-item test against the best thresholded lexical baseline.
    _, thr_l = max(((score_at("lex_pick", "lex_score", "lex_top3", t), t)
                    for t in [min(r['lex_score'] for r in per) + (max(r['lex_score'] for r in per) - min(r['lex_score'] for r in per)) * i / 40 for i in range(41)]),
                   key=lambda x: x[0][0])
    g = []
    for r in per:
        pick = "none" if r["lex_score"] < thr_l else r["lex_pick"]
        g.append(int(r["judge_top1"]) - int(pick == r["expected"]))
    mean = sum(g) / len(g)
    var = sum((x - mean) ** 2 for x in g) / (len(g) - 1)
    hw = 1.96 * math.sqrt(var / len(g))
    win = sum(1 for x in g if x > 0); loss = sum(1 for x in g if x < 0)
    print(f"\nSIGNED PER-ITEM GAIN vs thresholded BM25 (all 86 items):")
    print(f"  mean {mean:+.3f}  95% CI [{mean-hw:+.3f},{mean+hw:+.3f}]   judge-only-right {win}   copy-only-right {loss}")
    print(f"  (copy threshold {thr_l:.4f}, chosen to maximise ITS OWN accuracy on this corpus)")

if __name__ == "__main__":
    main()
