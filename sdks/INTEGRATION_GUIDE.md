# SDK Integration Guide

## Quick Integration Checklist

### TypeScript SDK

#### Installation
```bash
cd typescript
npm install
npm run build
```

#### Publish to npm
```bash
npm login
npm publish
```

Or for private registry:
```bash
npm publish --registry https://your-registry.com
```

#### Using in a Project
```bash
npm install @upp/sdk
```

```typescript
import { UppClient, UppWebSocket } from '@upp/sdk';

const client = new UppClient({
  baseUrl: 'http://localhost:8080',
  apiKey: 'your-key'
});

const ws = new UppWebSocket({
  url: 'ws://localhost:8080/upp/v1/ws'
});
```

---

### Python SDK

#### Installation
```bash
cd python
pip install -e .
```

#### Publish to PyPI
```bash
pip install build twine
python -m build
twine upload dist/*
```

#### Using in a Project
```bash
pip install upp-sdk
```

```python
from upp import UppClient, UppWebSocket
import asyncio

async def main():
    async with UppClient(
        base_url='http://localhost:8080',
        api_key='your-key'
    ) as client:
        markets = await client.list_markets()
```

---

## Environment Setup

### For Development

#### TypeScript
```bash
# Install dependencies
npm install

# Development workflow
npm run build:watch      # Auto-compile on changes
npm run type-check       # Type check only

# Format code
npx prettier --write src/
npx eslint src/ --fix

# Build for distribution
npm run build
npm run clean            # Remove dist/
```

#### Python
```bash
# Install with dev dependencies
pip install -e ".[dev]"

# Format and lint
black upp/
isort upp/

# Type checking
mypy upp/

# Linting
ruff check upp/
ruff check upp/ --fix

# Testing
pytest tests/
pytest tests/ -v         # Verbose
pytest tests/ --cov      # Coverage
```

---

## Configuration Examples

### TypeScript REST Client

```typescript
import { UppClient } from '@upp/sdk';

// Basic
const client = new UppClient({
  baseUrl: 'http://localhost:8080'
});

// With authentication
const client = new UppClient({
  baseUrl: 'http://localhost:8080',
  apiKey: 'your-api-key-here'
});

// With custom timeout
const client = new UppClient({
  baseUrl: 'http://localhost:8080',
  timeout: 60000 // 60 seconds
});

// With custom fetch (e.g., for Node.js)
import fetch from 'node-fetch';
const client = new UppClient({
  baseUrl: 'http://localhost:8080',
  fetch: fetch as any
});
```

### TypeScript WebSocket Client

```typescript
import { UppWebSocket } from '@upp/sdk';

const ws = new UppWebSocket({
  url: 'ws://localhost:8080/upp/v1/ws',
  reconnect: {
    enabled: true,
    maxAttempts: 10,
    initialDelayMs: 1000,
    maxDelayMs: 30000,
    backoffMultiplier: 2
  },
  heartbeatInterval: 30000
});

// Register callbacks
ws.on({
  onConnect: () => console.log('Connected'),
  onPrice: (update) => console.log('Price:', update),
  onOrderbook: (update) => console.log('Orderbook:', update),
  onError: (error) => console.error('Error:', error),
  onDisconnect: () => console.log('Disconnected')
});

// Connect and subscribe
await ws.connect();
await ws.subscribePrices(['upp:kalshi:MELON-240301'], 1000);
```

### Python REST Client

```python
from upp import UppClient
import asyncio

# Basic
client = UppClient(base_url='http://localhost:8080')

# With authentication
client = UppClient(
    base_url='http://localhost:8080',
    api_key='your-api-key-here'
)

# With custom timeout
client = UppClient(
    base_url='http://localhost:8080',
    timeout=60.0
)

# Using context manager (recommended)
async def main():
    async with UppClient(
        base_url='http://localhost:8080',
        api_key='your-key'
    ) as client:
        markets = await client.list_markets()
        print(markets)

asyncio.run(main())
```

### Python WebSocket Client

```python
from upp import UppWebSocket
import asyncio

async def main():
    ws = UppWebSocket(
        url='ws://localhost:8080/upp/v1/ws',
        reconnect=True,
        max_reconnect_attempts=10,
        initial_reconnect_delay=1.0,
        max_reconnect_delay=30.0,
        reconnect_backoff_multiplier=2.0,
        heartbeat_interval=30.0
    )

    # Register callbacks (fluent API)
    ws.on_connect(lambda: print('Connected'))
    ws.on_price(lambda u: print(f'Price: {u.prices}'))
    ws.on_orderbook(lambda u: print(f'Orderbook: {u.snapshots}'))
    ws.on_error(lambda e: print(f'Error: {e}'))
    ws.on_disconnect(lambda: print('Disconnected'))

    # Connect and subscribe
    await ws.connect()
    await ws.subscribe_prices(['upp:kalshi:MELON-240301'], interval_ms=1000)

    # Run until interrupted
    try:
        await asyncio.sleep(3600)
    finally:
        ws.disconnect()

asyncio.run(main())
```

---

## Common Use Cases

### TypeScript: List Markets and Get Prices

```typescript
import { UppClient, UppWebSocket } from '@upp/sdk';

async function main() {
  const client = new UppClient({ baseUrl: 'http://localhost:8080' });

  // Get markets
  const response = await client.listMarkets({ limit: 10 });
  console.log(`Found ${response.markets.length} markets`);

  for (const market of response.markets) {
    console.log(`${market.event.title}: ${market.market_type}`);

    // Get current pricing
    const pricing = market.pricing;
    console.log(`  Best Bid: ${pricing.best_bid}`);
    console.log(`  Best Ask: ${pricing.best_ask}`);
  }
}
```

