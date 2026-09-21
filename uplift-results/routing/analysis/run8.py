import ceiling as C
from ceiling import *
from run3 import seg_with
from math import comb
C.EXHAUSTIVE = True
JT1=sum(JUDGE_OK)/N
def mcnemar(items):
    b=sum(1 for (c,_),j in zip(items,JUDGE_OK) if j and not c); c_=sum(1 for (c,_),j in zip(items,JUDGE_OK) if c and not j)
    n=b+c_
    if n==0: return b,c_,1.0
    return b,c_,min(1.0,sum(comb(n,k) for k in range(n+1) if abs(k-n/2)>=abs(b-n/2))/2**n)

def build(drop=True, k1=K1, b=B, wname=0.0, bigrams=False):
    T=tok
    def bg(ts): return ts+["%s_%s"%(ts[i],ts[i+1]) for i in range(len(ts)-1)] if bigrams else ts
    body=[(seg_with(NEG,cands[k])[0] if drop else cands[k]) for k in KEYS]
    docs=[bg(T(f"{KEYS[i].replace('-',' ')}: {body[i]}")) for i in range(len(KEYS))]
    idx=bm25_index(docs)
    if wname:
        nd=[bg(T(KEYS[i].replace('-',' '))) for i in range(len(KEYS))]; nidx=bm25_index(nd)
    out=[]
    for p in PROMPTS:
        q=bg(T(p)); sc=bm25_scores(q,idx,k1,b)
        if wname:
            sn=bm25_scores(q,nidx,k1,b); sc=[sc[i]+wname*sn[i] for i in range(len(KEYS))]
        out.append(rank_from_scores(sc))
    return out

best=None
print("sweeping b, k1, bigrams over the drop-exclusion ranker (all thresholds exhaustive)")
print("%-34s %-14s %-8s %s"%("config","ceiling top1","gain","McNemar"))
for b in (0.0,0.25,0.5,0.75,1.0):
  for k1 in (0.9,1.2,1.5,2.0):
    for bgm in (False,True):
        picks=build(True,k1,b,0.0,bgm); r=evaluate(picks,EXPECTED,"abs")
        bb,cc,p=mcnemar(r["items"])
        key=(r["top1"],-abs(b-0.75),-abs(k1-1.5))
        if best is None or key>best[0]: best=(key,"b=%.2f k1=%.1f bigram=%d"%(b,k1,bgm),r,bb,cc,p,picks)
        if r["top1"]>=0.849:
            print("  b=%.2f k1=%.1f bigram=%d%-12s %5.1f (%2d/86)  %+5.1f    %2d:%-2d p=%.4f"
                  %(b,k1,bgm,"",r["top1"]*100,round(r["top1"]*N),(JT1-r["top1"])*100,bb,cc,p))
print("\nBEST: %s -> %.1f%% (%d/86) gain %+.1f  %d:%d p=%.4f"
      %(best[1],best[2]["top1"]*100,round(best[2]["top1"]*N),(JT1-best[2]["top1"])*100,best[3],best[4],best[5]))
r=best[2]
print("top-3 %.1f%% (judge 98.8, gain %+.1f) thr %.4f fired %d"%(r["top3"]*100,(0.988-r["top3"])*100,r["threshold"],r["fired"]))
m,lo,hi,_,_=signed(r["items"],JUDGE_OK); print("signed %+.3f [%+.3f,%+.3f]"%(m,lo,hi))

# subgroups: judge vs principled-strong vs best-tuned
princ=evaluate(build(True),EXPECTED,"abs")
basel=evaluate(build(False),EXPECTED,"abs")
print("\n%-22s%4s%8s%10s%8s%10s%8s%10s%8s"%("population","n","judge","published","gain","strong","gain","tuned","gain"))
for lab,pred in POPS:
    idx=[i for i in range(N) if pred(i)]
    ja=sum(JUDGE_OK[i] for i in idx)/len(idx)
    f=lambda rr: sum(rr["items"][i][0] for i in idx)/len(idx)
    print("%-22s%4d%8.1f%10.1f%+8.1f%10.1f%+8.1f%10.1f%+8.1f"
          %(lab,len(idx),ja*100,f(basel)*100,(ja-f(basel))*100,f(princ)*100,(ja-f(princ))*100,f(r)*100,(ja-f(r))*100))
