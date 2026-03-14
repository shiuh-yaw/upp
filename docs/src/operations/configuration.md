# Configuration Guide

Complete reference for configuring UPP for different environments and requirements.

## Configuration Sources

UPP reads configuration from (in order of precedence):

1. **Environment variables** (highest priority)
2. **Command-line flags**
3. **Config file** (~/.upp/config.toml or ./config/upp.toml)
4. **Defaults** (lowest priority)

## Environment Variables

### Core Configuration

```bash
# Server
SERVER_HOST=0.0.0.0              # Bind address
SERVER_PORT=8080                 # REST port
GRPC_PORT=50051                  # gRPC port

# Redis
REDIS_URL=redis://localhost:6379  # Connection string
REDIS_USERNAME=                   # Optional auth username
REDIS_PASSWORD=                   # Optional auth password
REDIS_POOL_SIZE=10                # Connection pool size
REDIS_TIMEOUT_SECS=30             # Connection timeout

# Logging
RUST_LOG=info                     # Log level: debug, info, warn, error
LOG_FORMAT=json                   # json or text
LOG_OUTPUT=stdout                 # stdout, file, or both
LOG_FILE=/var/log/upp.log        # If LOG_OUTPUT includes file

# Tracing (Jaeger)
JAEGER_AGENT_HOST=localhost      # Agent host
JAEGER_AGENT_PORT=6831           # Agent port
JAEGER_SAMPLER_TYPE=const        # const, probabilistic, rate_limiting
JAEGER_SAMPLER_PARAM=1           # 0-1 for probabilistic

# Metrics (Prometheus)
PROMETHEUS_ADDR=http://localhost:9090
METRICS_PORT=9090
METRICS_PATH=/metrics
```

### Cache Configuration

```bash
CACHE_TTL_SECONDS=300            # Default TTL
CACHE_MAX_SIZE=1000              # Max entries
CACHE_MAX_MEMORY_MB=512          # Max memory

# Per-endpoint TTLs
CACHE_TTL_MARKETS=60
CACHE_TTL_ORDERS=10
CACHE_TTL_PORTFOLIO=30
CACHE_TTL_HEALTH=5
CACHE_TTL_BACKTEST=31536000      # 1 year (never expires)
```

### Rate Limiting

```bash
# Per-client (per IP address)
RATE_LIMIT_PER_SECOND=10         # Requests per second
RATE_LIMIT_BURST=20              # Burst capacity

# Per-provider (global)
PROVIDER_RATE_LIMIT_PER_SECOND=100
PROVIDER_RATE_LIMIT_BURST=200

# WebSocket
WS_RATE_LIMIT_PER_SECOND=5
WS_MAX_CONNECTIONS=1000
WS_MESSAGE_QUEUE_SIZE=100
```

### Connection Pooling

```bash
HTTP_POOL_SIZE=50                # HTTP connections
HTTP_TIMEOUT_SECS=30             # Request timeout
HTTP_KEEPALIVE_SECS=30           # Keep-alive duration

GRPC_POOL_SIZE=10                # gRPC connections
```

### Provider Configuration

#### Kalshi

```bash
KALSHI_BASE_URL=https://api.kalshi.com
KALSHI_API_KEY=your_api_key
KALSHI_API_SECRET=your_api_secret
KALSHI_TIMEOUT_SECS=30
KALSHI_RATE_LIMIT_PER_SEC=100
```

#### Polymarket

```bash
POLYMARKET_BASE_URL=https://clob.polymarket.com
POLYMARKET_PRIVATE_KEY=0x...          # ECDSA private key (hex)
POLYMARKET_TIMEOUT_SECS=30
POLYMARKET_RATE_LIMIT_PER_SEC=100
```

#### Opinion.trade

```bash
OPINION_TRADE_BASE_URL=https://api.opinion.trade
OPINION_TRADE_API_KEY=your_api_key
OPINION_TRADE_TIMEOUT_SECS=30
OPINION_TRADE_RATE_LIMIT_PER_SEC=50
```

### WebSocket Configuration

```bash
WS_UPDATE_INTERVAL_MS=5000            # Market update frequency
WS_HEARTBEAT_INTERVAL_SECS=30         # Ping interval
WS_CONNECTION_TIMEOUT_SECS=30
WS_BACKPRESSURE_THRESHOLD=1000        # Message queue threshold
```

## Config File Format

Create `config/upp.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080
grpc_port = 50051

[redis]
url = "redis://localhost:6379"
pool_size = 10
timeout_secs = 30

[logging]
level = "info"
format = "json"
output = "stdout"

[cache]
ttl_seconds = 300
max_entries = 1000
max_memory_mb = 512

[rate_limiting]
per_client_qps = 10
per_client_burst = 20
per_provider_qps = 100
per_provider_burst = 200

[websocket]
max_connections = 1000
message_queue_size = 100
update_interval_ms = 5000

[providers.kalshi]
base_url = "https://api.kalshi.com"
api_key = "${KALSHI_API_KEY}"  # Read from env
api_secret = "${KALSHI_API_SECRET}"
timeout_secs = 30

[providers.polymarket]
base_url = "https://clob.polymarket.com"
private_key = "${POLYMARKET_PRIVATE_KEY}"
timeout_secs = 30

[providers.opinion_trade]
base_url = "https://api.opinion.trade"
api_key = "${OPINION_TRADE_API_KEY}"
timeout_secs = 30

[tracing]
jaeger_agent_host = "localhost"
jaeger_agent_port = 6831
sampler_type = "const"
sampler_param = 1.0

[metrics]
prometheus_addr = "http://localhost:9090"
port = 9090
path = "/metrics"
```

