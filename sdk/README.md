# UPP SDK - Rust Client for UPP Gateway

A production-grade, fully-typed Rust client library for the **UPP Gateway** REST API.

## Features

- **🎯 Fully Typed**: All API responses and requests are strongly typed with serde
- **⚡ Async/Await**: Built on tokio and reqwest for high-performance async operations
- **🔧 Builder Pattern**: Flexible client configuration via the builder pattern
- **📚 Comprehensive**: Covers all UPP Gateway endpoints
- **🛡️ Error Handling**: Rich, detailed error types with thiserror
- **📖 Well Documented**: Extensive doc comments and practical examples
- **✅ Tested**: Includes unit tests for serialization/deserialization

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
upp-sdk = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Basic Health Check

```rust
use upp_sdk::UppClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client
    let client = UppClient::new("http://localhost:9090")?;

    // Check health
    let health = client.health().await?;
    println!("Health: {:?}", health);

    Ok(())
}
```

### List Markets

```rust
use upp_sdk::UppClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = UppClient::new("http://localhost:9090")?;

    // List markets with filters
    let markets = client.list_markets(
        Some("polymarket"),  // provider
        Some("active"),      // status
        Some("crypto"),      // category
        Some(50),            // limit
        None                 // cursor
    ).await?;

    for market in &markets.markets {
        println!("{}: {}", market.id, market.title);
    }

    Ok(())
}
```

### Authenticated Operations

For endpoints requiring authentication, provide an API key:

```rust
use upp_sdk::UppClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = UppClient::builder()
        .base_url("http://localhost:9090")
        .api_key("your-api-key")
        .timeout(Duration::from_secs(30))
        .build()?;

    // Get portfolio summary
    let summary = client.portfolio_summary().await?;
    println!("Total balance: ${}", summary.total_balance);

    Ok(())
}
```

### Create an Order

```rust
use upp_sdk::{UppClient, CreateOrderRequest, OrderSide, OrderType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = UppClient::builder()
        .base_url("http://localhost:9090")
        .api_key("your-api-key")
        .build()?;

    let order = client.create_order(CreateOrderRequest {
        market_id: "market-123".to_string(),
        outcome_id: "yes".to_string(),
        side: OrderSide::Buy,
        quantity: 10.0,
        price: 0.50,
        order_type: OrderType::Limit,
    }).await?;

    println!("Order created: {}", order.order.id);

    Ok(())
}
```

### Run a Backtest

```rust
use upp_sdk::{UppClient, RunBacktestRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = UppClient::new("http://localhost:9090")?;

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

    println!("Backtest result:");
    println!("  Return: {:.2}%", result.total_return * 100.0);
    println!("  Sharpe Ratio: {:.2}", result.sharpe_ratio);
    println!("  Max Drawdown: {:.2}%", result.max_drawdown * 100.0);

    Ok(())
}
```

## API Endpoints

### Health & Status (Public)
- `health()` - GET /health
- `ready()` - GET /ready
- `metrics()` - GET /metrics

### Markets (Public)
- `list_markets()` - GET /upp/v1/markets
- `get_market()` - GET /upp/v1/markets/:market_id
- `get_orderbook()` - GET /upp/v1/markets/:market_id/orderbook
- `search_markets()` - GET /upp/v1/markets/search

### Candles (Public)
- `get_candles()` - GET /upp/v1/markets/:market_id/candles
- `get_latest_candle()` - GET /upp/v1/markets/:market_id/candles/latest

### Arbitrage (Public)
- `list_arbitrage()` - GET /upp/v1/arbitrage
- `arbitrage_summary()` - GET /upp/v1/arbitrage/summary
- `arbitrage_history()` - GET /upp/v1/arbitrage/history

### Price Index (Public)
- `price_index_stats()` - GET /upp/v1/price-index/stats

