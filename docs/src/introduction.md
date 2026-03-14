# Introduction to UPP

## What is Universal Prediction Protocol?

UPP (Universal Prediction Protocol) is a unified, production-grade platform for interacting with decentralized prediction markets across multiple providers. It abstracts away provider-specific APIs, offering a single standardized interface for querying markets, placing orders, and managing positions across Kalshi, Polymarket, Opinion.trade, and other supported exchanges.

Whether you're a quantitative trader, a market researcher, or building a prediction market application, UPP simplifies the complexity of multi-exchange trading and provides battle-tested tools for serious market participants.

## Why UPP Exists

The prediction market ecosystem is fragmented. Each major exchange—Kalshi (regulated US markets), Polymarket (global crypto-native platform), Opinion.trade (emerging alternative)—maintains its own API, authentication scheme, rate limiting, and quirks. Integrating with multiple exchanges means:

- **Duplicated code** across SDKs and adapters
- **Inconsistent error handling** and retry logic
- **Market surveillance challenges** without unified feeds
- **Complex position tracking** across multiple platforms

UPP solves this by providing:
- A **unified REST API** and **gRPC interface** that works across all providers
- A **Rust SDK** for low-latency, type-safe client development
- A **CLI tool** for ad-hoc queries and market research
- **WebSocket subscriptions** for real-time data feeds
- **Adapter pattern** for easy addition of new providers
- Built-in **caching, rate limiting, and resilience**

## Architecture at a Glance

UPP consists of four core components:

### Gateway (`gateway/`)

The central hub. An Axum-based REST and gRPC server that:
- Routes requests to the appropriate provider adapter
- Implements caching, rate limiting, and request deduplication
- Manages WebSocket subscriptions and fan-out to clients
- Provides structured logging and metrics (Prometheus)
- Handles authentication and per-tenant isolation

### SDK (`sdk/`)

A Rust client library for programmatic access. Features:
- Async/await builder pattern for safe request construction
- Both REST and WebSocket client variants
- Automatic retries with exponential backoff
- Type-safe error handling with `Result<T, Error>`
- Streaming support for market feeds and backtest results

### CLI (`cli/`)

A command-line tool for quick market queries without writing code:
- Health checks and provider status
- Market search and filtering
- Order management (view, place, cancel)
- Portfolio aggregation across exchanges
- Arbitrage detection between markets
- Historical data export

### Provider Adapters

Modular adapters that translate between UPP's protocol and each exchange:
- **Kalshi Adapter** — Regulated US prediction markets
- **Polymarket Adapter** — Global crypto-native platform
- **Opinion.trade Adapter** — Emerging alternative exchange
- Extensible pattern for adding new providers

## Supported Providers

UPP natively supports:

| Provider | Markets | Auth | Notes |
|----------|---------|------|-------|
| **Kalshi** | US-regulated binary events | API key + secret | Full order book, real-time updates |
| **Polymarket** | Global crypto-native | Private key (ECDSA) | Largest prediction market by volume |
| **Opinion.trade** | Emerging alternative | API key | Early-stage but growing liquidity |

Each provider has its own adapter in `gateway/src/adapters/` that handles:
- Authentication and credential refresh
- Market and order data translation
- Rate limiting and retry policies
- Provider-specific quirks and edge cases

## High-Level Data Flow

```
Client (REST/gRPC/CLI)
    ↓
  Gateway (Axum server)
    ↓
  Router + Middleware (auth, caching, rate limit)
    ↓
  Provider Adapters (Kalshi, Polymarket, Opinion.trade)
    ↓
  External Exchange APIs
```

WebSocket connections follow a different path:

```
Client (WebSocket)
    ↓
  Gateway WebSocket handler
    ↓
  Subscription manager (tracks active subscriptions)
    ↓
  Background poller (fetches data from adapters)
    ↓
  Fan-out to all subscribed clients
```

## Next Steps

- **New to UPP?** Start with the [Quickstart](getting-started/quickstart.md) — spin up the local stack and query your first market in 5 minutes.
- **Want to integrate UPP?** Check out the [Installation Guide](getting-started/installation.md) and [Rust SDK](sdk/rust.md).
- **Deploying to production?** See [Deployment](operations/deployment.md) for containerization and orchestration guides.
- **Contributing?** Read [Contributing](development/contributing.md) to set up your development environment.

## Key Features

- **Provider-agnostic API** — Single interface, multiple exchanges
- **High performance** — Caching, connection pooling, optimized adapters
- **Real-time feeds** — WebSocket subscriptions for live market data
- **Type safety** — Rust throughout with strong error types
- **Observability** — Prometheus metrics, structured logging, Jaeger tracing
- **Easy deployment** — Docker Compose for local dev, Kubernetes examples for production
- **Extensible** — Add new providers in minutes with the adapter pattern

## License

UPP is released under the Apache 2.0 license. See LICENSE in the repository.

## Community

- **GitHub Discussions** — Ask questions and share ideas
- **Discord** — Real-time community chat (link in GitHub)
- **Issues** — Report bugs or request features
