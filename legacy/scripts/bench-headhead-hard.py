#!/usr/bin/env python3
"""HARD head-to-head — designed to separate two strong 30B models.
Usage: bench-headhead-hard.py PORT MODEL_LABEL [SYSTEM_PROMPT]
Agentic: deep dependency chains, pagination-recovery, constraint-optimization, ambiguity,
conditional logic. Reasoning: competition/adversarial. Coding: executed, edge-case tests.
Objective grading; transcript -> logs/hard-<label>.json. Sampling temp 0.7 (safe for Muse)."""
import json, sys, re, subprocess, urllib.request

BASE = f"http://127.0.0.1:{sys.argv[1]}" if len(sys.argv) > 1 else "http://127.0.0.1:8085"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "unknown"
SYSTEM = sys.argv[3] if len(sys.argv) > 3 else ""
SAMP = dict(temperature=0.7, top_p=0.95, top_k=64)

def chat(messages, tools=None, max_tokens=16384):
    body = dict(messages=messages, max_tokens=max_tokens, **SAMP)
    if tools: body["tools"] = tools; body["tool_choice"] = "auto"
    try:
        req = urllib.request.Request(f"{BASE}/v1/chat/completions",
              json.dumps(body).encode(), {"Content-Type": "application/json"})
        d = json.loads(urllib.request.urlopen(req, timeout=500).read().decode(), strict=False)
    except Exception as e:
        return {"error": str(e)[:200], "message": {}}, {}
    if "choices" not in d: return {"error": str(d)[:200], "message": {}}, {}
    return d["choices"][0], d.get("timings", {})

def td(name, desc, props, req):
    return {"type": "function", "function": {"name": name, "description": desc,
            "parameters": {"type": "object", "properties": props, "required": req}}}
TOOLS = {
 "get_user_id": td("get_user_id", "Look up a user's id by name", {"name": {"type": "string"}}, ["name"]),
 "get_orders": td("get_orders", "List order ids for a user id", {"user_id": {"type": "integer"}}, ["user_id"]),
 "get_order_details": td("get_order_details", "Details for an order id: year, amount, status", {"order_id": {"type": "integer"}}, ["order_id"]),
 "list_accounts": td("list_accounts", "List the user's account ids", {}, []),
 "get_account_type": td("get_account_type", "Type of an account: checking or savings", {"account": {"type": "string"}}, ["account"]),
 "get_balance": td("get_balance", "Get balance for an account id", {"account": {"type": "string"}}, ["account"]),
 "get_transactions": td("get_transactions", "Get a page of transactions (paginated; each item has amount and category)", {"page": {"type": "integer"}}, ["page"]),
 "search_flights": td("search_flights", "Search flights (returns id, airline, price, duration_h)", {"origin": {"type": "string"}, "dest": {"type": "string"}}, ["origin", "dest"]),
 "book_flight_id": td("book_flight_id", "Book a flight by its id; returns a confirmation code", {"flight_id": {"type": "string"}}, ["flight_id"]),
 "search_contacts": td("search_contacts", "Find contacts by name (returns id, name, company)", {"name": {"type": "string"}}, ["name"]),
 "get_meetings": td("get_meetings", "List a contact's meetings (id, when)", {"contact_id": {"type": "integer"}}, ["contact_id"]),
 "cancel_meeting": td("cancel_meeting", "Cancel a meeting by its meeting id", {"meeting_id": {"type": "integer"}}, ["meeting_id"]),
 "get_stock_price": td("get_stock_price", "Current stock price for a ticker", {"symbol": {"type": "string"}}, ["symbol"]),
 "get_account_balance": td("get_account_balance", "Get the user's main account balance", {}, []),
 "get_weather": td("get_weather", "Weather for a city", {"city": {"type": "string"}}, ["city"]),
}
def T(*names): return [TOOLS[n] for n in names]

