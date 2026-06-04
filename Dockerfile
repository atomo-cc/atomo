# syntax=docker/dockerfile:1
#
# atomo-server image — runs the Atomo backend with **no Rust toolchain on the
# host**, and bundles a **generic Admin UI** served at /admin. The admin is
# service-agnostic (it introspects /meta/schema); service-specific views load as
# runtime plugins, so this build has no dependency on any service's source.
# cargo-chef caches the Rust dependency compile. Build: `docker build -t atomo-server .`

# ---- Admin UI: build the generic SPA (packages/* only — no services/). ----
FROM node:20-slim AS admin-builder
RUN corepack enable
WORKDIR /repo
COPY package.json pnpm-lock.yaml ./
RUN printf 'packages:\n  - "packages/*"\n' > pnpm-workspace.yaml
COPY packages ./packages
# The admin's tailwind.config.ts imports the repo-root design tokens (../../design-tokens).
COPY design-tokens.ts ./
# Install the admin app + its deps, build the SDK it imports, then the SPA with
# base=/admin/ so its assets resolve under the served path.
RUN pnpm install --filter "@atomo-cc/admin-ui..." --no-frozen-lockfile \
    && pnpm --filter "@atomo-cc/client-sdk" run build \
    && pnpm --filter "@atomo-cc/admin-ui" run build:server

# ---- Chef: toolchain + cargo-chef. Cached unless the base image changes. ----
FROM rust:slim-bookworm AS chef
# build-essential/pkg-config cover the C-compiler needs of transitive deps
# (e.g. ring used by rustls). We use rustls everywhere, so no OpenSSL dev headers.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
WORKDIR /src

# ---- Planner: distill the dependency graph into recipe.json (cheap). ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Builder: cook deps (the expensive, cached layer), then build the server. ----
FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
# recipe.json is byte-stable unless Cargo.toml/lock change, so this layer — the
# whole dependency compile — is reused across builds until dependencies change.
RUN cargo chef cook --release -p atomo_server --recipe-path recipe.json
# Only the workspace crates recompile after this; deps are already built.
COPY . .
RUN cargo build --release -p atomo_server

# ---- Runtime: the binary + the bundled admin SPA + CA certs. ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 atomo
WORKDIR /app
COPY --from=builder /src/target/release/atomo-server /usr/local/bin/atomo-server
# Generic Admin UI — served at /admin (ATOMO_ADMIN_DIR). Unset the env to disable.
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
