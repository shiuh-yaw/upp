# Quickstart — 5 Minutes to Your First Market Query

Let's get you trading (or at least querying) in 5 minutes using the local development stack.

## Step 1: Clone the Repository

```bash
git clone https://github.com/universal-prediction-protocol/upp.git
cd upp
```

## Step 2: Start the Local Stack

```bash
docker-compose up -d
```

Wait for all services to be healthy (usually 10-15 seconds):

```bash
docker-compose ps
```

You should see:
```
NAME                COMMAND             STATUS
upp-gateway         /app/gateway        Up (healthy)
upp-redis           redis-server        Up
upp-prometheus      prometheus          Up
upp-grafana         grafana             Up
upp-jaeger          jaeger              Up
```

## Step 3: Query Your First Market

Use curl to fetch available markets on Polymarket:

```bash
curl -X GET "http://localhost:8080/api/v1/markets?provider=polymarket&limit=5"
```

Expected response:
```json
{
  "markets": [
    {
      "id": "0x1234...abcd",
      "provider": "polymarket",
      "title": "Will ETH be above $5000 by end of Q2 2026?",
      "description": "Binary market on Ethereum price prediction",
      "outcomes": [
        {"id": "0", "name": "Yes", "price": 0.72},
        {"id": "1", "name": "No", "price": 0.28}
      ],
      "liquidity": 1250000,
      "volume_24h": 875000,
      "created_at": "2026-01-15T08:30:00Z",
      "expires_at": "2026-06-30T23:59:59Z"
    }
  ],
  "total": 1250,
  "cursor": "eyJvZmZzZXQiOiA1fQ=="
}
```

Great! You've successfully queried a live (mock) market.

## Step 4: Check Kalshi Markets

Let's try another provider. Query Kalshi's binary markets:

```bash
curl -X GET "http://localhost:8080/api/v1/markets?provider=kalshi&category=politics&limit=3"
```

Response:
```json
{
  "markets": [
    {
      "id": "ELECTION_2028_DEM",
      "provider": "kalshi",
      "title": "Will Democratic candidate win 2028 US Presidential Election?",
      "category": "politics",
      "outcomes": [
        {"id": "YES", "name": "Yes", "price": 0.58},
        {"id": "NO", "name": "No", "price": 0.42}
      ],
      "liquidity": 2100000,
      "volume_24h": 1500000,
      "created_at": "2025-10-01T00:00:00Z",
      "expires_at": "2028-11-05T23:59:59Z"
    }
  ],
  "total": 87,
  "cursor": "eyJvZmZzZXQiOiAzfQ=="
}
```

## Step 5: Search Markets by Text

Let's search for markets about AI:

```bash
curl -X GET "http://localhost:8080/api/v1/markets/search?q=artificial%20intelligence&limit=10"
```

Response:
```json
{
  "results": [
    {
      "id": "AGI_2030",
      "provider": "polymarket",
      "title": "Will Artificial General Intelligence be achieved by end of 2030?",
      "description": "Binary outcome market on AGI timeline",
      "relevance_score": 0.98,
      "outcomes": [
        {"id": "0", "name": "Yes", "price": 0.31},
        {"id": "1", "name": "No", "price": 0.69}
      ]
    }
  ],
  "total": 142
}
```

## Step 6: Check Gateway Health

Ensure all provider adapters are running:

```bash
curl -X GET "http://localhost:8080/api/v1/health"
```

Response:
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
      "status": "up",
      "latency_ms": 38,
      "last_sync": "2026-03-14T12:34:52Z"
    }
  },
  "cache": {
    "entries": 234,
    "memory_mb": 12.4
  }
}
```

## Step 7: Open the Dashboards

You now have full observability:

- **Grafana** — http://localhost:3000 (admin/admin)
  - Gateway metrics and performance dashboards
  - Real-time request rates and error tracking

- **Prometheus** — http://localhost:9090
  - Raw metrics explorer
  - Query builder for custom insights

- **Jaeger** — http://localhost:16686
  - Distributed tracing for multi-step requests
  - Latency profiling

## Next Steps

Now that you've verified the local stack works:

1. **Integrate via REST API** — See [REST API Reference](../api/rest.md) for all endpoints
2. **Build a Rust application** — See [Rust SDK Guide](../sdk/rust.md) for examples
3. **Use the CLI for scripting** — See [CLI Guide](../cli/README.md) for available commands
4. **Subscribe to real-time feeds** — See [WebSocket Protocol](../api/websocket.md)
5. **Deploy to production** — See [Deployment Guide](../operations/deployment.md)

## Troubleshooting

**Gateway not responding?**
```bash
docker-compose logs gateway
```

**Redis connection errors?**
```bash
docker-compose restart redis
```

**All services down?**
```bash
docker-compose down
docker-compose up -d
```

**Want to add real exchange credentials?** See [Configuration Guide](../operations/configuration.md) to set API keys for Kalshi, Polymarket, or Opinion.trade.

That's it! You're now ready to build with UPP.