### Backtest (Public)
- `list_strategies()` - GET /upp/v1/backtest/strategies
- `run_backtest()` - POST /upp/v1/backtest/run
- `compare_strategies()` - POST /upp/v1/backtest/compare

### Feeds (Public + Authenticated)
- `feed_status()` - GET /upp/v1/feeds/status
- `feed_stats()` - GET /upp/v1/feeds/stats
- `subscribe_feeds()` - POST /upp/v1/feeds/subscribe (requires auth)

### Orders (Authenticated)
- `create_order()` - POST /upp/v1/orders
- `list_orders()` - GET /upp/v1/orders
- `get_order()` - GET /upp/v1/orders/:order_id
- `cancel_order()` - DELETE /upp/v1/orders/:order_id
- `cancel_all_orders()` - POST /upp/v1/orders/cancel-all
- `estimate_order()` - POST /upp/v1/orders/estimate

### Trades (Authenticated)
- `list_trades()` - GET /upp/v1/trades

### Portfolio (Authenticated)
- `get_positions()` - GET /upp/v1/portfolio/positions
- `portfolio_summary()` - GET /upp/v1/portfolio/summary
- `get_balances()` - GET /upp/v1/portfolio/balances
- `portfolio_analytics()` - GET /upp/v1/portfolio/analytics

### Routing (Authenticated)
- `compute_route()` - POST /upp/v1/orders/route
- `execute_route()` - POST /upp/v1/orders/route/execute
- `route_stats()` - GET /upp/v1/orders/route/stats

## Error Handling

The SDK provides comprehensive error handling with the `UppSdkError` type:

```rust
use upp_sdk::UppClient;

#[tokio::main]
async fn main() {
    let client = UppClient::new("http://localhost:9090").unwrap();

    match client.health().await {
        Ok(health) => println!("Health: {:?}", health),
        Err(e) => match e {
            upp_sdk::UppSdkError::RequestFailed(e) => {
                println!("Request failed: {}", e);
            }
            upp_sdk::UppSdkError::ApiError { status, body } => {
                println!("API error {}: {}", status, body);
            }
            upp_sdk::UppSdkError::ValidationError(msg) => {
                println!("Validation error: {}", msg);
            }
            _ => println!("Error: {}", e),
        }
    }
}
```

## Configuration

### Builder Options

```rust
use upp_sdk::UppClient;
use std::time::Duration;

let client = UppClient::builder()
    .base_url("http://localhost:9090")          // Default
    .api_key("your-api-key")                    // Optional
    .timeout(Duration::from_secs(30))           // Default: 30s
    .build()?;
```

## Testing

Run the test suite:

```bash
cargo test
```

Run the example:

```bash
cargo run --example basic_usage
```

## Project Structure

```
upp-sdk/
├── Cargo.toml           # Package manifest
├── src/
│   ├── lib.rs          # Main library entry point
│   ├── client.rs       # HTTP client implementation
│   ├── types.rs        # All API types
│   └── error.rs        # Error types
├── examples/
│   └── basic_usage.rs  # Basic usage example
└── README.md           # This file
```

## Type System

All API responses are strongly typed:

```rust
// Markets
pub struct Market { ... }
pub struct MarketOutcome { ... }

// Orders
pub struct Order { ... }
pub enum OrderSide { Buy, Sell }
pub enum OrderType { Limit, Market }

// Portfolio
pub struct Position { ... }
pub struct Balance { ... }

// And many more...
```

All types implement `Serialize` and `Deserialize` for seamless JSON conversion.

## Performance

The SDK uses:
- `tokio` for async runtime
- `reqwest` with connection pooling for HTTP
- `rustls-tls` for TLS encryption
- `serde` for efficient JSON serialization

## Contributing

Contributions are welcome! Please ensure:
- Code follows Rust conventions
- All tests pass
- Doc comments are comprehensive
- Examples work correctly

## License

Apache License 2.0

## Support

For issues or questions:
1. Check existing examples in `examples/`
2. Review API documentation
3. Check error messages (they're descriptive)
