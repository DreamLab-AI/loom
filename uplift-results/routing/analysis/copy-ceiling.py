#!/usr/bin/env python3
"""Copy ceiling + gain over copy for the Sovereign System One routing eval.

Method follows "The Copy Ceiling: An Input-Exposure Control for Ontology-Grounded
Generation over Curated Corpora" (Loom paper v6, 2026-09-11).

The control it demands: when the gold answer derives from the same corpus that is
SHOWN to the model, some of the measured accuracy is faithful delivery of exposed
text rather than judgement. The copy ceiling is what a deterministic, judge-free
procedure over the shown context achieves; gain over copy is the signed per-item
difference between the judge and that ceiling.

Here the shown context is the 115 skill rubrics injected as choice options, and the
gold label is the skill whose rubric is among them — so the exposure is total and
the control is mandatory, not optional.

Two judge-free ceilings, because they bound different things:
  lexical   — BM25-style token overlap between prompt and rubric. No model at all.
  embedding — bge-small cosine, which is the façade's OWN shortlist ranking. This is
              the stronger and more honest ceiling: it is a component we already run,
              so any gain below it is gain the pipeline could have had for free.
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
    """Classic BM25 over the exposed rubrics. Deterministic, no model, no training."""
    N = len(docs)
    dl = [len(d) for d in docs]
    avgdl = sum(dl) / N
    df = Counter()
    for d in docs:
        for t in set(d):
            df[t] += 1

    out = []
    for i, d in enumerate(docs):
        tf = Counter(d)
        s = 0.0
        for t in set(query):
            if t not in tf:
                continue
            idf = math.log(1 + (N - df[t] + 0.5) / (df[t] + 0.5))
            s += idf * (tf[t] * (k1 + 1)) / (tf[t] + k1 * (1 - b + b * dl[i] / avgdl))
        out.append(s)
    return out

def embed(texts):
    """One request per text. Deliberately NOT batched: this endpoint mis-numbers
    `index` when concurrent requests merge (verified 2026-09-20), and positional
    order is the only field that is trustworthy."""
    out = []
    for i, t in enumerate(texts):
        req = urllib.request.Request(EMB, data=json.dumps(
            {"model": "bge-small-en-v1.5", "input": [t[:2000]]}).encode(),
            headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            out.append(json.load(r)["data"][0]["embedding"])
        if (i + 1) % 25 == 0:
            print(f"    embedded {i+1}/{len(texts)}", file=sys.stderr)
    return out

def cos(a, b):
    return sum(x * y for x, y in zip(a, b))  # bge returns unit vectors

def rank(scores, names, k=3):
    order = sorted(range(len(names)), key=lambda i: -scores[i])
    return [names[i] for i in order[:k]]

def main():
    cases = json.load(open("tests/system-one/routing-cases.json"))["cases"]
    cand = json.load(open("/tmp/candidates.json"))
    run = {c["index"]: c for c in json.load(open("/tmp/openjev-run.json"))["cases"]}

    names = sorted(cand)
    rubrics = [f"{n}: {cand[n]}" for n in names]
    # `none` is not in the skills tree and has no rubric to copy from, so a copy
    # procedure can never select it. That asymmetry is reported, not hidden.
    doc_toks = [toks(r) for r in rubrics]

    print("embedding 115 rubrics (one request each)…", file=sys.stderr)
    rub_emb = embed(rubrics)
    print("embedding 86 prompts…", file=sys.stderr)
    pr_emb = embed([c["prompt"] for c in cases])

    rows = []
    for i, c in enumerate(cases):
        exp = c["expected_skill"]
        q = toks(c["prompt"])
        lex = bm25_scores(q, doc_toks)
        emb = [cos(pr_emb[i], r) for r in rub_emb]
        r = run.get(i, {})
        rows.append({
            "expected": exp,
            "is_none": exp == "none",
            "class": c.get("class"),
            "late": bool(c.get("late_discriminative")),
            "judge_top1": bool(r.get("correct")),
            "judge_top3": bool(r.get("soft_correct")),
            "lex_top1": rank(lex, names, 1)[0] == exp,
            "lex_top3": exp in rank(lex, names, 3),
            "emb_top1": rank(emb, names, 1)[0] == exp,
            "emb_top3": exp in rank(emb, names, 3),
        })

    def pct(sel, key):
        s = [r for r in rows if sel(r)]
        return (100.0 * sum(r[key] for r in s) / len(s)) if s else float("nan"), len(s)

    allr = lambda r: True
    skill = lambda r: not r["is_none"]      # items a copy procedure can even reach
    nones = lambda r: r["is_none"]

    print("\n" + "=" * 78)
    print("COPY CEILING — judge-free procedures over the SAME exposed rubrics")
    print("method: Loom paper v6, 'The Copy Ceiling' (2026-09-11)")
    print("=" * 78)
    hdr = f"{'population':<26}{'n':>4}{'judge':>9}{'lexical':>10}{'embed':>9}{'gain/lex':>10}{'gain/emb':>10}"
    print(hdr); print("-" * 78)
    for label, sel in (("all items", allr), ("skill items (copyable)", skill), ("`none` items", nones),
                       ("late discriminator", lambda r: r["late"]),
                       ("near-neighbour", lambda r: r["class"] == "near-neighbour")):
        j, n = pct(sel, "judge_top1"); l, _ = pct(sel, "lex_top1"); e, _ = pct(sel, "emb_top1")
        print(f"{label:<26}{n:>4}{j:>8.1f}%{l:>9.1f}%{e:>8.1f}%{j-l:>+9.1f}{j-e:>+10.1f}")
    print("-" * 78)
    print("top-3 (soft):")
    for label, sel in (("all items", allr), ("skill items (copyable)", skill)):
        j, n = pct(sel, "judge_top3"); l, _ = pct(sel, "lex_top3"); e, _ = pct(sel, "emb_top3")
        print(f"{label:<26}{n:>4}{j:>8.1f}%{l:>9.1f}%{e:>8.1f}%{j-l:>+9.1f}{j-e:>+10.1f}")

    # Signed per-item gain over copy, the paper's headline statistic.
    print("\nSIGNED PER-ITEM GAIN OVER COPY (judge − ceiling), top-1:")
    for name, key in (("lexical", "lex_top1"), ("embedding", "emb_top1")):
        for label, sel in (("all items", allr), ("skill items", skill)):
            s = [r for r in rows if sel(r)]
            g = [int(r["judge_top1"]) - int(r[key]) for r in s]
            win = sum(1 for x in g if x > 0); loss = sum(1 for x in g if x < 0)
            mean = sum(g) / len(s)
            # Normal-approximation CI on the paired mean difference.
            var = sum((x - mean) ** 2 for x in g) / max(1, len(s) - 1)
            hw = 1.96 * math.sqrt(var / len(s))
            print(f"  {name:<10}{label:<14} mean {mean:+.3f}  95% CI [{mean-hw:+.3f},{mean+hw:+.3f}]"
                  f"   judge-only-right {win:>3}   copy-only-right {loss:>3}")

    json.dump(rows, open("/tmp/copy-ceiling-rows.json", "w"), indent=1)
    print("\nper-item rows → /tmp/copy-ceiling-rows.json")

if __name__ == "__main__":
    main()
