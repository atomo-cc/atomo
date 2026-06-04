# Deployment Guide

## Docker (no Rust required)

The supported way to run the backend in production-like setups is the container
image — no Rust toolchain on the host.

```bash
# Build the server image (multi-stage; Rust runs only in the build container)
docker build -t atomo-server .

# Or run the whole stack (Postgres + server) locally:
docker compose up --build        # http://localhost:3000
```

The image takes its configuration from environment variables — at minimum
`DATABASE_URL`, `ATOMO_SCHEMA_PATH`, and `JWT_SECRET` (see
[Configuration](/guide/configuration) for the full list). Provide your service's
`schema.ts` by mounting it at `ATOMO_SCHEMA_PATH` (the compose file mounts the CRM
demo schema by default). On boot the server parses the schema, runs migrations,
seeds the admin, and starts listening on `PORT`.

The image also **bundles the Admin UI**, served same-origin at `/admin` (e.g.
`http://localhost:3000/admin`) — no separate frontend deploy. It's enabled
automatically when the SPA is present (`ATOMO_ADMIN_DIR`, default `/app/admin`).

> Publishing: once pushed to a registry (e.g. `ghcr.io/<org>/atomo-server`),
> replace `build: .` in `docker-compose.yml` with `image: …` to pull instead of
> build.

## From source

```bash
pnpm build:core
cd services/<name>
pnpm build
```

See DEPLOYMENT_GUIDE.md at the repo root for full steps.
