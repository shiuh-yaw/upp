# Architecture

UPP is built with scalability, reliability, and extensibility in mind. This section explains the system design, component interactions, and how to extend UPP with new providers.

## Architecture Overview

The system follows a classic service-oriented architecture with a central gateway routing to pluggable provider adapters. Each component has a clear responsibility and can be scaled independently.

### Components at a Glance

| Component | Purpose | Technology |
|-----------|---------|-----------|
| **Gateway** | Central hub routing, caching, auth | Axum, Tokio, Rust |
| **Provider Adapters** | Exchange-specific logic | Pattern matching, trait objects |
| **Redis** | Distributed cache, rate limiting | Redis protocol |
| **gRPC Server** | High-performance service interface | protobuf, tonic |
| **Metrics & Tracing** | Observability | Prometheus, Jaeger |
| **CLI Tool** | Command-line interface | Clap, Tokio |
| **Rust SDK** | Client library | async/await, tokio |

## Key Design Principles

1. **Provider Agnosticism** — Core logic is provider-independent; adapters handle specifics
2. **Fault Isolation** — One provider's outage doesn't crash the gateway
3. **Caching First** — Minimize external API calls; cache aggressively where appropriate
4. **Type Safety** — Strong Rust types catch errors at compile time, not runtime
5. **Observable** — Every request is traceable; metrics are built in
6. **Extensible** — New providers need ~200 lines of adapter code

## High-Level Diagram

```
┌─────────────────────────────────────────────────────┐
│                    Clients                          │
│  ┌──────────────┬──────────────┬──────────────┐    │
│  │ REST Client  │ gRPC Client  │ WebSocket    │    │
│  │  (HTTP)      │   (H2)       │  Client      │    │
│  └──────────────┴──────────────┴──────────────┘    │
└─────────────────────────────────────────────────────┘
                       ↓
┌─────────────────────────────────────────────────────┐
│              Gateway (Axum)                         │
│  ┌──────────────────────────────────────────────┐  │
│  │ Router: Route /api/v1/... to handlers        │  │
│  │ Middleware: Auth, logging, tracing           │  │
│  │ WebSocket: Subscription manager, fan-out     │  │
│  └──────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
         ↓              ↓              ↓
    ┌────────────┐ ┌────────────┐ ┌─────────────┐
    │ Kalshi     │ │ Polymarket │ │ Opinion     │
    │ Adapter    │ │ Adapter    │ │ Adapter     │
    └────────────┘ └────────────┘ └─────────────┘
         ↓              ↓              ↓
    ┌────────────┐ ┌────────────┐ ┌─────────────┐
    │ Kalshi API │ │ Polymarket │ │ Opinion     │
    │            │ │ API        │ │ API         │
    └────────────┘ └────────────┘ └─────────────┘
```

## Request Flow

1. **Client sends request** — REST, gRPC, or WebSocket
2. **Gateway router matches path** — `/api/v1/markets` → `handle_markets()`
3. **Middleware executes**:
   - Auth validation
   - Request logging and tracing
   - Rate limiting check
4. **Cache lookup** — Check Redis for cached response
5. **If cache miss**:
   - Route to appropriate adapter(s)
   - Adapter queries external API
   - Response cached in Redis (TTL-based)
6. **Response sent** to client

## Pages in This Section

- **[System Design](overview.md)** — Detailed architecture diagrams and data flow
- **[Gateway Internals](gateway.md)** — Router, middleware, caching, WebSocket handling
- **[Provider Adapters](providers.md)** — How adapters work and how to add new ones

## Core Concepts

### Provider Adapter Pattern

Adapters implement a common trait:

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn get_markets(
        &self,
        filter: MarketFilter,
    ) -> Result<Vec<Market>, ProviderError>;

    async fn get_orders(
        &self,
        user_id: &str,
    ) -> Result<Vec<Order>, ProviderError>;

    async fn place_order(
        &self,
        order: OrderRequest,
    ) -> Result<OrderResponse, ProviderError>;

    // ... more methods
}
```

Each provider implements this trait, encapsulating provider-specific logic. The gateway is provider-agnostic; it just calls methods on the trait object.

### Caching Strategy

UPP uses intelligent caching:

| Endpoint | Cache TTL | Reason |
|----------|-----------|--------|
| `/markets` | 60s | Market data changes slowly |
| `/orders` | 10s | Orders change frequently, but polling is acceptable |
| `/portfolio` | 30s | Balance updates reasonably fast |
| `/health` | 5s | Short TTL for responsiveness |
| `/backtest` | None | Results are deterministic; cache forever |

Cache keys are constructed hierarchically:

```
markets:{provider}:{category}:{limit}:{offset}
orders:{provider}:{user_id}
portfolio:{provider}:{user_id}
```

### Rate Limiting

Per-provider and global rate limits prevent overwhelming external APIs:

```rust
// Per-provider: Polymarket allows ~100 req/s
// Per-client: Each client gets 10 req/s quota
// Burst: Allow 20 requests in 1 second spike

let rate_limiter = RateLimiter::new(
    quota: 10,              // req/sec
    burst: 20,              // max simultaneous
    window: Duration::from_secs(1),
);
```

### Error Handling & Resilience

Adapters return typed errors:

```rust
pub enum ProviderError {
    RateLimited(Duration),     // How long to wait
    NetworkError(String),
    InvalidCredentials,
    NotFound(String),
    InternalServerError,
}
```

Gateway implements retry logic:

```rust
// Retry with exponential backoff
// RateLimited → respect backoff duration
// NetworkError → retry up to 3 times
// InvalidCredentials → fail immediately
// NotFound → fail immediately
// InternalServerError → retry up to 2 times
```

## Scalability Considerations

- **Horizontal** — Gateway is stateless (except WebSocket subscriptions); add more instances
- **Vertical** — Tune Tokio runtime, connection pool sizes, cache size
- **Cache** — Use separate Redis cluster for production; local Redis fine for dev
- **Database** — Optional persistence layer for order history, audit logs
- **Load Balancing** — Put Nginx/HAProxy in front; route by gateway instance

## Next Section

Ready to dive deeper? Start with [System Design](overview.md) to understand data flow and architecture diagrams.
