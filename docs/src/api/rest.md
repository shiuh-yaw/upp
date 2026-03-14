# REST API Reference

Complete reference for HTTP/1.1 endpoints. All responses are JSON unless otherwise noted.

## Base URL

```
http://localhost:8080/api/v1
```

## Health & Status

### Health Check

Get server and provider health status.

**Request:**

```bash
curl -X GET "http://localhost:8080/api/v1/health"
```

**Response (200 OK):**

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "providers": {
    "polymarket": {
      "status": "up",
      "latency_ms": 45,
      "last_sync": "2026-03-14T12:34:56Z"
    },
    "kalshi": {
      "status": "up",
      "latency_ms": 62,
      "last_sync": "2026-03-14T12:34:50Z"
    },
    "opinion_trade": {
      "status": "down",
      "latency_ms": 0,
      "last_sync": "2026-03-14T11:45:20Z"
    }
  },
  "uptime_seconds": 86400,
  "cache": {
    "entries": 234,
    "memory_mb": 12.4
  }
}
```

## Markets

### List Markets

Get markets from a specific provider or search across all.

**Request:**

```bash
# Get 10 Polymarket markets
curl -X GET "http://localhost:8080/api/v1/markets?provider=polymarket&limit=10"

# Get markets by category (Kalshi)
curl -X GET "http://localhost:8080/api/v1/markets?provider=kalshi&category=politics&limit=20"

# Get markets with offset (pagination)
curl -X GET "http://localhost:8080/api/v1/markets?provider=polymarket&limit=10&offset=20"
```

**Response (200 OK):**

```json
{
  "markets": [
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
      "expires_at": "2026-06-30T23:59:59Z",
      "status": "active"
    }
  ],
  "total": 1250,
  "cursor": "eyJvZmZzZXQiOiAxMH0="
}
```

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `provider` | string | - | Market provider (polymarket, kalshi, opinion_trade) |
| `category` | string | - | Category filter (crypto, politics, sports, etc.) |
| `limit` | integer | 10 | Max results (max 100) |
| `offset` | integer | 0 | Pagination offset |
| `status` | string | active | Filter by status (active, resolved, cancelled) |

### Get Market Details

Fetch a specific market by ID.

**Request:**

```bash
curl -X GET "http://localhost:8080/api/v1/markets/0x1234...abcd"
```

**Response (200 OK):**

```json
{
  "market": {
    "id": "0x1234...abcd",
    "provider": "polymarket",
    "title": "Will ETH exceed $5000 by Q2 2026?",
    "description": "Binary prediction on Ethereum price",
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
    "volume_1h": 12500,
    "spread": 0.01,
    "created_at": "2026-01-15T08:30:00Z",
    "expires_at": "2026-06-30T23:59:59Z",
    "history": {
      "price_24h_ago": 0.68,
      "price_7d_ago": 0.60
    }
  }
}
```

### Search Markets

Full-text search across all markets.

**Request:**

```bash
# Search for AI-related markets
curl -X GET "http://localhost:8080/api/v1/markets/search?q=artificial+intelligence&limit=10"

# Search with provider filter
curl -X GET "http://localhost:8080/api/v1/markets/search?q=ethereum&provider=polymarket&limit=5"
```

**Response (200 OK):**

```json
{
  "results": [
    {
      "id": "0x5678...efgh",
      "provider": "polymarket",
      "title": "Will Artificial General Intelligence be achieved by 2030?",
      "description": "Binary market on AGI timeline",
      "relevance_score": 0.98,
      "outcomes": [
        {
          "id": "0",
          "name": "Yes",
          "price": 0.31
        },
        {
          "id": "1",
          "name": "No",
          "price": 0.69
        }
      ]
    }
  ],
  "total": 142
}
```

## Orders

### List Orders

Get user's orders (requires authentication).

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/orders?provider=polymarket&status=open"
```

**Response (200 OK):**

