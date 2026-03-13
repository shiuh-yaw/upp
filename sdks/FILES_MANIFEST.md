# SDK Files Manifest

## Directory Structure

```
/mnt/outputs/upp/sdks/
├── FEATURE_SUMMARY.md           Complete feature overview
├── INTEGRATION_GUIDE.md          Integration and usage guide
├── FILES_MANIFEST.md            This file
│
├── typescript/                  TypeScript SDK
│   ├── src/
│   │   ├── types.ts            Type definitions and interfaces
│   │   ├── client.ts           REST API client
│   │   ├── websocket.ts        WebSocket client
│   │   ├── mcp.ts              MCP helpers
│   │   └── index.ts            Main entry point
│   ├── package.json            npm package configuration
│   ├── tsconfig.json           TypeScript compiler configuration
│   └── README.md               Complete documentation
│
└── python/                      Python SDK
    ├── upp/
    │   ├── types.py            Pydantic models
    │   ├── client.py           REST API client
    │   ├── websocket.py        WebSocket client
    │   ├── mcp.py              MCP helpers
    │   └── __init__.py         Package initialization
    ├── pyproject.toml          Python package configuration
    └── README.md               Complete documentation
```

## TypeScript SDK Files

### `/typescript/src/types.ts` (600+ lines)

**Purpose:** Type definitions for all UPP data models

**Contents:**
- UniversalMarketId (with to_full_id, parse methods)
- Market, Event, Outcome
- MarketType, MarketStatus, MarketPricing, MarketVolume, MarketLifecycle
- MarketRules, MarketRegulatory, KycLevel
- Order, OrderFees, OrderStatus, Side, OrderType, TimeInForce
- Position, PositionStatus
- Trade, TradeRole
- PaginationRequest, PaginationResponse
- UppError
- ProviderManifest, WellKnown, HealthStatus
- Response wrappers (MarketsResponse, OrdersResponse, etc.)
- OrderbookSnapshot, OrderbookResponse, MergedOrderbookResponse
- CreateOrderRequest, EstimateOrderRequest, OrderEstimate
- CancelAllOrdersRequest, CancelAllOrdersResponse
- McpTool, McpToolsResponse, McpExecuteRequest/Response, McpSchemaResponse
- JsonRpcRequest, JsonRpcResponse
- PriceSubscription, OrderbookSubscription, PriceUpdate, OrderbookUpdate
- AgentCard

**Key Features:**
- Full JSDoc documentation
- Discriminated unions for enums
- Generic response wrappers
- Complete type safety

---

### `/typescript/src/client.ts` (600+ lines)

**Purpose:** Main REST API client

**Class:** `UppClient`

**Configuration:**
- baseUrl (required)
- apiKey (optional)
- timeout (default: 30000ms)
- fetch implementation (default: globalThis.fetch)

**Methods:**
- Health (3): health(), ready(), metrics()
- Discovery (6): getWellKnown(), listProviders(), getManifest(), negotiate(), checkProviderHealth(), checkAllProviderHealth()
- Markets (9): listMarkets(), searchMarkets(), getMarket(), getOrderbook(), getMergedOrderbook(), listCategories(), getResolution(), listResolutions()
- Settlement (2): listSettlementInstruments(), listSettlementHandlers()
- Trading (7): createOrder(), listOrders(), getOrder(), cancelOrder(), cancelAllOrders(), estimateOrder(), listTrades()
- Portfolio (3): listPositions(), getPortfolioSummary(), listPortfolioBalances()
- MCP (4): listMcpTools(), getMcpSchema(), executeMcpTool(), getAgentCard()

**Error Handling:**
- UppApiError exception class
- HTTP status codes
- Error codes and details
- Request IDs for tracking

**Key Features:**
- Automatic timeout handling
- Query parameter serialization
- Undefined value filtering
- Type-safe responses
- Bearer token authentication

---

### `/typescript/src/websocket.ts` (500+ lines)

**Purpose:** Real-time WebSocket client for market subscriptions

**Class:** `UppWebSocket`

