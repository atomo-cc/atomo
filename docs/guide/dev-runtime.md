# Development Runtime

`atomo dev` spins up an on‑the‑fly runtime for fast iteration. For core contributors, `atomo dev --workspace` runs the service directly inside the workspace for even faster incremental builds.

What it does
- Creates a service‑local runtime under `services/<name>/.atomo/runtime`
- Parses `schema.ts` and generates Rust/GraphQL code
- Validates filters using Hasura v2 operators
- Watches files and rebuilds incrementally

Workspace mode
- Command: `atomo dev --workspace [--service-path services/<name>] [-p 3000]`
- Uses the workspace target dir for incremental compilation
- Watches both core crates and service schema for hot reload
- Also boots the Admin UI dev server and proxies it under the same port

Proxied Admin UI (workspace mode)
- Admin UI dev server runs on `http://localhost:5173`
- The backend proxies it under the service port:
  - `/admin` and `/admin/*` → Admin UI
  - `/@vite/*`, `/@react-refresh`, `/src/*`, `/node_modules/*` → Vite assets
- This keeps a single unified port for API, playground, and Admin UI

Schema endpoint in dev
- Raw schema is served for tooling and Admin UI: `GET /schema.ts`
- The runtime resolves `schema.ts` from the service folder with multiple fallback paths
- Useful for Admin UI auto‑discovery and external tooling

Hot reload watchers
- Core crates: `crates/atomo_core/src`, `crates/atomo_schema/src`, `crates/atomo_server/src`
- Optional crates (if present): `crates/atomo_cli/src`, `crates/atomo/src`
- Service schema: `services/<name>/schema.ts`
- Behavior:
  - Schema change → regenerate runtime code, recompile, restart
  - Core change → incremental recompile, restart
  - CLI change (workspace only) → rebuild CLI first, then recompile/restart

Routes in dev
- GraphQL IDE: `GET /playground`
- GraphQL API: `POST /graphql` (GET for IDE)
- Admin UI (proxy): `GET /admin`, `GET /admin/*`
- Schema file: `GET /schema.ts`
- Metadata: `GET /meta/schema` (optional auth)

Environment
- `PORT` — service port (default `3000`)
- `DATABASE_URL` — Postgres connection string
- `JWT_SECRET` — required for production auth token signing

Tips
- Restart clean: delete `services/<name>/.atomo/runtime` if needed
- Override port: `atomo dev --port 4000`
- Workspace mode for core contributors: `atomo dev --workspace`

Troubleshooting
- Admin UI unavailable: ensure `packages/atomo-admin-ui` dev server is running; the proxy will display a helpful message if it isn’t.
- `schema.ts` not found: confirm the file exists in the service root; the runtime tries several relative paths.
Force isolated mode inside the monorepo
- Command: `atomo dev --isolated` (useful to test the service-only flow without workspace deps)
