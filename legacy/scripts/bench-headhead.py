#!/usr/bin/env python3
"""Head-to-head AGENTIC + reasoning + coding benchmark.
Usage: bench-headhead.py PORT MODEL_LABEL [SYSTEM_PROMPT]
 - Agentic tasks run a real multi-turn tool loop against a mock tool executor.
 - Coding tasks EXECUTE the generated code against asserts.
 - Objective auto-grading; full transcript saved to logs/headhead-<label>.json for review.
Sampling: temp 0.7 / top_p 0.95 / top_k 64 (safe for Muse; greedy loops). Reasoning left ON.
"""
import json, sys, time, re, subprocess, urllib.request

BASE = f"http://127.0.0.1:{sys.argv[1]}" if len(sys.argv) > 1 else "http://127.0.0.1:8085"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "unknown"
SYSTEM = sys.argv[3] if len(sys.argv) > 3 else ""
SAMP = dict(temperature=0.7, top_p=0.95, top_k=64)

def chat(messages, tools=None, max_tokens=3072):
    body = dict(messages=messages, max_tokens=max_tokens, **SAMP)
    if tools: body["tools"] = tools; body["tool_choice"] = "auto"
    try:
        req = urllib.request.Request(f"{BASE}/v1/chat/completions",
              json.dumps(body).encode(), {"Content-Type": "application/json"})
        d = json.loads(urllib.request.urlopen(req, timeout=400).read().decode(), strict=False)
    except Exception as e:
        return {"error": str(e)[:200], "message": {}}, {}
    if "choices" not in d: return {"error": str(d)[:200], "message": {}}, {}
    return d["choices"][0], d.get("timings", {})

# ── mock tools ──────────────────────────────────────────────────────────────
def tool_def(name, desc, props, req):
    return {"type": "function", "function": {"name": name, "description": desc,
            "parameters": {"type": "object", "properties": props, "required": req}}}
T_WEATHER = tool_def("get_weather", "Current weather for a city",
    {"city": {"type": "string", "description": "City name"}}, ["city"])
T_STOCK = tool_def("get_stock_price", "Current stock price for a ticker symbol",
    {"symbol": {"type": "string"}}, ["symbol"])
T_EMAIL = tool_def("send_email", "Send an email",
    {"to": {"type": "string"}, "subject": {"type": "string"}, "body": {"type": "string"}}, ["to", "body"])
T_REMIND = tool_def("create_reminder", "Create a reminder after N minutes",
    {"text": {"type": "string"}, "minutes": {"type": "integer"}}, ["text", "minutes"])
T_FLIGHT = tool_def("book_flight", "Book a flight",
    {"origin": {"type": "string"}, "dest": {"type": "string"}, "date": {"type": "string", "description": "YYYY-MM-DD"}}, ["origin", "dest", "date"])

def mock_exec(task_id, name, args, call_index):
    """Return a canned tool result string; some tasks inject an error to test recovery."""
    a = {k: str(v).lower() for k, v in args.items()}
    if name == "get_weather":
        city = a.get("city", "")
        if task_id == "AG6" and call_index == 0:
            return json.dumps({"error": "unknown city; use 'City, Country' format"})
        if "dubai" in city: return json.dumps({"city": city, "temp_c": 35, "cond": "sunny"})
        if "london" in city: return json.dumps({"city": city, "temp_c": 12, "cond": "rain"})
        return json.dumps({"city": args.get("city"), "temp_c": 22, "cond": "clear"})
    if name == "get_stock_price": return json.dumps({"symbol": args.get("symbol"), "price": 231.50})
    if name == "send_email": return json.dumps({"status": "sent", "to": args.get("to")})
    if name == "create_reminder": return json.dumps({"status": "scheduled", "minutes": args.get("minutes")})
    if name == "book_flight": return json.dumps({"status": "booked", "pnr": "XR7788"})
    return json.dumps({"ok": True})