**Configuration:**
- url (required)
- reconnect options (enabled, maxAttempts, initialDelayMs, maxDelayMs, backoffMultiplier)
- heartbeatInterval (default: 30000ms)
- WebSocket implementation (default: globalThis.WebSocket)

**Methods:**
- Connection (4): connect(), disconnect(), isConnected(), getSubscriptions()
- Subscriptions (3): subscribePrices(), subscribeOrderbook(), unsubscribe()
- Queries (1): getMarket()
- Events (1): on()

**Events:**
- onConnect
- onDisconnect
- onPrice
- onOrderbook
- onError

**Key Features:**
- JSON-RPC 2.0 protocol
- Automatic reconnection with exponential backoff
- Automatic heartbeat pings
- Fan-out message handling
- Typed callbacks
- Request timeout handling
- Subscription tracking

---

### `/typescript/src/mcp.ts` (100+ lines)

**Purpose:** MCP tool integration helpers

**Classes:**
- McpHelper (6 methods)
  - listTools()
  - getSchema()
  - executeTool()
  - findTool()
  - getToolSchema()
  - listToolNames()

- AgentCardProvider (2 methods)
  - getAgentCard()
  - buildAgentCard()

**Key Features:**
- Fluent method chaining
- Tool discovery and search
- Schema extraction
- Tool execution
- Agent card building

---

### `/typescript/src/index.ts` (20 lines)

**Purpose:** Main entry point and re-exports

**Exports:**
- UppClient, UppApiError
- UppWebSocket
- McpHelper, AgentCardProvider
- All types from types.ts
- VERSION constant

---

### `/typescript/package.json`

**Configuration:**
- name: @upp/sdk
- version: 1.0.0
- type: module (ESM)
- main: ./dist/index.js
- types: ./dist/index.d.ts
- Exports for submodules (client, websocket, mcp, types)

**Scripts:**
- build: tsc
- build:watch: tsc --watch
- clean: rm -rf dist
- prepublishOnly: npm run build
- type-check: tsc --noEmit

**Dev Dependencies:**
- typescript: ^5.3.0

**No Runtime Dependencies** (uses native fetch + ws)

---

### `/typescript/tsconfig.json`

**Configuration:**
- Target: ES2020
- Lib: ES2020, DOM
- Module: ESNext with bundler resolution
- Declaration files enabled
- Source maps enabled
- Strict mode enabled
- No unused variables/parameters
- No implicit returns
- No fallthrough switch cases

---

### `/typescript/README.md` (400+ lines)

**Sections:**
1. Installation (npm/yarn)
2. Quick Start (REST API, WebSocket, MCP examples)
3. API Reference (UppClient methods, UppWebSocket, McpHelper)
4. Configuration options
5. Error handling
6. Type safety examples
7. Building instructions
8. License

---

## Python SDK Files

### `/python/upp/types.py` (700+ lines)

**Purpose:** Pydantic v2 models for all UPP data types

**Enumerations (9):**
- MarketType (binary, categorical, scalar)
- MarketStatus (pending, open, halted, closed, resolved, disputed, voided)
- Side (buy, sell)
- OrderType (limit, market)
- TimeInForce (GTC, GTD, FOK, IOC)
- OrderStatus (pending, open, partially_filled, filled, cancelled, rejected, expired)
- PositionStatus (open, closed, settled, expired)
- KycLevel (none, basic, enhanced, institutional)
- TradeRole (maker, taker)

**Models (40+):**
- UniversalMarketId (with utility methods)
- Event, Outcome
- MarketPricing, MarketVolume, MarketLifecycle
- MarketRules, MarketRegulatory
- Market
- OrderFees, Order
- Position
- Trade
- PaginationRequest, PaginationResponse
- UppError, UppErrorDetail
- ProviderManifest, WellKnown, HealthStatus
- MarketsResponse, OrdersResponse, TradesResponse, PositionsResponse
- PortfolioSummary, PortfolioBalance, PortfolioBalancesResponse
- OrderbookSnapshot, OrderbookResponse, MergedOrderbookResponse
- CreateOrderRequest, EstimateOrderRequest, OrderEstimate
- CancelAllOrdersRequest, CancelAllOrdersResponse
- McpTool, McpToolsResponse, McpExecuteRequest, McpExecuteResponse, McpSchemaResponse
- JsonRpcRequest, JsonRpcResponse
- PriceSubscription, OrderbookSubscription
- PriceUpdate, OrderbookUpdate
- AgentCard

