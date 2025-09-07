# Server Routes in Dev

This page lists the routes available while developing locally and clarifies which ones are provided by `atomo dev` versus `atomo workspace-dev`.

Common (both modes)
- `/health` — health check
- `/ready` — readiness probe; checks DB connectivity
- `/graphql` — GraphQL endpoint (POST for operations)
- GraphQL IDE
  - `atomo dev`: `GET /graphql` serves the playground
  - `workspace-dev`: `GET /playground` serves GraphiQL
- `/schema.ts` — raw schema file served in development (used by Admin UI/tooling)

atomo dev (full platform server)
- `/` — root info
- `/info` — server info
- `/graphql` — GET (playground), POST (operations)
- `/meta/schema` — JSON metadata of models (optional auth)
- Auth (REST)
  - `POST /auth/login`
  - `POST /auth/logout`
  - `GET /auth/me`
- Audit (REST)
  - `GET /audit/logs`
  - `GET /audit/user/:user_id/activity`
  - `GET /audit/entity/:entity_type/:entity_id/audit`
  - `GET /audit/statistics`

workspace-dev (service‑scoped hot reload server)
- `/` — service health message (Workspace Dev)
- `/health` — health check
- `/graphql` — POST (operations)
- `/playground` — GraphiQL IDE
- `/schema.ts` — raw TS schema (multiple path fallbacks)
- Admin UI proxy (requires `packages/atomo-admin-ui` dev server on `:5173`)
  - `/admin` → 301 to `/admin/`
  - `/admin/` → proxies to `http://localhost:5173/`
  - `/admin/*` → proxies to `http://localhost:5173/*`
- Vite asset proxies (workspace‑dev only)
  - `/@vite/*`, `/@react-refresh`
  - `/@fs/*`, `/src/*`, `/node_modules/*`

Observability
- `/metrics` — Prometheus metrics in text exposition format (both modes)

Ports
- Service/API default: `http://localhost:3000`
- Admin UI dev server: `http://localhost:5173` (proxied under `/admin` in workspace‑dev)

Notes
- Admin proxy returns a helpful message if the UI dev server is not running.
- CORS: workspace‑dev enables permissive CORS for easier local development.
