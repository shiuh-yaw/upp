# Universal Prediction Protocol (UPP) - Adoption Assets

Professional, production-ready assets for deploying and adopting the Universal Prediction Protocol.

## Quick Links

- **Landing Page**: Open `docs/index.html` in a browser
- **API Documentation**: Visit `docs/swagger.html` for interactive API explorer
- **API Specification**: Read `docs/openapi.json` for the full OpenAPI 3.1.0 spec
- **Asset Overview**: See `ADOPTION_ASSETS.md` for detailed documentation

## What's Included

### Documentation (102 KB total)
- **Landing Page** (`docs/index.html` - 925 lines): Modern, responsive marketing page with hero, features, code examples, architecture diagram, and quick start guide
- **Swagger UI** (`docs/swagger.html` - 33 KB): Interactive API documentation with embedded OpenAPI spec
- **OpenAPI Spec** (`docs/openapi.json` - 37 KB): Complete REST API specification with 30 endpoints and 11 schemas

### SDK Configuration (5.2 KB total)
- **TypeScript/JavaScript** (`sdks/typescript/package.json`): npm-ready @upp/sdk package with ESM, CJS, and TypeScript support
- **Python** (`sdks/python/pyproject.toml`): PyPI-ready upp-sdk package with full tooling configuration

### Deployment (4.8 KB total)
- **Dockerfile**: Multi-stage build optimized for production (rust → debian)
- **Docker Compose**: Complete stack with gateway, PostgreSQL, Redis, Prometheus, and Grafana

## Running Locally

### Option 1: View Documentation (No Setup Required)
```bash
# Open landing page
open docs/index.html

# Open interactive API docs
open docs/swagger.html
```

### Option 2: Start Full Stack
```bash
# Build and start all services
docker-compose -f docker-compose-adoption.yml up -d

# Services available at:
# - REST API: http://localhost:8080
# - Swagger UI: http://localhost:8080/api/docs
# - Prometheus: http://localhost:9090
# - Grafana: http://localhost:3000
# - Database: localhost:5432
# - Cache: localhost:6379
```

### Option 3: Development Setup
```bash
# TypeScript SDK
cd sdks/typescript
npm install
npm run build
npm test

# Python SDK
cd sdks/python
pip install -e ".[dev]"
pytest
```

## API Endpoints

| Category | Endpoints | Examples |
|----------|-----------|----------|
| **Discovery** | /health, /ready, /metrics, /.well-known/* | Service health, provider list |
| **Markets** | /markets, /markets/search, /markets/:id, /orderbook | List, search, details, order flow data |
| **Trading** | /orders (POST/GET), /trades, /orders/:id (DELETE) | Create, list, cancel orders |
| **Portfolio** | /positions, /summary, /balances | Position tracking, PnL, available funds |
| **MCP** | /mcp/tools, /mcp/schema, /mcp/execute | AI agent integration |
| **Settlement** | /resolutions, /settlement/* | Resolution tracking and settlement data |

## Features Highlighted

✨ **Multi-Provider**: Unified access to Kalshi, Polymarket, Opinion.trade
✨ **Real-Time**: WebSocket support for live market data
✨ **AI-Native**: Built on Model Context Protocol (MCP)
✨ **Type-Safe**: Complete TypeScript and Python support
✨ **Production-Ready**: Circuit breakers, rate limiting, health checks
✨ **Open Standard**: Protocol-first design, MIT licensed

## SDK Usage Examples

### TypeScript
```typescript
import { UPPClient } from '@upp/sdk';

const client = new UPPClient({
  baseURL: 'http://localhost:8080',
  apiKey: process.env.UPP_API_KEY
});

// Fetch markets
const markets = await client.markets.list({
  query: 'Trump 2024',
  limit: 10
});

for (const market of markets) {
  console.log(market.name, market.probability);
}
```

### Python
```python
from upp import UPPClient

client = UPPClient(
    base_url="http://localhost:8080",
    api_key="your-api-key"
)

# Fetch markets
markets = client.markets.list(
    query="Trump 2024",
    limit=10
)

for market in markets:
    print(f"{market.name}: {market.probability}")
```

## Configuration

### Environment Variables
```bash
# Core
UPP_LISTEN_ADDR=0.0.0.0:8080
UPP_GRPC_ADDR=0.0.0.0:50051
UPP_LOG_LEVEL=info

# Providers
KALSHI_API_KEY=your-key
KALSHI_API_URL=https://api.kalshi.com
POLYMARKET_API_KEY=your-key
POLYMARKET_API_URL=https://api.polymarket.com
OPINION_API_KEY=your-key
OPINION_API_URL=https://api.opinion.trade

# Rate Limiting
UPP_RATE_LIMIT_REQUESTS=1000
UPP_RATE_LIMIT_WINDOW=60

# Circuit Breaker
UPP_CIRCUIT_BREAKER_THRESHOLD=50
UPP_CIRCUIT_BREAKER_TIMEOUT=30

# Database & Cache
UPP_DATABASE_URL=postgres://upp:upp@postgres:5432/upp
UPP_REDIS_URL=redis://redis:6379
```

## Design Details

### Landing Page Color Scheme
- **Background**: `#0a0a0f` (near-black)
- **Cards**: `#1a1a2e` (dark blue-gray)
- **Accent**: `#00d4ff` (bright cyan)
- **Text**: `#e0e0e0` (light gray)
- **Muted**: `#707090` (medium gray)

### Fonts
- **UI Text**: Inter (Google Fonts)
- **Code**: JetBrains Mono (Google Fonts)

### Responsive Design
- Mobile-first approach
- Grid layouts adapt from 3 columns → 2 columns → 1 column
- Touch-friendly button sizing
- Full viewport height hero section

## File Structure
```
upp/
├── docs/
│   ├── index.html              # Landing page
│   ├── swagger.html            # Interactive API docs
│   └── openapi.json            # API specification
├── sdks/
│   ├── typescript/
│   │   └── package.json        # npm package config
│   └── python/
│       └── pyproject.toml      # pip package config
├── Dockerfile                  # Production build
├── docker-compose-adoption.yml # Full stack compose
├── ADOPTION_ASSETS.md          # Detailed inventory
└── README_ADOPTION.md          # This file
```

## Quality Standards

All assets meet enterprise adoption requirements:

- ✅ **Self-contained**: No build tools required for docs
- ✅ **Standards-compliant**: OpenAPI 3.1.0, Docker Compose 3.8
- ✅ **Production-ready**: Full error handling, monitoring, health checks
- ✅ **Type-safe**: TypeScript definitions, Pydantic models
- ✅ **Monitored**: Prometheus metrics, Grafana dashboards
- ✅ **Documented**: Comprehensive inline and external docs
- ✅ **Open Source**: MIT license, no dependencies on proprietary tools
- ✅ **Professional**: Enterprise-grade design and styling

## Next Steps

1. **Customize** provider API keys in docker-compose configuration
2. **Deploy** using Docker Compose for local dev or Kubernetes for production
3. **Integrate** TypeScript or Python SDK into your application
4. **Monitor** using the included Prometheus and Grafana stack
5. **Extend** with custom adapters for additional providers

## Support & Documentation

- Full API reference: `docs/swagger.html`
- OpenAPI specification: `docs/openapi.json`
- Asset overview: `ADOPTION_ASSETS.md`
- Configuration guide: `docker-compose-adoption.yml`
- SDK examples: `sdks/typescript` and `sdks/python`

## License

MIT License - See individual files for license headers

---

**UPP Version**: 0.1.0
**Created**: March 2026
**Status**: Production Ready
