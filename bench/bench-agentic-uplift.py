#!/usr/bin/env python3
"""In-domain AGENTIC uplift A/B (raw vs scaffold) + general-agentic control.
- IN-DOMAIN: agentic tool-use where the correct action needs a specific ontology fact
  (verify a concept's critical dependency via check_availability, then provision).
  Scaffold supplies the fact; baseline must know it. Scaffold ENGAGES (prompt names the concept).
- GENERAL: out-of-domain agentic tasks (banking/flights) — scaffold should NOT engage (control:
  proves the scaffold is neutral/harmless where irrelevant).
Runs each task raw and scaffolded, auto-grades on the gold concept appearing in the tool trace.
Usage: bench-agentic-uplift.py PORT MODEL_LABEL
"""
import json, sys, re, urllib.request
from ontology_scaffold import scaffold_messages, scaffold

BASE = f"http://127.0.0.1:{sys.argv[1]}"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "unknown"
SAMP = dict(temperature=0.7, top_p=0.95, top_k=64)

def td(name, desc, props, req):
    return {"type": "function", "function": {"name": name, "description": desc,
            "parameters": {"type": "object", "properties": props, "required": req}}}
# in-domain tools (generic; the agent must fill the component arg from domain knowledge)
T_CHECK = td("check_availability", "Verify a specific component/mechanism/capability is available before provisioning", {"component": {"type": "string"}}, ["component"])
T_PROV = td("provision", "Provision a system once its dependency is verified", {"system": {"type": "string"}}, ["system"])
# general (out-of-domain) tools
T_ACCTS = td("list_accounts", "List account ids", {}, [])
T_ATYPE = td("get_account_type", "checking or savings", {"account": {"type": "string"}}, ["account"])
T_BAL = td("get_balance", "Balance for an account", {"account": {"type": "string"}}, ["account"])
T_TX = td("get_transactions", "Paginated transactions (amount, category)", {"page": {"type": "integer"}}, ["page"])
T_FL = td("search_flights", "Flights: id, airline, price, duration_h", {"origin": {"type": "string"}, "dest": {"type": "string"}}, ["origin", "dest"])
T_BK = td("book_flight_id", "Book a flight id; returns confirmation", {"flight_id": {"type": "string"}}, ["flight_id"])

def mock(name, args):
    a = {k: str(v).lower() for k, v in args.items()}
    if name == "check_availability": return json.dumps({"component": args.get("component"), "available": True})
    if name == "provision": return json.dumps({"system": args.get("system"), "status": "provisioned"})
    if name == "list_accounts": return json.dumps({"accounts": ["ACC1", "ACC2", "ACC3", "ACC4"]})
    if name == "get_account_type": return json.dumps({"type": {"acc1": "checking", "acc2": "savings", "acc3": "checking", "acc4": "checking"}.get(a.get("account"), "unknown")})
    if name == "get_balance": return json.dumps({"balance": {"acc1": 1500, "acc2": 9200, "acc3": 400, "acc4": 2100}.get(a.get("account"), 0)})
    if name == "get_transactions":
        p = {1: {"items": [{"amount": 100, "category": "food"}, {"amount": 200, "category": "rent"}], "has_more": True, "next_page": 2},
             2: {"items": [{"amount": 50, "category": "food"}, {"amount": 400, "category": "rent"}], "has_more": True, "next_page": 3},
             3: {"items": [{"amount": 300, "category": "food"}], "has_more": False}}
        return json.dumps(p.get(int(args.get("page", 1)), {"items": [], "has_more": False}))
    if name == "search_flights": return json.dumps({"flights": [{"id": "F1", "airline": "RedAir", "price": 380, "duration_h": 6}, {"id": "F2", "airline": "BlueJet", "price": 450, "duration_h": 7}, {"id": "F3", "airline": "BlueJet", "price": 520, "duration_h": 5}, {"id": "F4", "airline": "SkyWay", "price": 410, "duration_h": 9}]})
    if name == "book_flight_id": return json.dumps({"status": "booked", "confirmation": "XR7788", "id": args.get("flight_id")})
    return json.dumps({"ok": True})

