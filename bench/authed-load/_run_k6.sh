#!/usr/bin/env bash
set -euo pipefail

SCENARIO="${1:-mixed}"
cd /mnt/c/Users/Chris/Projects/atomo

echo "=== Scenario: $SCENARIO ==="
/tmp/k6 run \
  -e BASE=http://127.0.0.1:3099 \
  -e VUS=50 \
  -e DUR=60s \
  -e EMAIL=bench@load.dev \
  -e PASS=bench12345 \
  -e SCENARIO="$SCENARIO" \
  -e SEED=200 \
  bench/authed-load/load.js
