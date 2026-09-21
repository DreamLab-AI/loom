from ceiling import *
from run3 import report, drop_exclusions, seg_with
JT1=sum(JUDGE_OK)/N

# embeddings of the POSITIVE-ONLY rubric (exclusion clauses removed)
pos_texts=[f"{k}: {seg_with(NEG,cands[k])[0]}" for k in KEYS]
rv_pos=embed_all(pos_texts); pv=embed_all(PROMPTS)
emb_pos=[[cos(p,r) for r in rv_pos] for p in pv]
emb_raw=emb_scores()

def from_scores(S): return [rank_from_scores(s) for s in S]

print("\n-- embedding ceiling --")
report("emb raw rubric (published)", from_scores(emb_raw))
report("emb, exclusions dropped", from_scores(emb_pos))

# lexical score vectors for fusion
def lex_scores(rx=NEG, drop=True):
    docs=[tok(f"{k.replace('-',' ')}: {(seg_with(rx,cands[k])[0] if drop else cands[k])}") for k in KEYS]
    idx=bm25_index(docs); return [bm25_scores(tok(p), idx) for p in PROMPTS]
LX_drop=lex_scores(); LX_base=[None]*N
docs_b=[tok(f"{k}: {cands[k]}") for k in KEYS]; ib=bm25_index(docs_b)
LX_base=[bm25_scores(tok(p), ib) for p in PROMPTS]

print("\n-- reciprocal-rank fusion --")
for k in (10,30,60):
    report("RRF k=%d  base-BM25 + emb-raw"%k, from_scores(rrf([LX_base,emb_raw],k)))
    report("RRF k=%d  drop-BM25 + emb-pos"%k, from_scores(rrf([LX_drop,emb_pos],k)))
    report("RRF k=%d  drop-BM25 + emb-pos + emb-raw"%k, from_scores(rrf([LX_drop,emb_pos,emb_raw],k)))

print("\n-- z-score linear fusion --")
def z(v):
    m=sum(v)/len(v); s=(sum((x-m)**2 for x in v)/len(v))**.5 or 1.0
    return [(x-m)/s for x in v]
for a in (0.3,0.5,0.7):
    S=[[a*x+(1-a)*y for x,y in zip(z(L),z(E))] for L,E in zip(LX_drop,emb_pos)]
    report("z-fuse a=%.1f (drop-BM25 %.0f%% / emb-pos)"%(a,a*100), from_scores(S))