### TypeScript: Real-Time Price Updates

```typescript
import { UppWebSocket } from '@upp/sdk';

async function main() {
  const ws = new UppWebSocket({
    url: 'ws://localhost:8080/upp/v1/ws'
  });

  ws.on({
    onPrice: (update) => {
      console.log(`${update.market_id} at ${update.timestamp}`);
      Object.entries(update.prices).forEach(([outcome, price]) => {
        console.log(`  ${outcome}: ${price}`);
      });
    }
  });

  await ws.connect();
  await ws.subscribePrices(['upp:kalshi:MELON-240301', 'upp:kalshi:TRUMPC-240401']);

  // Prices update automatically via callback
  await new Promise(() => {}); // Run forever
}

main().catch(console.error);
```

### Python: Create and Monitor Orders

```python
from upp import UppClient, CreateOrderRequest, Side, OrderType
import asyncio

async def main():
    async with UppClient(
        base_url='http://localhost:8080',
        api_key='your-api-key'
    ) as client:
        # Create order
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
        orders = await client.list_orders(provider='kalshi.com')
        print(f"Total orders: {len(orders.orders)}")

        # Monitor position
        positions = await client.list_positions()
        for pos in positions.positions:
            print(f"{pos.market_title}: {pos.quantity} @ {pos.current_price}")
            print(f"  Unrealized P&L: {pos.unrealized_pnl}")

asyncio.run(main())
```

### Python: Real-Time Orderbook Updates

```python
from upp import UppWebSocket
import asyncio

async def main():
    ws = UppWebSocket(
        url='ws://localhost:8080/upp/v1/ws'
    )

    ws.on_orderbook(lambda update: display_orderbook(update))

    await ws.connect()
    await ws.subscribe_orderbook(
        ['upp:kalshi:MELON-240301'],
        depth=10,
        interval_ms=2000
    )

    try:
        await asyncio.sleep(3600)
    finally:
        ws.disconnect()

def display_orderbook(update):
    print(f"\nOrderbook for {update.market_id}:")
    for snapshot in update.snapshots:
        print(f"  {snapshot.outcome_id}:")
        print(f"    Bids: {snapshot.bids}")
        print(f"    Asks: {snapshot.asks}")

asyncio.run(main())
```

---

## Troubleshooting

### TypeScript

**"Cannot find module '@upp/sdk'"**
- Ensure npm install completed: `npm install @upp/sdk`
- Check TypeScript imports use correct paths
- Verify tsconfig.json moduleResolution is 'node' or 'bundler'

**WebSocket connection fails**
- Check WebSocket URL includes full path: `ws://host:port/upp/v1/ws`
- Verify gateway is running and accessible
- Check for CORS issues in browser console

**Type errors in IDE**
- Run `npm run type-check` to see all errors
- Ensure tsconfig.json is in project root
- IDE may need to be restarted

### Python

**"ModuleNotFoundError: No module named 'upp'"**
- Install SDK: `pip install upp-sdk` or `pip install -e .`
- Check Python version >= 3.9
- Verify virtual environment is activated

**"ImportError: cannot import name"**
- Check import statement: `from upp import UppClient`
- Verify installed version matches code

**WebSocket reconnection loops**
- Check WebSocket URL is accessible
- Verify network connectivity
- Check gateway logs for errors
- Reduce reconnection attempts for debugging

**Async/await errors**
- Ensure running in async context: `asyncio.run()`
- Use `async def` and `await` properly
- Python 3.7+ required for modern async syntax

---

## Performance Tips

### TypeScript

1. **Reuse client instance** — Create once, share across requests
2. **Use keepalive** — For WebSocket, relies on heartbeat
3. **Batch requests** — Aggregate multiple market queries
4. **Handle errors gracefully** — Catch UppApiError with specific codes

### Python

1. **Use context manager** — Automatic resource cleanup
2. **Connection pooling** — httpx reuses connections
3. **Batch async operations** — Use asyncio.gather() for parallel requests
4. **Monitor memory** — WebSocket callbacks should be lightweight

---

## Migration Guide

If moving from a different SDK or API:

### REST API Pattern Changes

**Before:**
```typescript
const data = await fetch('/markets').then(r => r.json());
```

**After:**
```typescript
const data = await client.listMarkets();
```

### Error Handling

**Before:**
```typescript
if (!response.ok) throw new Error(response.statusText);
```

**After:**
```typescript
try {
  const market = await client.getMarket(id);
} catch (error) {
  if (error instanceof UppApiError) {
    console.log(error.code, error.message);
  }
}
```

### WebSocket Pattern

**Before:**
```typescript
ws.addEventListener('message', (event) => {
  const data = JSON.parse(event.data);
});
```

**After:**
```typescript
ws.on({
  onPrice: (update) => { /* update.prices */ },
  onOrderbook: (update) => { /* update.snapshots */ }
});
```

---

## Support & Resources

- **Documentation:** See README.md in each SDK directory
- **Examples:** Check TypeScript and Python README files for complete examples
- **API Reference:** Gateway repository in `/gateway/src/main.rs`
- **Types:** Full type definitions in types.ts (TS) and types.py (Python)
