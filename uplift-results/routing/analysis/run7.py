import ceiling as C
from ceiling import *
from run3 import drop_exclusions, seg_with
from math import comb
C.EXHAUSTIVE = True
JT1 = sum(JUDGE_OK)/N

def mcnemar(items, judge_ok):
    b = sum(1 for (c,_),j in zip(items,judge_ok) if j and not c)      # judge-only right
    c_ = sum(1 for (c,_),j in zip(items,judge_ok) if c and not j)     # copy-only right
    n = b + c_
    if n == 0: return b, c_, 1.0
    p = sum(comb(n,k) for k in range(n+1) if abs(k - n/2) >= abs(b - n/2)) / 2**n
    return b, c_, min(1.0, p)

RESULTS = []
def report(name, picks, mode="abs", show=True):
    r = evaluate(picks, EXPECTED, mode)
    nd = sum(1 for i,(t3,bs,mg) in enumerate(picks) if t3[0]==EXPECTED[i])/N
    m,lo,hi,_,_ = signed(r["items"], JUDGE_OK)
    b,c_,p = mcnemar(r["items"], JUDGE_OK)
    if show:
        print("%-44s nd %5.1f | fair %5.1f (%2d/86) /%5.1f thr %7.4f f%2d | gain %+5.1f | %+.3f [%+.3f,%+.3f] | %2d:%-2d p=%.4f"
              % (name, nd*100, r["top1"]*100, round(r["top1"]*N), r["top3"]*100, r["threshold"], r["fired"],
                 (JT1-r["top1"])*100, m, lo, hi, b, c_, p))
    RESULTS.append((name, r, p)); return r

print("judge top-1 %.1f%% (%d/86)   [all thresholds EXHAUSTIVE midpoint]\n" % (JT1*100, sum(JUDGE_OK)))
report("BASELINE copy.rs (grid-bug fixed)", baseline_bm25())
print()
REGEXES = {
 "crude (literal 'not for', 46/115)": re.compile(r"\bnot for\b", re.I),
 "plain (4 markers, 52/115)": re.compile(r"\b(not for|rather than|instead of|use \S+ instead)\b", re.I),
 "full  (my regex, 58/115)": NEG,
}
for lab,rx in REGEXES.items():
    report("drop-exclusions: %s" % lab, drop_exclusions(rx))
print()
for st in (0,1):
  for bg in (0,1):
    if st or bg: report("drop-excl stem=%d bigram=%d"%(st,bg), drop_exclusions(NEG,st,bg))
for e in (3,5):
    report("drop-excl + PRF top-%d"%e, drop_exclusions(NEG, expand=e))
print()
report("BASELINE, margin decline", baseline_bm25(), "margin")
report("drop-excl, margin decline", drop_exclusions(NEG), "margin")
print()
report("embedding (published rubric)", [rank_from_scores(s) for s in emb_scores()])
pos=[f"{k}: {seg_with(NEG,cands[k])[0]}" for k in KEYS]
ep=[[cos(p,r) for r in embed_all(pos)] for p in embed_all(PROMPTS)]
report("embedding, exclusions dropped", [rank_from_scores(s) for s in ep])
lxd=[None]*N
docs=[tok(f"{k.replace('-',' ')}: {seg_with(NEG,cands[k])[0]}") for k in KEYS]; ix=bm25_index(docs)
LXD=[bm25_scores(tok(p),ix) for p in PROMPTS]
docsb=[tok(f"{k}: {cands[k]}") for k in KEYS]; ixb=bm25_index(docsb)
LXB=[bm25_scores(tok(p),ixb) for p in PROMPTS]
for k in (10,60):
    report("RRF k=%d drop-BM25 + emb-pos"%k, [rank_from_scores(s) for s in rrf([LXD,ep],k)])
    report("RRF k=%d base-BM25 + emb-raw"%k, [rank_from_scores(s) for s in rrf([LXB,emb_scores()],k)])

# Holm across the whole family
print("\n-- Holm-Bonferroni over the %d comparisons above --" % len(RESULTS))
order = sorted(range(len(RESULTS)), key=lambda i: RESULTS[i][2])
mtot = len(RESULTS)
for rank,i in enumerate(order):
    nm,r,p = RESULTS[i]
    adj = min(1.0, p*(mtot-rank))
    print("  %-44s raw p=%.4f  Holm p=%.4f  %s" % (nm, p, adj, "SIG" if adj<0.05 else "ns"))
