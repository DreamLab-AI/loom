import json,math,random
m=json.load(open('/tmp/mined-rows.json'))
cases=json.load(open('/home/devuser/workspace/project/agentbox/tests/system-one/routing-cases.json'))['cases']
# corrected labels
for r in m:
    g=cases[r['i']]['expected_skill']
    r['gold']=g
    r['jok']=r['judge_pick']==g
    r['lok']=r['lex_final']==g
    r['none']=(g=='none')
def stats(rows,lbl):
    n=len(rows); j=sum(r['jok'] for r in rows); l=sum(r['lok'] for r in rows)
    print(f"{lbl:<22} n={n:>3} judge={j:>3} ({j/n:.3f})  bm25={l:>3} ({l/n:.3f})  gain={(j-l)/n:+.3f}")
    return n,j,l
def ci(rows):
    b=sum(1 for r in rows if r['jok'] and not r['lok']); c=sum(1 for r in rows if r['lok'] and not r['jok']); n=len(rows)
    d=(b-c)/n; se=math.sqrt((b+c-(b-c)**2/n)/n**2) if (b+c) else 0
    return d,d-1.96*se,d+1.96*se,b,c
for pre in (0,1):
    if pre:
        for r in m:
            g='browser' if r['i']==14 else r['gold']
            r['jok']=r['judge_pick']==g; r['lok']=r['lex_final']==g; r['none']=(g=='none')
    print('=== labels:', 'PRE-correction (turn 14 = browser)' if pre else 'POST-correction (turn 14 = browser-automation)')
    stats(m,'ALL')
    d,lo,hi,b,c=ci(m); print(f"  paired diff {d:+.3f} [{lo:+.3f},{hi:+.3f}]  discordant b={b} c={c}")
    N=[r for r in m if r['none']]; S=[r for r in m if not r['none']]
    nn,nj,nl=stats(N,'  none turns'); sn,sj,sl=stats(S,'  skill turns')
    for w in (0.19,0.40,0.65):
        acc=lambda k: w*(sum(r[k] for r in N)/nn)+(1-w)*(sum(r[k] for r in S)/sn)
        print(f"   reweight none={w:.0%}: judge={acc('jok'):.3f} bm25={acc('lok'):.3f} gain={acc('jok')-acc('lok'):+.3f}")
    # bootstrap CI on reweighted gain at 65%
    random.seed(7)
    for w in (0.65,):
        ds=[]
        for _ in range(20000):
            sn_=[random.choice(S) for _ in S]; nn_=[random.choice(N) for _ in N]
            a=w*(sum(r['jok'] for r in nn_)/len(nn_))+(1-w)*(sum(r['jok'] for r in sn_)/len(sn_))
            bq=w*(sum(r['lok'] for r in nn_)/len(nn_))+(1-w)*(sum(r['lok'] for r in sn_)/len(sn_))
            ds.append(a-bq)
        ds.sort(); print(f"   bootstrap gain @none={w:.0%}: {sum(ds)/len(ds):+.3f} [{ds[500]:+.3f},{ds[19499]:+.3f}]")
    print()
