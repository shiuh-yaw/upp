# Rust Client Guide

Complete reference for the UPP Rust SDK with examples for every feature.

## Client Creation

### Basic Setup

```rust
use upp_sdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(
        "http://localhost:8080",
        "your_api_key"
    );

    // Use client...
    Ok(())
}
```

### Advanced Configuration

```rust
use upp_sdk::ClientBuilder;
use std::time::Duration;

let client = ClientBuilder::new()
    .base_url("http://localhost:8080")
    .api_key("your_api_key")
    .timeout(Duration::from_secs(30))
    .connect_timeout(Duration::from_secs(10))
    .max_retries(3)
    .cache_size(1000)
    .enable_compression(true)
    .build()?;
```

## Markets API

### List Markets

```rust
// Get 10 Polymarket markets
let markets = client.markets()
    .provider("polymarket")
    .limit(10)
    .fetch()
    .await?;

for market in markets {
    println!(
        "{}: {} (Liquidity: {})",
        market.id, market.title, market.liquidity
    );
}
```

### With Filters

```rust
let markets = client.markets()
    .provider("kalshi")
    .category("politics")
    .status("active")
    .limit(20)
    .offset(10)
    .fetch()
    .await?;
```

### Search Markets

```rust
let results = client.markets()
    .search("ethereum")
    .limit(10)
    .fetch()
    .await?;

for result in results {
    println!("{}: {} (score: {})",
        result.market_id,
        result.title,
        result.relevance_score
    );
}
```

### Get Single Market

```rust
let market = client.market("0x1234...abcd")
    .fetch()
    .await?;

println!("{}", market.title);
for outcome in market.outcomes {
    println!("  {} @ {}", outcome.name, outcome.price);
}
```

### Pagination

```rust
let mut all_markets = Vec::new();
let mut offset = 0;
let limit = 100;

loop {
    let page = client.markets()
        .provider("polymarket")
        .limit(limit)
        .offset(offset)
        .fetch()
        .await?;

    let page_len = page.len();
    all_markets.extend(page);

    if page_len < limit {
        break;  // Last page
    }

    offset += limit;
}

println!("Total markets: {}", all_markets.len());
```

## Orders API

### List Orders

```rust
let orders = client.orders()
    .provider("polymarket")
    .status("open")
    .fetch()
    .await?;

for order in orders {
    println!(
        "Order {}: {} {} shares @ {}",
        order.id, order.side, order.quantity, order.price
    );
}
```

### Get Order Details

```rust
let order = client.order("order_12345")
    .fetch()
    .await?;

println!("Order {}: {} / {}",
    order.id, order.filled, order.quantity
);
```

### Place Order

```rust
use upp_sdk::OrderRequest;

let order = client.orders()
    .place(OrderRequest {
        provider: "polymarket".to_string(),
        market_id: "0x1234...abcd".to_string(),
        side: "BUY".to_string(),
        outcome: "Yes".to_string(),
        price: 0.72,
        quantity: 100.0,
    })
    .await?;

println!("Order placed: {}", order.id);
```

### With Validation

```rust
use upp_sdk::OrderRequest;

let order_req = OrderRequest {
    provider: "polymarket".to_string(),
    market_id: "0x1234...abcd".to_string(),
    side: "BUY".to_string(),
    outcome: "Yes".to_string(),
    price: 0.72,
    quantity: 100.0,
};

// Validate before submitting
if order_req.price < 0.0 || order_req.price > 1.0 {
    eprintln!("Invalid price: must be between 0 and 1");
} else if order_req.quantity <= 0.0 {
    eprintln!("Invalid quantity: must be > 0");
} else {
    let order = client.orders().place(order_req).await?;
    println!("Order placed: {}", order.id);
}
```

### Cancel Order

```rust
client.order("order_12345")
    .cancel()
    .await?;

println!("Order cancelled");
```

## Portfolio API

### Get Portfolio Summary

```rust
let portfolio = client.portfolio()
    .fetch()
    .await?;

println!("Balance: ${}", portfolio.cash_balance);
println!("Total Value: ${}", portfolio.total_value);
println!("Total P&L: ${} ({:.1}%)",
    portfolio.total_pnl,
    portfolio.total_pnl_percent * 100.0
);

for position in portfolio.positions {
    println!(
        "  {} @ {} (P&L: ${}, {:.1}%)",
        position.outcome,
        position.current_price,
        position.pnl,
        position.pnl_percent * 100.0
    );
}
```

### Get Positions

```rust
let positions = client.positions()
    .provider("polymarket")
    .fetch()
    .await?;

// Group by outcome
use std::collections::HashMap;
let mut by_outcome: HashMap<String, Vec<_>> = HashMap::new();

for position in positions {
    by_outcome.entry(position.outcome.clone())
        .or_default()
        .push(position);
}

for (outcome, positions) in by_outcome {
    let total_qty: f64 = positions.iter().map(|p| p.quantity).sum();
    println!("{}: {} shares", outcome, total_qty);
}
```

