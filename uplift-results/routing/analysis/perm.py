import json,math
from itertools import product
from math import comb
m=json.load(open('/tmp/mined-rows.json'))
cases=json.load(open('/home/devuser/workspace/project/agentbox/tests/system-one/routing-cases.json'))['cases']
for r in m: r['g']=cases[r['i']]['expected_skill']; r['go']='browser' if r['i']==14 else r['g']
T_PUB=9.156341002956207; T_ORC=9.6568
def dist(n_disc):
    "distribution of (b-c) sums over 2^n sign patterns -> dict{value:count}"
    d={}
    for k in range(n_disc+1):   # k positives
        d[2*k-n_disc]=d.get(2*k-n_disc,0)+comb(n_disc,k)
    return d
def strat(key,t):
    for r in m:
        r['j']=r['judge_pick']==r[key]; r['l']=('none' if r['lex_score']<t else r['lex_pick'])==r[key]
    N=[r for r in m if r[key]=='none']; S=[r for r in m if r[key]!='none']
    f=lambda X:(sum(1 for r in X if r['j'] and not r['l']),sum(1 for r in X if r['l'] and not r['j']),len(X))
    return f(N),f(S)
def signflip_p(key,t,w):
    (bN,cN,nN),(bS,cS,nS)=strat(key,t)
    obs=w*(bN-cN)/nN+(1-w)*(bS-cS)/nS
    dN=dist(bN+cN); dS=dist(bS+cS)
    tot=0; hit=0
    for vN,cn in dN.items():
        for vS,cs in dS.items():
            d=w*vN/nN+(1-w)*vS/nS
            tot+=cn*cs
            if abs(d)>=abs(obs)-1e-12: hit+=cn*cs
    return obs,hit/tot,(bN,cN,nN),(bS,cS,nS)
print('Exact stratified sign-flip (permutation) test — the post-stratified analogue of McNemar')
print(f"{'cell':<24} {'w':>5} {'d':>8} {'exact p':>9}  strata discordant")
for key,lbl in [('go','PRE '),('g','POST')]:
    for t,tl in [(T_PUB,'published t'),(T_ORC,'oracle t   ')]:
        for w in (0.19,0.65):
            obs,p,N,S=signflip_p(key,t,w)
            print(f"{lbl+' + '+tl:<24} {w:5.2f} {obs:+8.3f} {p:9.4f}  none b={N[0]} c={N[1]} /{N[2]}; skill b={S[0]} c={S[1]} /{S[2]}")
    print()
# contribution decomposition at w=.65, oracle
for key,lbl in [('go','PRE'),('g','POST')]:
    (bN,cN,nN),(bS,cS,nS)=strat(key,T_ORC)
    w=.65
    print(f'{lbl} oracle w=.65: none contributes {w*(bN-cN)/nN:+.4f} ({100*w*(bN-cN)/nN/(w*(bN-cN)/nN+(1-w)*(bS-cS)/nS):.0f}% of effect) from {bN+cN} discordant turns; skill contributes {(1-w)*(bS-cS)/nS:+.4f} from {bS+cS}')
