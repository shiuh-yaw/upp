# UPP Gateway Observability Guide

This document describes the comprehensive observability setup for the UPP Gateway, including distributed tracing, metrics collection, and health checks.

## Architecture Overview

The observability stack consists of:

1. **Tracing**: OpenTelemetry (OTLP) → Jaeger
2. **Metrics**: Prometheus (collection) + Grafana (visualization)
3. **Health Checks**: Liveness & readiness checks via HTTP endpoints
4. **Logging**: Structured JSON logs via `tracing-subscriber`

## Quick Start

### Start the Full Monitoring Stack

```bash
docker compose --profile monitoring up
```

This brings up:
- Redis (cache)
- Gateway (REST + gRPC)
- Prometheus (metrics collection) — http://localhost:9090
- Grafana (dashboards) — http://localhost:3000
- Jaeger (distributed tracing) — http://localhost:16686

### Start Just Redis + Gateway

```bash
docker compose up -d redis
cd gateway && cargo run
```

Then:
- Gateway REST API: http://localhost:8080
- Gateway gRPC: localhost:50051
- Metrics: http://localhost:8080/metrics
- Health (liveness): http://localhost:8080/health/live
- Health (readiness): http://localhost:8080/health/ready

## Components

### 1. Tracing (`gateway/src/core/observability.rs`)

The `init_tracing()` function initializes the tracing system with:

#### Configuration

```rust
let config = TracingConfig {
    format: "json".to_string(),  // "json" or "pretty"
    level: "info".to_string(),
    enable_otlp: true,
    otlp_endpoint: "http://jaeger:4317".to_string(),
    module_levels: Some("upp_gateway=debug,tower_http=info".to_string()),
};

observability::init_tracing(&config)?;
```

#### Features

- **JSON Formatting**: Structured logs for log aggregation systems
- **Pretty Formatting**: Human-readable output for local development
- **Span Events**: Automatic capture of span entry/exit
- **Module-Level Control**: Fine-grained log level configuration
- **OTLP Export**: Send traces to Jaeger for distributed tracing visualization

#### Usage in Code

```rust
use tracing::{info, warn, debug, error, span, Level};

// Simple log
info!("Gateway started on 0.0.0.0:8080");

// With structured context
warn!(path = "/predict", status = 503, "Provider error");

// Span for request handling
let span = span!(Level::INFO, "http_request", method = "GET", path = "/health");
let _guard = span.enter();
// ... request logic
// Span exits automatically when _guard is dropped
```

### 2. Prometheus Metrics

The `PrometheusMetrics` struct provides comprehensive metric collection:

#### Key Metrics

| Metric | Type | Labels | Purpose |
|--------|------|--------|---------|
| `requests_total` | Counter | `method`, `path`, `status` | Total HTTP requests |
| `request_duration_seconds` | Histogram | `method`, `path`, `status` | Request latency distribution |
| `ws_messages_total` | Counter | — | Total WebSocket messages |
| `provider_requests_total` | Counter | `provider` | Requests to each provider |
| `active_ws_connections` | Gauge | — | Current WebSocket connections |
| `cache_size` | Gauge | — | Cache size in bytes |
| `connected_providers` | Gauge | — | Number of healthy providers |

#### Usage in Code

```rust
let metrics = Arc::new(PrometheusMetrics::new());

// Record HTTP request
metrics.record_request("GET", "/predict", 200, 45.5); // 45.5 ms

// WebSocket tracking
metrics.increment_ws_connections();
metrics.record_ws_message();
metrics.decrement_ws_connections();

// Provider tracking
metrics.record_provider_request("alphavantage");
metrics.set_connected_providers(3);

// Cache metrics
metrics.set_cache_size(1024 * 1024 * 50); // 50 MB
```

#### Prometheus Endpoint

```
GET /metrics HTTP/1.1

# Example output:
# HELP requests_total Total HTTP requests
# TYPE requests_total counter
requests_total{method="GET",path="/health",status="200"} 1523
requests_total{method="GET",path="/predict",status="200"} 342
requests_total{method="GET",path="/predict",status="503"} 12
```

### 3. Grafana Dashboards

The UPP Gateway dashboard (`config/grafana/dashboards/upp-gateway.json`) includes:

#### Panels

1. **Request Rate** (line chart)
   - Requests per second by endpoint and status
   - Identifies traffic patterns and anomalies

2. **Response Latency** (line chart)
   - P50, P90, P99 percentiles
   - Tracks performance degradation

3. **Active WebSocket Connections** (stat)
   - Current connected clients
   - Capacity planning indicator

4. **Connected Providers** (stat)
   - Health of provider integrations
   - Alerts when count reaches 0

5. **Cache Size** (stat)
   - Memory usage of cache
   - Identifies potential OOM issues

6. **Error Rate** (bar chart)
   - 5xx responses per endpoint
   - Critical for SLO tracking

7. **Provider Request Rate** (line chart)
   - Load distribution across providers
   - Identifies bottlenecks

8. **WebSocket Message Rate** (line chart)
   - Real-time client activity
   - Engagement metrics

9. **Status Code Distribution** (pie chart)
   - Overall success/error/other breakdown
   - Visual health indicator

#### Auto-Provisioning

Grafana automatically:
1. Provisions Prometheus datasource
2. Registers the UPP Gateway dashboard
3. Creates the "UPP" folder for organization

Access at: http://localhost:3000 (admin/admin)

### 4. Health Checks

Two HTTP endpoints provide dependency health status:

#### Liveness Check

```
GET /health/live HTTP/1.1

200 OK
{
  "status": "healthy",
  "checks": {
    "redis": {"status": "ok", "message": "Gateway is running"},
    "providers": {"status": "ok", "message": "Providers service available"},
    "cache": {"status": "ok", "message": "Cache service available"}
  },
  "timestamp": "2026-03-14T12:30:45Z"
}
```

