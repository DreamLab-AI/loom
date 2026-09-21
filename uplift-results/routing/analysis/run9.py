import ceiling as C
from ceiling import *
from run3 import seg_with
from math import comb
C.EXHAUSTIVE=True; JT1=sum(JUDGE_OK)/N
def mc(items):
    b=sum(1 for (c,_),j in zip(items,JUDGE_OK) if j and not c); c_=sum(1 for (c,_),j in zip(items,JUDGE_OK) if c and not j); n=b+c_
    return b,c_,(1.0 if n==0 else min(1.0,sum(comb(n,k) for k in range(n+1) if abs(k-n/2)>=abs(b-n/2))/2**n))
def rep(nm,picks):
    r=evaluate(picks,EXPECTED,"abs"); b,c_,p=mc(r["items"]); m,lo,hi,_,_=signed(r["items"],JUDGE_OK)
    print("%-40s %5.1f (%2d/86) /%5.1f | gain %+5.1f | %+.3f [%+.3f,%+.3f] | %2d:%-2d p=%.4f"
          %(nm,r["top1"]*100,round(r["top1"]*N),r["top3"]*100,(JT1-r["top1"])*100,m,lo,hi,b,c_,p)); return r
POSB=[seg_with(NEG,cands[k])[0] for k in KEYS]; NEGB=[seg_with(NEG,cands[k])[1] for k in KEYS]
pdocs=[tok(f"{KEYS[i].replace('-',' ')}: {POSB[i]}") for i in range(115)]
ndocs=[tok(NEGB[i]) for i in range(115)]
pi=bm25_index(pdocs); ni=bm25_index([d if d else ["\x00"] for d in ndocs])
print("all thresholds exhaustive; judge 88.4 (76/86)\n")
rep("strong: exclusions DROPPED", [rank_from_scores(bm25_scores(tok(p),pi)) for p in PROMPTS])
for vr in (1.0,1.5,2.0,3.0):
    out=[]
    for p in PROMPTS:
        q=tok(p); sp=bm25_scores(q,pi); sn=bm25_scores(q,ni)
        sc=[(-1e9 if (ndocs[i] and sn[i]>vr*sp[i]) else sp[i]) for i in range(115)]
        out.append(rank_from_scores(sc))
    rep("veto: drop option if neg > %.1fx pos"%vr, out)
