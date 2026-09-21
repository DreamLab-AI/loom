from ceiling import *
from run3 import drop_exclusions, seg_with
JT1=sum(JUDGE_OK)/N
docs=[tok(f"{k.replace('-',' ')}: {seg_with(NEG,cands[k])[0]}") for k in KEYS]
idx=bm25_index(docs)
raw=[]
for p in PROMPTS:
    sc=bm25_scores(tok(p), idx); o=sorted(range(len(KEYS)),key=lambda i:-sc[i])
    raw.append(([KEYS[i] for i in o[:3]], sc, o))

def ev(signal, label):
    pc=[(t3, signal(i), 0.0) for i,(t3,sc,o) in enumerate(raw)]
    r=evaluate(pc, EXPECTED, "abs"); m,lo,hi,jo,co=signed(r["items"],JUDGE_OK)
    print("%-42s fair %5.1f/%5.1f f%2d | gain %+5.1f | %+.3f [%+.3f,%+.3f] %s"
          %(label,r["top1"]*100,r["top3"]*100,r["fired"],(JT1-r["top1"])*100,m,lo,hi,"SIG" if lo>0 else "ns"))
    return r
print("-- decline signals over the strong (drop-exclusion) ranking --")
ev(lambda i: raw[i][1][raw[i][2][0]], "absolute best score")
ev(lambda i: raw[i][1][raw[i][2][0]]/max(1,len(tok(PROMPTS[i]))), "best / query length")
ev(lambda i: raw[i][1][raw[i][2][0]]-raw[i][1][raw[i][2][1]], "top-2 margin")
ev(lambda i: (raw[i][1][raw[i][2][0]]-raw[i][1][raw[i][2][1]])/(raw[i][1][raw[i][2][0]] or 1), "relative margin")
ev(lambda i: raw[i][1][raw[i][2][0]]/(sum(raw[i][1][j] for j in raw[i][2][:5]) or 1), "best / top-5 mass")
ev(lambda i: sum(1 for t in set(tok(PROMPTS[i])) if t in set(docs[raw[i][2][0]]))/max(1,len(set(tok(PROMPTS[i])))), "query-term coverage of the top doc")

print("\n-- does turn 14's re-adjudication still decide significance? --")
import copy as _c
for lab, exp14, jok14 in (("PRE-adjudication  (browser)","browser",False),
                          ("POST-adjudication (browser-automation)","browser-automation",True)):
    E=list(EXPECTED); E[14]=exp14; J=list(JUDGE_OK); J[14]=jok14
    for nm, picks in (("published ceiling",baseline_bm25()),("strong ceiling",drop_exclusions(NEG))):
        r=evaluate(picks,E,"abs")
        g=[(1 if J[i] else 0)-(1 if r["items"][i][0] else 0) for i in range(N)]
        m=sum(g)/N; v=sum((x-m)**2 for x in g)/(N-1); se=(v/N)**.5
        print("  %-38s %-18s judge %5.1f  ceiling %5.1f  gain %+5.1f  %+.3f [%+.3f,%+.3f] %s"
              %(lab,nm,sum(J)/N*100,r["top1"]*100,(sum(J)/N-r["top1"])*100,m,m-1.96*se,m+1.96*se,
                "SIG" if m-1.96*se>0 else "ns"))