def mock(task, name, args, idx):
    a = {k: str(v).lower() for k, v in args.items()}
    if name == "get_user_id": return json.dumps({"user_id": 42})
    if name == "get_orders": return json.dumps({"orders": [1001, 1002, 1003, 1004]})
    if name == "get_order_details":
        db = {1001: {"year": 2026, "month": 3, "amount": 30, "status": "delivered"},
              1002: {"year": 2025, "month": 11, "amount": 70, "status": "delivered"},   # wrong year
              1003: {"year": 2026, "month": 6, "amount": 50, "status": "cancelled"},    # wrong status
              1004: {"year": 2026, "month": 8, "amount": 40, "status": "delivered"}}    # keep / most recent delivered
        return json.dumps(db.get(int(args.get("order_id", 0)), {}))
    if name == "list_accounts": return json.dumps({"accounts": ["ACC1", "ACC2", "ACC3", "ACC4"]})
    if name == "get_account_type":
        return json.dumps({"type": {"acc1": "checking", "acc2": "savings", "acc3": "checking", "acc4": "checking"}.get(a.get("account"), "unknown")})
    if name == "get_balance":
        return json.dumps({"balance": {"acc1": 1500, "acc2": 9200, "acc3": 400, "acc4": 2100}.get(a.get("account"), 0)})
    if name == "get_transactions":
        pages = {1: {"items": [{"amount": 100, "category": "food"}, {"amount": 200, "category": "rent"}], "has_more": True, "next_page": 2},
                 2: {"items": [{"amount": 50, "category": "food"}, {"amount": 400, "category": "rent"}], "has_more": True, "next_page": 3},
                 3: {"items": [{"amount": 300, "category": "food"}], "has_more": False}}
        return json.dumps(pages.get(int(args.get("page", 1)), {"items": [], "has_more": False}))
    if name == "search_flights":
        return json.dumps({"flights": [{"id": "F1", "airline": "RedAir", "price": 380, "duration_h": 6},   # cheapest but RedAir
                                       {"id": "F2", "airline": "BlueJet", "price": 450, "duration_h": 7},   # correct
                                       {"id": "F3", "airline": "BlueJet", "price": 520, "duration_h": 5},
                                       {"id": "F4", "airline": "SkyWay", "price": 410, "duration_h": 9}]})   # cheaper but 9h
    if name == "book_flight_id": return json.dumps({"status": "booked", "confirmation": "XR7788", "id": args.get("flight_id")})
    if name == "search_contacts":
        return json.dumps({"contacts": [{"id": 1, "name": "John Smith", "company": "Acme Corp"},
                                        {"id": 2, "name": "John Doe", "company": "Globex"}]})
    if name == "get_meetings":
        cid = int(args.get("contact_id", 0))
        return json.dumps({"meetings": [{"id": 501, "when": "tomorrow"}, {"id": 502, "when": "next week"}] if cid == 1 else [{"id": 601, "when": "friday"}]})
    if name == "cancel_meeting": return json.dumps({"status": "cancelled", "meeting_id": args.get("meeting_id")})
    if name == "get_stock_price": return json.dumps({"symbol": args.get("symbol"), "price": 231.50})
    if name == "get_account_balance": return json.dumps({"balance": 5000})
    if name == "get_weather": return json.dumps({"city": args.get("city"), "temp_c": 22})
    return json.dumps({"ok": True})

