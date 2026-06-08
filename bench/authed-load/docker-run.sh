#!/usr/bin/env bash
# Build + run the authed mixed-workload benchmark entirely inside Docker.
# Both the server and k6 run co-located in one container (--network host).
#
# Usage (from repo root, inside WSL):
#   DATABASE_URL=postgresql://user:pass@127.0.0.1:5432/db ./bench/authed-load/docker-run.sh
#   DATABASE_URL=... VUS=100 DUR=120s ./bench/authed-load/docker-run.sh

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

VUS="${VUS:-50}"
DUR="${DUR:-60s}"
TAG="atomo-bench-load"
DB="${DATABASE_URL:?DATABASE_URL must be set}"
ADMIN_EMAIL="bench@load.dev"
ADMIN_PASSWORD="bench12345"

echo "=== Authed Load Bench (Docker, co-located) ==="
echo "  VUs=$VUS  Duration=$DUR"
echo ""

# 1. Build image
echo "→ Building Docker image (cached layers reused)..."
docker build -t "$TAG" -f bench/authed-load/Dockerfile . --quiet

# 2. Run: server + k6 in one container
echo "→ Starting server + k6..."
echo ""
docker run --rm --network host \
  -e DATABASE_URL="$DB" \
  -e ATOMO_SCHEMA_PATH=/bench/schema.ts \
  -e ADMIN_EMAIL="$ADMIN_EMAIL" \
  -e ADMIN_PASSWORD="$ADMIN_PASSWORD" \
  -e RUST_LOG=warn \
  "$TAG" bash -c "
    atomo-server -p 3099 &
    PID=\$!
    for i in \$(seq 1 30); do
      curl -sf http://127.0.0.1:3099/health >/dev/null 2>&1 && break
      sleep 0.5
    done
    echo '   Server ready'
    echo ''
    k6 run \
      -e BASE=http://127.0.0.1:3099 \
      -e VUS=$VUS \
      -e DUR=$DUR \
      -e EMAIL=$ADMIN_EMAIL \
      -e PASS=$ADMIN_PASSWORD \
      load.js
    kill \$PID 2>/dev/null || true
  "

echo ""
echo "=== Done ==="