## Arbitrage API

### Find Opportunities

```rust
let opportunities = client.arbitrage()
    .min_spread(0.05)  // 5% minimum
    .limit(20)
    .fetch()
    .await?;

for opp in opportunities {
    println!(
        "{}: Buy @ {} on {}, Sell @ {} on {} ({}% spread)",
        opp.outcome,
        opp.buy_exchange.price,
        opp.buy_exchange.provider,
        opp.sell_exchange.price,
        opp.sell_exchange.provider,
        opp.spread_percent * 100.0
    );
    println!("  Max volume: {}, Potential profit: ${}",
        opp.max_volume, opp.potential_profit
    );
}
```

## Backtest API

### Run Backtest

```rust
use upp_sdk::{BacktestRequest, BacktestTrade};

let backtest = client.backtest(BacktestRequest {
    market_id: "0x1234...abcd".to_string(),
    provider: "polymarket".to_string(),
    start_date: "2025-09-01".to_string(),
    end_date: "2026-03-14".to_string(),
    initial_balance: 10000.0,
    trades: vec![
        BacktestTrade {
            date: "2025-10-01".to_string(),
            side: "BUY".to_string(),
            outcome: "Yes".to_string(),
            quantity: 1000.0,
            price: 0.50,
        },
        BacktestTrade {
            date: "2026-01-15".to_string(),
            side: "SELL".to_string(),
            outcome: "Yes".to_string(),
            quantity: 1000.0,
            price: 0.72,
        },
    ],
}).await?;

println!("Initial: ${}", backtest.initial_balance);
println!("Final: ${}", backtest.final_balance);
println!("P&L: ${} ({:.1}%)",
    backtest.total_pnl,
    backtest.total_pnl_percent * 100.0
);
println!("Sharpe Ratio: {}", backtest.sharpe_ratio);
println!("Max Drawdown: {:.1}%", backtest.max_drawdown_percent * 100.0);
```

## WebSocket Subscriptions

### Subscribe to Markets

```rust
use futures::StreamExt;

let mut stream = client.markets()
    .stream("polymarket")
    .interval(Duration::from_secs(5))
    .subscribe()
    .await?;

while let Some(update) = stream.next().await {
    match update {
        Ok(market) => {
            println!("{}: {} outcomes", market.id, market.outcomes.len());
            for outcome in market.outcomes {
                println!("  {} @ {}", outcome.name, outcome.price);
            }
        }
        Err(e) => {
            eprintln!("Stream error: {}", e);
            break;
        }
    }
}
```

### Subscribe to Orders

```rust
use futures::StreamExt;

let mut stream = client.orders()
    .stream()
    .subscribe()
    .await?;

while let Some(result) = stream.next().await {
    match result {
        Ok(order) => {
            println!("Order update: {}", order.id);
            println!("  Status: {}", order.status);
            println!("  Filled: {} / {}", order.filled, order.quantity);
        }
        Err(e) => {
            eprintln!("Stream error: {}", e);
        }
    }
}
```

### Subscribe to Portfolio

```rust
use futures::StreamExt;

let mut stream = client.portfolio()
    .stream()
    .subscribe()
    .await?;

while let Some(result) = stream.next().await {
    match result {
        Ok(update) => {
            println!("Portfolio update");
            println!("  Balance: ${}", update.cash_balance);
            println!("  Total P&L: ${}", update.total_pnl);
        }
        Err(e) => {
            eprintln!("Stream error: {}", e);
        }
    }
}
```

## Error Handling

### Pattern Matching

```rust
use upp_sdk::ClientError;

match client.markets().fetch().await {
    Ok(markets) => {
        println!("Got {} markets", markets.len());
    }
    Err(ClientError::InvalidProvider { provider }) => {
        eprintln!("Provider not supported: {}", provider);
    }
    Err(ClientError::Unauthorized) => {
        eprintln!("Invalid API key");
    }
    Err(ClientError::RateLimited { retry_after }) => {
        eprintln!("Rate limited, retry after {}s", retry_after.as_secs());
    }
    Err(ClientError::NotFound { resource }) => {
        eprintln!("Resource not found: {}", resource);
    }
    Err(ClientError::Network { source }) => {
        eprintln!("Network error: {}", source);
    }
    Err(ClientError::Timeout) => {
        eprintln!("Request timed out");
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

### Retry Logic

```rust
use tokio::time::sleep;
use std::time::Duration;