def run_agentic(user, tools, scaffolded, max_steps=12):
    msgs = [{"role": "user", "content": user}]
    if scaffolded:
        msgs = scaffold_messages(msgs, budget_tokens=1500)
    trace, final = [], ""
    for _ in range(max_steps):
        body = dict(messages=msgs, tools=tools, tool_choice="auto", max_tokens=8192, **SAMP)
        try:
            req = urllib.request.Request(f"{BASE}/v1/chat/completions", json.dumps(body).encode(), {"Content-Type": "application/json"})
            ch = json.loads(urllib.request.urlopen(req, timeout=400).read().decode(), strict=False)["choices"][0]
        except Exception as e:
            return trace, f"ERROR:{e}"
        m = ch["message"]; tcs = m.get("tool_calls") or []
        if not tcs:
            final = (m.get("content") or "").strip(); break
        msgs.append({"role": "assistant", "content": m.get("content"), "tool_calls": tcs})
        for tc in tcs:
            try: args = json.loads(tc["function"]["arguments"] or "{}")
            except Exception: args = {}
            trace.append({"name": tc["function"]["name"], "args": args})
            msgs.append({"role": "tool", "tool_call_id": tc.get("id", "0"), "content": mock(tc["function"]["name"], args)})
    return trace, final

def gold_in(text, q):
    t = " " + text.lower().replace("-", " ").replace("_", " ") + " "
    if q["gold"].lower().replace("-", " ") in t: return True
    if q.get("gold_acr") and re.search(rf"\b{re.escape(q['gold_acr'])}\b", t): return True
    if q.get("gold_words") and all(w in t for w in q["gold_words"]): return True
    return False

# ---- IN-DOMAIN agentic tasks (concept + gold dependency) ----
IND = [
 {"id": "1inch", "title": "1inch", "gold": "Automated Market Maker", "gold_acr": "amm", "gold_words": ["automated", "market", "maker"]},
 {"id": "2d-lidar", "title": "2D LiDAR", "gold": "Simultaneous Localisation and Mapping", "gold_acr": "slam", "gold_words": ["localisation", "mapping"]},
 {"id": "axelar", "title": "Axelar", "gold": "Validator Set", "gold_acr": "", "gold_words": ["validator"]},
 {"id": "crdt", "title": "CRDT", "gold": "Eventual Consistency", "gold_acr": "", "gold_words": ["eventual", "consistency"]},
 {"id": "balancer", "title": "Balancer", "gold": "Arbitrage", "gold_acr": "", "gold_words": ["arbitrage"]},
 {"id": "actuators", "title": "Actuators", "gold": "Feedback Control", "gold_acr": "", "gold_words": ["feedback", "control"]},
 {"id": "6dof", "title": "6DoF Tracking", "gold": "XR Applications", "gold_acr": "", "gold_words": ["applications"]},
 {"id": "aerial-robot", "title": "Aerial Robot", "gold": "Flight Control System", "gold_acr": "", "gold_words": ["flight", "control"]},
]
IND_TOOLS = [T_CHECK, T_PROV]
def ind_prompt(t):
    return (f"You are provisioning **{t['title']}** for a client. Before provisioning, you MUST verify that its single "
            f"most critical required underlying mechanism/dependency is available: call check_availability(component=...) "
            f"with that SPECIFIC component name, then call provision(system='{t['title']}'). Do not ask the user; decide yourself.")
def ind_grade(trace, final, q):
    checks = [c for c in trace if c["name"] == "check_availability"]
    prov = [c for c in trace if c["name"] == "provision"]
    hit = any(gold_in(str(c["args"].get("component", "")), q) for c in checks) or gold_in(final, q)
    if hit and prov: return (1.0, f"checked {q['gold']} + provisioned")
    if hit: return (0.7, "named dependency, no provision")
    if checks: return (0.2, f"checked wrong: {checks[0]['args'].get('component')}")
    return (0.0, "no correct dependency")

# ---- GENERAL agentic control tasks (out of domain; scaffold should not engage) ----
def g_HA2(tr, f):
    typed = sum(1 for c in tr if c["name"] == "get_account_type")
    return (1.0, "4000/ACC4") if typed >= 3 and re.search(r"(?<!\d)4000(?!\d)", f.replace(",", "")) and "acc4" in f.lower() else (0.0, "miss")
