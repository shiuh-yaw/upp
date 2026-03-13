# UPP Gateway — Multi-stage Docker build
#
# Stage 1: Build the Rust binary with optimizations
# Stage 2: Minimal runtime image (~30MB)

# ── Build Stage ─────────────────────────────────────────────
FROM rust:1.82-slim-bookworm AS builder

WORKDIR /build

# Cache dependencies — copy manifests first, then dummy build
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    echo '// stub' > src/lib.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

# Copy real source and build
COPY src/ src/
RUN cargo build --release

# ── Runtime Stage ───────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/upp-gateway /usr/local/bin/upp-gateway

# Non-root user
RUN groupadd -r upp && useradd -r -g upp upp
USER upp

EXPOSE 8080

ENV UPP_HOST=0.0.0.0
ENV UPP_PORT=8080
ENV RUST_LOG=upp_gateway=info,tower_http=info

ENTRYPOINT ["upp-gateway"]