def run_agentic(task_id, user, tools, max_steps=5):
    msgs = ([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": user}]
    trace, final = [], ""
    for step in range(max_steps):
        ch, _ = chat(msgs, tools=tools)
        if "error" in ch: return trace, f"ERROR:{ch['error']}"
        m = ch["message"]
        tcs = m.get("tool_calls") or []
        if not tcs:
            final = (m.get("content") or "").strip(); break
        msgs.append({"role": "assistant", "content": m.get("content"), "tool_calls": tcs})
        for tc in tcs:
            try: args = json.loads(tc["function"]["arguments"] or "{}")
            except Exception: args = {"_raw": tc["function"].get("arguments")}
            trace.append({"name": tc["function"]["name"], "args": args})
            res = mock_exec(task_id, tc["function"]["name"], args, len([t for t in trace if t["name"] == tc["function"]["name"]]) - 1)
            msgs.append({"role": "tool", "tool_call_id": tc.get("id", "0"), "content": res})
    return trace, final

# ── code execution ───────────────────────────────────────────────────────────
def run_code(content, tests):
    m = re.search(r"```(?:python)?\s*(.*?)```", content, re.S) or re.search(r"(def .+)", content, re.S)
    code = (m.group(1) if m else content)
    src = code + "\n\n" + tests
    try:
        p = subprocess.run([sys.executable, "-c", src], capture_output=True, text=True, timeout=15)
        return (p.returncode == 0), (p.stderr.strip()[-120:] if p.returncode else "ok")
    except Exception as e:
        return False, str(e)[:120]

# ── tasks ────────────────────────────────────────────────────────────────────
def g_tool_called(trace, name, argkey=None, argval=None):
    for t in trace:
        if t["name"] == name:
            if argkey is None: return True
            v = str(t["args"].get(argkey, "")).lower()
            return argval.lower() in v
    return False

AG = [
 ("AG1", "Agentic", "What's the weather in Tokyo right now?", [T_WEATHER],
   lambda tr, f: (1.0, "get_weather(Tokyo)") if g_tool_called(tr, "get_weather", "city", "tokyo") else (0.0, f"tools={[t['name'] for t in tr]}")),
 ("AG2", "Agentic", "What is Apple's stock trading at?", [T_WEATHER, T_STOCK, T_REMIND],
   lambda tr, f: (1.0, "get_stock_price") if g_tool_called(tr, "get_stock_price") and not g_tool_called(tr, "get_weather") else (0.0, f"tools={[t['name'] for t in tr]}")),
 ("AG3", "Agentic", "What is 15% of 240? Answer directly.", [T_WEATHER, T_STOCK],
   lambda tr, f: (1.0, "no tool + 36") if (not tr and "36" in f) else (0.5, "answered but called tool") if "36" in f else (0.0, f"tools={[t['name'] for t in tr]} f={f[:40]}")),
 ("AG4", "Agentic", "Remind me to call mom in half an hour.", [T_REMIND],
   lambda tr, f: (1.0, "minutes=30") if g_tool_called(tr, "create_reminder", "minutes", "30") else (0.0, f"tr={tr}")),
 ("AG5", "Agentic", "Check the weather in Paris and email a summary of it to alice@example.com.", [T_WEATHER, T_EMAIL],
   lambda tr, f: (1.0, "weather->email chain") if (g_tool_called(tr, "get_weather", "city", "paris") and g_tool_called(tr, "send_email", "to", "alice@example.com")) else (0.5, "partial chain") if (g_tool_called(tr, "get_weather") or g_tool_called(tr, "send_email")) else (0.0, f"tr={[t['name'] for t in tr]}")),
 ("AG6", "Agentic", "Get the weather in Tokyo.", [T_WEATHER],
   lambda tr, f: (1.0, "recovered after error") if len([t for t in tr if t["name"] == "get_weather"]) >= 2 else (0.5, "handled gracefully") if ("format" in f.lower() or "country" in f.lower() or "japan" in f.lower()) else (0.0, f"no recovery tr={len(tr)} f={f[:40]}")),
 ("AG7", "Agentic", "Which is warmer right now, London or Dubai?", [T_WEATHER],
   lambda tr, f: (1.0, "queried both + Dubai") if (g_tool_called(tr, "get_weather", "city", "london") and g_tool_called(tr, "get_weather", "city", "dubai") and "dubai" in f.lower()) else (0.5, "partial") if "dubai" in f.lower() else (0.0, f"tr={[t['args'].get('city') for t in tr]} f={f[:40]}")),
 ("AG8", "Agentic", "Book me a flight from JFK to LAX on 2026-09-15.", [T_FLIGHT],
   lambda tr, f: (1.0, "JFK->LAX") if g_tool_called(tr, "book_flight", "origin", "jfk") and g_tool_called(tr, "book_flight", "dest", "lax") else (0.0, f"tr={tr}")),
]

RE = [
 ("RE1", "Reasoning", "How many distinct arrangements of the letters in 'MISSISSIPPI' are there? Give just the number.", "34650"),
 ("RE2", "Reasoning", "You have 12 identical-looking coins; one is counterfeit and differs in weight. Using a balance scale, what is the minimum number of weighings that guarantees identifying the fake AND whether it's heavier or lighter? Just the number.", "3"),
 ("RE3", "Reasoning", "Reversing the digits of a two-digit number and adding it to the original gives 121. The tens digit is 3 more than the units digit. What is the original number? Just the number.", "74"),
 ("RE4", "Reasoning", "Three light switches are outside a windowless room; each controls one of three bulbs inside. You may flip switches freely but may enter the room only once. Briefly, how do you determine which switch controls which bulb?", "__HEAT__"),
 ("RE5", "Reasoning", "A snail climbs a 30-foot well: each day it climbs 3 feet, each night it slips back 2 feet. On which day does it first reach the top? Just the number.", "28"),
 ("RE6", "Reasoning", "In a race you overtake the person in 2nd place. What position are you now in? One word.", "second"),
]

CD = [
 ("CD1", "Coding", "Write a Python function `flatten(lst)` that recursively flattens an arbitrarily nested list of integers into a flat list. Return only the function in a code block.",
   "assert flatten([1,[2,[3,4],5],6])==[1,2,3,4,5,6]\nassert flatten([])==[]\nassert flatten([[[1]]])==[1]"),
 ("CD2", "Coding", "Write a Python function `nth_prime(n)` returning the nth prime (1-indexed; nth_prime(1)==2). Return only the function in a code block.",
   "assert nth_prime(1)==2\nassert nth_prime(6)==13\nassert nth_prime(10)==29"),
 ("CD3", "Coding", "This function is buggy; fix it so it correctly reports whether a string is a palindrome ignoring case and non-alphanumeric chars:\n```python\ndef is_pal(s):\n    return s==s[::-1]\n```\nReturn only the corrected function `is_pal` in a code block.",
   "assert is_pal('A man, a plan, a canal: Panama')==True\nassert is_pal('race a car')==False\nassert is_pal('')==True"),
 ("CD4", "Coding", "Write a Python function `two_sum(nums, target)` returning indices of the two numbers that add to target (assume exactly one solution). Return only the function in a code block.",
   "r=two_sum([2,7,11,15],9); assert sorted(r)==[0,1]\nr=two_sum([3,2,4],6); assert sorted(r)==[1,2]"),
]

def score_re(expected, content):
    c = content.lower()
    if expected == "__HEAT__":
        return (1.0, "heat trick") if ("heat" in c or "warm" in c or "hot" in c) else (0.0, "no heat insight")
    return (1.0, "correct") if expected.lower() in c else (0.0, f"want {expected}, got '{content[:50]}'")

# ── run ───────────────────────────────────────────────────────────────────────
def main():
    out = {"model": LABEL, "system": SYSTEM, "cats": {}, "tasks": []}
    def add(tid, cat, s, detail, extra):
        out["cats"].setdefault(cat, [0, 0]); out["cats"][cat][0] += s; out["cats"][cat][1] += 1
        out["tasks"].append(dict(id=tid, cat=cat, score=s, detail=detail, **extra))
        st = "PASS" if s >= 1 else ("PART" if s > 0 else "FAIL")
        print(f"  [{st}] {tid} {cat:9s} {s:.1f} | {detail[:70]}", flush=True)

    print(f"\n=== HEAD-TO-HEAD: {LABEL} ({BASE}) sys='{SYSTEM[:30]}' ===", flush=True)
    for tid, cat, user, tools, grader in AG:
        tr, final = run_agentic(tid, user, tools)
        s, d = grader(tr, final)
        add(tid, cat, s, d, {"trace": tr, "final": final[:200]})
    for tid, cat, prompt, expected in RE:
        ch, _ = chat(([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": prompt}])
        content = "" if "error" in ch else (ch["message"].get("content") or "").strip()
        s, d = score_re(expected, content)
        add(tid, cat, s, d, {"final": content[:200]})
    for tid, cat, prompt, tests in CD:
        ch, _ = chat(([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": prompt}])
        content = "" if "error" in ch else (ch["message"].get("content") or "").strip()
        ok, why = run_code(content, tests)
        add(tid, cat, 1.0 if ok else 0.0, ("tests pass" if ok else f"FAIL: {why}"), {"final": content[:300]})

    tot = sum(t["score"] for t in out["tasks"]); n = len(out["tasks"])
    print(f"\n  {'CATEGORY':10s} score")
    for cat, (sc, ct) in out["cats"].items():
        print(f"  {cat:10s} {sc:.1f}/{ct}  ({sc/ct*100:.0f}%)")
    out["overall_pct"] = round(tot / n * 100, 1)
    print(f"\n  OVERALL: {tot:.1f}/{n} = {out['overall_pct']}%")
    path = f"logs/headhead-{LABEL.replace(' ','_').replace('/','_')}.json"
    json.dump(out, open(path, "w"), indent=1)
    print(f"  saved {path}")
    print(f"JSON_SUMMARY: {json.dumps({'model':LABEL,'overall':out['overall_pct'],'cats':{k:round(v[0]/v[1]*100) for k,v in out['cats'].items()}})}")

if __name__ == "__main__":
    main()
