from ceiling import *
b = baseline_bm25()
for mode in ("abs",):
    r = evaluate(b, EXPECTED, mode)
    print("BASELINE bm25  no-decline top1=%.3f" % (sum(1 for t in b if t[0][0]==EXPECTED[i]) for i in [0]) if 0 else "")
nd = sum(1 for i,(t3,bs,mg) in enumerate(b) if t3[0]==EXPECTED[i])/N
nd3 = sum(1 for i,(t3,bs,mg) in enumerate(b) if EXPECTED[i] in t3)/N
print("BASELINE BM25 no-decline: top1 %.1f%% top3 %.1f%%" % (nd*100, nd3*100))
r = evaluate(b, EXPECTED, "abs")
print("BASELINE BM25 fair      : top1 %.1f%% top3 %.1f%% thr %.4f fired %d" % (r["top1"]*100, r["top3"]*100, r["threshold"], r["fired"]))
print("judge top1 %.1f%%" % (sum(JUDGE_OK)/N*100))
print("gain %.1f" % ((sum(JUDGE_OK)/N - r["top1"])*100))
m,lo,hi,jo,co = signed(r["items"], JUDGE_OK)
print("signed %.3f [%.3f,%.3f] judge-only %d copy-only %d" % (m,lo,hi,jo,co))
