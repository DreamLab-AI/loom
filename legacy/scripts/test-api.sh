#!/usr/bin/env bash
set -euo pipefail

HOST="${1:-localhost}"
PORT="${2:-8080}"
BASE="http://${HOST}:${PORT}"

echo "── Testing llama.cpp server at ${BASE} ──"
echo

# 1. Health check
echo "1. Health check..."
if curl -sf "${BASE}/health" | python3 -m json.tool; then
    echo "   OK"
else
    echo "   FAILED - server not responding"
    exit 1
fi
echo

# 2. List models
echo "2. Models endpoint..."
curl -sf "${BASE}/v1/models" | python3 -m json.tool
echo

# 3. Simple completion
echo "3. Chat completion (simple)..."
time curl -sf "${BASE}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{
        "messages": [
            {"role": "user", "content": "What is 2+2? Answer in one word."}
        ],
        "max_tokens": 32
    }' | python3 -m json.tool
echo

# 4. Reasoning / thinking test
echo "4. Reasoning test (should show thinking)..."
time curl -sf "${BASE}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{
        "messages": [
            {"role": "user", "content": "How many r'\''s are in the word strawberry? Think step by step."}
        ],
        "max_tokens": 512
    }' | python3 -m json.tool
echo

# 5. Tool calling test
echo "5. Tool calling test..."
time curl -sf "${BASE}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{
        "messages": [
            {"role": "user", "content": "What is the weather in London right now?"}
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get the current weather for a location",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "City name"
                            }
                        },
                        "required": ["location"]
                    }
                }
            }
        ],
        "max_tokens": 256
    }' | python3 -m json.tool
echo

# 6. Streaming test
echo "6. Streaming test..."
curl -sf "${BASE}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d '{
        "messages": [
            {"role": "user", "content": "Count from 1 to 5, one number per line."}
        ],
        "max_tokens": 64,
        "stream": true
    }' | while IFS= read -r line; do
        if [[ "$line" == data:* ]] && [[ "$line" != "data: [DONE]" ]]; then
            content=$(echo "${line#data: }" | python3 -c "import sys,json; d=json.load(sys.stdin); c=d.get('choices',[{}])[0].get('delta',{}).get('content',''); print(c, end='')" 2>/dev/null)
            printf "%s" "$content"
        fi
    done
echo
echo
echo "   Streaming OK"
echo

# 7. Metrics
echo "7. Server metrics..."
curl -sf "${BASE}/metrics" | head -20
echo "..."
echo

echo "── All tests passed ──"
