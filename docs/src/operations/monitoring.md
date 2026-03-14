# Monitoring & Observability

UPP provides comprehensive observability through Prometheus metrics, Grafana dashboards, Jaeger distributed tracing, and structured logging.

## Architecture

```
Gateway
  ├─ Prometheus metrics (pull-based)
  ├─ Jaeger tracing (push to agent)
  └─ Structured logs (stdout/files)
    ↓
Prometheus (collects metrics)
  ↓
Grafana (visualizes metrics)

Jaeger Agent (receives traces)
  ↓
Jaeger Collector (aggregates)
  ↓
Jaeger UI (visualizes traces)
```

## Prometheus Metrics

All metrics are automatically exposed at `/metrics`.

### Request Metrics

```
upp_api_requests_total{endpoint="/api/v1/markets", method="GET", status="200"}
upp_api_requests_duration_seconds{endpoint="/api/v1/markets", method="GET"}
upp_api_requests_active{endpoint="/api/v1/markets", method="GET"}
```

### Provider Metrics

```
upp_provider_requests_total{provider="polymarket", method="get_markets", status="success"}
upp_provider_requests_duration_seconds{provider="polymarket", method="get_markets"}
upp_provider_errors_total{provider="polymarket", error="rate_limited"}
upp_provider_cache_hits{provider="polymarket"}
upp_provider_cache_misses{provider="polymarket"}
```

### Cache Metrics

```
upp_cache_entries{cache="redis"}
upp_cache_memory_bytes{cache="redis"}
upp_cache_hit_ratio{cache="redis"}
upp_cache_ttl_seconds{cache="redis"}
```

### WebSocket Metrics

```
upp_websocket_connections_active
upp_websocket_subscriptions{channel="markets:polymarket"}
upp_websocket_messages_sent{channel="markets:polymarket"}
upp_websocket_messages_received
```

### System Metrics

```
upp_gateway_uptime_seconds
upp_gateway_version_info{version="0.1.0", commit="abc123"}
process_cpu_seconds_total
process_resident_memory_bytes
process_open_fds
```

### Query Examples

Get request rate over last 5 minutes:

```promql
rate(upp_api_requests_total[5m])
```

Get 95th percentile latency:

```promql
histogram_quantile(0.95, upp_api_requests_duration_seconds)
```

Get error rate:

```promql
rate(upp_api_requests_total{status!="200"}[5m]) /
rate(upp_api_requests_total[5m])
```

Cache hit ratio:

```promql
upp_cache_hit_ratio{cache="redis"}
```

## Grafana Dashboards

Dashboards are pre-configured in docker-compose and available at `http://localhost:3000`.

### Gateway Overview Dashboard

Key metrics:

- Request rate (req/s)
- Error rate (%)
- P50, P95, P99 latencies
- Active connections
- Cache hit ratio
- WebSocket subscriptions

### Provider Health Dashboard

Per-provider view:

- Availability (uptime %)
- Latency (ms)
- Error count and rate
- Cache hits vs. misses
- Request volume

### System Dashboard

Infrastructure metrics:

- CPU usage (%)
- Memory usage (%)
- Disk I/O
- Network I/O
- Open connections
- Goroutines (Rust tasks)

### Custom Dashboard Example

Create a dashboard to track trades per provider:

```json
{
  "dashboard": {
    "title": "Trade Activity",
    "panels": [
      {
        "title": "Orders Placed",
        "targets": [
          {
            "expr": "rate(upp_api_requests_total{endpoint=\"/api/v1/orders\", method=\"POST\"}[5m])"
          }
        ]
      },
      {
        "title": "Order Success Rate",
        "targets": [
          {
            "expr": "rate(upp_api_requests_total{endpoint=\"/api/v1/orders\", method=\"POST\", status=\"201\"}[5m]) / rate(upp_api_requests_total{endpoint=\"/api/v1/orders\", method=\"POST\"}[5m])"
          }
        ]
      }
    ]
  }
}
```

## Alerting Rules

Define alert rules in `config/prometheus_rules.yml`:

```yaml
groups:
- name: upp_alerts
  rules:
  - alert: GatewayDown
    expr: up{job="upp-gateway"} == 0
    for: 1m
    annotations:
      summary: "Gateway is down"

  - alert: HighErrorRate
    expr: |
      rate(upp_api_requests_total{status!="200"}[5m]) /
      rate(upp_api_requests_total[5m]) > 0.05
    for: 5m
    annotations:
      summary: "Error rate > 5%"

  - alert: HighLatency
    expr: |
      histogram_quantile(0.95, upp_api_requests_duration_seconds) > 1
    for: 5m
    annotations:
      summary: "P95 latency > 1s"

  - alert: RedisDown
    expr: redis_up == 0
    for: 1m
    annotations:
      summary: "Redis is down"

  - alert: ProviderDown
    expr: upp_provider_availability < 0.95
    for: 10m
    annotations:
      summary: "Provider availability < 95%"

  - alert: CacheFull
    expr: upp_cache_memory_bytes > 1000000000  # 1GB
    annotations:
      summary: "Cache memory > 1GB"

  - alert: TooManyWebSockets
    expr: upp_websocket_connections_active > 500
    annotations:
      summary: "Active WebSocket connections > 500"
```

### Alerting Channels

Configure notification destinations in Prometheus:

