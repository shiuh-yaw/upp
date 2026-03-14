# Testing Guide

Comprehensive guide to testing UPP across unit, integration, and end-to-end scenarios.

## Test Architecture

```
Unit Tests (fast, isolated)
  ├─ Functions, types, error handling
  ├─ Mocked dependencies
  └─ Thousands of tests, <1s to run

Integration Tests (medium, components)
  ├─ Handlers, adapters, routing
  ├─ Real Redis, mock APIs
  └─ Hundreds of tests, 5-30s to run

E2E Tests (slow, full system)
  ├─ Complete request flows
  ├─ Real providers (or high-fidelity mocks)
  └─ Dozens of tests, 1-5 minutes to run

Property-Based Tests (randomized)
  ├─ Invariants that must always hold
  ├─ Generated random inputs
  └─ Verify edge cases
```

## Unit Tests

### Basic Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_price_bounds() {
        let outcome = Outcome {
            id: "test".into(),
            name: "Yes".into(),
            price: 0.5,
            probability: 0.5,
        };

        assert!(outcome.price >= 0.0 && outcome.price <= 1.0);
    }

    #[test]
    #[should_panic(expected = "invalid")]
    fn test_invalid_price_panics() {
        let _outcome = Outcome {
            id: "test".into(),
            name: "Yes".into(),
            price: 1.5,  // Invalid
            probability: 0.5,
        };
    }
}
```

### Async Tests

```rust
#[tokio::test]
async fn test_async_market_fetch() {
    let adapter = create_test_adapter();
    let result = adapter.get_markets(MarketFilter::default()).await;

    assert!(result.is_ok());
    let markets = result.unwrap();
    assert!(!markets.is_empty());
}

#[tokio::test]
async fn test_concurrent_requests() {
    let adapter = std::sync::Arc::new(create_test_adapter());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let a = adapter.clone();
            tokio::spawn(async move {
                a.get_markets(MarketFilter::default()).await
            })
        })
        .collect();

    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.unwrap().is_ok());
    }
}
```

### Testing Error Cases

```rust
#[test]
fn test_invalid_market_filter() {
    let filter = MarketFilter {
        limit: 1001,  // Max is 100
        ..Default::default()
    };

    let result = validate_filter(&filter);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ValidationError::LimitExceeded);
}

#[tokio::test]
async fn test_rate_limit_error() {
    let mut adapter = create_test_adapter();
    adapter.set_rate_limited(true);

    let result = adapter.get_markets(MarketFilter::default()).await;
    assert!(matches!(result, Err(ProviderError::RateLimited(_))));
}

#[tokio::test]
async fn test_timeout() {
    let adapter = create_slow_test_adapter(Duration::from_secs(10));

    let result = tokio::time::timeout(
        Duration::from_millis(100),
        adapter.get_markets(MarketFilter::default())
    ).await;

    assert!(result.is_err());
}
```

## Mocking

### Using Mockito

Mock HTTP API responses:

```rust
#[cfg(test)]
mod tests {
    use mockito::mock;

    #[tokio::test]
    async fn test_market_list_with_mock() {
        let _m = mock("GET", "/api/markets")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[
                {"id":"1","name":"Market 1","price":0.5},
                {"id":"2","name":"Market 2","price":0.3}
            ]"#)
            .create();

        let adapter = PolymarketAdapter::new_with_url(
            mockito::server_url()
        );

        let markets = adapter.get_markets(MarketFilter::default()).await.unwrap();
        assert_eq!(markets.len(), 2);
    }

    #[tokio::test]
    async fn test_rate_limit_with_mock() {
        let _m = mock("GET", "/api/markets")
            .with_status(429)
            .with_header("Retry-After", "60")
            .create();

        let adapter = PolymarketAdapter::new_with_url(mockito::server_url());

        let result = adapter.get_markets(MarketFilter::default()).await;
        assert!(matches!(result, Err(ProviderError::RateLimited(_))));
    }
}
```

### Custom Mock Adapter

```rust
#[cfg(test)]
pub struct MockAdapter {
    pub markets: Vec<Market>,
    pub should_fail: bool,
    pub fail_error: Option<ProviderError>,
}

