# M1 isolated verification

Run from the task worktree only. Rust commands below execute in WSL Docker.
Never point these tests at an existing application database: the serial suite
creates and drops its own fixture tables, including `users`.

The ignored `.env` contains a locally generated random `M1_PASSWORD` and a fresh
`M1_DATABASE` name. Keep it private. The fixed Compose project is unique to this
task; it publishes no host ports. PostgreSQL, cargo registry and target volumes
belong exclusively to this project. Do not use `down -v`.

```powershell
wsl --exec docker compose -f tests/m1/compose.yaml build rust
wsl --exec docker compose -f tests/m1/compose.yaml up -d db
wsl --exec sh -lc 'set -a; . tests/m1/.env; set +a; docker compose -f tests/m1/compose.yaml exec -T -e M1_PASSWORD -e M1_DATABASE db sh < tests/m1/init-db.sh'
wsl --exec docker compose -f tests/m1/compose.yaml run --rm rust cargo test --workspace
wsl --exec docker compose -f tests/m1/compose.yaml run --rm rust cargo test --workspace -- --ignored --test-threads=1
wsl --exec docker compose -f tests/m1/compose.yaml run --rm rust cargo build -p atomo_server -p atomo_cli
wsl --exec docker compose -f tests/m1/compose.yaml run --rm rust cargo fmt --all -- --check
wsl --exec docker compose -f tests/m1/compose.yaml run --rm rust cargo clippy --workspace --all-targets -- -D warnings
```

Create a fresh database name in `.env` before a complete PostgreSQL rerun; the
initializer creates it without deleting previous databases. It also rotates the
two local database-role passwords without printing them. Keep shell scripts LF.

The strict lint result and the explicitly qualified diagnostic command are
recorded in [REPORT.md](REPORT.md). Do not equate the latter with a clean strict
lint gate. `catalog.sh` performs only read-only inspection of the isolated test
database, using its bootstrap role to confirm null rows remain physically stored.

`m1_security.rs` in the schema crate tests parser contracts. The server crate's
`m1_security.rs` drives production GraphQL, action and Worker CRUD routers through
HTTP request/response objects against real PostgreSQL. It includes rollback,
source spoofing, owner takeover, tenant mismatch and job-delegation negatives.
It does not start a separate listening server or a browser.

`final-checks.sh` runs the final Rust commands in one container and records each
actual Cargo exit code in `final-results.jsonl`. Its overall status excludes the
known strict-lint failure; always inspect the individual results before review.

The access-property revision uses wsl --exec sh tests/m1/access-host.sh targeted and then wsl --exec sh tests/m1/access-host.sh full. Each creates a fresh database without removing any existing database or volume. Individual actual Cargo exit codes are recorded in local access-results.jsonl; the script returns failure if any command fails. Runtime-loader tests include the real Atomo builder's rejection before database initialization; HTTP tests cover roles and owners for both schema formats.

The defineModel revision uses the separate define-host.sh targeted/full commands under WSL. Each creates a fresh task-owned database and records actual exits in local define-results.jsonl. Historical access/sync evidence is retained. The new compatibility HTTP matrix checks both authorization and rejection before migrations.
