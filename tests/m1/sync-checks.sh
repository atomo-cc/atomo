#!/bin/bash
# Execute only inside the task's WSL Docker tool container.
set -u
cd /work
run() {
    local name="$1"
    shift
    "$@" > "tests/m1/sync-$name.log" 2>&1
    local status=$?
    printf '{"check":"%s","exitCode":%s}\n' "$name" "$status" >> tests/m1/sync-results.jsonl
    printf '%s: exit %s\n' "$name" "$status"
    return "$status"
}
if [ "${1:-}" = targeted ]; then
    run parser cargo test -p atomo_schema || exit $?
    run rls cargo test -p atomo_server --test rls_enforcement -- --ignored --test-threads=1 || exit $?
    run security cargo test -p atomo_server --test m1_security -- --ignored --test-threads=1 || exit $?
else
    run unit cargo test --workspace
    run postgres cargo test --workspace -- --ignored --test-threads=1
    run build cargo build -p atomo_server -p atomo_cli
    run fmt cargo fmt --all -- --check
    run clippy-strict cargo clippy --workspace --all-targets -- -D warnings
fi
