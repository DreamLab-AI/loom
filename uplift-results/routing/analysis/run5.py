import random
from ceiling import *
from run3 import drop_exclusions, seg_with
JT1=sum(JUDGE_OK)/N
base=evaluate(baseline_bm25(), EXPECTED, "abs")
strong_picks=drop_exclusions(NEG)
strong=evaluate(strong_picks, EXPECTED, "abs")

def subgroup(items, pred):
    idx=[i for i in range(N) if pred(i)]
    if not idx: return None
    ja=sum(JUDGE_OK[i] for i in idx)/len(idx)
    ca=sum(items[i][0] for i in idx)/len(idx)
    return len(idx), ja*100, ca*100, (ja-ca)*100

print("population                     n   judge   BM25(published)  gain      BM25-strong  gain")
for lab,pred in POPS:
    a=subgroup(base["items"],pred); b=subgroup(strong["items"],pred)
    print("%-26s %4d  %5.1f   %8.1f  %+7.1f   %10.1f  %+6.1f"%(lab,a[0],a[1],a[2],a[3],b[2],b[3]))

print("\ntop-3:  judge 98.8  published %.1f (gain %+.1f)  strong %.1f (gain %+.1f)"
      % (base["top3"]*100,(0.988-base["top3"])*100, strong["top3"]*100,(0.988-strong["top3"])*100))
print("thresholds: published %.4f fired %d | strong %.4f fired %d"%(base["threshold"],base["fired"],strong["threshold"],strong["fired"]))

# paired bootstrap on the signed gain, seeded, 10000 resamples (the paper's protocol)
def boot(items, R=10000, seed=1):
    rnd=random.Random(seed)
    g=[(1 if JUDGE_OK[i] else 0)-(1 if items[i][0] else 0) for i in range(N)]
    ms=[]
    for _ in range(R):
        s=[g[rnd.randrange(N)] for _ in range(N)]; ms.append(sum(s)/N)
    ms.sort(); return sum(g)/N, ms[int(.025*R)], ms[int(.975*R)]
print("\npaired bootstrap (10k, seed 1):")
print("  published ceiling  %+.3f [%+.3f,%+.3f]"%boot(base["items"]))
print("  strong ceiling     %+.3f [%+.3f,%+.3f]"%boot(strong["items"]))

# threshold sensitivity of the strong ceiling: accuracy at every grid point
sig=[p[1] for p in strong_picks]; lo,hi=min(sig),max(sig)
accs=[]
for i in range(SWEEP):
    th=lo+(hi-lo)*i/(SWEEP-1)
    it=[]
    for k,(t3,bs,mg) in enumerate(strong_picks):
        it.append((EXPECTED[k]==NONE) if bs<th else (t3[0]==EXPECTED[k]))
    accs.append(sum(it)/N)
print("\nstrong ceiling top-1 across the 41-point threshold grid: min %.1f max %.1f, "
      "points >= published 76.7: %d/41" % (min(accs)*100, max(accs)*100, sum(1 for a in accs if a>=0.767)))

# which turns flip
print("\nturns the published ceiling misses but the strong one gets:")
for i in range(N):
    if strong["items"][i][0] and not base["items"][i][0]:
        print("  %2d [%s] %-22s judge=%-22s %s"%(i,CLS[i],EXPECTED[i],JUDGE_PICK[i],PROMPTS[i][:66]))
print("turns the strong ceiling loses:")
for i in range(N):
    if base["items"][i][0] and not strong["items"][i][0]:
        print("  %2d [%s] %-22s %s"%(i,CLS[i],EXPECTED[i],PROMPTS[i][:66]))