Use for: Load balancer health checks, basic liveness verification.

#### Readiness Check

```
GET /health/ready HTTP/1.1

200 OK (or 503 if dependencies unavailable)
{
  "status": "healthy|degraded|unhealthy",
  "checks": {
    "redis": {
      "status": "ok|warning|error",
      "message": "Redis connection available",
      "latency_ms": 1.5
    },
    "providers": {
      "status": "ok|warning|error",
      "message": "3 providers connected"
    },
    "cache": {
      "status": "ok|warning|error",
      "message": "Cache size: 52428800 bytes"
    }
  },
  "timestamp": "2026-03-14T12:30:45Z"
}
```

Use for: Traffic admission control, graceful shutdown, deployment orchestration.

#### Implementation

```rust
use crate::core::observability::{HealthCheck, PrometheusMetrics};

let metrics = Arc::new(PrometheusMetrics::new());
let health = HealthCheck::new(metrics.clone());

// In your Axum router:
router
    .route("/health/live", get(|| async {
        Json(health.liveness())
    }))
    .route("/health/ready", get(|| async {
        let status = health.readiness();
        let code = if status.status == "healthy" {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        (code, Json(status))
    }))
```

## Prometheus Recording Rules

Pre-computed metrics reduce dashboard load times:

| Rule | Expression | Purpose |
|------|-----------|---------|
| `upp:requests:rate1m` | Request rate per endpoint | Traffic monitoring |
| `upp:requests:success_rate` | 2xx / total % | SLO tracking |
| `upp:requests:error_rate` | 5xx / total % | Error SLO tracking |
| `upp:latency:p99` | 99th percentile latency | Performance SLO |
| `upp:latency:p95` | 95th percentile latency | Performance tracking |
| `upp:latency:p50` | Median latency | Baseline performance |

### Alerting Rules

Alerts fire automatically:

1. **High Error Rate**: Error rate > 5% for 1 minute
2. **Slow Responses**: P99 latency > 1 second for 2 minutes
3. **WebSocket Spike**: New connections > 10/sec for 1 minute
4. **Provider Outage**: No providers connected for 2 minutes

View alerts in Prometheus: http://localhost:9090/alerts

## Environment Variables

Configure observability via environment variables:

```bash
# Logging
RUST_LOG=upp_gateway=debug,tower_http=info
UPP_LOG_FORMAT=json          # "json" or "pretty"
UPP_LOG_LEVEL=info           # "debug", "info", "warn", "error"

# Tracing/OpenTelemetry
OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317
OTEL_EXPORTER_OTLP_TIMEOUT=10000  # ms
```

## Performance Considerations

### Memory Footprint

- **Metrics Storage**: ~5 MB for 1000 sample metrics
- **Trace Buffer**: ~50 MB for 10k spans (in-memory)
- **Log Buffer**: ~10 MB per million log lines

### Query Performance

- **Prometheus Scrape**: 5-10s per gateway instance
- **Grafana Dashboard**: Sub-second rendering for 6-hour range
- **Jaeger Trace Search**: 100-500ms for recent traces

### Optimization Tips

1. **Increase Scrape Intervals**: For high-cardinality metrics, increase `scrape_interval` beyond 10s
2. **Metric Cardinality**: Limit unique label combinations (avoid unbounded path labels)
3. **Retention**: Prometheus default 15 days; adjust in docker-compose
4. **Log Sampling**: In production, consider sampling high-volume logs

## Troubleshooting

### No metrics appearing in Prometheus

1. Check `/metrics` endpoint manually:
   ```bash
   curl http://localhost:8080/metrics
   ```

2. Verify Prometheus targets:
   - Visit http://localhost:9090/targets
   - Check "State" is "UP"

3. Check Prometheus logs:
   ```bash
   docker logs upp-prometheus
   ```

### Grafana dashboard shows "No data"

1. Verify datasource:
   - Settings → Data Sources → Prometheus
   - Test connection

2. Check time range:
   - Dashboard top-right time picker
   - Must have metrics older than now

3. Check metric names:
   - In Prometheus: http://localhost:9090/graph
   - Type metric name and execute query

### Jaeger shows no traces

1. Verify `OTEL_EXPORTER_OTLP_ENDPOINT` is correct
2. Check gateway logs for OTLP export errors
3. Verify Jaeger receiver is enabled (should be by default)

### High cardinality warning in Prometheus

If Prometheus memory usage spikes:

1. Identify high-cardinality metrics:
   ```
   topk(10, count by (__name__) (count by (__name__, job, instance, le, quantile) ({__name__=~".+"})))
   ```

2. Add label restrictions in `prometheus.yml`:
   ```yaml
   metric_relabel_configs:
     - source_labels: [__name__]
       regex: "request_duration_seconds"
       action: keep
   ```

## Integration Examples

### Health Check in Kubernetes

```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 5
```

### Metrics Scrape in Kubernetes

```yaml
---
apiVersion: v1
kind: ServiceMonitor
metadata:
  name: upp-gateway
spec:
  selector:
    matchLabels:
      app: upp-gateway
  endpoints:
    - port: metrics
      interval: 10s
```

### Alert Notification

In Prometheus AlertManager config:

```yaml
global:
  slack_api_url: "https://hooks.slack.com/services/..."

route:
  receiver: slack

receivers:
  - name: slack
    slack_configs:
      - channel: "#alerts"
        title: "UPP Gateway Alert"
```

## Next Steps

1. Customize dashboard panels for your use case
2. Define SLO thresholds for your environment
3. Configure alert receivers (Slack, PagerDuty, etc.)
4. Set up log aggregation (ELK, Loki, etc.)
5. Implement distributed tracing context propagation in providers
