# UPP SDK for Python

Auto-generated Python client library for the Universal Prediction Protocol gateway. Provides fully typed async/await access to all REST API endpoints, WebSocket subscriptions, and MCP tools using Pydantic models.

## Installation

```bash
pip install upp-sdk
```

Or from source:

```bash
git clone https://github.com/universal-prediction-protocol/sdks
cd sdks/python
pip install -e .
```

## Quick Start

### REST API Client

```python
import asyncio
from upp import UppClient

async def main():
    # Create client
    client = UppClient(
        base_url='http://localhost:8080',
        api_key='your-api-key'  # optional
    )

    async with client:
        # List markets
        markets = await client.list_markets(
            provider='kalshi.com',
            status='open',
            limit=20
        )
        print(f"Found {len(markets.markets)} markets")

        # Get a specific market
        market = await client.get_market('upp:kalshi:MELON-240301')
        print(f"Market: {market.event.title}")
        print(f"Type: {market.market_type}")

        # Get market orderbook
        orderbook = await client.get_orderbook('upp:kalshi:MELON-240301', depth=10)
        print(f"Orderbook for {len(orderbook.orderbook)} outcomes")

        # Create an order (requires API key)
        from upp import CreateOrderRequest, Side, OrderType
        order = await client.create_order(
            CreateOrderRequest(
                provider='kalshi.com',
                market_id='MELON-240301',
                outcome_id='YES',
                side=Side.BUY,
                order_type=OrderType.LIMIT,
                price='0.45',
                quantity=100
            )
        )
        print(f"Order created: {order.id}")

        # List orders
        orders = await client.list_orders(provider='kalshi.com', status='open')
        print(f"Open orders: {orders.orders}")

        # Get portfolio
        positions = await client.list_positions()
        summary = await client.get_portfolio_summary()
        print(f"Total Value: {summary.total_value}")
        print(f"Total P&L: {summary.total_pnl}")

asyncio.run(main())
```

### WebSocket Real-Time Updates

```python
import asyncio
from upp import UppWebSocket

async def main():
    ws = UppWebSocket(url='ws://localhost:8080/upp/v1/ws')

    # Register callbacks
    ws.on_connect(lambda: print('Connected'))
    ws.on_price(lambda u: print(f'Price update: {u.prices}'))
    ws.on_orderbook(lambda u: print(f'Orderbook update: {u.snapshots}'))
    ws.on_error(lambda e: print(f'Error: {e}'))

    # Connect
    await ws.connect()

    # Subscribe to price updates (every 1 second)
    await ws.subscribe_prices(
        ['upp:kalshi:MELON-240301', 'upp:kalshi:TRUMPC-240401'],
        interval_ms=1000
    )

    # Subscribe to orderbook updates (every 2 seconds, depth 10)
    await ws.subscribe_orderbook(
        ['upp:kalshi:MELON-240301'],
        depth=10,
        interval_ms=2000
    )

    # Keep running
    try:
        await asyncio.sleep(3600)
    finally:
        ws.disconnect()

asyncio.run(main())
```

### MCP Integration

```python
import asyncio
from upp import UppClient, McpHelper

async def main():
    client = UppClient(base_url='http://localhost:8080')
    mcp = McpHelper(client)

    async with client:
        # List available tools
        tools = await mcp.list_tools()
        for tool in tools:
            print(f"- {tool.name}: {tool.description}")

        # Get MCP schema
        schema = await mcp.get_schema()

        # Execute a tool
        result = await mcp.execute_tool('get_market', {
            'market_id': 'upp:kalshi:MELON-240301'
        })
        print(f'Tool result: {result}')

        # Get agent card for A2A integration
        agent_card = await client.get_agent_card()

asyncio.run(main())
```

## API Reference

### UppClient

Main REST API client with async methods for all endpoints.

#### Configuration

```python
client = UppClient(
    base_url='http://localhost:8080',  # required
    api_key='your-api-key',            # optional
    timeout=30.0                       # optional, seconds
)
```

#### Context Manager

```python
async with UppClient(...) as client:
    # Use client
    pass
# Client automatically closed
```

#### Public Methods

**Discovery:**
- `get_well_known()` — Get well-known endpoint info
- `list_providers()` — List all available providers
- `get_manifest(provider)` — Get provider manifest
- `negotiate(provider)` — Negotiate capabilities
- `check_provider_health(provider)` — Check provider health
- `check_all_provider_health()` — Check all providers

