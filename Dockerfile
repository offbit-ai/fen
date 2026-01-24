# Build stage
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build for release
RUN cargo build --release -p fen-api

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/fen-api /usr/local/bin/fen-api

# Create data directory
RUN mkdir -p /app/data /app/rules

# Set environment variables
ENV BIND_ADDRESS=0.0.0.0:3000
ENV DATABASE_PATH=/app/data/fen.redb
ENV RULES_PATH=/app/rules
ENV RUST_LOG=info,fen_api=debug,fen_ingestion=debug,fen_rules=debug

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the binary
CMD ["fen-api"]