def run_agentic(task, user, tools, max_steps=16):
    msgs = ([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": user}]
    trace, final = [], ""
    for _ in range(max_steps):
        ch, _ = chat(msgs, tools=tools)
        if "error" in ch: return trace, f"ERROR:{ch['error']}"
        m = ch["message"]; tcs = m.get("tool_calls") or []
        if not tcs: final = (m.get("content") or "").strip(); break
        msgs.append({"role": "assistant", "content": m.get("content"), "tool_calls": tcs})
        for tc in tcs:
            try: args = json.loads(tc["function"]["arguments"] or "{}")
            except Exception: args = {}
            n = tc["function"]["name"]
            idx = len([t for t in trace if t["name"] == n])
            trace.append({"name": n, "args": args})
            msgs.append({"role": "tool", "tool_call_id": tc.get("id", "0"), "content": mock(task, n, args, idx)})
    return trace, final

def calls(trace, name): return [t for t in trace if t["name"] == name]
def num_in(s, n): return re.search(rf"(?<!\d){n}(?!\d)", s.replace(",", "")) is not None

# ── HARD AGENTIC (deep: filters, full traversal, multi-constraint, disambiguation, branching) ──
def gr_HA1(tr, f):  # keep orders in 2026 AND delivered: 1001(30)+1004(40)=70
    details = len(calls(tr, "get_order_details"))
    if details >= 4 and num_in(f, 70): return (1.0, "filtered 2026+delivered -> 70")
    if calls(tr, "get_orders") and details >= 1: return (0.5, f"chain ok, wrong filter (f={f[:30]})")
    return (0.0, f"no chain tr={[t['name'] for t in tr][:6]}")
def gr_HA2(tr, f):  # checking only: total 1500+400+2100=4000; ACC4(2100)>half
    typed = len(calls(tr, "get_account_type")); lf = f.lower()
    if typed >= 3 and num_in(f, 4000) and "acc4" in lf: return (1.0, "checking total 4000, ACC4>half")
    if calls(tr, "get_balance") and (num_in(f, 4000) or "acc4" in lf): return (0.5, f"partial (f={f[:30]})")
    return (0.0, f"f={f[:40]}")
def gr_HA3(tr, f):  # food across all 3 pages: 100+50+300=450
    pages = len(calls(tr, "get_transactions"))
    if pages >= 3 and num_in(f, 450): return (1.0, "full pagination + food filter -> 450")
    if num_in(f, 450): return (0.7, f"got 450 but only {pages} pages")
    if pages >= 1: return (0.4, f"pages={pages} f={f[:30]}")
    return (0.0, "no pagination")
def gr_HA4(tr, f):  # cheapest <8h AND not RedAir = F2($450); F1=RedAir trap, F4=9h trap
    booked = calls(tr, "book_flight_id")
    bid = str(booked[0]["args"].get("flight_id", "")).upper() if booked else ""
    if bid == "F2" and "xr7788" in f.lower(): return (1.0, "booked F2 + confirmation")
    if bid == "F2": return (0.7, "booked F2 but no confirmation reported")
    return (0.0, f"booked {bid or 'none'} (F1=RedAir/F4=9h traps)")
def gr_HA5(tr, f):  # Acme John=id1; cancel his tomorrow meeting id 501
    cancels = calls(tr, "cancel_meeting")
    cid = int(cancels[0]["args"].get("meeting_id", 0)) if cancels else 0
    got = any(int(c["args"].get("contact_id", 0)) == 1 for c in calls(tr, "get_meetings"))
    if cid == 501: return (1.0, "cancelled Acme John's tomorrow mtg (501)")
    if got and not cancels: return (0.5, "found right John's meetings, didn't cancel")
    if cancels: return (0.2, f"cancelled wrong meeting {cid}")
    if calls(tr, "search_contacts"): return (0.3, "searched, didn't complete")
    return (0.0, "no disambiguation")
def gr_HA6(tr, f):  # AAPL>200 & bal 5000>=2000 -> floor(5000/231.5)=21
    checked = bool(calls(tr, "get_stock_price")) and bool(calls(tr, "get_account_balance"))
    if checked and num_in(f, 21): return (1.0, "both conditions + 21 shares")
    if num_in(f, 21): return (0.6, "21 but skipped a check")
    if calls(tr, "get_stock_price"): return (0.3, f"partial f={f[:30]}")
    return (0.0, f"f={f[:40]}")

# ── BRUTAL tier: failure-mode targeting ──
def gr_BR1(tr, f):  # most-recent DELIVERED = 1004 ($40) +8% tax = 43.20
    d = len(calls(tr, "get_order_details"))
    if d >= 3 and ("43.2" in f or num_in(f, 43)): return (1.0, "most-recent delivered +8% = 43.20")
    if calls(tr, "get_orders") and d >= 1: return (0.5, f"chain, wrong result (f={f[:30]})")
    return (0.0, "no chain")
def gr_BR2(tr, f):  # NO credit-score tool -> must decline, not fabricate
    lf = f.lower()
    fabricated = bool(re.search(r"\b([3-8]\d\d)\b", f)) and "score" in lf
    admits = any(w in lf for w in ["can't", "cannot", "unable", "no tool", "don't have", "do not have", "not available", "no access", "unavailable", "not able"])
    if fabricated: return (0.0, "HALLUCINATED a score")
    if admits: return (1.0, "declined (no such tool)")
    return (0.5, f"ambiguous f={f[:40]}")
def gr_BR3(tr, f):  # 22C=71.6F, NOT above 75F -> No
    lf = f.lower()
    yes = any(w in lf for w in ["yes,", "warmer than 75", "above 75", "hotter", "is warmer"])
    no = any(w in lf for w in ["no,", "not above", "not warmer", "below 75", "71.6", "cooler", "less than 75", "not hotter"])
    if bool(calls(tr, "get_weather")) and no and not yes: return (1.0, "C->F, correctly 'no'")
    if yes: return (0.0, "wrongly said above 75F")
    return (0.5, f"unclear f={f[:40]}")
def gr_BR5(tr, f):  # nothing under $300 -> must NOT book
    booked = calls(tr, "book_flight_id"); lf = f.lower()
    none_words = any(w in lf for w in ["no flight", "none", "not available", "no option", "under $300", "unavailable", "couldn't find", "cheapest is", "no flights", "nothing"])
    if not booked and none_words: return (1.0, "recognized impossibility")
    if booked: return (0.0, "booked despite over-budget")
    return (0.5, f"didn't book, unclear f={f[:40]}")

AG = [
 ("HA1", T("get_user_id", "get_orders", "get_order_details"), "What is the total dollar amount of Bob's orders that were placed in 2026 AND have status 'delivered'? Give the final number.", gr_HA1),
 ("HA2", T("list_accounts", "get_account_type", "get_balance"), "Considering ONLY my checking accounts: what is their combined balance, and which single checking account holds more than half of that checking total?", gr_HA2),
 ("HA3", T("get_transactions"), "Across ALL pages of my transactions, what is the total amount in the 'food' category?", gr_HA3),
 ("HA4", T("search_flights", "book_flight_id"), "Book the cheapest flight from NYC to LA that is under 8 hours AND not operated by RedAir, then tell me the confirmation code.", gr_HA4),
 ("HA5", T("search_contacts", "get_meetings", "cancel_meeting"), "Cancel the meeting happening tomorrow with John from Acme Corp.", gr_HA5),
 ("HA6", T("get_stock_price", "get_account_balance", "get_weather"), "If AAPL is above $200 and my account balance is at least $2000, buy the maximum whole number of AAPL shares I can afford with my balance and tell me how many; otherwise tell me the weather in Miami.", gr_HA6),
 ("BR1", T("get_user_id", "get_orders", "get_order_details"), "Find Bob's most RECENT delivered order and tell me its total including 8% sales tax.", gr_BR1),
 ("BR2", T("list_accounts", "get_balance"), "What is my current credit score?", gr_BR2),
 ("BR3", T("get_weather"), "Right now in Miami, is it warmer than 75 degrees Fahrenheit? Answer yes or no with the reason.", gr_BR3),
 ("BR5", T("search_flights", "book_flight_id"), "Book me any flight from NYC to LA for under $300.", gr_BR5),
]

# ── HARD REASONING ── (expected substring after normalizing)
RE = [
 ("HR1", "How many ordered pairs (a,b) of positive integers satisfy a+b=1000 where NEITHER a nor b contains the digit 0? Give just the number.", ["738"]),
 ("HR2", "It takes 5 machines 5 minutes to make 5 widgets. How many minutes would it take 100 machines to make 100 widgets? Just the number.", ["5"]),
 ("HR3", "Alice has 3 brothers and 2 sisters. How many sisters does one of Alice's brothers have? Just the number.", ["3"]),
 ("HR4", "Two fair six-sided dice are rolled. What is the probability that the sum is 7 OR both dice show the same value? Give a reduced fraction.", ["1/3"]),
 ("HR5", "How many trailing zeros are there in 100! (100 factorial)? Just the number.", ["24"]),
 ("HR6", "What is the 100th digit after the decimal point of 1/7? Just the digit.", ["8"]),
]
def score_re(exps, content):
    c = content.lower().replace(",", "")
    for e in exps:
        if e in ["738", "24"]:
            if num_in(c, e): return (1.0, "correct")
        elif e in c: return (1.0, "correct")
    return (0.0, f"want {exps[0]}, got '{content[:60]}'")

# ── HARD CODING ── (executed)
CD = [
 ("HC1", "Write `longest_palindrome(s)` returning a longest palindromic substring of s. Return only the function in a python code block.",
   "r=longest_palindrome('babad'); assert r==r[::-1] and len(r)==3 and r in 'babad'\nr=longest_palindrome('cbbd'); assert r=='bb'\nassert longest_palindrome('')==''\nassert longest_palindrome('a')=='a'"),
 ("HC2", "Write `min_coins(coins, amount)` returning the fewest coins summing to amount, or -1 if impossible (unbounded supply). Return only the function in a python code block.",
   "assert min_coins([1,2,5],11)==3\nassert min_coins([2],3)==-1\nassert min_coins([1],0)==0\nassert min_coins([1,5,10,25],63)==6"),
 ("HC3", "This binary search is buggy; fix it to return the index of target in the sorted list `a`, or -1:\n```python\ndef bsearch(a,t):\n    lo,hi=0,len(a)\n    while lo<hi:\n        m=(lo+hi)//2\n        if a[m]==t: return m\n        if a[m]<t: lo=m\n        else: hi=m\n    return -1\n```\nReturn only the corrected `bsearch` in a python code block.",
   "assert bsearch([1,3,5,7,9],7)==3\nassert bsearch([1,3,5,7,9],1)==0\nassert bsearch([1,3,5,7,9],9)==4\nassert bsearch([1,3,5,7,9],4)==-1\nassert bsearch([],1)==-1"),
 ("HC4", "Write `merge(intervals)` that merges overlapping intervals (list of [start,end]) and returns the merged list sorted by start. Return only the function in a python code block.",
   "assert merge([[1,3],[2,6],[8,10],[15,18]])==[[1,6],[8,10],[15,18]]\nassert merge([[1,4],[4,5]])==[[1,5]]\nassert merge([])==[]\nassert merge([[1,4],[2,3]])==[[1,4]]"),
 ("HC5", "Write `evaluate(expr)` that evaluates an arithmetic string with + - * / (integer operands, standard precedence, no parentheses) WITHOUT using eval(). Integer division truncates toward zero. Return only the function in a python code block.",
   "assert evaluate('3+4*2')==11\nassert evaluate('10-2*3')==4\nassert evaluate('2*3+4*5')==26\nassert evaluate('100/3')==33\nassert evaluate('7')==7"),
]
def run_code(content, tests):
    m = re.search(r"```(?:python)?\s*(.*?)```", content, re.S) or re.search(r"(def .+)", content, re.S)
    code = m.group(1) if m else content
    try:
        p = subprocess.run([sys.executable, "-c", code + "\n\n" + tests], capture_output=True, text=True, timeout=15)
        return (p.returncode == 0), (p.stderr.strip().splitlines()[-1][:100] if p.returncode else "ok")
    except Exception as e:
        return False, str(e)[:100]

def main():
    out = {"model": LABEL, "cats": {}, "tasks": []}
    def add(tid, cat, s, detail, extra):
        out["cats"].setdefault(cat, [0, 0]); out["cats"][cat][0] += s; out["cats"][cat][1] += 1
        out["tasks"].append(dict(id=tid, cat=cat, score=s, detail=detail, **extra))
        st = "PASS" if s >= 1 else ("PART" if s > 0 else "FAIL")
        print(f"  [{st}] {tid} {cat:9s} {s:.1f} | {detail[:70]}", flush=True)
    print(f"\n=== HARD HEAD-TO-HEAD: {LABEL} ({BASE}) sys='{SYSTEM[:25]}' ===", flush=True)
    for tid, tools, user, grader in AG:
        tr, final = run_agentic(tid, user, tools)
        s, d = grader(tr, final); add(tid, "Agentic", s, d, {"trace": [t["name"] for t in tr], "final": final[:180]})
    for tid, prompt, exps in RE:
        ch, _ = chat(([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": prompt}])
        content = "" if "error" in ch else (ch["message"].get("content") or "").strip()
        s, d = score_re(exps, content); add(tid, "Reasoning", s, d, {"final": content[:150]})
    for tid, prompt, tests in CD:
        ch, _ = chat(([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": prompt}])
        content = "" if "error" in ch else (ch["message"].get("content") or "").strip()
        ok, why = run_code(content, tests); add(tid, "Coding", 1.0 if ok else 0.0, "tests pass" if ok else f"FAIL: {why}", {"final": content[:250]})
    tot = sum(t["score"] for t in out["tasks"]); n = len(out["tasks"])
    print(f"\n  {'CATEGORY':10s} score")
    for cat, (sc, ct) in out["cats"].items(): print(f"  {cat:10s} {sc:.1f}/{ct}  ({sc/ct*100:.0f}%)")
    out["overall_pct"] = round(tot / n * 100, 1)
    print(f"\n  OVERALL: {tot:.1f}/{n} = {out['overall_pct']}%")
    json.dump(out, open(f"logs/hard-{LABEL.replace(' ','_').replace('/','_')}.json", "w"), indent=1)
    print(f"JSON_SUMMARY: {json.dumps({'model':LABEL,'overall':out['overall_pct'],'cats':{k:round(v[0]/v[1]*100) for k,v in out['cats'].items()}})}")

if __name__ == "__main__":
    main()
