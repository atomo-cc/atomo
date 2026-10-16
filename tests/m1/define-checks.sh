#!/bin/bash
# Run inside the isolated WSL Docker tool container. Never print environment values.
set -u
cd /work
result=0
run() {
    local name="$1"
    shift
    "$@" > "tests/m1/define-$name.log" 2>&1
    local status=$?
    printf '{"check":"%s","exitCode":%s}\n' "$name" "$status" >> tests/m1/define-results.jsonl
    printf '%s: exit %s\n' "$name" "$status"
    if [ "$status" -ne 0 ]; then result=1; fi
    return "$status"
}
if [ "${1:-}" = targeted ]; then
    run runtime cargo test -p atomo --test define_model || exit "$result"
    run schema cargo test -p atomo_schema || exit "$result"
    run http cargo test -p atomo_server --test m1_security -- --ignored --test-threads=1 || exit "$result"
else
    run unit cargo test --workspace
    run postgres cargo test --workspace -- --ignored --test-threads=1
    run build cargo build -p atomo_server -p atomo_cli
    run fmt cargo fmt --all -- --check
    run clippy-strict cargo clippy --workspace --all-targets -- -D warnings
fi
exit "$result"
