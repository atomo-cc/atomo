# Dockerfile for CRM Service
# This creates a self-contained Docker image that includes:
# - Atomo Server (Rust binary)
# - CRM Service configuration
# - Admin UI (React app)
# - All plugins and workflows

# Stage 1: Build Rust backend
FROM rust:1.70 as rust-builder

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build Atomo Server
RUN cargo build --release -p atomo_server

# Stage 2: Build Admin UI
FROM node:18 as ui-builder

WORKDIR /build

# Copy package files
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY packages/ ./packages/

# Install dependencies
RUN npm install -g pnpm
RUN pnpm install

# Build Admin UI with CRM customizations
RUN pnpm build --filter atomo-admin-ui

# Stage 3: Final runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -u 1000 atomo

# Create app directory
WORKDIR /app

# Copy Rust binary
COPY --from=rust-builder /build/target/release/atomo-server /usr/local/bin/
RUN chmod +x /usr/local/bin/atomo-server

# Copy Admin UI static files
COPY --from=ui-builder /build/packages/atomo-admin-ui/dist ./public/

# Copy CRM service configuration
COPY services/crm-service/ ./config/

# Set ownership
RUN chown -R atomo:atomo /app

# Switch to app user
USER atomo

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

# Set environment variables
ENV ATOMO_CONFIG_DIR=/app/config
ENV ATOMO_PUBLIC_DIR=/app/public
ENV PORT=8080

# Start command
CMD ["atomo-server", "--config-dir", "/app/config", "--port", "8080"]
