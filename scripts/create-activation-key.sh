#!/usr/bin/env bash
# Create an activation code via the cloud API. The code is printed once — give it to staff to enter
# on the till (Settings → Cloud).
#
# Usage:
#   ./scripts/create-activation-key.sh [BASE_URL]
#
# Examples:
#   ./scripts/create-activation-key.sh                    # uses http://localhost:8080/api
#   ./scripts/create-activation-key.sh https://cloud.traqr.co.uk/api
#
# If the server uses ADMIN_API_KEY, set it here so the script can call the API:
#   export ADMIN_KEY=your-secret
#   ./scripts/create-activation-key.sh

set -e
BASE_URL="${1:-http://localhost:8080/api}"

# Create org and store by name, get a store-scoped activation key (single use, no expiry).
BODY='{
  "org_name": "Demo Org",
  "org_slug": "demo-org",
  "store_name": "Store 1",
  "scope_type": "store",
  "max_uses": 1
}'

HEADERS=(-H "Content-Type: application/json")
if [ -n "${ADMIN_KEY:-}" ]; then
  HEADERS+=(-H "X-Admin-Key: $ADMIN_KEY")
fi

echo "POST $BASE_URL/admin/activation-keys"
RESP=$(curl -s -w "\n%{http_code}" "${HEADERS[@]}" -d "$BODY" "$BASE_URL/admin/activation-keys")
HTTP_CODE=$(echo "$RESP" | tail -n1)
BODY_RESP=$(echo "$RESP" | sed '$d')

if [ "$HTTP_CODE" != "200" ]; then
  echo "Error ($HTTP_CODE): $BODY_RESP"
  exit 1
fi

KEY=$(echo "$BODY_RESP" | jq -r '.activation_key')
echo ""
echo "Activation code — give this to staff to enter on the till (Settings → Cloud). You only see it once:"
echo ""
echo "$KEY"
echo ""