**Markets:**
- `list_markets(...)` — List markets with optional filters
- `search_markets(query, ...)` — Search markets
- `get_market(market_id)` — Get specific market
- `get_orderbook(market_id, ...)` — Get market orderbook
- `get_merged_orderbook(market_id, ...)` — Get cross-provider orderbook
- `list_categories()` — List market categories
- `get_resolution(market_id)` — Get market resolution
- `list_resolutions()` — List all resolutions

**Trading (requires API key):**
- `create_order(request)` — Create a new order
- `list_orders(...)` — List all orders
- `get_order(order_id, ...)` — Get specific order
- `cancel_order(order_id, ...)` — Cancel an order
- `cancel_all_orders(provider, ...)` — Cancel all orders
- `estimate_order(request)` — Estimate order cost
- `list_trades(...)` — List all trades

**Portfolio (requires API key):**
- `list_positions(...)` — List all positions
- `get_portfolio_summary(...)` — Get portfolio summary
- `list_portfolio_balances(...)` — Get portfolio balances

**Infrastructure:**
- `health()` — Check gateway health
- `ready()` — Check gateway readiness
- `metrics()` — Get Prometheus metrics

**MCP:**
- `list_mcp_tools()` — List available MCP tools
- `get_mcp_schema()` — Get MCP schema
- `execute_mcp_tool(tool, params)` — Execute MCP tool
- `get_agent_card()` — Get agent card for A2A integration

### UppWebSocket

Real-time WebSocket client for market subscriptions.

#### Configuration

```python
ws = UppWebSocket(
    url='ws://localhost:8080/upp/v1/ws',  # required
    reconnect=True,                        # optional, enable auto-reconnect
    max_reconnect_attempts=10,             # optional
    initial_reconnect_delay=1.0,           # optional, seconds
    max_reconnect_delay=30.0,              # optional, seconds
    reconnect_backoff_multiplier=2.0,      # optional
    heartbeat_interval=30.0                # optional, seconds
)
```

#### Methods

- `on_connect(callback)` — Register connect callback
- `on_disconnect(callback)` — Register disconnect callback
- `on_price(callback)` — Register price update callback
- `on_orderbook(callback)` — Register orderbook callback
- `on_error(callback)` — Register error callback
- `connect()` — Connect to WebSocket server
- `disconnect()` — Close WebSocket connection
- `subscribe_prices(market_ids, interval_ms)` — Subscribe to prices
- `subscribe_orderbook(market_ids, depth, interval_ms)` — Subscribe to orderbook
- `unsubscribe(channel, market_ids)` — Unsubscribe from channel
- `get_market(market_id)` — Get market (one-off request)
- `is_connected()` — Check connection state

#### Callbacks

```python
ws.on_connect(lambda: print("Connected"))
ws.on_disconnect(lambda: print("Disconnected"))
ws.on_price(lambda update: print(f"Price: {update}"))
ws.on_orderbook(lambda update: print(f"Orderbook: {update}"))
ws.on_error(lambda error: print(f"Error: {error}"))
```

### McpHelper

Helpers for working with MCP tools.

#### Methods

- `list_tools()` — List all available tools
- `get_schema()` — Get MCP schema
- `execute_tool(tool, params)` — Execute a tool
- `find_tool(name)` — Find tool by name
- `get_tool_schema(tool_name)` — Get tool's input schema
- `list_tool_names()` — List all tool names

## Error Handling

The client raises `UppApiError` for API errors:

```python
from upp import UppClient, UppApiError

client = UppClient(base_url='http://localhost:8080')

async with client:
    try:
        market = await client.get_market('invalid-market-id')
    except UppApiError as e:
        print(f"Error: {e.code} - {e.message}")
        print(f"Status: {e.status}")
        print(f"Details: {e.details}")
```

## Type Safety

All API responses are Pydantic models with full type hints:

```python
async with client:
    markets = await client.list_markets(status='open')
    # Type: MarketsResponse

    for market in markets.markets:
        # Type: Market
        print(f"{market.event.title}: {market.market_type}")

    positions = await client.list_positions()
    # Type: PositionsResponse

    for position in positions.positions:
        # Type: Position
        print(f"{position.market_title}: {position.unrealized_pnl}")
```

## Development

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Format code
black upp/
isort upp/

# Run type checker
mypy upp/

# Run linter
ruff check upp/

# Run tests
pytest tests/
```

## License

Apache-2.0
