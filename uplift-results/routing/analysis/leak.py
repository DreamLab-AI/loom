import json,re,math,statistics as st
from collections import Counter
cases=json.load(open('/home/devuser/workspace/project/agentbox/tests/system-one/routing-cases.json'))['cases']
cand=json.load(open('/tmp/candidates.json'))
STOP=set('a an the and or of to for in on with by is are be this that it its as at from when use used using not do does what which how i we you my our your me them they he she his her but if then than so via per over under into out up down all any some more most other another each every'.split())
def toks(s):
    return [w for w in re.findall(r"[a-z0-9][a-z0-9\-']+", s.lower()) if w not in STOP and len(w)>2]
keys=list(cand.keys())
rub={k:set(toks(v)) for k,v in cand.items()}
rows=[]
for i,c in enumerate(cases):
    g=c['expected_skill']
    if g=='none': continue
    p=set(toks(c['prompt']))
    if not p: continue
    gold=len(p & rub[g])/len(p)
    others=[len(p & rub[k])/len(p) for k in keys if k!=g]
    rank=sum(1 for o in others if o>=gold)  # how many rubrics beat gold
    rows.append((i,c['class'],c['provenance'],gold,st.mean(others),max(others),rank,len(p)))
print(f"{'i':>3} {'class':<14} {'prov':<18} {'gold%':>6} {'mean%':>6} {'max%':>6} {'#beat':>5} {'ptok':>4}")
for r in rows:
    print(f"{r[0]:>3} {r[1]:<14} {r[2]:<18} {r[3]*100:>6.1f} {r[4]*100:>6.1f} {r[5]*100:>6.1f} {r[6]:>5} {r[7]:>4}")
print()
print('N non-none =',len(rows))
print('mean gold overlap %.1f%%'%(100*st.mean(r[3] for r in rows)))
print('mean random-rubric overlap %.1f%%'%(100*st.mean(r[4] for r in rows)))
print('gold is argmax-overlap in %d/%d (%.0f%%)'%(sum(1 for r in rows if r[6]==0),len(rows),100*sum(1 for r in rows if r[6]==0)/len(rows)))
print('gold in top-3 overlap %d/%d'%(sum(1 for r in rows if r[6]<3),len(rows)))
for prov in set(r[2] for r in rows):
    sub=[r for r in rows if r[2]==prov]
    print(f'  {prov:<18} n={len(sub):>2} gold={100*st.mean(x[3] for x in sub):5.1f}% rand={100*st.mean(x[4] for x in sub):5.1f}% lift={st.mean(x[3] for x in sub)/st.mean(x[4] for x in sub):.1f}x argmax={sum(1 for x in sub if x[6]==0)}/{len(sub)}')