def g_HA3(tr, f):
    p = sum(1 for c in tr if c["name"] == "get_transactions")
    return (1.0, "450") if p >= 3 and re.search(r"(?<!\d)450(?!\d)", f) else (0.0, "miss")
def g_HA4(tr, f):
    bk = [c for c in tr if c["name"] == "book_flight_id"]
    return (1.0, "F2") if bk and str(bk[0]["args"].get("flight_id", "")).upper() == "F2" else (0.0, "miss")
GEN = [
 ("g_checking", [T_ACCTS, T_ATYPE, T_BAL], "Considering ONLY my checking accounts: combined balance, and which single checking account holds >half of that total?", g_HA2),
 ("g_food", [T_TX], "Across ALL pages of my transactions, total amount in the 'food' category?", g_HA3),
 ("g_flight", [T_FL, T_BK], "Book the cheapest flight NYC->LA under 8 hours and not on RedAir; give the confirmation code.", g_HA4),
]

def main():
    scaffold("warmup")
    res = {"model": LABEL, "indomain": [], "general": []}
    print(f"=== AGENTIC UPLIFT A/B: {LABEL} ===", flush=True)
    print("-- IN-DOMAIN (scaffold-relevant) --", flush=True)
    for q in IND:
        eng = bool(scaffold(ind_prompt(q)))
        tr_b, f_b = run_agentic(ind_prompt(q), IND_TOOLS, False)
        tr_s, f_s = run_agentic(ind_prompt(q), IND_TOOLS, True)
        sb, db = ind_grade(tr_b, f_b, q); ss, ds = ind_grade(tr_s, f_s, q)
        res["indomain"].append({"id": q["id"], "gold": q["gold"], "engaged": eng, "raw": sb, "scaf": ss, "raw_d": db, "scaf_d": ds})
        fl = "UP" if ss > sb else ("DN" if ss < sb else "=")
        print(f"  [{fl}] {q['id']:14s} eng={int(eng)} raw={sb:.1f} scaf={ss:.1f} | {q['gold'][:30]}", flush=True)
    print("-- GENERAL (out-of-domain control) --", flush=True)
    for tid, tools, prompt, grader in GEN:
        eng = bool(scaffold(prompt))
        tr_b, f_b = run_agentic(prompt, tools, False)
        tr_s, f_s = run_agentic(prompt, tools, True)
        sb, _ = grader(tr_b, f_b); ss, _ = grader(tr_s, f_s)
        res["general"].append({"id": tid, "engaged": eng, "raw": sb, "scaf": ss})
        fl = "UP" if ss > sb else ("DN" if ss < sb else "=")
        print(f"  [{fl}] {tid:14s} eng={int(eng)} raw={sb:.1f} scaf={ss:.1f}", flush=True)
    ind = res["indomain"]; gen = res["general"]
    def pct(rows, k): return round(sum(r[k] for r in rows) / len(rows) * 100, 1) if rows else 0
    res["summary"] = {"indomain_raw": pct(ind, "raw"), "indomain_scaf": pct(ind, "scaf"),
                      "indomain_engaged": sum(r["engaged"] for r in ind),
                      "general_raw": pct(gen, "raw"), "general_scaf": pct(gen, "scaf"),
                      "general_engaged": sum(r["engaged"] for r in gen)}
    json.dump(res, open(f"../logs/agentic-uplift-{LABEL.replace(' ','_')}.json", "w"), indent=1)
    s = res["summary"]
    print(f"\n  IN-DOMAIN  raw {s['indomain_raw']}% -> scaffold {s['indomain_scaf']}%  (uplift {s['indomain_scaf']-s['indomain_raw']:+.1f}) | engaged {s['indomain_engaged']}/{len(ind)}")
    print(f"  GENERAL    raw {s['general_raw']}% -> scaffold {s['general_scaf']}%  (delta {s['general_scaf']-s['general_raw']:+.1f}) | engaged {s['general_engaged']}/{len(gen)} (expect 0)")

if __name__ == "__main__":
    main()
