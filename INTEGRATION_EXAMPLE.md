# UPP Gateway Observability Integration Example

This document shows practical examples of integrating the observability module into your gateway code.

## 1. Initialize Tracing at Startup

Add this to `gateway/src/main.rs`:

```rust
use crate::core::observability::{TracingConfig, init_tracing};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing before anything else
    let tracing_config = TracingConfig {
        format: std::env::var("UPP_LOG_FORMAT")
            .unwrap_or_else(|_| "json".to_string()),
        level: std::env::var("UPP_LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string()),
        enable_otlp: std::env::var("OTEL_ENABLED")
            .unwrap_or_else(|_| "true".to_string()) == "true",
        otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string()),
        module_levels: std::env::var("RUST_LOG").ok(),
    };

    init_tracing(&tracing_config)?;

    info!("UPP Gateway starting...");

    // Continue with rest of startup...
}
```

## 2. Create Metrics in AppState

Update the `AppState` struct:

```rust
use crate::core::observability::PrometheusMetrics;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<ProviderRegistry>,
    pub cache: Arc<MarketCache>,
    pub ws_manager: Arc<WebSocketManager>,
    pub metrics: Arc<PrometheusMetrics>,  // Add this
    // ... other fields
}

// In main() when creating AppState:
let metrics = Arc::new(PrometheusMetrics::new());
let app_state = AppState {
    registry: Arc::new(registry),
    cache: Arc::new(cache),
    ws_manager: Arc::new(ws_manager),
    metrics: metrics.clone(),
    // ...
};
```

## 3. Add Metrics Endpoints to Router

Add these routes to your Axum router:

```rust
use axum::response::IntoResponse;
use crate::core::observability::HealthCheck;

async fn metrics_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    state.metrics.metrics_handler()
}

async fn health_live(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let health = HealthCheck::new(state.metrics.clone());
    Json(health.liveness())
}

async fn health_ready(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let health = HealthCheck::new(state.metrics.clone());
    let status = health.readiness();
    let code = if status.status == "healthy" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(status))
}

// Add to router:
let app = app
    .route("/metrics", get(metrics_handler))
    .route("/health/live", get(health_live))
    .route("/health/ready", get(health_ready))
    .with_state(app_state);
```

## 4. Record HTTP Request Metrics

Create middleware to record all requests. Add to `gateway/src/middleware/`:

**File**: `gateway/src/middleware/metrics.rs`

```rust
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::info;
use crate::core::observability::PrometheusMetrics;
use std::sync::Arc;

pub async fn metrics_middleware(
    State(metrics): State<Arc<PrometheusMetrics>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    let status = response.status().as_u16();

    // Record the metric
    metrics.record_request(&method, &path, status, duration_ms);

    // Log at appropriate level
    if status >= 500 {
        warn!(
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            "Request failed"
        );
    } else if duration_ms > 1000.0 {
        info!(
            method = %method,
            path = %path,
            status = status,
            duration_ms = duration_ms,
            "Slow request detected"
        );
    }

    response
}
```

Add middleware to router:

```rust
use axum::middleware;
use crate::middleware::metrics::metrics_middleware;

let app = app
    .layer(middleware::from_fn_with_state(
        app_state.metrics.clone(),
        metrics_middleware,
    ))
    // ... other middleware
```

## 5. Track WebSocket Connections

In your WebSocket handler:

```rust
use crate::core::observability::PrometheusMetrics;

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        // Increment connection count
        state.metrics.increment_ws_connections();

        info!("WebSocket connection opened");

        // Handle connection...
        let (mut sender, mut receiver) = socket.split();

        while let Some(msg) = receiver.next().await {
            // Record message
            state.metrics.record_ws_message();

            // Process message...
        }

        // Decrement connection count
        state.metrics.decrement_ws_connections();
        info!("WebSocket connection closed");
    })
}
```

## 6. Track Provider Requests

When making requests to prediction providers:

```rust
use tracing::instrument;

#[instrument(skip(state), fields(provider = %provider_name))]
async fn fetch_from_provider(
    state: &AppState,
    provider_name: &str,
    request: PredictionRequest,
) -> Result<PredictionResponse> {
    let start = Instant::now();

    info!("Fetching from provider");

    // Make request to provider
    let response = make_provider_request(provider_name, &request).await?;

    // Record metric
    state.metrics.record_provider_request(provider_name);

    debug!(
        provider = provider_name,
        duration_ms = start.elapsed().as_secs_f64() * 1000.0,
        "Provider request completed"
    );

    Ok(response)
}
```

## 7. Update Cache Metrics

After cache operations:

```rust
use crate::core::observability::PrometheusMetrics;

pub async fn update_cache_metrics(
    state: &AppState,
) {
    // Get cache size from moka cache
    let cache_size = state.cache.weighted_size();

    // Get connected providers
    let provider_count = state.registry.connected_providers().len();

    // Update metrics
    state.metrics.set_cache_size(cache_size);
    state.metrics.set_connected_providers(provider_count as u64);
}
```

