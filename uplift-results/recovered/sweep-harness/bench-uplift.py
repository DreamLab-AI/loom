#!/usr/bin/env python3
"""Ontology knowledge-uplift A/B: baseline vs scaffold, per model.
For each relation-completion question, run WITHOUT and WITH the ontology scaffold,
auto-grade whether the response names the ontology's ground-truth related concept,
and track whether the scaffold actually engaged. Isolates knowledge uplift.
Usage: bench-uplift.py PORT MODEL_LABEL
"""
import json, sys, re, time, urllib.request
from ontology_scaffold import scaffold, scaffold_messages

BASE = f"http://127.0.0.1:{sys.argv[1]}"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "unknown"
SAMP = dict(temperature=0.7, top_p=0.95, top_k=64)
Q = json.load(open("ontology-uplift-questions.json"))

def post(messages):
    body = dict(messages=messages, max_tokens=8192, **SAMP)
    try:
        req = urllib.request.Request(f"{BASE}/v1/chat/completions", json.dumps(body).encode(), {"Content-Type": "application/json"})
        d = json.loads(urllib.request.urlopen(req, timeout=400).read().decode(), strict=False)
        return (d["choices"][0]["message"].get("content") or "").strip()
    except Exception as e:
        return f"ERROR:{e}"

def hit(resp, q):
    r = " " + resp.lower().replace("-", " ").replace("_", " ") + " "
    gold = q["gold"].lower().replace("-", " ")
    if gold in r:
        return True
    if q["gold_acr"] and re.search(rf"\b{re.escape(q['gold_acr'])}\b", r):
        return True
    if q["gold_words"] and all(w in r for w in q["gold_words"]):
        return True
    return False

DECLINE = ["i'm not familiar", "i am not familiar", "not aware of", "don't have specific", "no widely recognized",
           "not a standard", "not a recognized", "i couldn't find", "i don't know", "unable to find", "no information"]

def main():
    scaffold("warmup")  # warm the index cache so case 1 latency is clean
    rows = []
    b_hits = s_hits = engaged = 0
    print(f"=== UPLIFT A/B: {LABEL} ({BASE}) — {len(Q)} questions ===", flush=True)
    for q in Q:
        base_msgs = [{"role": "user", "content": q["question"]}]
        scaf_msgs = scaffold_messages(base_msgs, budget_tokens=1500)
        eng = scaf_msgs != base_msgs and any(m.get("role") == "system" for m in scaf_msgs)
        br = post(base_msgs)
        sr = post(scaf_msgs)
        bh, sh = hit(br, q), hit(sr, q)
        b_hits += bh; s_hits += sh; engaged += eng
        base_declined = any(x in br.lower() for x in DECLINE)
        rows.append({"id": q["id"], "domain": q["domain"], "gold": q["gold"], "engaged": eng,
                     "base_hit": bh, "scaf_hit": sh, "base_declined": base_declined,
                     "base": br[:200], "scaf": sr[:200]})
        flag = "UP" if (sh and not bh) else ("=" if sh == bh else "DN")
        print(f"  [{flag}] {q['id'][:22]:22s} eng={int(eng)} base={int(bh)} scaf={int(sh)} | gold={q['gold'][:32]}", flush=True)
    n = len(Q)
    eng_rows = [r for r in rows if r["engaged"]]
    ne = len(eng_rows)
    be = sum(r["base_hit"] for r in eng_rows); se = sum(r["scaf_hit"] for r in eng_rows)
    out = {"model": LABEL, "n": n, "engaged": engaged,
           "baseline_hits": b_hits, "scaffold_hits": s_hits,
           "baseline_pct": round(b_hits / n * 100, 1), "scaffold_pct": round(s_hits / n * 100, 1),
           "engaged_baseline_pct": round(be / ne * 100, 1) if ne else 0,
           "engaged_scaffold_pct": round(se / ne * 100, 1) if ne else 0,
           "base_declines": sum(r["base_declined"] for r in rows), "rows": rows}
    json.dump(out, open(f"../logs/uplift-{LABEL.replace(' ','_')}.json", "w"), indent=1)
    print(f"\n  scaffold engaged: {engaged}/{n}")
    print(f"  OVERALL   baseline {out['baseline_pct']}%  ->  scaffold {out['scaffold_pct']}%  (uplift {out['scaffold_pct']-out['baseline_pct']:+.1f} pts)")
    print(f"  ENGAGED-ONLY ({ne})  baseline {out['engaged_baseline_pct']}%  ->  scaffold {out['engaged_scaffold_pct']}%  (uplift {out['engaged_scaffold_pct']-out['engaged_baseline_pct']:+.1f} pts)")
    print(f"  baseline declined (said 'don't know'): {out['base_declines']}/{n}")

if __name__ == "__main__":
    main()
