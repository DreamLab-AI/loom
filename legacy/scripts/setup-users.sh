#!/bin/bash
set -e

ADMIN_TOKEN=$(curl -s http://localhost:3000/api/v1/auths/signin -H "Content-Type: application/json" -d '{"email":"openwebui@xrsystems.uk","password":"Entering5-Pound3-Immersion4-Lustily1"}' | python3 -c "import json,sys; print(json.load(sys.stdin)['token'])")
AUTH="Authorization: Bearer $ADMIN_TOKEN"

RILEY_ID="a1316bd9-bbf8-4116-a264-624b6ccd83da"
FINN_ID="acb93f47-be8c-4178-8192-9dd2b41592aa"

echo "Resetting Riley..."
curl -s "http://localhost:3000/api/v1/users/$RILEY_ID/update" -X POST -H "$AUTH" -H "Content-Type: application/json" \
  -d '{"name":"Riley","email":"riley@teamofohares.com","role":"user","profile_image_url":"/static/favicon.png","password":"changeme123"}'
echo ""

echo "Resetting Finn..."
curl -s "http://localhost:3000/api/v1/users/$FINN_ID/update" -X POST -H "$AUTH" -H "Content-Type: application/json" \
  -d '{"name":"Finn","email":"finn@teamofohares.com","role":"user","profile_image_url":"/static/favicon.png","password":"changeme123"}'
echo ""

echo "Verifying Riley login..."
RILEY_TOKEN=$(curl -s http://localhost:3000/api/v1/auths/signin -H "Content-Type: application/json" \
  -d '{"email":"riley@teamofohares.com","password":"changeme123"}' | python3 -c "import json,sys; print(json.load(sys.stdin).get('token','FAIL'))")

if [ "$RILEY_TOKEN" != "FAIL" ]; then
    echo "Riley: OK"
    curl -s http://localhost:3000/api/v1/models/list -H "Authorization: Bearer $RILEY_TOKEN" | python3 -c "
import json,sys; d=json.load(sys.stdin)
for m in d.get('items',[]): print(f'  Model: {m.get(\"name\")}')
print(f'  Total: {d.get(\"total\",0)}')
"
else
    echo "Riley: FAILED"
fi
