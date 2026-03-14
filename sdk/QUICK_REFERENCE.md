# UPP Rust SDK - Quick Reference

## Client Creation

```rust
// Minimal setup
let client = UppClient::new("http://localhost:9090")?;

// Full configuration
let client = UppClient::builder()
    .base_url("http://localhost:9090")
    .api_key("your-api-key")
    .timeout(Duration::from_secs(60))
    .build()?;
```

## Health Checks

```rust
client.health().await?;           // GET /health
client.ready().await?;             // GET /ready
client.metrics().await?;           // GET /metrics
```

## Markets

```rust
// List all markets
let markets = client.list_markets(
    Some("polymarket"),            // provider
    Some("active"),                // status
    Some("crypto"),                // category
    Some(50),                      // limit
    None                           // cursor
).await?;

// Get specific market
let market = client.get_market("market-123").await?;

// Get orderbook
let orderbook = client.get_orderbook("market-123").await?;

// Search markets
let results = client.search_markets(
    Some("Bitcoin"),               // query
    Some("polymarket"),            // provider
    Some("crypto"),                // category
    Some(10)                       // limit
).await?;
```

## Candles

```rust
// Get historical candles
let candles = client.get_candles(
    "market-123",                  // market_id
    Some("yes"),                   // outcome_id
    Some("1h"),                    // resolution
    Some("2026-03-01T00:00:00Z"),  // from
    Some("2026-03-14T00:00:00Z"),  // to
    Some(100)                      // limit
).await?;

// Get latest candle
let latest = client.get_latest_candle(
    "market-123",
    Some("yes"),
    Some("1h")
).await?;
```

## Arbitrage

```rust
client.list_arbitrage().await?;              // Get all opportunities
client.arbitrage_summary().await?;           // Get summary stats
client.arbitrage_history(Some(100)).await?;  // Get history with limit
```

## Price Index

```rust
client.price_index_stats().await?;
```

## Backtest

```rust
// List strategies
let strategies = client.list_strategies().await?;

// Run backtest
let result = client.run_backtest(RunBacktestRequest {
    strategy: "momentum".to_string(),
    market_id: "market-123".to_string(),
    outcome_id: "yes".to_string(),
    resolution: "1h".to_string(),
    params: None,
    initial_capital: 10000.0,
    fee_rate: 0.001,
    slippage_rate: 0.0005,
    max_position: Some(5000.0),
}).await?;

// Compare strategies
let comparison = client.compare_strategies(CompareStrategiesRequest {
    market_id: "market-123".to_string(),
    outcome_id: "yes".to_string(),
    resolution: "1h".to_string(),
    strategies: vec!["momentum".to_string(), "mean-reversion".to_string()],
}).await?;
```

## Feeds

```rust
// Public endpoints
client.feed_status().await?;       // Get all feed status
client.feed_stats().await?;        // Get feed statistics

// Authenticated
client.subscribe_feeds(FeedSubscriptionRequest {
    feed_ids: vec!["feed-1".to_string(), "feed-2".to_string()],
    market_ids: Some(vec!["market-1".to_string()]),
}).await?;
```

## Orders (Requires API Key)

```rust
// Create order
client.create_order(CreateOrderRequest {
    market_id: "market-123".to_string(),
    outcome_id: "yes".to_string(),
    side: OrderSide::Buy,
    quantity: 10.0,
    price: 0.50,
    order_type: OrderType::Limit,
}).await?;

// List orders
client.list_orders().await?;

// Get specific order
client.get_order("order-456").await?;

// Cancel order
client.cancel_order("order-456").await?;

// Cancel all orders
client.cancel_all_orders().await?;

// Estimate order
client.estimate_order(EstimateOrderRequest {
    market_id: "market-123".to_string(),
    outcome_id: "yes".to_string(),
    side: OrderSide::Buy,
    quantity: 10.0,
}).await?;
```

## Trades (Requires API Key)

```rust
client.list_trades().await?;
```

## Portfolio (Requires API Key)

```rust
// Positions
client.get_positions().await?;

// Summary
client.portfolio_summary().await?;

// Balances
client.get_balances().await?;

// Analytics
client.portfolio_analytics().await?;
```

## Routing (Requires API Key)

```rust
// Compute route
let route = client.compute_route(ComputeRouteRequest {
    market_id: "market-123".to_string(),
    outcome_id: "yes".to_string(),
    side: OrderSide::Buy,
    quantity: 10.0,
}).await?;

// Execute route
client.execute_route(ExecuteRouteRequest {
    route_id: route.route_id,
}).await?;

// Route statistics
client.route_stats().await?;
```

## Error Handling

```rust
match client.health().await {
    Ok(health) => println!("Healthy: {:?}", health),
    Err(e) => match e {
        UppSdkError::RequestFailed(err) => eprintln!("Network error: {}", err),
        UppSdkError::ApiError { status, body } => eprintln!("API error {}: {}", status, body),
        UppSdkError::ValidationError(msg) => eprintln!("Invalid input: {}", msg),
        UppSdkError::ConfigError(msg) => eprintln!("Config error: {}", msg),
        _ => eprintln!("Error: {}", e),
    }
}
```

## Common Patterns

### Checking Gateway Availability

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = UppClient::new("http://localhost:9090")?;

    match client.ready().await {
        Ok(ready) if ready.ready => println!("Gateway is ready"),
        _ => println!("Gateway is not ready"),
    }

    Ok(())
}
```

### Retry Logic with Backoff

```rust
use std::time::Duration;

async fn with_retry<F, T>(mut f: F, max_retries: u32) -> Result<T>
where
    F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>>>>,
{
    let mut retries = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if retries < max_retries => {
                retries += 1;
                tokio::time::sleep(Duration::from_millis(100 * retries as u64)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Batch Operations

```rust
// Create multiple orders
let orders = vec!["market-1", "market-2", "market-3"];
for market_id in orders {
    let result = client.create_order(CreateOrderRequest {
        market_id: market_id.to_string(),
        outcome_id: "yes".to_string(),
        side: OrderSide::Buy,
        quantity: 10.0,
        price: 0.50,
        order_type: OrderType::Limit,
    }).await?;
    println!("Created order: {}", result.order.id);
}
```

### Monitoring Positions

```rust
loop {
    let positions = client.get_positions().await?;
    let summary = client.portfolio_summary().await?;

    println!("Positions: {}", positions.positions.len());
    println!("Portfolio PnL: {}", summary.total_unrealized_pnl);

    tokio::time::sleep(Duration::from_secs(60)).await;
}
```

## Types Quick Reference

### Enums
```rust
OrderSide::Buy      // BUY
OrderSide::Sell     // SELL

OrderType::Limit    // LIMIT
OrderType::Market   // MARKET
```

### Main Request Types
- `CreateOrderRequest`
- `EstimateOrderRequest`
- `RunBacktestRequest`
- `CompareStrategiesRequest`
- `ComputeRouteRequest`
- `ExecuteRouteRequest`
- `FeedSubscriptionRequest`

### Main Response Types
- `HealthResponse`
- `Market`, `MarketsResponse`
- `OrderbookResponse`
- `Order`, `OrdersResponse`
- `Trade`, `TradesResponse`
- `Position`, `PositionsResponse`
- `Balance`, `BalancesResponse`
- `PortfolioSummaryResponse`
- `AnalyticsResponse`
- `Candle`, `CandlesResponse`
- `BacktestResponse`
- `ArbitrageOpportunity`, `ArbitrageSummaryResponse`

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_market_serialize

# Run example
cargo run --example basic_usage
```

## Documentation

```bash
# Generate and open documentation
cargo doc --open

# Check code without building
cargo check
```
