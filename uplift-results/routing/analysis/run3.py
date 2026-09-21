from ceiling import *
JT1 = sum(JUDGE_OK)/N
def report(name, picks, mode="abs", quiet=False):
    r = evaluate(picks, EXPECTED, mode)
    nd = sum(1 for i,(t3,bs,mg) in enumerate(picks) if t3[0]==EXPECTED[i])/N
    m,lo,hi,jo,co = signed(r["items"], JUDGE_OK)
    if not quiet:
        print("%-46s nd %5.1f | fair %5.1f/%5.1f f%2d | gain %+5.1f | %+.3f [%+.3f,%+.3f] %2d/%2d %s"
              % (name, nd*100, r["top1"]*100, r["top3"]*100, r["fired"], (JT1-r["top1"])*100, m,lo,hi,jo,co,
                 "SIG" if lo>0 else "ns"))
    return r, m, lo, hi

# ---- robustness of the segmentation: three regexes of increasing crudeness ----
REGEXES = {
 "crude  (literal 'not for' only)": re.compile(r"\bnot for\b", re.I),
 "plain  (not for|use X instead|rather than|instead of)":
     re.compile(r"\b(not for|rather than|instead of|use \S+ instead)\b", re.I),
 "full   (the regex above)": NEG,
}
def seg_with(rx, rubric):
    parts=[p for p in SPLIT.split(rubric) if p.strip()]; pos=[];neg=[]
    for p in parts:
        m=rx.search(p)
        if m: pos.append(p[:m.start()]); neg.append(p[m.start():])
        else: pos.append(p)
    return " ".join(pos), " ".join(neg)

def drop_exclusions(rx, stemming=False, bigrams=False, expand=0):
    T = tok_s if stemming else tok
    def bg(ts): return ts + ["%s_%s"%(ts[i],ts[i+1]) for i in range(len(ts)-1)] if bigrams else ts
    docs=[bg(T(f"{k.replace('-',' ')}: {seg_with(rx,cands[k])[0]}")) for k in KEYS]
    idx=bm25_index(docs); out=[]
    for p in PROMPTS:
        q=bg(T(p)); sc=bm25_scores(q, idx)
        if expand:  # RM3-style pseudo-relevance feedback over the exposure itself
            order=sorted(range(len(KEYS)), key=lambda i:-sc[i])[:expand]
            fb=Counter()
            for i in order: fb.update(set(docs[i]))
            qw={t:1.0 for t in set(q)}
            for t,c in fb.items():
                if c>=max(2,expand-1): qw[t]=qw.get(t,0.0)+0.3
            sc=bm25_scores(q, idx, qweights=qw)
        out.append(rank_from_scores(sc))
    return out

print("judge top-1 %.1f%%   BASELINE below\n"%(JT1*100))
report("BASELINE copy.rs", baseline_bm25())
print("\n-- drop-exclusions, segmentation robustness --")
for lab,rx in REGEXES.items():
    n=sum(1 for k in KEYS if rx.search(cands[k]))
    report("%s [%d/115 rubrics]"%(lab,n), drop_exclusions(rx))
print("\n-- drop-exclusions + preprocessing (full regex) --")
for st in (0,1):
  for bgm in (0,1):
    report("drop-excl stem=%d bigram=%d"%(st,bgm), drop_exclusions(NEG, st, bgm))
print("\n-- query expansion (RM3 over the exposure) --")
for e in (2,3,5):
    report("drop-excl + PRF top-%d"%e, drop_exclusions(NEG, expand=e))
print("\n-- decline rule: absolute vs margin --")
base=drop_exclusions(NEG)
report("drop-excl, ABSOLUTE decline", base, "abs")
report("drop-excl, MARGIN decline", base, "margin")
report("BASELINE, MARGIN decline", baseline_bm25(), "margin")
