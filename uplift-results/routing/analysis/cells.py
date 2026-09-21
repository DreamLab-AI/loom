import json,math,random
from math import comb
m=json.load(open('/tmp/mined-rows.json'))
cases=json.load(open('/home/devuser/workspace/project/agentbox/tests/system-one/routing-cases.json'))['cases']
for r in m: r['g']=cases[r['i']]['expected_skill']; r['go']='browser' if r['i']==14 else r['g']
T_PUB=9.156341002956207; T_ORC=9.6568   # inside the plateau (9.5715, 9.742)

def mcnemar_exact(b,c):
    n=b+c
    if n==0: return 1.0
    k=min(b,c)
    p=sum(comb(n,i) for i in range(0,k+1))/2**n*2
    return min(1.0,p)

def cell(key,t,label,W=(0.19,0.65),B=40000,seed=17):
    for r in m:
        r['j']=r['judge_pick']==r[key]
        r['l']=('none' if r['lex_score']<t else r['lex_pick'])==r[key]
    N=[r for r in m if r[key]=='none']; S=[r for r in m if r[key]!='none']
    bN=sum(1 for r in N if r['j'] and not r['l']); cN=sum(1 for r in N if r['l'] and not r['j'])
    bS=sum(1 for r in S if r['j'] and not r['l']); cS=sum(1 for r in S if r['l'] and not r['j'])
    b,c=bN+bS,cN+cS
    out={'label':label,'nN':len(N),'nS':len(S),'bN':bN,'cN':cN,'bS':bS,'cS':cS,
         'judge':sum(r['j'] for r in m)/86,'bm25':sum(r['l'] for r in m)/86,
         'unw_d':(b-c)/86,'unw_p':mcnemar_exact(b,c)}
    se=math.sqrt((b+c-(b-c)**2/86)/86**2) if b+c else 0
    out['unw_ci']=((b-c)/86-1.96*se,(b-c)/86+1.96*se)
    for w in W:
        d=w*(bN-cN)/len(N)+(1-w)*(bS-cS)/len(S)
        # analytic SE: independent strata, within-stratum var of paired diff
        def sv(rows,bb,cc):
            n=len(rows); dd=(bb-cc)/n
            return (bb+cc-(bb-cc)**2/n)/n**2
        se=math.sqrt(w*w*sv(N,bN,cN)+(1-w)**2*sv(S,bS,cS))
        random.seed(seed); ds=[]
        for _ in range(B):
            n_=[random.choice(N) for _ in N]; s_=[random.choice(S) for _ in S]
            a=w*(sum(r['j'] for r in n_)/len(n_))+(1-w)*(sum(r['j'] for r in s_)/len(s_))
            q=w*(sum(r['l'] for r in n_)/len(n_))+(1-w)*(sum(r['l'] for r in s_)/len(s_))
            ds.append(a-q)
        ds.sort()
        p_boot=2*min(sum(1 for x in ds if x<=0),sum(1 for x in ds if x>=0))/B
        out[w]=(d,ds[int(.025*B)],ds[int(.975*B)-1],d-1.96*se,d+1.96*se,min(1.0,p_boot))
    return out

rows=[cell('go',T_PUB,'PRE  + published t'),
      cell('g', T_PUB,'POST + published t'),
      cell('go',T_ORC,'PRE  + TRUE oracle t'),
      cell('g', T_ORC,'POST + TRUE oracle t')]
print(f"{'cell':<24} {'judge':>6} {'bm25':>6} | {'unweighted d':>13} {'95% CI':>18} {'exactP':>7} | b,c")
for o in rows:
    print(f"{o['label']:<24} {100*o['judge']:6.1f} {100*o['bm25']:6.1f} | {o['unw_d']:+13.3f} [{o['unw_ci'][0]:+.3f},{o['unw_ci'][1]:+.3f}] {o['unw_p']:7.4f} | b={o['bN']+o['bS']} c={o['cN']+o['cS']}")
print()
for w in (0.19,0.65):
    print(f"--- post-stratified to none={w:.0%} ---")
    print(f"{'cell':<24} {'d':>8} {'boot 95%':>20} {'analytic 95%':>20} {'bootP':>7} | strata (b,c)")
    for o in rows:
        d,lo,hi,alo,ahi,p=o[w]
        print(f"{o['label']:<24} {d:+8.3f} [{lo:+.3f},{hi:+.3f}] [{alo:+.3f},{ahi:+.3f}] {p:7.4f} | none b={o['bN']} c={o['cN']} (n={o['nN']}); skill b={o['bS']} c={o['cS']} (n={o['nS']})")
    print()
