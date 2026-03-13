# Production Hardening Integration Guide

This document explains how to integrate the hardening features from `src/core/hardening.rs` into your gateway handlers and provider calls.

## Overview

Feature 6 introduces the following production-ready components:

1. **Structured Error Types** (`GatewayError`) — Replace ad-hoc error handling with typed errors
2. **Circuit Breaker** — Per-provider protection against cascading failures
3. **Retry with Exponential Backoff** — Automatic retry with jitter for transient failures
4. **Request Timeout Middleware** — Prevent hung requests
5. **Graceful Shutdown** — Clean shutdown with signal handling
6. **Configuration Validation** — Validate all config at startup

## 1. Structured Error Handling

Replace manual error creation with typed errors:

### Before
```rust
fn internal_error(e: &anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
    warn!("Internal error: {:#}", e);
    (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INTERNAL", &e.to_string())))
}
```

### After
```rust
use crate::core::hardening::GatewayError;

// In handlers, return GatewayError which implements IntoResponse
async fn get_market(Path(market_id): Path<String>) -> Result<Json<Market>, GatewayError> {
    let market = registry.get(&market_id)
        .ok_or_else(|| GatewayError::not_found(format!("Market {} not found", market_id)))?;
    Ok(Json(market))
}
```

## 2. Circuit Breaker Integration

### Setup in main.rs
```rust
use crate::core::hardening::{CircuitBreakerRegistry, CircuitBreakerConfig};

// In main()
let circuit_breakers = Arc::new(CircuitBreakerRegistry::new(CircuitBreakerConfig::default()));

// Add to AppState
pub struct AppState {
    pub circuit_breakers: Arc<CircuitBreakerRegistry>,
    // ... other fields
}
```

### Usage in Handler
```rust
use crate::core::hardening::GatewayError;

async fn list_markets(
    State(state): State<AppState>,
    Query(query): Query<ListMarketsQuery>,
) -> Result<Json<AggregatedMarkets>, GatewayError> {
    // Check circuit breaker for Kalshi provider
    let cb = state.circuit_breakers.get_or_create("kalshi");
    cb.check()?; // Returns Err if circuit is open

    // Make provider call
    match make_kalshi_request().await {
        Ok(result) => {
            cb.record_success();
            Ok(Json(result))
        }
        Err(e) => {
            cb.record_failure();
            Err(GatewayError::provider_error("kalshi".to_string(), e.to_string()))
        }
    }
}
```

## 3. Retry with Exponential Backoff

### Default Configuration
- Max retries: 3
- Base delay: 100ms
- Max delay: 5s
- Jitter: enabled (±20%)

### Usage Example
```rust
use crate::core::hardening::{retry_with_backoff, RetryConfig};

async fn fetch_markets(provider_id: &str) -> Result<Vec<Market>, GatewayError> {
    let config = RetryConfig::default();

    retry_with_backoff(config, || async {
        provider_adapter.list_markets().await
            .map_err(|e| GatewayError::provider_error(provider_id.to_string(), e.to_string()))
    }).await
}
```

### Custom Configuration
```rust
let config = RetryConfig {
    max_retries: 5,
    base_delay: Duration::from_millis(50),
    max_delay: Duration::from_secs(10),
    jitter: true,
};
```

## 4. Request Timeout Middleware

### Setup in Router
```rust
use crate::core::hardening::{timeout_middleware, TimeoutConfig};
use tower::Layer;

// Create timeout middleware
let timeout_config = TimeoutConfig::default();

// Wrap router with timeout middleware
let app = Router::new()
    // ... routes
    .layer(tower::ServiceBuilder::new()
        .layer(axum::middleware::from_fn(move |req, next| {
            timeout_middleware(timeout_config.clone(), req, next)
        }))
    );
```

### Configuration
```rust
pub struct TimeoutConfig {
    pub rest_timeout: Duration,      // Default: 30s
    pub grpc_timeout: Duration,      // Default: 10s
}

// Customize
let config = TimeoutConfig {
    rest_timeout: Duration::from_secs(60),
    grpc_timeout: Duration::from_secs(15),
};
```

## 5. Graceful Shutdown

### Already Integrated in main.rs
```rust
// Setup graceful shutdown signal handler (SIGINT/SIGTERM)
let shutdown_signal = async {
    use tokio::signal;
    let _ = signal::ctrl_c().await;
    info!("Received shutdown signal, initiating graceful shutdown...");
};

// Run server with graceful shutdown
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal)
    .await?;
```

### Shutdown Sequence
1. Stop accepting new requests
2. Wait for in-flight requests to drain (30s timeout)
3. Close WebSocket connections
4. Flush metrics
5. Exit

## 6. Configuration Validation

### Already Integrated in main.rs
```rust
// Validate configuration at startup
ConfigValidator::validate_all(&config).await?;
```

### Validation Checks
- ✓ Port range valid (1-65535)
- ✓ Provider URLs configured
- ✓ Rate limit config sane (burst > 0, rps > 0)
- ✓ TLS cert paths exist (if configured)

## Integration Checklist

### Phase 1: Error Handling (Quick Win)
- [ ] Replace `internal_error()` function with `GatewayError` returns
- [ ] Update all handler signatures to return `Result<T, GatewayError>`
- [ ] Remove manual JSON error construction

### Phase 2: Circuit Breakers (Provider Resilience)
- [ ] Add `CircuitBreakerRegistry` to `AppState`
- [ ] Wrap provider calls in `circuit_breaker.check()`
- [ ] Call `record_success()` and `record_failure()` appropriately
- [ ] Test circuit breaker recovery with provider outages

