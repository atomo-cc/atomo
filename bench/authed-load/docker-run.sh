#!/usr/bin/env bash
# Build + run the authed mixed-workload benchmark.
# Server runs in Docker (--network host); k6 runs on the host (WSL).
# Both are co-located — no network hop.
#
# Prerequisites: Docker, k6 installed in WSL
#   sudo apt install k6  (or see https://grafana.com/docs/k6/latest/set-up/install-k6/)
#
# Usage (from repo root, inside WSL):
#   DATABASE_URL=postgresql://user:pass@127.0.0.1:5432/db ./bench/authed-load/docker-run.sh
#   DATABASE_URL=... VUS=100 DUR=120s ./bench/authed-load/docker-run.sh

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

VUS="${VUS:-50}"
DUR="${DUR:-60s}"
PORT="${PORT:-3099}"
TAG="atomo-bench-load"
DB="${DATABASE_URL:?DATABASE_URL must be set}"
ADMIN_EMAIL="bench@load.dev"
ADMIN_PASSWORD="bench12345"

echo "=== Authed Load Bench (Docker server + host k6, co-located) ==="
echo "  VUs=$VUS  Duration=$DUR  Port=$PORT"
echo ""

# 1. Build server image
echo "→ Building Docker image..."
docker build -t "$TAG" -f bench/authed-load/Dockerfile . --quiet

# 2. Start server in Docker (background)
echo "→ Starting server..."
CID=$(docker run -d --rm --network host \
  -e DATABASE_URL="$DB" \
  -e ATOMO_SCHEMA_PATH=/bench/schema.ts \
  -e ADMIN_EMAIL="$ADMIN_EMAIL" \
  -e ADMIN_PASSWORD="$ADMIN_PASSWORD" \
  -e RATE_LIMIT_RPS=999999 \
  -e RUST_LOG=warn \
  "$TAG")
trap "docker stop $CID >/dev/null 2>&1 || true" EXIT

# Wait for ready
for i in $(seq 1 30); do
  curl -sf "http://127.0.0.1:$PORT/health" >/dev/null 2>&1 && break
  sleep 0.5
done
echo "   Server ready (container ${CID:0:12})"
echo ""

# 3. Run k6 from host
k6 run \
  -e BASE="http://127.0.0.1:$PORT" \
  -e VUS="$VUS" \
  -e DUR="$DUR" \
  -e EMAIL="$ADMIN_EMAIL" \
  -e PASS="$ADMIN_PASSWORD" \
  bench/authed-load/load.js

echo ""
echo "=== Done ==="
