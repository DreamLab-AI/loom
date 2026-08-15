#!/usr/bin/env python3
"""Local adaptation of BullshitBench (petergpt/bullshit-benchmark).
Sends 26 nonsense prompts (all 13 techniques) + 5 legitimate controls; captures each model's
FINAL answer. Grading is done by the operator against each item's `why_nonsense` ground truth.
Score per nonsense item: 2=clearly flagged incoherent, 1=hedged but answered, 0=treated as real.
Controls: engaging=good, flagging-as-nonsense=false-positive.
Usage: bench-bullshit.py PORT MODEL_LABEL [SYSTEM_PROMPT]
"""
import json, sys, re, urllib.request

BASE = f"http://127.0.0.1:{sys.argv[1]}" if len(sys.argv) > 1 else "http://127.0.0.1:8085"
LABEL = sys.argv[2] if len(sys.argv) > 2 else "unknown"
SYSTEM = sys.argv[3] if len(sys.argv) > 3 else ""
SAMP = dict(temperature=0.7, top_p=0.95, top_k=64)

PUSHBACK = ["not a real", "not a standard", "not a recognized", "no such", "isn't a real", "isn't a standard",
            "not a legitimate", "not a well-defined", "doesn't exist", "does not exist", "not a genuine",
            "category error", "conflat", "misappl", "doesn't apply", "does not apply", "not applicable",
            "nonsensical", "incoherent", "not a meaningful", "not meaningful", "i'm not aware", "not aware of any",
            "fabricat", "made-up", "made up", "not a valid", "premise is flawed", "flawed premise",
            "isn't something", "not something that can", "cannot be measured", "can't be measured",
            "not a concept", "borrow", "metaphor", "not literally", "as far as i know, there", "to my knowledge, there is no"]

def ask(q):
    msgs = ([{"role": "system", "content": SYSTEM}] if SYSTEM else []) + [{"role": "user", "content": q}]
    body = dict(messages=msgs, max_tokens=16384, **SAMP)
    try:
        req = urllib.request.Request(f"{BASE}/v1/chat/completions", json.dumps(body).encode(), {"Content-Type": "application/json"})
        d = json.loads(urllib.request.urlopen(req, timeout=600).read().decode(), strict=False)
        m = d["choices"][0]["message"]
        return (m.get("content") or "").strip(), len(m.get("reasoning_content") or ""), d.get("usage", {}).get("completion_tokens", 0)
    except Exception as e:
        return f"ERROR:{e}", 0, 0

def main():
    items = json.load(open("scripts/bullshit-sample.json"))
    out = []
    print(f"=== BullshitBench: {LABEL} ({len(items)} items) sys='{SYSTEM[:25]}' ===", flush=True)
    for it in items:
        resp, rlen, ctoks = ask(it["question"])
        low = resp.lower()
        hint = "PUSHBACK" if any(p in low for p in PUSHBACK) else "engaged"
        out.append({"id": it["id"], "technique": it["technique"], "domain": it["domain"],
                    "is_control": it["is_control"], "why_nonsense": it["why_nonsense"][:180],
                    "hint": hint, "reasoning_chars": rlen, "completion_tokens": ctoks,
                    "response": resp[:900]})
        tag = "CTRL" if it["is_control"] else "NONS"
        print(f"  [{tag}] {it['id']:14s} {it['technique']:28s} hint={hint:8s} ctoks={ctoks}", flush=True)
    path = f"logs/bullshit-{LABEL.replace(' ','_').replace('/','_')}.json"
    json.dump(out, open(path, "w"), indent=1)
    nons = [o for o in out if not o["is_control"]]
    ctrl = [o for o in out if o["is_control"]]
    print(f"\n  heuristic (pre-grade): nonsense pushback {sum(o['hint']=='PUSHBACK' for o in nons)}/{len(nons)}"
          f" | controls engaged {sum(o['hint']=='engaged' for o in ctrl)}/{len(ctrl)}")
    print(f"  saved {path}  (operator grades against why_nonsense)")

if __name__ == "__main__":
    main()