**Key Features:**
- Pydantic validation
- Full docstrings
- Field aliases for snake_case conversion
- Extra fields handling where appropriate
- Enum serialization

---

### `/python/upp/client.py` (700+ lines)

**Purpose:** REST API client with async/await

**Class:** `UppClient`

**Constructor:**
```python
UppClient(
    base_url: str,
    api_key: Optional[str] = None,
    timeout: float = 30.0
)
```

**Methods (35+):**
- Context manager support (__aenter__, __aexit__)
- close() - explicit cleanup
- Health (3): health(), ready(), metrics()
- Discovery (6): get_well_known(), list_providers(), get_manifest(), negotiate(), check_provider_health(), check_all_provider_health()
- Markets (9): list_markets(), search_markets(), get_market(), get_orderbook(), get_merged_orderbook(), list_categories(), get_resolution(), list_resolutions()
- Settlement (2): list_settlement_instruments(), list_settlement_handlers()
- Trading (7): create_order(), list_orders(), get_order(), cancel_order(), cancel_all_orders(), estimate_order(), list_trades()
- Portfolio (3): list_positions(), get_portfolio_summary(), list_portfolio_balances()
- MCP (4): list_mcp_tools(), get_mcp_schema(), execute_mcp_tool(), get_agent_card()

**Error Handling:**
- UppApiError exception
- Pydantic validation errors
- HTTP errors

**Key Features:**
- Async/await pattern with httpx
- Context manager for resource cleanup
- Response validation
- Optional parameter filtering
- Bearer token authentication
- Full type hints
- Comprehensive docstrings

---

### `/python/upp/websocket.py` (500+ lines)

**Purpose:** WebSocket client with async support

**Class:** `UppWebSocket`

**Constructor:**
```python
UppWebSocket(
    url: str,
    reconnect: bool = True,
    max_reconnect_attempts: int = 10,
    initial_reconnect_delay: float = 1.0,
    max_reconnect_delay: float = 30.0,
    reconnect_backoff_multiplier: float = 2.0,
    heartbeat_interval: float = 30.0
)
```

**Methods (14):**
- Callbacks (5): on_connect(), on_disconnect(), on_price(), on_orderbook(), on_error()
- Connection (4): connect(), disconnect(), is_connected()
- Subscriptions (3): subscribe_prices(), subscribe_orderbook(), unsubscribe()
- Queries (1): get_market()

**Key Features:**
- JSON-RPC 2.0 protocol
- Auto-reconnection with exponential backoff
- Automatic heartbeat
- Callback-based event handling
- Proper asyncio task management
- WebSocket protocol implementation
- Fan-out message routing
- Type hints and docstrings

---

### `/python/upp/mcp.py` (100+ lines)

**Purpose:** MCP tool integration helpers

**Classes:**

**McpHelper (6 methods):**
- list_tools() - Get available tools
- get_schema() - Get MCP schema
- execute_tool() - Execute tool
- find_tool() - Find by name
- get_tool_schema() - Get tool schema
- list_tool_names() - List all names

**AgentCardProvider (2 methods):**
- get_agent_card() - Get card
- build_agent_card() - Build complete card with tools

**Key Features:**
- Tool discovery
- Schema introspection
- Tool execution
- Agent card building
- Full docstrings

---

### `/python/upp/__init__.py` (70 lines)

**Purpose:** Package initialization and clean exports

**Exports:**
- UppClient, UppApiError
- UppWebSocket
- McpHelper, AgentCardProvider
- All type classes and enums (40+)
- __version__ = "1.0.0"
- __all__ list for explicit exports

**Key Features:**
- Single-line imports: `from upp import UppClient, Market, PriceUpdate`
- Clean namespace
- Version tracking

---

### `/python/pyproject.toml`

**Project Metadata:**
- name: upp-sdk
- version: 1.0.0
- description: Universal Prediction Protocol SDK for Python
- Python requirement: >=3.9
- License: Apache-2.0

