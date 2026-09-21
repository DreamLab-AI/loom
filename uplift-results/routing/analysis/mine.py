#!/usr/bin/env python3
"""Mine the routing eval for findings the headline numbers hide.

Three questions the data can actually answer:
  1. Is openjev's `confidence` calibrated? ADR-2089 recorded a WRONG pick at 0.94 on
     Jev and concluded confidence must never gate. Does that hold for this engine?
  2. Are the judge and BM25 COMPLEMENTARY? 15 judge-only-right vs 7 copy-only-right
     means an oracle union scores higher than either — which is the precondition for
     a cascade that escalates only the hard cases.
  3. Where exactly does the judge earn its 4.96s? Which items, and do they share a
     property we could detect cheaply and in advance?
"""
import json, math, re, statistics, sys, urllib.request
from collections import Counter, defaultdict

EMB = "http://192.168.2.132:9997/v1/embeddings"
STOP = set("a an the of to for and or in on with without is are be this that it its as at by from into over under when "
           "use used using not never only your you we our their they them there here what which who how why do does "
           "did can could should would may might will shall must if then than else also more most less least very".split())

def toks(s):
    return [t for t in re.findall(r"[a-z0-9]+", s.lower()) if t not in STOP and len(t) > 2]

def bm25(query, docs, k1=1.5, b=0.75):
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

cases = json.load(open("tests/system-one/routing-cases.json"))["cases"]
cand = json.load(open("/tmp/candidates.json"))
run = {c["index"]: c for c in json.load(open("/tmp/openjev-run.json"))["cases"]}
names = sorted(cand)
docs = [toks(f"{n}: {cand[n]}") for n in names]

rows = []
for i, c in enumerate(cases):
    s = bm25(toks(c["prompt"]), docs)
    order = sorted(range(len(names)), key=lambda j: -s[j])
    top1, top2 = order[0], order[1]
    r = run.get(i, {})
    rows.append({
        "i": i, "prompt": c["prompt"], "expected": c["expected_skill"],
        "is_none": c["expected_skill"] == "none", "class": c.get("class"),
        "late": bool(c.get("late_discriminative")),
        "judge_pick": r.get("choice"), "judge_ok": bool(r.get("correct")),
        "judge_conf": r.get("confidence"), "ms": r.get("ms"),
        "lex_pick": names[top1], "lex_score": s[top1],
        "lex_margin": s[top1] - s[top2],
        "lex_ok_raw": names[top1] == c["expected_skill"],
    })

LEX_THR = 9.1563  # the oracle-tuned decline threshold from the fair ceiling
for r in rows:
    r["lex_final"] = "none" if r["lex_score"] < LEX_THR else r["lex_pick"]
    r["lex_ok"] = r["lex_final"] == r["expected"]

print("=" * 80)
print("1. IS `confidence` CALIBRATED?  (ADR-2089 recorded a wrong Jev pick at 0.94)")
print("=" * 80)
conf = [r for r in rows if isinstance(r["judge_conf"], (int, float))]
bins = [(0.0, .5), (.5, .7), (.7, .85), (.85, .95), (.95, 1.01)]
print(f"{'confidence bin':<18}{'n':>4}{'accuracy':>11}{'mean conf':>11}{'gap':>8}")
for lo, hi in bins:
    b = [r for r in conf if lo <= r["judge_conf"] < hi]
    if not b:
        continue
    acc = sum(r["judge_ok"] for r in b) / len(b)
    mc = statistics.mean(r["judge_conf"] for r in b)
    print(f"{f'[{lo:.2f},{hi:.2f})':<18}{len(b):>4}{100*acc:>10.1f}%{mc:>11.3f}{acc-mc:>+8.3f}")
