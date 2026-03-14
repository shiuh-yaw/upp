# UPP Gateway — Deployment Guide

## Table of Contents

1. [Development Setup](#development-setup)
2. [Docker Deployment](#docker-deployment)
3. [Production Checklist](#production-checklist)
4. [Cloud Deployment](#cloud-deployment)
5. [Monitoring & Observability](#monitoring--observability)
6. [Security Hardening](#security-hardening)
7. [Scaling](#scaling)
8. [Troubleshooting](#troubleshooting)

---

## Development Setup

### Prerequisites

- Rust 1.75+ (`rustup update stable`)
- Redis 7+ (optional for dev, `brew install redis` / `apt install redis-server`)
- Protocol Buffers compiler (`protoc`) for gRPC codegen

### Local Build

```bash
cd upp/gateway
cargo build

# Run with hot-reload (requires cargo-watch)
cargo install cargo-watch
cargo watch -x run

# Run tests
cargo test
```

### Dev Mode Defaults

In development mode (the default), the gateway runs with relaxed settings:

- **Auth disabled** — all requests pass through without credentials
- **In-memory storage** — no Redis required, data resets on restart
- **Mock data sources** — historical ingestion generates synthetic price data
- **Verbose logging** — pretty-printed logs at INFO level

### Environment Variables for Dev

```bash
export UPP_REST_PORT=9090
export UPP_GRPC_PORT=9091
export RUST_LOG=upp_gateway=debug,tower_http=debug
```

---

## Docker Deployment

### Single Container

```bash
docker build -t upp-gateway -f Dockerfile .
docker run -d \
  --name upp-gateway \
  -p 9090:9090 \
  -p 9091:9091 \
  -e RUST_LOG=info \
  upp-gateway
```

### Docker Compose (Gateway + Redis)

```bash
docker-compose up -d
```

The `docker-compose.yml` provides:
- UPP Gateway on ports 9090 (REST) / 9091 (gRPC)
- Redis 7 on port 6379
- Health checks on both services
- Automatic restart policies
- Shared network for inter-service communication

### Multi-Stage Dockerfile

The included Dockerfile uses a multi-stage build:

1. **Builder stage** — `rust:1.75-slim` compiles with `--release`, `lto=true`, `strip=true`
2. **Runtime stage** — `debian:bookworm-slim` with only the binary (~15MB)

This produces a minimal production image with no build tools or source code.

---

## Production Checklist

Before going live, verify each item:

### Authentication

- [ ] Set `UPP_AUTH_REQUIRED=true`
- [ ] Configure JWT secret: `UPP_JWT_SECRET=<random-256-bit-secret>`
- [ ] Create initial API keys via the `/upp/v1/auth/keys` endpoint
- [ ] Verify protected endpoints reject unauthenticated requests
- [ ] Set up key rotation schedule (recommended: 90 days)

### Storage

- [ ] Deploy Redis for persistent state: `UPP_REDIS_URL=redis://host:6379`
- [ ] Configure Redis authentication if exposed: `redis://:password@host:6379`
- [ ] Enable Redis persistence (AOF or RDB) for order/trade durability
- [ ] Set up Redis Sentinel or Cluster for HA

### Networking

- [ ] Place gateway behind a reverse proxy (nginx/Traefik/Envoy)
- [ ] Terminate TLS at the proxy layer
- [ ] Configure CORS origins for your specific domains
- [ ] Set up health check endpoints in your load balancer (`/health`, `/ready`)

### Rate Limiting

- [ ] Review rate limit thresholds for your expected load
- [ ] Configure per-tier limits appropriate to your client base
- [ ] Set up alerting on `upp_requests_rate_limited_total` metric

### Observability

- [ ] Connect Prometheus to scrape `/metrics`
- [ ] Import the Grafana dashboard (see [Monitoring](#monitoring--observability))
- [ ] Set up alerting on error rates and latency percentiles
- [ ] Configure structured JSON logging: `UPP_LOG_FORMAT=json`
- [ ] Ship logs to your aggregator (ELK, Loki, Datadog, etc.)

### Provider Credentials

- [ ] Obtain API keys from each prediction market provider
- [ ] Configure provider-specific credentials via environment variables
- [ ] Test connectivity to each provider's sandbox/testnet first
- [ ] Set up circuit breaker thresholds per provider

---

## Cloud Deployment

### AWS (ECS/Fargate)

```json
{
  "containerDefinitions": [{
    "name": "upp-gateway",
    "image": "your-ecr-repo/upp-gateway:latest",
    "portMappings": [
      {"containerPort": 9090, "protocol": "tcp"},
      {"containerPort": 9091, "protocol": "tcp"}
    ],
    "environment": [
      {"name": "UPP_AUTH_REQUIRED", "value": "true"},
      {"name": "UPP_LOG_FORMAT", "value": "json"},
      {"name": "RUST_LOG", "value": "info"}
    ],
    "secrets": [
      {"name": "UPP_REDIS_URL", "valueFrom": "arn:aws:ssm:..."},
      {"name": "UPP_JWT_SECRET", "valueFrom": "arn:aws:ssm:..."}
    ],
    "healthCheck": {
      "command": ["CMD-SHELL", "curl -f http://localhost:9090/health || exit 1"],
      "interval": 30,
      "timeout": 5,
      "retries": 3
    },
    "logConfiguration": {
      "logDriver": "awslogs",
      "options": {
        "awslogs-group": "/ecs/upp-gateway",
        "awslogs-region": "us-east-1"
      }
    }
  }]
}
```

Use ElastiCache (Redis) for persistent state, and an ALB for REST traffic / NLB for gRPC.

### GCP (Cloud Run)

```bash
gcloud run deploy upp-gateway \
  --image gcr.io/your-project/upp-gateway \
  --port 9090 \
  --memory 512Mi \
  --cpu 1 \
  --min-instances 1 \
  --max-instances 10 \
  --set-env-vars "UPP_AUTH_REQUIRED=true,UPP_LOG_FORMAT=json" \
  --set-secrets "UPP_JWT_SECRET=upp-jwt-secret:latest,UPP_REDIS_URL=upp-redis-url:latest"
```

Use Memorystore for Redis. Note: Cloud Run supports HTTP/2 (gRPC) natively on port 9090 with `--use-http2`.

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: upp-gateway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: upp-gateway
  template:
    metadata:
      labels:
        app: upp-gateway
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      containers:
      - name: gateway
        image: upp-gateway:latest
        ports:
        - containerPort: 9090
          name: http
        - containerPort: 9091
          name: grpc
        env:
        - name: UPP_AUTH_REQUIRED
          value: "true"
        - name: UPP_LOG_FORMAT
          value: "json"
        - name: UPP_REDIS_URL
          valueFrom:
            secretKeyRef:
              name: upp-secrets
              key: redis-url
        - name: UPP_JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: upp-secrets
              key: jwt-secret
        resources:
          requests:
            cpu: 250m
            memory: 256Mi
          limits:
            cpu: "1"
            memory: 512Mi
        livenessProbe:
          httpGet:
            path: /health
            port: 9090
          initialDelaySeconds: 5
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 9090
          initialDelaySeconds: 3
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: upp-gateway
spec:
  selector:
    app: upp-gateway
  ports:
  - name: http
    port: 80
    targetPort: 9090
  - name: grpc
    port: 9091
    targetPort: 9091
```

---

## Monitoring & Observability

### Prometheus Metrics

The `/metrics` endpoint exposes 20+ metrics in Prometheus text format:

**Request Metrics**
- `upp_requests_total` — total HTTP requests
- `upp_requests_ok` — successful (2xx) responses
- `upp_requests_errors` — error (4xx/5xx) responses
- `upp_requests_rate_limited` — rate-limited requests

**WebSocket Metrics**
- `upp_websocket_connections` — active WebSocket connections
- `upp_websocket_channels` — active price channels
- `upp_websocket_subscriptions` — active subscriptions

**Business Metrics**
- `upp_arbitrage_scans_total` — arbitrage scan cycles
- `upp_arbitrage_active` — current active opportunities
- `upp_price_index_ticks_total` — ticks ingested into price index
- `upp_smart_router_routes_computed` — routes computed
- `upp_live_feed_messages_total` — messages from live feeds
- `upp_ingestion_ticks_total` — historical ticks ingested
- `upp_api_keys_total` / `upp_api_keys_active` — API key counts

### Grafana Dashboard

Import the metrics into Grafana and set up panels for:

1. Request rate and error rate (5-minute rolling average)
2. Active WebSocket connections over time
3. Arbitrage opportunity count and profit tracking
4. Price index tick ingestion rate
5. Rate limiting events by endpoint class
6. API key usage patterns

### Built-in Dashboard

Access the real-time monitoring dashboard at `http://localhost:9090/dashboard`. It provides 8 panels with auto-refresh (5s) including gateway health, market overview, feed status, arbitrage tracker, and system metrics.

### Structured Logging

Set `UPP_LOG_FORMAT=json` for machine-parseable log output:

```json
{"timestamp":"2026-03-14T10:30:00Z","level":"INFO","target":"upp_gateway","message":"Order created","order_id":"ord_abc123","market_id":"BTC-2026-Q1","provider":"kalshi"}
```

---

## Security Hardening

### API Key Best Practices

- Store keys in a secrets manager (AWS Secrets Manager, HashiCorp Vault, etc.)
- Never log full API keys — the gateway only logs key prefixes
- Set expiration on all keys (`expires_in_days` parameter)
- Use provider-scoped keys when possible (limit `providers` array)
- Monitor key usage via the Prometheus metrics

### Network Security

- Always terminate TLS at the reverse proxy
- Use mTLS for gRPC inter-service communication
- Restrict Redis to private networks only (no public exposure)
- Set `Access-Control-Allow-Origin` to your specific domains

### Rate Limiting

Rate limiting protects against abuse. The four endpoint classes are:

- **Light**: Read-only data endpoints (markets, candles, health)
- **Standard**: Search, list, and analytics queries
- **Heavy**: Write operations (orders, route execution)
- **WebSocket**: Connection and subscription management

Each class has independent burst and sustained rate limits, configurable per-client via their access tier.

---

## Scaling

### Horizontal Scaling

The gateway is stateless when backed by Redis. Scale horizontally by:

1. Running multiple gateway instances behind a load balancer
2. Pointing all instances at the same Redis cluster
3. Using sticky sessions for WebSocket connections (or a WebSocket-aware LB)

### Performance Characteristics

On a single instance (4 vCPU, 2GB RAM):

- REST API: ~50,000 requests/second (cached market data)
- WebSocket: ~10,000 concurrent connections
- Order processing: ~5,000 orders/second
- Price index: ~100,000 ticks/second ingestion

### Resource Recommendations

| Workload | CPU | Memory | Instances |
|----------|-----|--------|-----------|
| Development | 1 core | 256MB | 1 |
| Small (< 100 users) | 2 cores | 512MB | 1-2 |
| Medium (< 1000 users) | 4 cores | 1GB | 2-4 |
| Large (> 1000 users) | 8 cores | 2GB | 4+ |

---

## Troubleshooting

### Gateway Won't Start

```bash
# Check port availability
lsof -i :9090
lsof -i :9091

# Verify Redis connectivity (if configured)
redis-cli -u $UPP_REDIS_URL ping

# Enable debug logging
RUST_LOG=debug cargo run
```

### Provider Connectivity Issues

The circuit breaker will trip after 5 consecutive failures. Check:

```bash
# View feed status
curl http://localhost:9090/upp/v1/feeds/status

# Check circuit breaker state in logs
RUST_LOG=upp_gateway::core::circuit_breaker=debug cargo run
```

### Rate Limiting Issues

```bash
# Check current limits for a client
curl -v http://localhost:9090/upp/v1/markets | grep X-RateLimit

# Response headers include:
# X-RateLimit-Remaining: 198
# X-RateLimit-Limit: 200
```

### Memory Usage

The in-memory caches (Moka, DashMap) grow with active markets. If memory is a concern:

- Reduce cache TTL via configuration
- Use Redis as the primary cache backend
- Monitor the `upp_price_index_markets_tracked` metric
