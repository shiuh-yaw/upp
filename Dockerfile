# Multi-stage build for UPP Gateway with cargo-chef for dependency caching
# Supports: default, all, mock features via build args

# ────── Stage 1: Chef (dependency caching) ──────
FROM rust:latest AS chef

WORKDIR /build

# Install cargo-chef (--locked ensures compatible dependency versions)
RUN cargo install cargo-chef --locked

# ────── Stage 2: Planner ──────
FROM chef AS planner

WORKDIR /build
COPY . .

# Generate recipe.json for dependency caching
RUN cargo chef prepare --recipe-path recipe.json

# ────── Stage 3: Builder ──────
FROM chef AS builder

WORKDIR /build

ARG FEATURES=default
ARG CARGO_PROFILE=release

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy recipe.json from planner
COPY --from=planner /build/recipe.json recipe.json

# Build dependencies (cached layer)
# When FEATURES=default, omit --features flag (default features are automatic)
RUN if [ "${FEATURES}" = "default" ]; then \
        cargo chef cook --${CARGO_PROFILE} --recipe-path recipe.json; \
    else \
        cargo chef cook --${CARGO_PROFILE} --features "${FEATURES}" --recipe-path recipe.json; \
    fi

# Copy source code
COPY . .

# Build the application
RUN if [ "${FEATURES}" = "default" ]; then \
        cargo build --${CARGO_PROFILE}; \
    else \
        cargo build --${CARGO_PROFILE} --features "${FEATURES}"; \
    fi

# ────── Stage 4: Runtime ──────
FROM debian:bookworm-slim

WORKDIR /app

ARG BUILD_DATE
ARG VCS_REF
ARG VERSION=0.1.0

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 upp && chown -R upp:upp /app

# Copy binary from builder
COPY --from=builder --chown=upp:upp /build/target/release/upp-gateway /app/upp-gateway

# Copy configuration
COPY --chown=upp:upp config/ /app/config/

# OCI metadata labels
LABEL org.opencontainers.image.created="${BUILD_DATE}"
LABEL org.opencontainers.image.authors="UPP Team"
LABEL org.opencontainers.image.url="https://github.com/universalprotocol/upp"
LABEL org.opencontainers.image.documentation="https://github.com/universalprotocol/upp/blob/main/README.md"
LABEL org.opencontainers.image.source="https://github.com/universalprotocol/upp"
LABEL org.opencontainers.image.version="${VERSION}"
LABEL org.opencontainers.image.revision="${VCS_REF}"
LABEL org.opencontainers.image.vendor="UPP"
LABEL org.opencontainers.image.licenses="Apache-2.0"
LABEL org.opencontainers.image.title="UPP Gateway"
LABEL org.opencontainers.image.description="Universal Prediction Protocol Gateway with gRPC and REST API"

# Expose ports
EXPOSE 8080 50051

# Environment variables
ENV RUST_LOG=info
ENV UPP_LISTEN_ADDR=0.0.0.0:8080
ENV UPP_GRPC_ADDR=0.0.0.0:50051
ENV UPP_LOG_LEVEL=info

# Health check via curl
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Switch to non-root user
USER upp

# Run the gateway
ENTRYPOINT ["/app/upp-gateway"]
CMD ["serve"]