```json
{
  "orders": [
    {
      "id": "order_12345",
      "market_id": "0x1234...abcd",
      "provider": "polymarket",
      "side": "BUY",
      "outcome": "Yes",
      "price": 0.72,
      "quantity": 100,
      "filled": 75,
      "remaining": 25,
      "status": "PARTIALLY_FILLED",
      "created_at": "2026-03-14T10:00:00Z",
      "updated_at": "2026-03-14T11:30:00Z"
    }
  ],
  "total": 5
}
```

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `provider` | string | Filter by provider |
| `status` | string | Filter by status (open, filled, cancelled) |
| `market_id` | string | Filter by market |
| `limit` | integer | Max results |

### Get Order Details

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/orders/order_12345"
```

**Response (200 OK):**

```json
{
  "order": {
    "id": "order_12345",
    "market_id": "0x1234...abcd",
    "provider": "polymarket",
    "side": "BUY",
    "outcome": "Yes",
    "price": 0.72,
    "quantity": 100,
    "filled": 100,
    "status": "FILLED",
    "created_at": "2026-03-14T10:00:00Z",
    "filled_at": "2026-03-14T10:05:00Z"
  }
}
```

### Place Order

Create a new market order.

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -H "Content-Type: application/json" \
  -X POST "http://localhost:8080/api/v1/orders" \
  -d '{
    "provider": "polymarket",
    "market_id": "0x1234...abcd",
    "side": "BUY",
    "outcome": "Yes",
    "price": 0.72,
    "quantity": 100
  }'
```

**Response (201 Created):**

```json
{
  "order": {
    "id": "order_12346",
    "market_id": "0x1234...abcd",
    "provider": "polymarket",
    "side": "BUY",
    "outcome": "Yes",
    "price": 0.72,
    "quantity": 100,
    "filled": 0,
    "status": "OPEN",
    "created_at": "2026-03-14T12:00:00Z"
  }
}
```

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `provider` | string | Yes | Market provider |
| `market_id` | string | Yes | Market ID |
| `side` | string | Yes | BUY or SELL |
| `outcome` | string | Yes | Outcome name or ID |
| `price` | float | Yes | Price [0, 1] |
| `quantity` | float | Yes | Quantity to trade |

### Cancel Order

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -X DELETE "http://localhost:8080/api/v1/orders/order_12345"
```

**Response (200 OK):**

```json
{
  "success": true,
  "message": "Order cancelled"
}
```

## Portfolio

### Get Portfolio

Get user's current positions and balances.

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/portfolio"
```

**Response (200 OK):**

```json
{
  "portfolio": {
    "user_id": "user123",
    "cash_balance": 2500,
    "total_value": 7500,
    "positions": [
      {
        "market_id": "0x1234...abcd",
        "provider": "polymarket",
        "outcome": "Yes",
        "quantity": 100,
        "entry_price": 0.65,
        "current_price": 0.72,
        "pnl": 700,
        "pnl_percent": 0.107
      },
      {
        "market_id": "0x5678...efgh",
        "provider": "kalshi",
        "outcome": "No",
        "quantity": 500,
        "entry_price": 0.40,
        "current_price": 0.38,
        "pnl": -100,
        "pnl_percent": -0.05
      }
    ],
    "total_pnl": 600,
    "total_pnl_percent": 0.08
  }
}
```

### Get Positions

Get detailed position information with aggregation options.

**Request:**

```bash
# Get all positions
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/portfolio/positions"

# Get positions for specific provider
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/portfolio/positions?provider=polymarket"

# Group by outcome
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/portfolio/positions?group_by=outcome"
```

**Response (200 OK):**

```json
{
  "positions": [
    {
      "market_id": "0x1234...abcd",
      "provider": "polymarket",
      "outcome": "Yes",
      "quantity": 100,
      "entry_price": 0.65,
      "current_price": 0.72,
      "pnl": 700,
      "pnl_percent": 0.107
    }
  ],
  "summary": {
    "total_positions": 2,
    "total_quantity": 600,
    "total_pnl": 600,
    "total_pnl_percent": 0.08
  }
}
```

## Arbitrage

### Find Arbitrage Opportunities