### Phase 3: Retry Logic (Transient Failures)
- [ ] Wrap critical provider calls with `retry_with_backoff()`
- [ ] Exclude retry on 4xx client errors
- [ ] Test exponential backoff with simulated failures

### Phase 4: Timeouts (Request Fairness)
- [ ] Add timeout middleware to router
- [ ] Configure per-endpoint timeouts
- [ ] Monitor timeout rate in metrics

### Phase 5: Graceful Shutdown (Clean Shutdowns)
- [ ] Verify signal handling in main()
- [ ] Test shutdown with in-flight requests
- [ ] Verify WebSocket cleanup

### Phase 6: Config Validation (Startup Safety)
- [ ] Test validator with invalid configs
- [ ] Document all required environment variables
- [ ] Add startup health checks

## Example: Full Integration

```rust
use crate::core::hardening::*;

async fn list_markets_with_hardening(
    State(state): State<AppState>,
    Query(query): Query<ListMarketsQuery>,
) -> Result<Json<AggregatedMarkets>, GatewayError> {
    let request_id = uuid::Uuid::new_v4().to_string();
    info!(request_id = %request_id, "list_markets request");

    // For each provider
    let mut results = Vec::new();
    for provider_id in state.registry.provider_ids() {
        // 1. Check circuit breaker
        let cb = state.circuit_breakers.get_or_create(&provider_id);
        if let Err(e) = cb.check() {
            warn!(provider = %provider_id, "Circuit breaker open");
            results.push(ProviderResult {
                provider: provider_id,
                data: None,
                error: Some("Circuit breaker open".to_string()),
                latency_ms: 0,
            });
            continue;
        }

        // 2. Retry with exponential backoff
        let config = RetryConfig::default();
        let result = retry_with_backoff(config, || async {
            state.registry.get(&provider_id)
                .ok_or_else(|| anyhow!("Provider not found"))?
                .list_markets(query.clone().into())
                .await
        }).await;

        // 3. Record success/failure
        match result {
            Ok(markets) => {
                cb.record_success();
                results.push(ProviderResult {
                    provider: provider_id,
                    data: Some(markets),
                    error: None,
                    latency_ms: 0,
                });
            }
            Err(e) => {
                cb.record_failure();
                warn!(provider = %provider_id, error = %e, "Provider call failed");
                results.push(ProviderResult {
                    provider: provider_id,
                    data: None,
                    error: Some(e.to_string()),
                    latency_ms: 0,
                });
            }
        }
    }

    Ok(Json(AggregatedMarkets {
        markets: Vec::new(),
        provider_results: results,
        total: 0,
        errors: Vec::new(),
    }))
}
```

## Testing

### Circuit Breaker Testing
```rust
#[tokio::test]
async fn test_circuit_breaker_opens_after_failures() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        recovery_timeout: Duration::from_millis(100),
        half_open_max_requests: 2,
    };
    let cb = CircuitBreaker::new(config);

    // Trip circuit
    for _ in 0..3 {
        cb.record_failure();
    }
    assert!(cb.check().is_err());

    // Wait for recovery
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Should transition to HalfOpen
    assert!(cb.check().is_ok());
    assert_eq!(cb.get_state(), CircuitState::HalfOpen);

    // Recover after successful request
    cb.record_success();
    assert_eq!(cb.get_state(), CircuitState::Closed);
}
```

### Retry Testing
```rust
#[tokio::test]
async fn test_retry_with_backoff() {
    let mut attempts = 0;
    let config = RetryConfig {
        max_retries: 3,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        jitter: false,
    };

    let result = retry_with_backoff(config, || async {
        attempts += 1;
        if attempts < 3 {
            Err(anyhow!("Temporary failure"))
        } else {
            Ok("success")
        }
    }).await;

    assert!(result.is_ok());
    assert_eq!(attempts, 3);
}
```

## Monitoring & Observability

### Metrics to Instrument
```rust
// In handlers, track:
- Circuit breaker state changes
- Retry attempts and successes
- Timeout occurrences
- Error types and frequencies
- Request latency per provider

// Example:
metrics::counter!("circuit_breaker.opened", 1, "provider" => "kalshi");
metrics::histogram!("request.latency_ms", latency_ms as f64);
metrics::counter!("retry.attempts", 1, "attempt" => attempt.to_string());
```

### Tracing Integration
All hardening components use `tracing` macros:
- `info!()` — Circuit breaker state changes, config validation
- `warn!()` — Circuit breaker failures, retries, timeouts
- `error!()` — Internal errors, validation failures

To enable full tracing:
```bash
RUST_LOG=upp_gateway=debug,tower_http=debug cargo run
```

## Performance Considerations

### Circuit Breaker Overhead
- Atomic operations only: ~1µs per check
- Thread-safe via atomics + Mutex
- No allocations in hot path

### Retry Overhead
- Linear with max_retries (default 3)
- Exponential delay prevents thundering herd
- Jitter enabled by default (±20%)

### Timeout Overhead
- Single tokio::time::timeout per request
- ~100ns overhead per request

### Configuration Validation
- Runs once at startup
- Non-blocking async URL checks (disabled by default to avoid startup delays)

## Production Deployment Checklist

- [ ] Enable debug logging during initial deployment
- [ ] Monitor circuit breaker states via metrics
- [ ] Set up alerts for repeated circuit breaker trips
- [ ] Configure appropriate timeout values for your SLAs
- [ ] Test graceful shutdown under load
- [ ] Verify all config validations pass
- [ ] Monitor retry frequency — high rates indicate provider issues
- [ ] Track error rates by type (timeout, circuit open, etc.)