#[async_trait]
impl ProviderAdapter for MockAdapter {
    async fn get_markets(
        &self,
        _filter: MarketFilter,
    ) -> Result<Vec<Market>, ProviderError> {
        if self.should_fail {
            Err(self.fail_error.clone())
        } else {
            Ok(self.markets.clone())
        }
    }
    // ... other methods
}
```

Use in tests:

```rust
#[test]
fn test_with_mock_adapter() {
    let adapter = MockAdapter {
        markets: vec![create_test_market()],
        should_fail: false,
        fail_error: None,
    };

    // Test code using adapter
}
```

## Integration Tests

### Testing Handlers

```rust
#[tokio::test]
async fn test_markets_handler() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/markets?provider=polymarket&limit=10")
                .method("GET")
                .build()
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body = response.into_body();
    let text = String::from_utf8(body.to_vec()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert!(json["markets"].is_array());
    assert!(json["markets"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_invalid_provider_error() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/markets?provider=invalid")
                .method("GET")
                .build()
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);  // Bad request

    let body = response.into_body();
    let text = String::from_utf8(body.to_vec()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert_eq!(json["error"]["code"], "INVALID_PROVIDER");
}
```

### Testing with Redis

```rust
#[tokio::test]
async fn test_caching_with_redis() {
    // Start Redis container (requires docker)
    let redis = testcontainers::clients::Cli::default()
        .run(testcontainers::images::redis::Redis::default());
    let host = format!("redis://127.0.0.1:{}", redis.get_host_port_ipv4(6379));

    let cache = RedisCache::new(&host).await.unwrap();

    // Test cache operations
    let key = "test:key";
    let value = "test_value";

    cache.set(key, value, Duration::from_secs(60)).await.unwrap();
    let retrieved = cache.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.to_string()));

    // Verify expiration
    cache.set(key, value, Duration::from_millis(100)).await.ok();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let expired = cache.get(key).await.unwrap();
    assert_eq!(expired, None);
}
```

## Benchmarking

### Criterion Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_market_search(c: &mut Criterion) {
    c.bench_function("search 1000 markets", |b| {
        b.iter(|| {
            search_markets(
                black_box("ethereum"),
                black_box(1000)
            )
        })
    });
}

fn bench_order_placement(c: &mut Criterion) {
    c.bench_function("place order", |b| {
        b.iter(|| {
            place_order(black_box(create_test_order()))
        })
    });
}

criterion_group!(benches, bench_market_search, bench_order_placement);
criterion_main!(benches);
```

Run benchmarks:

```bash
cargo bench
cargo bench -- --verbose
cargo bench market_search  # Specific benchmark
```

## Property-Based Testing

