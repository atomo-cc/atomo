FROM rust:latest AS builder
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ENV CARGO_TARGET_DIR=/tmp/t
RUN cargo build --release -p atomo_server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl gnupg && \
    gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg \
        --keyserver hkp://keyserver.ubuntu.com:80 \
        --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D68 && \
    echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" \
        > /etc/apt/sources.list.d/k6.list && \
    apt-get update && apt-get install -y --no-install-recommends k6 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/t/release/atomo-server /usr/local/bin/atomo-server
COPY bench/authed-load/schema.ts /bench/schema.ts
COPY bench/authed-load/setup.sql /bench/setup.sql
COPY bench/authed-load/load.js /bench/load.js

WORKDIR /bench