async fn fetch_with_retry(
    client: &upp_sdk::Client,
    max_retries: u32,
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    let mut retries = 0;

    loop {
        match client.markets().provider("polymarket").fetch().await {
            Ok(markets) => return Ok(markets),
            Err(ClientError::RateLimited { retry_after }) => {
                if retries < max_retries {
                    sleep(retry_after).await;
                    retries += 1;
                } else {
                    return Err("Max retries exceeded".into());
                }
            }
            Err(e) => return Err(Box::new(e)),
        }
    }
}
```

## Advanced Patterns

### Connection Pooling

```rust
use std::sync::Arc;

// Create client once, share across threads
let client = Arc::new(Client::new("http://localhost:8080", "api_key"));

let handles: Vec<_> = (0..10)
    .map(|i| {
        let c = Arc::clone(&client);
        tokio::spawn(async move {
            let markets = c.markets()
                .provider("polymarket")
                .fetch()
                .await
                .unwrap();
            println!("Thread {}: {} markets", i, markets.len());
        })
    })
    .collect();

futures::future::join_all(handles).await;
```

### Caching

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

struct CachedClient {
    client: upp_sdk::Client,
    cache: RwLock<HashMap<String, Vec<Market>>>,
}

impl CachedClient {
    async fn get_markets(&self, provider: &str) -> Result<Vec<Market>> {
        // Check cache
        if let Ok(cache) = self.cache.read().await.get(provider) {
            return Ok(cache.clone());
        }

        // Fetch from API
        let markets = self.client.markets()
            .provider(provider)
            .fetch()
            .await?;

        // Store in cache
        self.cache.write().await.insert(
            provider.to_string(),
            markets.clone(),
        );

        Ok(markets)
    }
}
```

### Batch Operations

```rust
// Fetch markets from all providers in parallel
async fn fetch_all_providers(
    client: &upp_sdk::Client,
) -> Result<HashMap<String, Vec<Market>>> {
    let providers = vec!["polymarket", "kalshi", "opinion_trade"];

    let futures: Vec<_> = providers.iter()
        .map(|p| {
            client.markets()
                .provider(p)
                .limit(10)
                .fetch()
        })
        .collect();

    let results = futures::future::try_join_all(futures).await?;

    let mut markets = HashMap::new();
    for (provider, market_list) in providers.iter().zip(results) {
        markets.insert(provider.to_string(), market_list);
    }

    Ok(markets)
}
```

## Testing

### Mock Client

```rust
#[cfg(test)]
mod tests {
    use upp_sdk::MockClient;

    #[tokio::test]
    async fn test_markets() {
        let client = MockClient::new()
            .with_markets(vec![
                Market {
                    id: "test_market".into(),
                    title: "Test Market".into(),
                    ..Default::default()
                }
            ]);

        let markets = client.markets()
            .fetch()
            .await
            .unwrap();

        assert_eq!(markets.len(), 1);
        assert_eq!(markets[0].id, "test_market");
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod tests {
    use upp_sdk::Client;

    #[tokio::test]
    #[ignore]  // Run with: cargo test -- --include-ignored
    async fn test_real_api() {
        let client = Client::new(
            "http://localhost:8080",
            &std::env::var("UPP_API_KEY").unwrap()
        );

        let markets = client.markets()
            .provider("polymarket")
            .limit(1)
            .fetch()
            .await
            .unwrap();

        assert!(!markets.is_empty());
    }
}
```

## Performance Tips

1. **Reuse client** — Create once, share across requests
2. **Use connection pooling** — Default enabled, adjust pool size if needed
3. **Enable caching** — SDK caches responses by default
4. **Batch operations** — Use parallel requests with `futures::join_all()`
5. **Stream data** — Use WebSocket subscriptions instead of polling
6. **Set timeouts** — Catch stuck requests early
7. **Handle backpressure** — Don't spawn unlimited concurrent tasks

## Troubleshooting

### Connection Refused

```
Error: connection refused
```

Ensure gateway is running:
```bash
docker-compose up -d gateway
```

### Unauthorized

```
Error: 401 Unauthorized
```

Check API key:
```rust
let client = Client::new(
    "http://localhost:8080",
    "correct_api_key"  // Not "api_key"
);
```

### Rate Limited

```
Error: 429 Too Many Requests
```

Implement exponential backoff:
```rust
let mut backoff = Duration::from_millis(100);
loop {
    match client.markets().fetch().await {
        Ok(m) => break,
        Err(ClientError::RateLimited { .. }) => {
            sleep(backoff).await;
            backoff *= 2;
        }
        Err(e) => return Err(e),
    }
}
```

### Timeout

```
Error: request timed out
```

Increase timeout:
```rust
let client = ClientBuilder::new()
    .base_url("http://localhost:8080")
    .api_key("api_key")
    .timeout(Duration::from_secs(60))
    .build()?;
```