Call this periodically in a background task:

```rust
// In main() after creating AppState:
let state_clone = app_state.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        interval.tick().await;
        update_cache_metrics(&state_clone).await;
    }
});
```

## 8. Add Structured Logging Throughout

Examples in different parts of your code:

**In registry**:
```rust
use tracing::info;

impl ProviderRegistry {
    pub fn register_provider(&mut self, provider: Provider) -> Result<()> {
        info!(
            provider_id = %provider.id,
            endpoint = %provider.endpoint,
            "Registering new provider"
        );
        // ...
    }
}
```

**In cache layer**:
```rust
use tracing::debug;

impl MarketCache {
    pub async fn get(&self, key: &str) -> Option<CacheEntry> {
        debug!(key = %key, "Cache lookup");
        // ...
    }

    pub async fn set(&self, key: String, value: CacheEntry) {
        debug!(
            key = %key,
            ttl_secs = value.ttl,
            "Cache insert"
        );
        // ...
    }
}
```

**In request handling**:
```rust
use tracing::{info, warn, error, Span};

#[instrument(skip(state), fields(request_id = %uuid::Uuid::new_v4()))]
async fn handle_prediction(
    State(state): State<AppState>,
    Json(request): Json<PredictionRequest>,
) -> Result<Json<PredictionResponse>> {
    info!("Received prediction request");

    // Check cache
    if let Some(cached) = state.cache.get(&request.market).await {
        debug!("Cache hit");
        return Ok(Json(cached));
    }

    // Fetch from providers
    let result = match fetch_from_provider(&state, &request).await {
        Ok(response) => {
            info!("Prediction successful");
            response
        }
        Err(e) => {
            error!(error = %e, "Prediction failed");
            return Err(e);
        }
    };

    // Store in cache
    state.cache.set(request.market.clone(), result.clone()).await;

    Ok(Json(result))
}
```

## 9. Configuration via Environment

Set up your `.env` file:

```bash
# Logging
UPP_LOG_FORMAT=json
UPP_LOG_LEVEL=info
RUST_LOG=upp_gateway=debug,tower_http=info,axum=info

# OpenTelemetry / Jaeger
OTEL_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Gateway
UPP_HOST=0.0.0.0
UPP_PORT=8080
UPP_REDIS_URL=redis://localhost:6379
```

## 10. Running with Full Observability

Start the stack:

```bash
# Start monitoring services
docker compose --profile monitoring up -d

# Run gateway with observability
cd gateway
RUST_LOG=upp_gateway=debug,tower_http=info cargo run
```

Test the endpoints:

```bash
# Metrics
curl http://localhost:8080/metrics

# Health checks
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready

# Gateway API
curl http://localhost:8080/predict -X POST -H "Content-Type: application/json" \
  -d '{"market": "BTC/USD"}'

# View in Grafana
open http://localhost:3000
# Login: admin/admin
# Dashboard: UPP Gateway Dashboard

# View traces in Jaeger
open http://localhost:16686
# Search: Service = upp-gateway
```

## 11. Advanced: Span Attributes

Add custom attributes to spans:

```rust
use tracing::{instrument, Span};

#[instrument(
    skip(state),
    fields(
        provider_id = %request.provider,
        market = %request.market,
        cache_hit = false,
    )
)]
async fn predict(
    State(state): State<AppState>,
    Json(request): Json<PredictionRequest>,
) -> Result<Json<PredictionResponse>> {
    let span = Span::current();

    // Update span field
    if let Some(cached) = state.cache.get(&request.market).await {
        span.record("cache_hit", true);
        return Ok(Json(cached));
    }

    // ... continue with prediction
}
```

## 12. Testing Observability

Unit test metrics:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = PrometheusMetrics::new();

        metrics.record_request("GET", "/health", 200, 5.0);
        assert_eq!(metrics.requests_total.load(Ordering::Relaxed), 1);

        metrics.increment_ws_connections();
        assert_eq!(metrics.active_ws_connections.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_metrics_handler() {
        let metrics = Arc::new(PrometheusMetrics::new());
        metrics.record_request("POST", "/predict", 200, 100.0);

        let output = metrics.metrics_handler();
        assert!(output.contains("requests_total"));
        assert!(output.contains("request_duration_seconds"));
    }
}
```

## Summary

Integration checklist:

- [ ] Initialize tracing in `main.rs`
- [ ] Add `PrometheusMetrics` to `AppState`
- [ ] Add `/metrics`, `/health/live`, `/health/ready` routes
- [ ] Add metrics middleware for all requests
- [ ] Track WebSocket connections
- [ ] Record provider request metrics
- [ ] Update cache metrics periodically
- [ ] Add structured logging throughout codebase
- [ ] Configure environment variables
- [ ] Start monitoring stack with `docker compose --profile monitoring up`
- [ ] Verify metrics endpoint returns data
- [ ] Verify Grafana dashboard shows data
- [ ] Test health check endpoints

Once complete, you'll have full observability into your UPP Gateway!
