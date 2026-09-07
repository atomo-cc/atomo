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

## Cache, model history and audit

`ServerConfig.cache`, `.history` and `.audit` are independent configuration objects. Environment parsing is strict; malformed JSON, unknown fields and invalid limits fail startup. Cache model override names must exist in the active schema; history/audit policy names should be checked against the intended model/entity types.

- `ATOMO_HISTORY_CONFIG`: JSON `HistoryConfig`, default `{"default":{"mode":"full"},"models":{},"maintenance_interval_secs":60,"batch_size":1000}`.
- `ATOMO_AUDIT_CONFIG`: JSON `AuditConfig`, same object structure; audit modes are `full`, `metadata`, `off`.
- `ATOMO_CACHE_ENABLED`, `ATOMO_CACHE_MODE`, `ATOMO_CACHE_TTL_SECS`, `ATOMO_CACHE_TTI_SECS`, `ATOMO_CACHE_CLEANUP_INTERVAL_SECS`, `ATOMO_CACHE_MAX_ENTRIES`, `ATOMO_CACHE_MAX_BYTES`, `ATOMO_CACHE_MAX_ENTRY_BYTES`, `ATOMO_CACHE_REFRESH_AFTER_SECS`, `ATOMO_CACHE_MULTI_INSTANCE`, `ATOMO_CACHE_MODELS`: see the complete [cache parameter table](/guide/caching).

History and audit limits do not apply to media blobs, durable jobs, aggregate event streams, container logs or build caches. See [Storage lifecycle](/guide/storage-lifecycle) for policy examples, retention scope, replay restrictions and diagnostics.

## Admin UI development backend

Set `VITE_API_URL` in the Vite process environment or `packages/atomo-admin-ui/.env.local` to an HTTP(S) backend URL, for example `http://127.0.0.1:3000`. Vite loads this setting for all API proxies, including relative `/meta`, `/schema.ts` and `/storage` requests; Axios uses the same value. The `/ws` proxy derives `ws:` or `wss:` from the backend protocol. Without the setting, the development proxy targets `http://localhost:3000`. Do not include credentials in the URL.

This is frontend development/build configuration, not a Rust `ServerConfig` setting. Production deployments can keep the existing same-origin arrangement by leaving it unset.
