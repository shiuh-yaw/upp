# API Reference

UPP provides multiple interfaces for interacting with prediction markets: REST, gRPC, and WebSocket. This section documents all APIs with examples.

## Available Interfaces

| Interface | Protocol | Use Case | Latency |
|-----------|----------|----------|---------|
| **REST API** | HTTP/1.1 | Web apps, general clients | 50-200ms |
| **gRPC** | HTTP/2 | Backends, service-to-service | 10-50ms |
| **WebSocket** | WS/WSS | Real-time feeds, subscriptions | 10-100ms |

## Authentication

All endpoints (except `/health`) require authentication.

### REST API

Include an `Authorization` header:

```bash
curl -H "Authorization: Bearer YOUR_API_KEY" \
  http://localhost:8080/api/v1/markets
```

### gRPC

Pass credentials in the gRPC metadata:

```rust
let mut request = tonic::Request::new(GetMarketsRequest::default());
request.metadata_mut().insert(
    "authorization",
    format!("Bearer {}", api_key).parse().unwrap(),
);
let response = client.get_markets(request).await?;
```

### WebSocket

Send credentials in the first message after connecting:

```javascript
const ws = new WebSocket('ws://localhost:8080/api/v1/feed');
ws.onopen = () => {
  ws.send(JSON.stringify({
    action: 'authenticate',
    token: 'YOUR_API_KEY'
  }));
};
```

## Response Formats

### Success Response (REST)

```json
{
  "data": {
    "markets": [...]
  },
  "meta": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp": "2026-03-14T12:34:56Z"
  }
}
```

### Error Response

```json
{
  "error": {
    "code": "INVALID_PROVIDER",
    "message": "Unknown provider: 'invalid'",
    "details": {
      "available_providers": ["polymarket", "kalshi", "opinion_trade"]
    }
  },
  "meta": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp": "2026-03-14T12:34:56Z"
  }
}
```

### Error Codes

| Code | HTTP Status | Meaning |
|------|-------------|---------|
| `INVALID_PROVIDER` | 400 | Unknown provider |
| `INVALID_INPUT` | 400 | Malformed request |
| `UNAUTHORIZED` | 401 | Missing or invalid credentials |
| `RATE_LIMITED` | 429 | Too many requests |
| `NOT_FOUND` | 404 | Resource not found |
| `INTERNAL_ERROR` | 500 | Server error |

## Common Concepts

### Market

A binary prediction market:

```json
{
  "id": "0x1234...abcd",
  "provider": "polymarket",
  "title": "Will ETH exceed $5000 by Q2 2026?",
  "description": "Binary prediction on Ethereum price",
  "category": "crypto",
  "outcomes": [
    {
      "id": "0",
      "name": "Yes",
      "price": 0.72,
      "probability": 0.72
    },
    {
      "id": "1",
      "name": "No",
      "price": 0.28,
      "probability": 0.28
    }
  ],
  "liquidity": 1250000,
  "volume_24h": 875000,
  "created_at": "2026-01-15T08:30:00Z",
  "expires_at": "2026-06-30T23:59:59Z"
}
```

### Order

A placed trade:

```json
{
  "id": "order_12345",
  "market_id": "0x1234...abcd",
  "provider": "polymarket",
  "side": "BUY",
  "outcome": "Yes",
  "price": 0.72,
  "quantity": 100,
  "filled": 100,
  "status": "FILLED",
  "created_at": "2026-03-14T12:00:00Z"
}
```

### Portfolio

User's current positions:

```json
{
  "user_id": "user123",
  "balance": 5000,
  "positions": [
    {
      "market_id": "0x1234...abcd",
      "outcome": "Yes",
      "quantity": 100,
      "entry_price": 0.65,
      "current_price": 0.72,
      "pnl": 700,
      "pnl_percent": 0.14
    }
  ],
  "total_pnl": 750,
  "total_pnl_percent": 0.15
}
```

## Pages in This Section

- **[REST API](rest.md)** — HTTP endpoints for markets, orders, portfolio, and more
- **[gRPC Services](grpc.md)** — High-performance Protocol Buffer services
- **[WebSocket Protocol](websocket.md)** — Real-time subscriptions and feeds

## Quick Start

Get markets in 30 seconds:

```bash
# REST
curl -H "Authorization: Bearer api_key" \
  "http://localhost:8080/api/v1/markets?provider=polymarket&limit=5"

# Or with CLI
upp markets list --provider polymarket --limit 5

# Or with Rust SDK
use upp_sdk::Client;
let client = Client::new("http://localhost:8080", "api_key");
let markets = client.get_markets().provider("polymarket").limit(5).fetch().await?;
```

## Rate Limiting

API has two rate limits:

1. **Per-client** — 10 requests/second (burst to 20)
2. **Per-provider** — 100 requests/second (burst to 200)

When limited, the response includes a `Retry-After` header:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 5
X-RateLimit-Reset: 1710425096
```

## Pagination

For large result sets, use cursor-based pagination:

```bash
# Get first page
curl "http://localhost:8080/api/v1/markets?limit=10"

# Response includes cursor
{
  "markets": [...],
  "cursor": "eyJvZmZzZXQiOiAxMH0="
}

# Get next page
curl "http://localhost:8080/api/v1/markets?limit=10&cursor=eyJvZmZzZXQiOiAxMH0="
```

## API Versions

Current version: **v1**

APIs are versioned in the path: `/api/v1/`

Breaking changes will be released as `/api/v2/`

## Monitoring

All API calls are tracked:

- **Request count** — Prometheus metric `upp_api_requests_total`
- **Latency** — Metric `upp_api_latency_ms`
- **Errors** — Metric `upp_api_errors_total`

Access dashboards:
- Grafana: http://localhost:3000
- Prometheus: http://localhost:9090