Identify price discrepancies across exchanges.

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -X GET "http://localhost:8080/api/v1/arbitrage/opportunities?min_spread=0.05&limit=10"
```

**Response (200 OK):**

```json
{
  "opportunities": [
    {
      "market_id": "0x1234...abcd",
      "title": "Will ETH exceed $5000 by Q2 2026?",
      "outcome": "Yes",
      "buy_exchange": {
        "provider": "kalshi",
        "price": 0.68,
        "liquidity": 500000
      },
      "sell_exchange": {
        "provider": "polymarket",
        "price": 0.74,
        "liquidity": 1200000
      },
      "spread": 0.06,
      "spread_percent": 8.8,
      "max_volume": 50000,
      "potential_profit": 3000
    }
  ]
}
```

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `min_spread` | float | 0.02 | Minimum spread (0.02 = 2%) |
| `limit` | integer | 20 | Max results |

## Backtesting

### Run Backtest

Simulate trading strategy over historical data.

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -H "Content-Type: application/json" \
  -X POST "http://localhost:8080/api/v1/backtest" \
  -d '{
    "market_id": "0x1234...abcd",
    "provider": "polymarket",
    "start_date": "2025-09-01",
    "end_date": "2026-03-14",
    "initial_balance": 10000,
    "trades": [
      {
        "date": "2025-10-01",
        "side": "BUY",
        "outcome": "Yes",
        "quantity": 1000,
        "price": 0.50
      },
      {
        "date": "2026-01-15",
        "side": "SELL",
        "outcome": "Yes",
        "quantity": 1000,
        "price": 0.72
      }
    ]
  }'
```

**Response (200 OK):**

```json
{
  "backtest": {
    "market_id": "0x1234...abcd",
    "initial_balance": 10000,
    "final_balance": 12200,
    "total_pnl": 2200,
    "total_pnl_percent": 0.22,
    "max_drawdown": -800,
    "max_drawdown_percent": -0.08,
    "sharpe_ratio": 1.45,
    "results": [
      {
        "date": "2025-10-01",
        "action": "BUY",
        "price": 0.50,
        "quantity": 1000,
        "balance": 9500
      },
      {
        "date": "2026-01-15",
        "action": "SELL",
        "price": 0.72,
        "quantity": 1000,
        "balance": 12200,
        "pnl": 220
      }
    ]
  }
}
```

## Routing & Smart Order Routing

### Smart Route

Find best execution across exchanges.

**Request:**

```bash
curl -H "Authorization: Bearer api_key" \
  -H "Content-Type: application/json" \
  -X POST "http://localhost:8080/api/v1/routing/smart" \
  -d '{
    "market_id": "0x1234...abcd",
    "side": "BUY",
    "outcome": "Yes",
    "quantity": 500,
    "target_price": 0.75,
    "max_slippage": 0.01
  }'
```

**Response (200 OK):**

```json
{
  "route": {
    "exchanges": [
      {
        "provider": "polymarket",
        "quantity": 300,
        "price": 0.72,
        "total": 216
      },
      {
        "provider": "kalshi",
        "quantity": 200,
        "price": 0.73,
        "total": 146
      }
    ],
    "total_quantity": 500,
    "weighted_avg_price": 0.724,
    "total_cost": 362,
    "slippage": 0.004,
    "slippage_percent": 0.54
  }
}
```

## Error Responses

### Invalid Provider (400)

```json
{
  "error": {
    "code": "INVALID_PROVIDER",
    "message": "Unknown provider: 'invalid'",
    "details": {
      "available_providers": ["polymarket", "kalshi", "opinion_trade"]
    }
  }
}
```

### Rate Limited (429)

```json
{
  "error": {
    "code": "RATE_LIMITED",
    "message": "Too many requests"
  }
}
```

Headers include:
```
Retry-After: 60
X-RateLimit-Reset: 1710425096
```

### Not Found (404)

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Market not found: 0x1234"
  }
}
```

### Unauthorized (401)

```json
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "Invalid or missing API key"
  }
}
```
