# Development Runtime

`atomo dev` starts the full Atomo platform server for fast iteration. It auto-detects whether you're inside the monorepo (workspace mode) or in a standalone service (isolated mode).

## Isolated mode

Command: `atomo dev` (outside the workspace) or `atomo dev --isolated`

Runs the real `atomo-server` binary directly — no code generation or compilation step. The server reads `schema.ts` at startup and provides the full platform: auth, GraphQL, jobs, CQRS projections, realtime, workflows.

What it does:
- Finds (or builds) the `atomo-server` binary from the workspace `target/` directory
- Loads `.env` from the service directory
- Passes `ATOMO_SCHEMA_PATH` and `PORT` to the server process
- Watches `schema.ts` for changes and restarts the server on modification (2s debounce)

Environment (set in `.env`):
- `DATABASE_URL` — Postgres connection string (required)
- `ADMIN_EMAIL` / `ADMIN_PASSWORD` — seeds an admin user on first boot
- `JWT_SECRET` — token signing key (defaults to insecure dev secret if unset)
- `PORT` — service port (default `3000`)
- `CORS_ORIGINS` — allowed origins for CORS (e.g. `*` for dev; omit for same-origin only)

## Workspace mode

Command: `atomo dev --workspace [--service-path services/<name>] [-p 3000]`

Uses the workspace target dir for incremental compilation. Generates Rust code from `schema.ts` (models, resolvers, GraphQL schema) and watches both core crates and service schema for hot reload. Also boots the Admin UI dev server and proxies it under the same port.

Proxied Admin UI (workspace mode):
- Admin UI dev server runs on `http://localhost:5173`
- The backend proxies it under the service port:
  - `/admin` and `/admin/*` → Admin UI
  - `/@vite/*`, `/@react-refresh`, `/src/*`, `/node_modules/*` → Vite assets
- This keeps a single unified port for API, playground, and Admin UI

Hot reload watchers:
- Core crates: `crates/atomo_core/src`, `crates/atomo_schema/src`, `crates/atomo_server/src`
- Optional crates (if present): `crates/atomo_cli/src`, `crates/atomo/src`
- Service schema: `services/<name>/schema.ts`
- Behavior:
  - Schema change → regenerate runtime code, recompile, restart
  - Core change → incremental recompile, restart
  - CLI change (workspace only) → rebuild CLI first, then recompile/restart

## Routes

- GraphQL IDE: `GET /playground`
- GraphQL API: `POST /graphql` (GET for IDE)
- Auth: `POST /auth/login`, `GET /auth/me`
- Admin UI: `GET /admin` (workspace mode proxy, or bundled)
- Schema file: `GET /schema.ts`
- Health: `GET /health`
- Jobs: `/jobs` (lease API)

## Tips

- Override port: `atomo dev --port 4000`
- Force isolated mode inside the monorepo: `atomo dev --isolated`
- Workspace mode for core contributors: `atomo dev --workspace`
- Restart clean (workspace mode): delete `services/<name>/.atomo/runtime`

## Troubleshooting

- `atomo-server not found`: run `cargo build -p atomo_server --bin atomo-server` first
- Admin UI unavailable in workspace mode: ensure `packages/atomo-admin-ui` dev server is running
- `schema.ts` not found: confirm the file exists in the service root
