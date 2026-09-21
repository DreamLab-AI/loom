from ceiling import *
JT1 = sum(JUDGE_OK)/N

def report(name, picks, mode="abs"):
    r = evaluate(picks, EXPECTED, mode)
    nd  = sum(1 for i,(t3,bs,mg) in enumerate(picks) if t3[0]==EXPECTED[i])/N
    m,lo,hi,jo,co = signed(r["items"], JUDGE_OK)
    print("%-52s nd %5.1f | fair %5.1f/%5.1f fired %2d | gain %+5.1f | %+.3f [%+.3f,%+.3f] %2d/%2d %s"
          % (name, nd*100, r["top1"]*100, r["top3"]*100, r["fired"], (JT1-r["top1"])*100, m, lo, hi, jo, co,
             "SIG" if lo>0 else "ns"))
    return r

print("judge top-1 %.1f%%\n" % (JT1*100))
report("BASELINE copy.rs (single bag, abs decline)", baseline_bm25())
print()
# ablations, one at a time
for wn in (0.0, 1.0, 2.0):
  for wneg in (0.0, 0.5, 1.0, 1.5, 2.0):
    report("fielded w_name=%.1f w_neg=%.1f" % (wn, wneg), fielded(w_name=wn, w_neg=wneg))
print()
for st in (False, True):
  for bg in (False, True):
    report("w_name=1 w_neg=1 stem=%d bigram=%d" % (st, bg), fielded(w_name=1.0, w_neg=1.0, stemming=st, bigrams=bg))
