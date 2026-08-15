#!/usr/bin/env bash
set -euo pipefail
# Reusable single-stream throughput probe for llama-server (OpenAI API).
# Usage: bench-throughput.sh PORT MODEL_ID [LABEL] [GEN_TOKENS]
# Measures: prefill tok/s (long ~2.4k-token prompt), sustained decode tok/s (greedy),
#           TTFT, and (if spec-decode on) draft acceptance from the server journal.
# Same probe is run on Gemma (:8084) and Muse Glimmer (:8085) for an apples-to-apples compare.

PORT="${1:?port}"; MODEL="${2:?model id}"; LABEL="${3:-$MODEL}"; GEN="${4:-300}"
URL="http://127.0.0.1:${PORT}/v1/chat/completions"

# ~2.4k-token deterministic prompt (fixed, so prefill is comparable across models).
PROMPT="$(python3 -c "print('You are analyzing a large distributed database system. ' + ('Consensus protocols such as Raft and Paxos keep replicas in agreement despite node and network failures; leaders replicate a log, followers acknowledge, and entries commit once a quorum persists them. ' * 90) + ' Summarize the key failure-recovery guarantees.')")"

req() { # $1=maxtok $2=prompt
  python3 - "$MODEL" "$1" "$2" <<'PY'
import json,sys
model,mx,prompt=sys.argv[1],int(sys.argv[2]),sys.argv[3]
print(json.dumps({"model":model,"messages":[{"role":"user","content":prompt}],
  "max_tokens":mx,"temperature":0,"stream":False}))
PY
}

echo "── baseline: $LABEL (port $PORT) ──"
# warm-up (load slot / caches)
curl -s "$URL" -H 'Content-Type: application/json' -d "$(req 8 hi)" >/dev/null 2>&1 || true

resp="$(curl -s "$URL" -H 'Content-Type: application/json' -d "$(req "$GEN" "$PROMPT")" 2>/dev/null)"
echo "$resp" | python3 -c "
import json,sys
d=json.load(sys.stdin)
t=d.get('timings',{}) or {}
u=d.get('usage',{}) or {}
if t:
    print(f\"  prefill : {t.get('prompt_n','?')} tok @ {t.get('prompt_per_second',0):.1f} tok/s\")
    print(f\"  decode  : {t.get('predicted_n','?')} tok @ {t.get('predicted_per_second',0):.1f} tok/s\")
    print(f\"  TTFT    : {t.get('prompt_ms',0):.0f} ms\")
    dn=t.get('draft_n'); da=t.get('draft_n_accepted')
    if dn: print(f\"  spec    : draft {da}/{dn} accepted = {100*da/dn:.1f}%\")
else:
    print('  (no timings) usage=',u)
"