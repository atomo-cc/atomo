# syntax=docker/dockerfile:1
#
# atomo-server image — runs the Atomo backend with **no Rust toolchain on the
# host**. Generic and service-agnostic: it bundles no admin UI. cargo-chef caches
# the dependency compile as its own layer, so app-only changes rebuild in ~1-3 min
# instead of recompiling the whole tree (~12 min). Build: `docker build -t atomo-server .`

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

# ---- Runtime: just the binary + CA certs. ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 atomo
WORKDIR /app
COPY --from=builder /src/target/release/atomo-server /usr/local/bin/atomo-server
# The schema is supplied at runtime (mounted or baked). Defaults below can be
# overridden with -e / compose `environment:`.
#
# Admin UI: the server serves a SPA at /admin only when ATOMO_ADMIN_DIR points at
# a built admin bundle (absent here). The image stays generic — an app that wants
# an admin UI mounts its own build there; a game/relay backend ships none.
ENV ATOMO_SCHEMA_PATH=/app/schema.ts \
    HOST=0.0.0.0 \
    PORT=3000
EXPOSE 3000
USER atomo
CMD ["atomo-server"]
