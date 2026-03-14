# Universal Prediction Protocol (UPP)

An open standard and high-performance gateway for interoperable prediction markets. UPP connects **Kalshi**, **Polymarket**, and **Opinion.trade** through a unified API — enabling cross-platform trading, arbitrage detection, smart order routing, and real-time price aggregation.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      UPP Gateway                            │
│                                                             │
│  REST API (Axum 0.7)  │  gRPC (Tonic 0.12)  │  WebSocket  │
│                                                             │
│  ┌──────────┐ ┌──────────────┐ ┌───────────────────────┐   │
│  │ Provider  │ │ Smart Order  │ │ Price Index / Candles │   │
│  │ Registry  │ │   Router     │ │ (1m, 5m, 1h, 1d)     │   │
│  └──────────┘ └──────────────┘ └───────────────────────┘   │
│  ┌──────────┐ ┌──────────────┐ ┌───────────────────────┐   │
│  │ Arbitrage│ │  Auth / Rate │ │ Historical Ingestion  │   │
│  │ Scanner  │ │   Limiting   │ │      Pipeline         │   │
│  └──────────┘ └──────────────┘ └───────────────────────┘   │
│  ┌──────────┐ ┌──────────────┐ ┌───────────────────────┐   │
│  │ Circuit  │ │  Backtesting │ │   Live WebSocket      │   │
│  │ Breakers │ │    Engine    │ │      Feeds            │   │
│  └──────────┘ └──────────────┘ └───────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │          MCP + A2A Agent Protocol Support            │   │
│  └─────────────────────────────────────────────────────┘   │
└─────┬──────────────────┬──────────────────┬─────────────────┘
      │                  │                  │
  ┌───▼───┐         ┌───▼────┐        ┌───▼────────┐
  │ Kalshi│         │Polymar-│        │ Opinion    │
  │  API  │         │ket API │        │ .trade API │
  └───────┘         └────────┘        └────────────┘
```

## Features

**Core Gateway**
- Unified REST + gRPC + WebSocket API across all supported prediction market providers
- Provider registry with capability negotiation and health-aware load balancing
- Multi-layer caching (Moka + DashMap) with configurable TTL per provider

**Trading**
- Cross-platform order placement, cancellation, and status tracking
- Smart order routing — splits orders across providers for best execution
- Portfolio aggregation with real-time P&L, analytics, and risk metrics

**Market Data**
- Real-time WebSocket price feed with fan-out to subscribers
- Price index with OHLCV candle aggregation at 1m, 5m, 1h, 1d resolutions
- Historical data ingestion pipeline with pluggable data sources

**Analysis**
- Arbitrage scanner running every 5 seconds across all provider pairs
- Backtesting engine with 4 built-in strategies (momentum, mean-reversion, breakout, MACD)
- Strategy comparison with Sharpe ratio, max drawdown, win rate metrics

**Infrastructure**
- API key + JWT authentication with tiered access (Free, Standard, Pro, Enterprise)
- Per-endpoint rate limiting (Light / Standard / Heavy / WebSocket categories)
- Circuit breakers per provider with configurable failure thresholds
- Prometheus metrics endpoint with 20+ gauges and counters
- CORS, gzip compression, and structured JSON logging

**Tooling**
- `upp-sdk` — typed Rust client library with builder pattern (40+ methods)
- `upp` CLI — 28 commands for market data, trading, backtesting, and config
- Web monitoring dashboard at `/dashboard` with auto-refresh charts
- MCP (Model Context Protocol) and A2A agent protocol support

## Quick Start

### Prerequisites

- Rust 1.75+ (2021 edition)
- Redis (optional, for shared state — falls back to in-memory)

### Build & Run

```bash
# Clone and build
cd upp/gateway
cargo build --release

# Run with defaults (dev mode, in-memory storage)
cargo run --release

# Or with Redis + production auth
UPP_REDIS_URL=redis://localhost:6379 \
UPP_AUTH_REQUIRED=true \
cargo run --release
```

The gateway starts on:
- **REST API**: `http://localhost:9090`
- **gRPC**: `localhost:9091`
- **Dashboard**: `http://localhost:9090/dashboard`

### Using the CLI

