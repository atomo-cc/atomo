# Installation

## Run without Rust (Docker)

The quickest way to run the backend with **no Rust toolchain** on your machine —
Postgres + the server, one command:

```bash
git clone https://github.com/Chris533/atomo.git
cd atomo
docker compose up --build        # http://localhost:3000
curl http://localhost:3000/health   # -> OK
```

The image bundles the **Admin UI** — open <http://localhost:3000/admin> once it's
up. `docker compose` builds the server (and the SPA) inside containers — neither
Rust nor Node touches your host — and wires it to the CRM demo schema + a fresh
Postgres. To run a different model, repoint the `server.volumes` schema mount in
`docker-compose.yml`. See [Deployment](/guide/tutorials/deployment) for running
the image in production.

## Prerequisites (from source)
Only needed if you build from source (contributors / core work):
- Rust 1.70+
- Node.js 18+
- pnpm 8+

## From Source (Monorepo)
```bash
# Clone and install
git clone https://github.com/Chris533/atomo.git
cd atomo
pnpm install

# Build Rust workspace
cargo build --workspace
```

## Frontend
```

Current MVP commands:

```bash
pnpm dev:admin
pnpm --filter @atomo-cc/client-sdk dev
pnpm --filter atomo-crm-service generate
```

Rust workspace builds are still available for core work with `cargo build --workspace`, but they are not the first step for the current Admin UI + SDK + CRM demo loop.

Next: see Quick Start at `/guide/getting-started`.
