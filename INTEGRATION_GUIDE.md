# UPP Integration Guide

## TypeScript SDK Usage

```typescript
import { UppClient, Market, Order, ArbitrageOpportunity } from '@upp/sdk';

const client = new UppClient({
  baseUrl: 'https://api.upp.dev',
  apiKey: 'your-api-key' // optional for public endpoints
});

// Health check
const health = await client.health();
console.log(health.status);

// List markets
const markets = await client.listMarkets({ limit: 20 });
console.log(markets.data);

// Search markets
const results = await client.searchMarkets('bitcoin', 'polymarket', 50);

// Get specific market
const market = await client.getMarket('market-123');

// List arbitrage opportunities
const opportunities = await client.listArbitrage();

// Trading (requires API key)
const order = await client.createOrder({
  market_id: 'market-123',
  side: 'buy',
  price: 0.55,
  quantity: 100
});

// Portfolio (requires API key)
const portfolio = await client.getPortfolioSummary();
console.log(portfolio.total_pnl);
```

## Python SDK Usage

```python
from upp import UppClient, Market, Order, OrderSide

client = UppClient(base_url='https://api.upp.dev', api_key='your-api-key')

# Health check
health = client.health()
print(health.status)

# List markets
markets = client.list_markets(limit=20)
for market in markets.data:
    print(f"{market.name}: {market.price}")

# Search markets
results = client.search_markets('bitcoin', provider='polymarket')

# Get market details
market = client.get_market('market-123')

# Arbitrage opportunities
opps = client.list_arbitrage()
for opp in opps.data:
    print(f"Spread: {opp.spread_percent}%")

# Trading (requires API key)
order = client.create_order({
    'market_id': 'market-123',
    'side': OrderSide.BUY,
    'price': 0.55,
    'quantity': 100
})

# Portfolio (requires API key)
portfolio = client.get_portfolio_summary()
print(f"P&L: {portfolio.total_pnl}")

# Context manager for automatic cleanup
with UppClient('https://api.upp.dev') as client:
    health = client.health()
```

## Graceful Shutdown Integration (Rust)

In `main.rs`:

```rust
use std::sync::Arc;
use crate::core::shutdown::{ShutdownCoordinator, start_shutdown_handler};

#[tokio::main]
async fn main() -> Result<()> {
    // ... initialization ...

    // Create shutdown coordinator
    let shutdown = Arc::new(ShutdownCoordinator::new());

    // Spawn signal handler (runs in background)
    start_shutdown_handler(shutdown.clone());

    // Get shutdown flag for background tasks
    let shutdown_flag = shutdown.flag();

    // Start arbitrage scanner with shutdown awareness
    let scanner_shutdown = shutdown_flag.clone();
    tokio::spawn(async move {
        loop {
            if scanner_shutdown.load(Ordering::SeqCst) {
                info!("Arbitrage scanner shutting down");
                break;
            }
            // ... scan logic ...
            sleep(Duration::from_secs(5)).await;
        }
    });

    // Start price indexer with shutdown awareness
    let indexer_shutdown = shutdown_flag.clone();
    tokio::spawn(async move {
        loop {
            if indexer_shutdown.load(Ordering::SeqCst) {
                info!("Price indexer shutting down");
                break;
            }
            // ... indexing logic ...
            sleep(Duration::from_secs(5)).await;
        }
    });

    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown.wait_for_signal().await;
        })
        .await?;

    // Execute graceful shutdown sequence
    shutdown.shutdown_gracefully(
        Some(ws_manager.clone()),
        Some(arbitrage_scanner.clone()),
        Some(price_index.clone()),
        Some(storage.clone()),
    ).await;

    info!("Server shut down successfully");
    Ok(())
}
```

## File Locations

TypeScript SDK:
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/sdks/typescript/src/index.ts`
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/sdks/typescript/package.json`
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/sdks/typescript/tsconfig.json`

Python SDK:
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/sdks/python/upp/__init__.py`
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/sdks/python/setup.py`
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/sdks/python/pyproject.toml`

Rust Shutdown Module:
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/gateway/src/core/shutdown.rs`
- `/sessions/stoic-compassionate-turing/mnt/outputs/upp/gateway/src/core/mod.rs` (updated)

## API Coverage

Both SDKs implement 40+ endpoints from the OpenAPI spec:

**Discovery**: health, providers, manifest, negotiate
**Markets**: list, search, get, orderbook, categories
**Arbitrage**: list, summary, history
**Price History**: candles, latest candle, indices, resolutions
**Trading**: create order, list orders, get order, cancel order, estimate
**Portfolio**: positions, summary, balances, analytics
**Smart Routing**: compute route, execute route, stats
**Feeds**: status, stats, subscribe
**Backtesting**: list strategies, run backtest, compare
**MCP**: list tools, execute tool, schema
**Auth**: API key management
**Infrastructure**: health, ready, metrics