Using `proptest`:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_outcome_probabilities_sum_to_one(
        prices in prop::collection::vec(0.0f64..=1.0, 2..5)
    ) {
        let outcomes: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(i, &price)| Outcome {
                id: i.to_string(),
                name: format!("Outcome {}", i),
                price,
                probability: price,
            })
            .collect();

        let total: f64 = outcomes.iter().map(|o| o.probability).sum();
        prop_assert!((total - 1.0).abs() < 0.0001);
    }

    #[test]
    fn prop_order_quantity_positive(qty in 0.1f64..1000000.0) {
        prop_assert!(qty > 0.0);
        prop_assert!(qty.is_finite());
    }
}
```

## End-to-End Tests

### Full Request Flow

```rust
#[tokio::test]
async fn test_complete_trading_flow() {
    // Setup
    let app = create_test_app().await;
    let client = TestClient::new(app);

    // 1. Get markets
    let markets = client.get_markets("polymarket").await.unwrap();
    assert!(!markets.is_empty());
    let market_id = &markets[0].id;

    // 2. Get market details
    let market = client.get_market(market_id).await.unwrap();
    assert_eq!(market.id, *market_id);

    // 3. Place order
    let order = client.place_order(PlaceOrderRequest {
        market_id: market_id.clone(),
        side: "BUY".to_string(),
        outcome: "Yes".to_string(),
        price: 0.5,
        quantity: 100.0,
    }).await.unwrap();
    assert!(!order.id.is_empty());

    // 4. Check portfolio
    let portfolio = client.get_portfolio().await.unwrap();
    assert_eq!(portfolio.positions.len(), 1);
    assert_eq!(portfolio.positions[0].quantity, 100.0);

    // 5. Cancel order
    client.cancel_order(&order.id).await.unwrap();

    // 6. Verify order is gone
    let orders = client.get_orders().await.unwrap();
    assert!(!orders.iter().any(|o| o.id == order.id && o.status == "OPEN"));
}
```

## Test Coverage

Track coverage with Tarpaulin:

```bash
# Install
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html

# View report
open tarpaulin-report.html

# Set minimum coverage
cargo tarpaulin --minimum 80
```

## Running Tests

### All Tests

```bash
# Unit and integration tests
cargo test

# With output
cargo test -- --nocapture

# Specific module
cargo test adapters::
```

### Test Organization

```bash
# Unit tests only (no integration)
cargo test --lib

# Integration tests only
cargo test --test '*'

# Doc tests
cargo test --doc

# Single test
cargo test test_market_price_bounds
```

### Parallel vs. Sequential

```bash
# Parallel (default, faster)
cargo test

# Sequential (needed for stateful tests)
cargo test -- --test-threads=1

# Show test names without running
cargo test --no-run
```

## Test Best Practices

1. **One assertion per test** (or related assertions)
2. **Use descriptive names** — `test_invalid_price_rejects_order` vs. `test_1`
3. **Setup/teardown** — Use fixtures and builders
4. **Avoid test interdependencies** — Tests should be independent
5. **Test behavior, not implementation** — What matters to users?
6. **Mock external dependencies** — Don't test providers in unit tests
7. **Use parametrized tests** — Test multiple inputs
8. **Document non-obvious tests** — Why is this edge case important?

### Parametrized Tests

```rust
#[test]
fn test_market_validation() {
    let test_cases = vec![
        (0.0, true),     // Valid min
        (0.5, true),     // Valid mid
        (1.0, true),     // Valid max
        (-0.1, false),   // Invalid negative
        (1.1, false),    // Invalid over 1.0
    ];

    for (price, should_be_valid) in test_cases {
        let market = Market {
            outcomes: vec![Outcome { price, .. }],
            ..
        };

        assert_eq!(market.is_valid(), should_be_valid,
            "Price {} should be {}",
            price,
            if should_be_valid { "valid" } else { "invalid" }
        );
    }
}
```

## Debugging Tests

### Verbose Output

```bash
# Show println! output
cargo test -- --nocapture

# Show test names
cargo test -- --nocapture --test-threads=1

# Backtrace on panic
RUST_BACKTRACE=1 cargo test
```

### GDB Debugging

```bash
cargo build
rust-gdb ./target/debug/deps/gateway-...

(gdb) break test_name
(gdb) run --test test_name
(gdb) backtrace
(gdb) print variable_name
```

## CI/CD Integration

GitHub Actions automatically runs:

```yaml
- name: Run tests
  run: cargo test --verbose

- name: Check coverage
  run: cargo tarpaulin --minimum 75

- name: Run benchmarks
  run: cargo bench
```

## Coverage Goals

- **Core logic** (adapters, handlers): >90%
- **Utilities**: >80%
- **Configuration**: >70%
- **Overall**: >80%

Aim for quality, not quantity. A few well-written tests beat many superficial ones.
