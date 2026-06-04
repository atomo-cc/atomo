# syntax=docker/dockerfile:1
#
# atomo-server image — lets users run the Atomo backend with **no Rust toolchain
# on the host**. Rust only runs inside the build stage; the runtime image carries
# just the compiled binary. Build: `docker build -t atomo-server .`

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
# The schema is supplied at runtime (mounted or baked). Defaults below can be
# overridden with -e / compose `environment:`.
ENV ATOMO_SCHEMA_PATH=/app/schema.ts \
    HOST=0.0.0.0 \
    PORT=3000
EXPOSE 3000
USER atomo
CMD ["atomo-server"]
