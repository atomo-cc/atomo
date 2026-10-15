#!/bin/bash
# Run inside the isolated Rust container only. Do not print environment values.
set -u
cd /work
result=0
: > tests/m1/final-results.jsonl
run() {
    local name="$1"
    shift
    "$@" > "tests/m1/$name.log" 2>&1
    local status=$?
    printf '{"check":"%s","exitCode":%s}\n' "$name" "$status" >> tests/m1/final-results.jsonl
    printf '%s: exit %s\n' "$name" "$status"
    if [ "$name" != clippy-strict-final ] && [ "$status" -ne 0 ]; then result=1; fi
}
run unit-final cargo test --workspace
run postgres-acceptance cargo test --workspace -- --ignored --test-threads=1
run build-final cargo build -p atomo_server -p atomo_cli
run fmt-final cargo fmt --all -- --check
run clippy-strict-final cargo clippy --workspace --all-targets -- -D warnings
run clippy-review cargo clippy --workspace --all-targets -- -D warnings -A clippy::double_must_use
exit "$result"