wrong = [r for r in conf if not r["judge_ok"]]
right = [r for r in conf if r["judge_ok"]]
if wrong and right:
    print(f"\nmean confidence when RIGHT {statistics.mean(r['judge_conf'] for r in right):.3f}"
          f"   when WRONG {statistics.mean(r['judge_conf'] for r in wrong):.3f}")
    print(f"highest confidence on a WRONG pick: {max(r['judge_conf'] for r in wrong):.3f}")
    # Rank-based separation: P(conf_right > conf_wrong) over all pairs = AUC.
    pairs = [(a["judge_conf"], b["judge_conf"]) for a in right for b in wrong]
    auc = sum((1.0 if a > b else 0.5 if a == b else 0.0) for a, b in pairs) / len(pairs)
    print(f"AUC of confidence for predicting correctness: {auc:.3f}   (0.5 = useless)")

print("\n" + "=" * 80)
print("2. ARE THE JUDGE AND BM25 COMPLEMENTARY?")
print("=" * 80)
both = sum(r["judge_ok"] and r["lex_ok"] for r in rows)
jo = sum(r["judge_ok"] and not r["lex_ok"] for r in rows)
lo_ = sum(r["lex_ok"] and not r["judge_ok"] for r in rows)
neither = sum(not r["judge_ok"] and not r["lex_ok"] for r in rows)
n = len(rows)
print(f"both right {both:>3}   judge only {jo:>3}   copy only {lo_:>3}   neither {neither:>3}   (n={n})")
print(f"judge alone {100*(both+jo)/n:.1f}%   copy alone {100*(both+lo_)/n:.1f}%   ORACLE UNION {100*(both+jo+lo_)/n:.1f}%")
kappa_obs = (both + neither) / n
print(f"agreement {100*kappa_obs:.1f}%  — disagreement is where any cascade can pay")

print("\n" + "=" * 80)
print("3. CASCADE: can a cheap signal decide when to spend 4.96s?")
print("=" * 80)
print("Rule: trust BM25 when its top-1 MARGIN over top-2 is large; else escalate to the judge.")
print(f"{'margin thr':>11}{'escalated':>11}{'accuracy':>11}{'mean ms':>10}{'vs judge':>10}")
judge_acc = 100 * sum(r["judge_ok"] for r in rows) / n
judge_ms = statistics.mean(r["ms"] for r in rows if r["ms"])
margins = sorted(r["lex_margin"] for r in rows)
grid = [0.0] + [margins[int(len(margins) * f)] for f in (.1, .2, .3, .4, .5, .6, .7, .8, .9)] + [margins[-1] + 1]
seen = set()
for thr in grid:
    if round(thr, 3) in seen:
        continue
    seen.add(round(thr, 3))
    esc = [r for r in rows if r["lex_margin"] < thr]
    keep = [r for r in rows if r["lex_margin"] >= thr]
    ok = sum(r["judge_ok"] for r in esc) + sum(r["lex_ok"] for r in keep)
    acc = 100 * ok / n
    ms = (sum(r["ms"] or 0 for r in esc)) / n  # BM25 cost is ~0
    print(f"{thr:>11.2f}{100*len(esc)/n:>10.0f}%{acc:>10.1f}%{ms:>9.0f}{acc-judge_acc:>+10.1f}")
print(f"{'judge always':>11}{'100%':>11}{judge_acc:>10.1f}%{judge_ms:>9.0f}{0.0:>+10.1f}")

print("\n" + "=" * 80)
print("4. WHERE THE JUDGE EARNS IT — the items copy gets wrong and the judge gets right")
print("=" * 80)
win = [r for r in rows if r["judge_ok"] and not r["lex_ok"]]
lose = [r for r in rows if r["lex_ok"] and not r["judge_ok"]]
def profile(label, rs):
    if not rs:
        print(f"{label}: none"); return
    cl = Counter(r["class"] for r in rs)
    print(f"{label} (n={len(rs)}): none-items {sum(r['is_none'] for r in rs)}, "
          f"late-discriminator {sum(r['late'] for r in rs)}, classes {dict(cl)}")
profile("JUDGE-ONLY-RIGHT", win)
profile("COPY-ONLY-RIGHT ", lose)
print("\nthe 7 items copy gets right and the judge does not:")
for r in lose:
    print(f"  expected {r['expected']:<28} judge said {str(r['judge_pick']):<28} conf {r['judge_conf']}")

json.dump(rows, open("/tmp/mined-rows.json", "w"), indent=1)