**Dependencies:**
- httpx >= 0.24.0
- websockets >= 12.0
- pydantic >= 2.0

**Dev Dependencies:**
- pytest, pytest-asyncio
- black, isort
- mypy, ruff

**Tool Configs:**
- Black: 100 char line length
- Isort: black profile
- Mypy: strict mode
- Ruff: standard rules

---

### `/python/README.md` (400+ lines)

**Sections:**
1. Installation (pip, from source)
2. Quick Start (REST API, WebSocket, MCP with async examples)
3. API Reference
4. Configuration options
5. Context manager usage
6. Error handling
7. Type safety
8. Development workflow
9. License

---

## Root Documentation Files

### `/FEATURE_SUMMARY.md`

**Contents:**
- Overview of Feature 5
- Detailed breakdown of both SDKs
- Line counts and method counts
- API coverage verification
- Key features for each language
- Code quality standards
- File structure
- Testing recommendations
- Summary of completeness

---

### `/INTEGRATION_GUIDE.md`

**Contents:**
- Installation steps for both SDKs
- Build and publish instructions
- Environment setup for development
- Configuration examples
- Common use case examples
- Troubleshooting guide
- Performance tips
- Migration guide
- Support and resources

---

### `/FILES_MANIFEST.md`

**Contents:** This file
- Complete directory structure
- Purpose of each file
- Line counts and key contents
- Configuration details
- Key features
- Documentation sections

---

## Summary Statistics

| Metric | TypeScript | Python | Total |
|--------|-----------|--------|-------|
| Source Files | 5 | 5 | 10 |
| Configuration Files | 3 | 1 | 4 |
| Documentation Files | 3 | 3 | 6 |
| Total Lines of Code | 2000+ | 2000+ | 4000+ |
| Type Definitions/Models | 30+ | 40+ | 70+ |
| Public Methods | 40+ | 35+ | 75+ |
| Enumerations | 8 | 9 | 17 |
| **Total Files** | **8** | **6** | **14+** |

---

## Language-Specific Details

### TypeScript

**Module System:** ESM (ECMAScript Modules)

**Build Output:**
- dist/index.js - Compiled JavaScript
- dist/index.d.ts - TypeScript declarations
- dist/*.js.map - Source maps
- dist/*.d.ts.map - Declaration maps

**Import Statements:**
```typescript
import { UppClient, UppWebSocket } from '@upp/sdk';
import type { Market, Order } from '@upp/sdk';
import { UppClient } from '@upp/sdk/client';
```

**Package Distribution:**
- npm registry as @upp/sdk
- Submodule exports via package.json exports field
- Declaration files included

---

### Python

**Module System:** Standard Python packages with setuptools

**Installation Output:**
```
upp-sdk/
├── upp/
│   ├── __init__.py
│   ├── types.py
│   ├── client.py
│   ├── websocket.py
│   └── mcp.py
└── upp-sdk-1.0.0.dist-info/
```

**Import Statements:**
```python
from upp import UppClient, UppWebSocket
from upp.types import Market, Order, MarketStatus
from upp.mcp import McpHelper
```

**Package Distribution:**
- PyPI as upp-sdk
- Source distribution (sdist)
- Wheel distribution
- Metadata via pyproject.toml

---

## Version Information

- **SDK Version:** 1.0.0
- **TypeScript Version:** 5.3.0+
- **Python Version:** 3.9+
- **Pydantic Version:** 2.0+
- **httpx Version:** 0.24.0+
- **websockets Version:** 12.0+

---

## License

All files are licensed under Apache-2.0.
See LICENSE file in the root of each SDK package.

---

## Checksums

To verify file integrity:

### TypeScript
```bash
find typescript/src -name "*.ts" | xargs wc -l
find typescript -name "*.json" -o -name "*.md" | xargs wc -l
```

### Python
```bash
find python/upp -name "*.py" | xargs wc -l
find python -name "*.toml" -o -name "*.md" | xargs wc -l
```

---

**Last Updated:** 2026-03-13
**Generated for:** UPP Gateway v1.0.0