## Environment-Specific Configs

### Development

```bash
# config/dev.toml
[logging]
level = "debug"

[cache]
ttl_seconds = 60

[rate_limiting]
per_client_qps = 1000  # Relaxed for testing
```

Load with:

```bash
export UPP_CONFIG_FILE=config/dev.toml
./gateway
```

### Staging

```bash
# config/staging.toml
[server]
host = "0.0.0.0"

[logging]
level = "info"

[cache]
ttl_seconds = 300
max_memory_mb = 256

[rate_limiting]
per_client_qps = 50
per_client_burst = 100
```

### Production

```bash
# config/prod.toml
[server]
host = "0.0.0.0"

[logging]
level = "warn"
output = "file"
file = "/var/log/upp.log"

[cache]
ttl_seconds = 600
max_memory_mb = 1024

[rate_limiting]
per_client_qps = 10
per_client_burst = 20

[redis]
url = "redis://redis-cluster:6379"
pool_size = 50

[tracing]
sampler_type = "probabilistic"
sampler_param = 0.1  # Sample 10% in production
```

## Secrets Management

### Environment Variables

```bash
# Never commit API keys!
export KALSHI_API_KEY=$(aws secretsmanager get-secret-value --secret-id kalshi-key --query SecretString --output text)
export POLYMARKET_PRIVATE_KEY=$(aws secretsmanager get-secret-value --secret-id polymarket-key --query SecretString --output text)
```

### Docker Secrets

```bash
# Create secrets
echo "your_api_key" | docker secret create kalshi_key -

# Use in compose
services:
  gateway:
    secrets:
      - kalshi_key
      - polymarket_key
    environment:
      KALSHI_API_KEY_FILE: /run/secrets/kalshi_key
```

### Kubernetes Secrets

```bash
kubectl create secret generic upp-secrets \
  --from-literal=kalshi-key=... \
  --from-literal=polymarket-key=... \
  -n upp
```

Reference in deployment:

```yaml
env:
- name: KALSHI_API_KEY
  valueFrom:
    secretKeyRef:
      name: upp-secrets
      key: kalshi-key
```

## Performance Tuning

### For High Throughput

```toml
[redis]
pool_size = 100

[rate_limiting]
per_client_qps = 50
per_client_burst = 100
per_provider_qps = 200
per_provider_burst = 400

[cache]
max_entries = 10000
max_memory_mb = 2048
ttl_seconds = 600

[websocket]
max_connections = 5000
update_interval_ms = 2000
```

### For Low Latency

```toml
[http]
keepalive_secs = 60
pool_size = 50

[cache]
ttl_seconds = 300  # Shorter TTL for fresher data

[tracing]
sampler_type = "const"
sampler_param = 0.01  # Sample 1% to reduce overhead

[logging]
level = "warn"  # Less logging = faster
```

### For Low Memory

```toml
[redis]
pool_size = 5

[cache]
max_entries = 100
max_memory_mb = 64
ttl_seconds = 60

[websocket]
max_connections = 100
message_queue_size = 10
```

## Validation

Check configuration validity:

```bash
# Test config without starting
./gateway --config config/prod.toml --validate

# Output
✓ Configuration valid
✓ Redis connection OK
✓ All providers configured
✓ Jaeger agent reachable
```

## Hot Reload

Reload configuration without restarting (when supported):

```bash
# Send SIGHUP to process
kill -HUP <pid>

# Check logs
docker logs gateway | grep "Configuration reloaded"
```

Reloadable configs:
- Log level
- Cache TTLs
- Rate limit thresholds
- Feature flags

Non-reloadable (require restart):
- Server port
- Redis connection
- Tracing settings

## Migration Guide

### Upgrading v0.1 to v0.2

Change in config:

```bash
# Old (v0.1)
CACHE_ENABLED=true

# New (v0.2)
CACHE_TTL_SECONDS=300  # Set to 0 to disable
```

### Environment Variable Renames

```bash
# Old → New
LOG_LEVEL → RUST_LOG
REDIS_HOST → REDIS_URL
REDIS_PORT (removed, use REDIS_URL)
```

## Reference

### All Environment Variables

```
SERVER_*                    Server configuration
REDIS_*                     Redis connection
RUST_LOG                    Logging
LOG_*                       Logging output
CACHE_*                     Caching
RATE_LIMIT_*               Rate limiting
PROVIDER_*                  Provider-specific
WS_*                        WebSocket
HTTP_*                      HTTP pooling
GRPC_*                      gRPC
JAEGER_*                    Tracing
PROMETHEUS_*               Metrics
KALSHI_*                    Kalshi provider
POLYMARKET_*               Polymarket provider
OPINION_*                  Opinion.trade provider
```

See full list in `config/defaults.toml` in repository.

## Debugging Configuration

### Log actual configuration at startup

```bash
RUST_LOG=debug ./gateway 2>&1 | grep -i config
```

### Test Redis connection

```bash
redis-cli -u $REDIS_URL ping
```

### Test provider authentication

```bash
curl -H "Authorization: Bearer $KALSHI_API_KEY" \
  https://api.kalshi.com/health
```

### Validate Jaeger connectivity

```bash
nc -zv $JAEGER_AGENT_HOST $JAEGER_AGENT_PORT
```