```bash
cd upp/cli
cargo install --path .

# Configure
upp config set-url http://localhost:9090
upp config set-key YOUR_API_KEY

# Check health
upp health

# Browse markets
upp markets list --provider kalshi --limit 10
upp markets search "bitcoin" --limit 5

# View portfolio
upp portfolio summary
upp portfolio positions
upp portfolio analytics

# Place orders
upp orders create --market BTC-2026-Q1 --side buy --price 0.65 --quantity 100

# Run backtests
upp backtest strategies
upp backtest run --strategy momentum --market BTC-2026-Q1
upp backtest compare --market BTC-2026-Q1 --strategies momentum,mean_reversion,macd

# Smart routing
upp route compute --market BTC-2026-Q1 --side buy --quantity 500
```

### Using the SDK

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

    // Market data (no auth required)
    let markets = client.list_markets(None, None, None, Some(10), None).await?;
    let arb = client.list_arbitrage().await?;

    // Trading (requires auth)
    let order = client.create_order(upp_sdk::CreateOrderRequest {
        market_id: "BTC-2026-Q1".into(),
        outcome_id: "yes".into(),
        side: upp_sdk::OrderSide::Buy,
        quantity: 100.0,
        price: 0.65,
        order_type: upp_sdk::OrderType::Limit,
    }).await?;

    Ok(())
}
```

## API Reference

### Public Endpoints (no auth)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Gateway health check |
| GET | `/ready` | Readiness probe |
| GET | `/metrics` | Prometheus metrics |
| GET | `/dashboard` | Web monitoring UI |
| GET | `/upp/v1/discovery/providers` | List registered providers |
| GET | `/upp/v1/discovery/:provider` | Provider capability manifest |
| POST | `/upp/v1/discovery/negotiate` | Capability negotiation |
| GET | `/upp/v1/markets` | List markets (filterable) |
| GET | `/upp/v1/markets/search` | Full-text market search |
| GET | `/upp/v1/markets/:id` | Get market details |
| GET | `/upp/v1/markets/:id/orderbook` | Market orderbook |
| GET | `/upp/v1/markets/:id/candles` | OHLCV candle data |
| GET | `/upp/v1/markets/:id/candles/latest` | Latest candle |
| GET | `/upp/v1/arbitrage` | Arbitrage opportunities |
| GET | `/upp/v1/arbitrage/summary` | Arbitrage summary stats |
| GET | `/upp/v1/arbitrage/history` | Arbitrage execution history |
| GET | `/upp/v1/price-index/stats` | Price index statistics |
| GET | `/upp/v1/feeds/status` | Live feed connection status |
| GET | `/upp/v1/feeds/stats` | Feed throughput stats |
| GET | `/upp/v1/backtest/strategies` | Available strategies |
| POST | `/upp/v1/backtest/run` | Run a backtest |
| POST | `/upp/v1/backtest/compare` | Compare strategies |
| GET | `/upp/v1/ingestion/stats` | Ingestion pipeline stats |
| POST | `/upp/v1/ingestion/ingest` | Ingest market data |
| POST | `/upp/v1/ingestion/ingest-recent` | Bulk ingest recent data |
| GET | `/upp/v1/ws` | WebSocket price feed |

### Protected Endpoints (auth required)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/upp/v1/orders` | Create order |
| GET | `/upp/v1/orders` | List orders |
| GET | `/upp/v1/orders/:id` | Get order |
| DELETE | `/upp/v1/orders/:id` | Cancel order |
| POST | `/upp/v1/orders/cancel-all` | Cancel all orders |
| POST | `/upp/v1/orders/estimate` | Estimate order cost |
| GET | `/upp/v1/trades` | List trades |
| GET | `/upp/v1/portfolio/positions` | Portfolio positions |
| GET | `/upp/v1/portfolio/summary` | Portfolio summary |
| GET | `/upp/v1/portfolio/balances` | Account balances |
| GET | `/upp/v1/portfolio/analytics` | Portfolio analytics |
| POST | `/upp/v1/orders/route` | Compute smart route |
| POST | `/upp/v1/orders/route/execute` | Execute route |
| GET | `/upp/v1/orders/route/stats` | Routing stats |
| POST | `/upp/v1/feeds/subscribe` | Subscribe to feeds |
| POST | `/upp/v1/auth/keys` | Create API key |
| GET | `/upp/v1/auth/keys` | List API keys |
| POST | `/upp/v1/auth/keys/revoke` | Revoke API key |

## Authentication

UPP supports two authentication methods:

**API Key** — pass via `X-API-Key` header:
```bash
curl -H "X-API-Key: upp_k_abc123..." http://localhost:9090/upp/v1/orders
```

**JWT Bearer Token** — pass via `Authorization` header:
```bash
curl -H "Authorization: Bearer eyJhbG..." http://localhost:9090/upp/v1/orders
```

### Managing API Keys

