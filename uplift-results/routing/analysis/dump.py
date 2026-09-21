import json,sys
m=json.load(open('/tmp/mined-rows.json'))
cases=json.load(open('/home/devuser/workspace/project/agentbox/tests/system-one/routing-cases.json'))['cases']
cand=json.load(open('/tmp/candidates.json'))
idx=[int(x) for x in sys.argv[1:]]
def rub(k):
    if k=='none' or k is None: return '(none)'
    v=cand.get(k)
    if v is None: return '*** NOT IN CANDIDATES ***'
    return v if isinstance(v,str) else json.dumps(v)
for i in idx:
    r=m[i]; c=cases[i]
    print('='*100)
    print(f"[{i}] class={c['class']} prov={c['provenance']} late={c.get('late_discriminative')}")
    print('PROMPT:',r['prompt'])
    print('GOLD:',c['expected_skill'],'| JUDGE:',r['judge_pick'],f"(conf {r['judge_conf']:.2f})",'| BM25:',r['lex_pick'],f"({r['lex_score']:.1f}, margin {r['lex_margin']:.1f})")
    print('RATIONALE:',c['rationale'])
    print('--- GOLD RUBRIC ---'); print(rub(c['expected_skill']))
    if r['judge_pick']!=c['expected_skill']:
        print('--- JUDGE RUBRIC ---'); print(rub(r['judge_pick']))
    if r['lex_pick'] not in (c['expected_skill'], r['judge_pick']):
        print('--- BM25 RUBRIC ---'); print(rub(r['lex_pick']))
