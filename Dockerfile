# Multi-stage build for UPP Gateway

# Stage 1: Build
FROM rust:1.77-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/upp-gateway /app/upp-gateway

# Copy configuration
COPY config/ /app/config/

# Expose ports
EXPOSE 8080 50051

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD /app/upp-gateway health || exit 1

# Environment variables
ENV RUST_LOG=info
ENV UPP_LISTEN_ADDR=0.0.0.0:8080
ENV UPP_GRPC_ADDR=0.0.0.0:50051
ENV UPP_LOG_LEVEL=info

# Run the gateway
ENTRYPOINT ["/app/upp-gateway"]
CMD ["serve"]