```bash
# Create a new key
curl -X POST http://localhost:9090/upp/v1/auth/keys \
  -H "Content-Type: application/json" \
  -d '{"client_name": "my-bot", "tier": "pro", "expires_in_days": 90}'

# List keys
curl http://localhost:9090/upp/v1/auth/keys

# Revoke a key
curl -X POST http://localhost:9090/upp/v1/auth/keys/revoke \
  -d '{"key_prefix": "upp_k_abc12345..."}'
```

Client tiers: `free`, `standard`, `pro`, `enterprise` — each with different rate limits.

## Configuration

Environment variables (all optional, sensible defaults):

| Variable | Default | Description |
|----------|---------|-------------|
| `UPP_REST_PORT` | `9090` | REST API port |
| `UPP_GRPC_PORT` | `9091` | gRPC port |
| `UPP_REDIS_URL` | (none) | Redis URL for shared state |
| `UPP_AUTH_REQUIRED` | `false` | Enable production auth |
| `UPP_JWT_SECRET` | (none) | HS256 JWT secret |
| `UPP_LOG_FORMAT` | `pretty` | `pretty` or `json` |
| `RUST_LOG` | `info` | Log level filter |

Rate limit defaults (per-client, per-endpoint class):

| Class | Burst | Sustained |
|-------|-------|-----------|
| Light | 200 | 100 rps |
| Standard | 50 | 20 rps |
| Heavy | 10 | 2 rps |
| WebSocket | 100 | 50 rps |

## Project Structure

```
upp/
├── gateway/                 # Core gateway (Rust binary + library)
│   ├── src/
│   │   ├── main.rs          # Entry point, routes, handlers (~2500 lines)
│   │   ├── core/            # Business logic modules
│   │   │   ├── types.rs     # Unified types (UniversalMarketId, Position, etc.)
│   │   │   ├── portfolio.rs # Portfolio aggregation & analytics
│   │   │   ├── arbitrage.rs # Cross-platform arbitrage detection
│   │   │   ├── price_index.rs # OHLCV candle aggregation
│   │   │   ├── smart_router.rs # Optimal order routing
│   │   │   ├── backtest.rs  # Backtesting engine (4 strategies)
│   │   │   └── historical.rs # Data ingestion pipeline
│   │   ├── transport/       # Network layer
│   │   │   ├── websocket.rs # WebSocket fan-out
│   │   │   └── live_feed.rs # Provider WebSocket connections
│   │   ├── providers/       # Provider adapters
│   │   │   ├── kalshi.rs    # Kalshi exchange adapter
│   │   │   ├── polymarket.rs # Polymarket adapter
│   │   │   └── opinion.rs   # Opinion.trade adapter
│   │   ├── middleware/       # Request pipeline
│   │   │   ├── auth.rs      # Auth (API key, JWT, key management)
│   │   │   └── rate_limit.rs # Token-bucket rate limiter
│   │   └── storage/         # Persistence layer
│   ├── tests/               # Integration tests
│   ├── static/              # Dashboard HTML
│   └── Cargo.toml
├── sdk/                     # Rust client library
│   ├── src/
│   │   ├── lib.rs           # Public API
│   │   ├── client.rs        # HTTP client (40+ methods)
│   │   ├── types.rs         # Request/response types
│   │   └── error.rs         # Error types
│   └── Cargo.toml
├── cli/                     # Command-line tool
│   ├── src/
│   │   ├── main.rs          # 28 commands with clap v4
│   │   ├── config.rs        # Config persistence (~/.upp/)
│   │   └── output.rs        # Colored table formatting
│   └── Cargo.toml
├── proto/                   # Protocol Buffers (gRPC)
├── schemas/                 # OpenAPI spec
├── spec/                    # UPP protocol specification
├── docs/                    # Additional documentation
├── config/                  # Configuration examples
├── scripts/                 # Build/deploy scripts
├── Dockerfile               # Multi-stage production build
├── docker-compose.yml       # Full stack with Redis
└── Makefile                 # Common tasks
```

## Testing

```bash
cd upp/gateway

# Run all tests (lib + bin + integration)
cargo test

# Run with output
cargo test -- --nocapture

# Run specific module
cargo test core::backtest
cargo test middleware::auth

# Current: 243+ tests (100 lib + 100 bin + 43 integration)
```

## Docker

```bash
# Build
docker build -t upp-gateway .

# Run
docker run -p 9090:9090 -p 9091:9091 upp-gateway

# With Redis
docker-compose up -d
```

## License

Apache-2.0
