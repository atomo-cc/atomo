FROM rust:latest AS builder
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
ENV CARGO_TARGET_DIR=/tmp/t
RUN cargo build --release -p atomo_server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /tmp/t/release/atomo-server /usr/local/bin/atomo-server
COPY bench/authed-load/schema.ts /bench/schema.ts
WORKDIR /bench
EXPOSE 3099
CMD ["atomo-server", "-p", "3099"]
