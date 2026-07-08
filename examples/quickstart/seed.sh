#!/usr/bin/env bash
# Seed the quickstart blog with sample data.
# Usage: ./seed.sh

set -euo pipefail
BASE="${BASE_URL:-http://localhost:3000}"

echo "Logging in..."
TOKEN=$(curl -sf "$BASE/auth/login" \
  -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"admin123"}' | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -z "$TOKEN" ]; then
  echo "Login failed — is the server running? (docker compose up)"
  exit 1
fi

GQL="$BASE/graphql"
AUTH="authorization: Bearer $TOKEN"

echo "Creating posts..."
for i in 1 2 3; do
  curl -sf "$GQL" -H "$AUTH" -H 'content-type: application/json' -d "$(cat <<JSON
{"query":"mutation { create(model: \"Post\", data: { title: \"Post $i\", content: \"This is sample post number $i.\", status: \"published\" }) }"}
JSON
)" > /dev/null
done

echo "Creating comments..."
# Get first post id
POST_ID=$(curl -sf "$GQL" -H "$AUTH" -H 'content-type: application/json' \
  -d '{"query":"{ records(model: \"Post\", limit: 1) }"}' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -n "$POST_ID" ]; then
  curl -sf "$GQL" -H "$AUTH" -H 'content-type: application/json' -d "$(cat <<JSON
{"query":"mutation { create(model: \"Comment\", data: { body: \"Great post!\", authorName: \"Alice\", postId: \"$POST_ID\" }) }"}
JSON
)" > /dev/null
  curl -sf "$GQL" -H "$AUTH" -H 'content-type: application/json' -d "$(cat <<JSON
{"query":"mutation { create(model: \"Comment\", data: { body: \"Thanks for sharing.\", authorName: \"Bob\", postId: \"$POST_ID\" }) }"}
JSON
)" > /dev/null
fi

echo "Done. Open http://localhost:3000/admin or query http://localhost:3000/graphql"