```yaml
alerting:
  alertmanagers:
  - static_configs:
    - targets:
      - alertmanager:9093

alertmanager_config:
  global:
    resolve_timeout: 5m
  route:
    receiver: default
  receivers:
  - name: default
    slack_configs:
    - api_url: https://hooks.slack.com/services/YOUR/WEBHOOK/URL
    email_configs:
    - to: ops@example.com
      from: alerting@example.com
      smarthost: smtp.example.com:587
```

## Jaeger Distributed Tracing

Access Jaeger UI at `http://localhost:16686`.

### Trace Structure

Each request creates a trace with spans:

```
GET /api/v1/markets
  ├─ [auth] Authenticate request (1ms)
  ├─ [cache] Check Redis cache (2ms)
  ├─ [adapter] Call Polymarket adapter (45ms)
  │   ├─ [auth] ECDSA sign request (2ms)
  │   ├─ [http] HTTP call to API (40ms)
  │   └─ [parse] Parse response (3ms)
  ├─ [cache_write] Store in Redis (1ms)
  └─ [serialize] Serialize response (2ms)
```

### Sampling Configuration

Control what percentage of traces are sampled:

```bash
export JAEGER_SAMPLER_TYPE=probabilistic
export JAEGER_SAMPLER_PARAM=0.1  # Sample 10% of traces
```

Or:

```bash
export JAEGER_SAMPLER_TYPE=const
export JAEGER_SAMPLER_PARAM=1  # Sample all traces
```

### Trace Queries

In Jaeger UI, query by:

- **Service**: upp-gateway
- **Operation**: get_markets, place_order, etc.
- **Tags**: provider=polymarket, endpoint=/api/v1/markets
- **Min Duration**: Find slow requests
- **Status**: Find errors

### Performance Analysis

Use traces to find bottlenecks:

1. Open Jaeger UI
2. Filter: Service = upp-gateway, Operation = place_order
3. Find slowest traces
4. Expand spans to see which step is slow
5. Optimize accordingly

Example findings:

- High adapter latency → check provider API or network
- High serialization latency → optimize response type
- High cache lookup latency → check Redis performance

## Structured Logging

Logs are written to stdout in JSON format for easy parsing:

```json
{
  "timestamp": "2026-03-14T12:34:56.789Z",
  "level": "INFO",
  "message": "Request completed",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "method": "GET",
  "path": "/api/v1/markets",
  "status": 200,
  "duration_ms": 45,
  "provider": "polymarket",
  "cached": false
}
```

### Log Levels

```bash
export RUST_LOG=debug    # Verbose, includes all details
export RUST_LOG=info     # Standard, normal operation
export RUST_LOG=warn     # Warnings and errors only
export RUST_LOG=error    # Errors only
```

### Parsing Logs

With `jq`:

```bash
# Get all errors
docker logs upp-gateway | jq 'select(.level == "ERROR")'

# Get slow requests (> 100ms)
docker logs upp-gateway | jq 'select(.duration_ms > 100)'

# Count by endpoint
docker logs upp-gateway | jq -s 'group_by(.path) | map({path: .[0].path, count: length})'
```

### Centralized Logging

Send logs to ELK stack or Loki:

```bash
# With docker-compose
services:
  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"

  gateway:
    logging:
      driver: loki
      options:
        loki-url: http://localhost:3100/loki/api/v1/push
        loki-batch-size: "100"
```

## Health Checks

### Endpoint

```bash
curl http://localhost:8080/api/v1/health
```

Response:

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "providers": {
    "polymarket": {"status": "up", "latency_ms": 45},
    "kalshi": {"status": "up", "latency_ms": 62},
    "opinion_trade": {"status": "down", "latency_ms": 0}
  },
  "uptime_seconds": 86400,
  "cache": {"entries": 234, "memory_mb": 12.4}
}
```

### Status Codes

- **200** — All healthy
- **202** — Degraded (some providers down)
- **503** — Unavailable (critical issues)

## Metrics Collection

### Prometheus Scrape Config

```yaml
scrape_configs:
  - job_name: 'upp'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Multi-Instance Setup

```yaml
scrape_configs:
  - job_name: 'upp'
    consul_sd_configs:
      - server: 'consul:8500'
        services: ['upp']
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
```

## Dashboard Templates

### Error Rate Alert

```
Alert when error rate > 5% for 5 minutes
```

### Latency Alert

```
Alert when P95 latency > 1 second for 5 minutes
```

### Availability Alert

```
Alert when provider availability < 95% for 10 minutes
```

## Troubleshooting

### No Metrics

1. Check if metrics endpoint is accessible: `curl http://localhost:8080/metrics`
2. Verify Prometheus is scraping: http://localhost:9090/targets
3. Check logs for errors: `docker logs upp-gateway`

### Jaeger Traces Not Appearing

1. Verify Jaeger agent is running: `curl http://localhost:6831 > /dev/null && echo "up"`
2. Check gateway logs for trace errors
3. Ensure `JAEGER_SAMPLER_PARAM` is not 0

### Missing Dashboards

1. Verify Grafana is running: http://localhost:3000
2. Add Prometheus data source: Configuration → Data Sources
3. Import dashboard JSON from repo: `dashboards/*.json`

## Best Practices

1. **Set up alerts** — Catch issues before users report them
2. **Monitor all layers** — Gateway, Redis, providers, system
3. **Trace slow requests** — Use Jaeger to identify bottlenecks
4. **Aggregate logs** — Centralize for searchability
5. **Test alerting** — Verify notifications are sent
6. **Review metrics regularly** — Spot trends early
7. **Set reasonable thresholds** — Avoid alert fatigue
