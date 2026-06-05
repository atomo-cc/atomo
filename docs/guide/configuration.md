# Configuration

## Environment
- Copy `.env.example` to `.env` in repo root or service dir.
- Common vars: `DATABASE_URL`, `RUST_LOG`, `JWT_SECRET` (prod), `CORS_ORIGINS`, `ATOMO_ENV` (set to `production` in prod).
- Admin bootstrap: `ADMIN_EMAIL`, `ADMIN_PASSWORD` — create an admin user on server start (create-once by email; a later `ADMIN_PASSWORD` change is ignored with a `WARN` — set `ADMIN_RESET_PASSWORD=true` to rotate it on boot; see Auth).
- GraphQL limits: `GRAPHQL_MAX_DEPTH` (default 20), `GRAPHQL_MAX_COMPLEXITY` (default 200).
- Rate limiting (per-IP token bucket): `RATE_LIMIT_RPS` (default 100), `RATE_LIMIT_WINDOW_SECS` (default 60).
- Observability: `/metrics` exports Prometheus metrics; enable logs via `RUST_LOG` (e.g., `info`).
 - Log format: `LOG_FORMAT=json` for JSON logs; otherwise pretty logs.
 - Security headers: override CSP with `CSP` if needed; disable all security headers via `DISABLE_SECURITY_HEADERS=true` (not recommended).
- Auth hashing: argon2id (default); legacy bcrypt hashes are verified for migration.

## Service Config
- Each service includes metadata in `package.json` under `atomo`:
```json
{
  "atomo": {
    "service": "crm",
    "configFile": "./atomo.config.ts",
    "schemaFile": "./schema.ts",
    "pluginsDir": "./plugins",
    "workflowsDir": "./workflows",
    "adminDir": "./admin"
  }
}
```

## Ports & Server
- `atomo dev` starts a server (default port 3000). Override with `--port`.
- Standalone server: `atomo-server --config-dir services/<name> --port 3000`.
 - Readiness probe: `GET /ready` checks DB connectivity.
