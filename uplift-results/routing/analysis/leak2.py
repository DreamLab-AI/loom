import json,re,statistics as st
cases=json.load(open('/home/devuser/workspace/project/agentbox/tests/system-one/routing-cases.json'))['cases']
cand=json.load(open('/tmp/candidates.json'))
STOP=set('a an the and or of to for in on with by is are be this that it its as at from when use uses used using not do does what which how i we you my our your me them they he she his her but if then than so via per over under into out up down all any some more most other another each every want need just get got make made'.split())
def stem(w):
    for suf in ('ing','ies','ed','es','s'):
        if len(w)>4 and w.endswith(suf):
            w=w[:-len(suf)]
            if suf=='ies': w+='y'
            break
    return w
def toks(s): return set(stem(w) for w in re.findall(r"[a-z0-9][a-z0-9\-']+", s.lower()) if w not in STOP and len(w)>2)
keys=list(cand.keys()); rub={k:toks(v) for k,v in cand.items()}
rows=[]
for i,c in enumerate(cases):
    g=c['expected_skill']
    if g=='none': continue
    p=toks(c['prompt'])
    gold=len(p&rub[g])/len(p)
    sc=sorted(((len(p&rub[k])/len(p),k) for k in keys),reverse=True)
    beat=sum(1 for s,k in sc if s>gold)
    tie=sum(1 for s,k in sc if s==gold and k!=g)
    rows.append((i,c['class'],gold,st.mean(s for s,_ in sc if _!=g),beat,tie,sc[0][1],sc[0][0],len(p)))
print('N=',len(rows))
print('mean gold overlap %.1f%%  mean non-gold %.1f%%  lift %.1fx'%(100*st.mean(r[2] for r in rows),100*st.mean(r[3] for r in rows),st.mean(r[2] for r in rows)/st.mean(r[3] for r in rows)))
strict=sum(1 for r in rows if r[4]==0 and r[5]==0)
loose=sum(1 for r in rows if r[4]==0)
print('gold is UNIQUE argmax of raw unigram overlap: %d/%d (%.0f%%)'%(strict,len(rows),100*strict/len(rows)))
print('gold is tied-or-better argmax: %d/%d (%.0f%%)'%(loose,len(rows),100*loose/len(rows)))
print('gold in top-3: %d/%d'%(sum(1 for r in rows if r[4]<3),len(rows)))
print()
print('TOP LEAKERS (prompt vocabulary already in its gold rubric):')
for r in sorted(rows,key=lambda x:-x[2])[:12]:
    print(f"  [{r[0]:>2}] {100*r[2]:5.1f}% of prompt tokens are in the gold rubric | {cases[r[0]]['expected_skill']}")
    print(f"        {cases[r[0]]['prompt'][:120]}")
print()
print('LOWEST (gold least lexically supported):')
for r in sorted(rows,key=lambda x:x[2])[:8]:
    print(f"  [{r[0]:>2}] {100*r[2]:5.1f}% | gold={cases[r[0]]['expected_skill']} | rubrics beating gold: {r[4]}")
    print(f"        {cases[r[0]]['prompt'][:120]}")
