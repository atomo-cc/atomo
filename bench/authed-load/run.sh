#!/usr/bin/env bash
# Authed mixed-workload benchmark: JWT + GraphQL CRUD + read cache + event sourcing.
#
# Prerequisites:
#   - Postgres running, DATABASE_URL set
#   - k6 installed (https://grafana.com/docs/k6/latest/set-up/install-k6/)
#   - cargo (builds the server in release mode)
#
# Usage:
#   DATABASE_URL=postgresql://... ./run.sh
#   DATABASE_URL=postgresql://... VUS=100 DUR=120s ./run.sh

set -euo pipefail
cd "$(dirname "$0")"

VUS="${VUS:-50}"
DUR="${DUR:-60s}"
PORT="${PORT:-3099}"
ADMIN_EMAIL="bench@load.dev"
ADMIN_PASSWORD="bench12345"

echo "=== Authed Load Bench ==="
echo "  VUs=$VUS  Duration=$DUR  Port=$PORT"
echo ""

# 1. Build server (release)
echo "→ Building atomo-server (release)..."
cargo build -p atomo_server --release --quiet 2>&1

# 2. Start server in background
echo "→ Starting server on :$PORT..."
ATOMO_SCHEMA_PATH="$(pwd)/schema.ts" \
  PORT=$PORT \
  ADMIN_EMAIL=$ADMIN_EMAIL \
  ADMIN_PASSWORD=$ADMIN_PASSWORD \
  RUST_LOG=warn \
  cargo run -p atomo_server --release --quiet -- -p $PORT &
SERVER_PID=$!

cleanup() { kill $SERVER_PID 2>/dev/null || true; }
trap cleanup EXIT

# Wait for server to be ready
for i in $(seq 1 30); do
  if curl -sf "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done
echo "   Server ready (pid $SERVER_PID)"

# 3. Seed data
echo "→ Seeding bench data..."
psql "$DATABASE_URL" -f setup.sql --quiet 2>/dev/null || true

# 4. Run k6
echo "→ Running k6 (${VUS} VUs, ${DUR})..."
echo ""
k6 run \
  -e BASE="http://127.0.0.1:$PORT" \
  -e VUS="$VUS" \
  -e DUR="$DUR" \
  -e EMAIL="$ADMIN_EMAIL" \
  -e PASS="$ADMIN_PASSWORD" \
  load.js

echo ""
echo "=== Done ==="
