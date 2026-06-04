# syntax=docker/dockerfile:1
#
# atomo-server image — lets users run the Atomo backend (and bundled Admin UI)
# with **no Rust or Node toolchain on the host**. Both compilers run only inside
# build stages; the runtime image carries just the server binary + the built SPA.
# Build: `docker build -t atomo-server .`

# ---- Admin UI stage: build the SPA, served same-origin at /admin ----
FROM node:20-slim AS admin-builder
RUN corepack enable
WORKDIR /repo
# Workspace for the SPA build. The admin UI imports CRM-service components via
# relative paths (services/crm-service/admin-ui/...), so services/ must be present
# and its deps installed — hence packages/* + services/* and a full install.
COPY package.json pnpm-lock.yaml ./
RUN printf 'packages:\n  - "packages/*"\n  - "services/*"\n' > pnpm-workspace.yaml
COPY packages ./packages
COPY services ./services
# Build the SDK the admin UI imports, then the SPA with base=/admin/ so its assets
# resolve under the served path.
RUN pnpm install --no-frozen-lockfile \
    && pnpm --filter "@atomo-cc/client-sdk" run build \
    && pnpm --filter "@atomo-cc/admin-ui" run build:server

# ---- Build stage: compile the server from the Cargo workspace ----
FROM rust:slim-bookworm AS builder
# build-essential/pkg-config cover the C-compiler needs of transitive deps
# (e.g. ring used by rustls). We use rustls everywhere, so no OpenSSL dev headers.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release -p atomo_server

# ---- Runtime stage: just the binary + CA certs ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 atomo
WORKDIR /app
COPY --from=builder /src/target/release/atomo-server /usr/local/bin/atomo-server
# Bundled Admin UI SPA — the server serves it at /admin when ATOMO_ADMIN_DIR exists.
COPY --from=admin-builder /repo/packages/atomo-admin-ui/dist /app/admin
# The schema is supplied at runtime (mounted or baked). Defaults below can be
# overridden with -e / compose `environment:`.
ENV ATOMO_SCHEMA_PATH=/app/schema.ts \
    ATOMO_ADMIN_DIR=/app/admin \
    HOST=0.0.0.0 \
    PORT=3000
EXPOSE 3000
USER atomo
CMD ["atomo-server"]
