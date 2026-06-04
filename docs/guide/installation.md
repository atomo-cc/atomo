# Installation

## Start a new project (no Rust)

To scaffold **your own** project, use the JS creator — it writes a `schema.ts`, a
`docker-compose.yml` (pulling the prebuilt server image), and a README, then you
run it with Docker. No Rust, no clone:

```bash
npm create @atomo-cc/app my-crm          # or: -- --template ecommerce
cd my-crm
docker compose up                        # API on :3000
```

Templates: `crm` (default), `blog`, `ecommerce`, `default`.

## Try the demo (no Rust)

To run the bundled CRM demo from the repo instead:

```bash
git clone https://github.com/atomo-cc/atomo.git
cd atomo
docker compose up --build        # http://localhost:3000
curl http://localhost:3000/health   # -> OK
```

`docker compose` builds the server inside a container — Rust never touches your
host — and wires it to the CRM demo schema + a fresh Postgres. To run a different
model, repoint the `server.volumes` schema mount in `docker-compose.yml`. The
image is **generic and ships no admin UI** (the server serves one at `/admin` only
if you provide `ATOMO_ADMIN_DIR`). See [Deployment](/guide/tutorials/deployment)
for running the image in production.

## Prerequisites (from source)
Only needed if you build from source (contributors / core work):
- Rust 1.70+
- Node.js 18+
- pnpm 8+

## From Source (Monorepo)
```bash
# Clone and install
git clone https://github.com/atomo-cc/atomo.git
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
