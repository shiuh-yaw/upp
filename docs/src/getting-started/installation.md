# Installation

Multiple paths to get UPP running: Docker containers, pre-built binaries, or from source. Choose what works best for your environment.

## Option 1: Docker (Recommended)

### Using Docker Compose (Simplest)

Perfect for local development. Includes gateway, Redis, Prometheus, Grafana, and Jaeger:

```bash
git clone https://github.com/universal-prediction-protocol/upp.git
cd upp
docker-compose up -d
```

The docker-compose.yml includes:

```yaml
services:
  gateway:
    build:
      context: .
      dockerfile: Dockerfile.gateway
    ports:
      - "8080:8080"
      - "50051:50051"
    environment:
      RUST_LOG: info
      REDIS_URL: redis://redis:6379
      PROMETHEUS_PUSH_ADDR: http://prometheus:9090
    depends_on:
      - redis
      - prometheus

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin

  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"
      - "6831:6831/udp"
```

Verify everything started:

```bash
docker-compose ps
curl http://localhost:8080/api/v1/health
```

### Using Individual Docker Images

If you prefer to manage containers manually:

```bash
# Pull the gateway image
docker pull ghcr.io/universal-prediction-protocol/gateway:latest

# Run with environment
docker run -d \
  --name upp-gateway \
  -p 8080:8080 \
  -p 50051:50051 \
  -e REDIS_URL=redis://host.docker.internal:6379 \
  -e RUST_LOG=info \
  ghcr.io/universal-prediction-protocol/gateway:latest
```

## Option 2: Pre-built Binaries

Download pre-compiled binaries from [GitHub Releases](https://github.com/universal-prediction-protocol/upp/releases):

```bash
# macOS (Intel)
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/gateway-darwin-x86_64
chmod +x gateway-darwin-x86_64
./gateway-darwin-x86_64

# macOS (Apple Silicon)
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/gateway-darwin-arm64
chmod +x gateway-darwin-arm64
./gateway-darwin-arm64

# Linux (x86_64)
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/gateway-linux-x86_64
chmod +x gateway-linux-x86_64
./gateway-linux-x86_64

# Windows
# Download gateway-windows-x86_64.exe from releases page
# Or via PowerShell:
Invoke-WebRequest `
  -Uri "https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/gateway-windows-x86_64.exe" `
  -OutFile "gateway.exe"
```

Then configure the environment:

```bash
export REDIS_URL=redis://localhost:6379
export RUST_LOG=info
./gateway-linux-x86_64
```

## Option 3: Build from Source

### Prerequisites

- Rust 1.70+ ([install via rustup](https://rustup.rs/))
- Git
- C compiler (gcc on Linux, clang on macOS, MSVC on Windows)
- pkg-config (Linux)

### Build Steps

```bash
# Clone repository
git clone https://github.com/universal-prediction-protocol/upp.git
cd upp

# Build gateway
cargo build --release -p gateway

# Build CLI
cargo build --release -p cli

# Build SDK (library only)
cargo build --release -p sdk

# Run the gateway
./target/release/gateway
```

Gateway will start on `localhost:8080` and gRPC on `50051`.

### Development Build (Faster Iteration)

For local development, use debug builds:

```bash
cargo build -p gateway
./target/debug/gateway
```

Slower startup, faster compilation for quick iteration.

## Option 4: Install CLI Tool

The CLI tool can be installed independently for querying markets without the server:

### From Cargo (if published)

```bash
cargo install upp-cli
upp health
upp markets list --provider polymarket --limit 10
```

### From Pre-built Binary

```bash
# macOS
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/upp-cli-darwin-arm64
chmod +x upp-cli-darwin-arm64
sudo mv upp-cli-darwin-arm64 /usr/local/bin/upp

# Linux
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/upp-cli-linux-x86_64
chmod +x upp-cli-linux-x86_64
sudo mv upp-cli-linux-x86_64 /usr/local/bin/upp

# Verify
upp --version
```

### From Source

```bash
cargo build --release -p cli
sudo cp ./target/release/upp-cli /usr/local/bin/upp
```

## Environment Configuration

Before running, configure these environment variables:

```bash
# Redis connection (required for caching)
export REDIS_URL=redis://localhost:6379

# Logging
export RUST_LOG=info  # or debug, warn, error

# API Keys (optional, for real market access)
export KALSHI_API_KEY=your_kalshi_key
export KALSHI_API_SECRET=your_kalshi_secret
export POLYMARKET_PRIVATE_KEY=0x...  # ECDSA private key (hex)
export OPINION_TRADE_API_KEY=your_opinion_key

# Server configuration
export SERVER_HOST=0.0.0.0
export SERVER_PORT=8080
export GRPC_PORT=50051

# Cache settings
export CACHE_TTL_SECONDS=300
export CACHE_MAX_SIZE=1000

# Metrics
export PROMETHEUS_ADDR=http://localhost:9090
export JAEGER_AGENT_HOST=localhost
export JAEGER_AGENT_PORT=6831
```

## Verify Installation

Test that everything is working:

```bash
# Health check
curl http://localhost:8080/api/v1/health

# Get markets
curl http://localhost:8080/api/v1/markets?provider=polymarket&limit=1

# CLI version
upp --version

# CLI health
upp health
```

## Next Steps

- **Quickstart** — See [5-Minute Quickstart](quickstart.md) to run your first query
- **REST API** — See [REST API Reference](../api/rest.md) for all endpoints
- **Rust SDK** — See [Rust Client Guide](../sdk/rust.md) for programmatic access
- **Deployment** — See [Deployment Guide](../operations/deployment.md) for production setup
- **Configuration** — See [Configuration Guide](../operations/configuration.md) for detailed config options

## Troubleshooting

**"Connection refused" on localhost:8080**
- Ensure the gateway is running: `docker-compose ps` or check process
- Check if port 8080 is in use: `lsof -i :8080` (macOS/Linux)
- Try a different port: `SERVER_PORT=8081 ./gateway`

**"Failed to connect to Redis"**
- Ensure Redis is running: `redis-cli ping` should return PONG
- Check Redis URL: Default is `redis://localhost:6379`
- Start Redis if missing: `docker run -d -p 6379:6379 redis:7`

**"Protobuf compilation errors" when building from source**
- Ensure protoc is installed: `protoc --version`
- Install on macOS: `brew install protobuf`
- Install on Linux: `apt-get install protobuf-compiler`

**Build fails with "linking with 'cc' failed"**
- Missing C compiler
- macOS: Install Xcode: `xcode-select --install`
- Linux (Ubuntu): `apt-get install build-essential`
- Windows: Install Visual Studio Build Tools
