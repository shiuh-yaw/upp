# UPP SDK for TypeScript

Auto-generated TypeScript client library for the Universal Prediction Protocol gateway. Provides fully typed access to all REST API endpoints, WebSocket subscriptions, and MCP tools.

## Installation

```bash
npm install @upp/sdk
```

Or with yarn:

```bash
yarn add @upp/sdk
```

## Quick Start

### REST API Client

```typescript
import { UppClient } from '@upp/sdk';

const client = new UppClient({
  baseUrl: 'http://localhost:8080',
  apiKey: 'your-api-key' // optional
});

// List markets
const markets = await client.listMarkets({
  provider: 'kalshi.com',
  status: 'open',
  limit: 20
});

// Get a specific market
const market = await client.getMarket('upp:kalshi:MELON-240301');
console.log(`Market: ${market.event.title}`);
console.log(`Type: ${market.market_type}`);

// Get market orderbook
const orderbook = await client.getOrderbook('upp:kalshi:MELON-240301', {
  depth: 10
});

// Create an order (requires API key)
const order = await client.createOrder({
  provider: 'kalshi.com',
  market_id: 'MELON-240301',
  outcome_id: 'YES',
  side: 'buy',
  order_type: 'limit',
  tif: 'GTC',
  price: '0.45',
  quantity: 100
});

// List orders
const orders = await client.listOrders({
  provider: 'kalshi.com',
  status: 'open'
});

// Get portfolio
const positions = await client.listPositions();
const summary = await client.getPortfolioSummary();
console.log(`Total Value: ${summary.total_value}`);
console.log(`Total P&L: ${summary.total_pnl}`);
```

### WebSocket Real-Time Updates

```typescript
import { UppWebSocket } from '@upp/sdk';

const ws = new UppWebSocket({
  url: 'ws://localhost:8080/upp/v1/ws',
  reconnect: {
    enabled: true,
    maxAttempts: 10,
    initialDelayMs: 1000
  }
});

// Register callbacks
ws.on({
  onConnect: () => console.log('Connected'),
  onPrice: (update) => {
    console.log(`Price update for ${update.market_id}:`, update.prices);
  },
  onOrderbook: (update) => {
    console.log(`Orderbook update for ${update.market_id}:`, update.snapshots);
  },
  onError: (error) => console.error('WebSocket error:', error)
});

// Connect
await ws.connect();

// Subscribe to price updates (every 1 second)
await ws.subscribePrices(
  ['upp:kalshi:MELON-240301', 'upp:kalshi:TRUMPC-240401'],
  1000
);

// Subscribe to orderbook updates (every 2 seconds, depth 10)
await ws.subscribeOrderbook(
  ['upp:kalshi:MELON-240301'],
  10,
  2000
);

// Unsubscribe
await ws.unsubscribe('prices', ['upp:kalshi:MELON-240301']);

// Disconnect
ws.disconnect();
```

### MCP Integration

```typescript
import { UppClient, McpHelper } from '@upp/sdk';

const client = new UppClient({
  baseUrl: 'http://localhost:8080'
});

const mcp = new McpHelper(client);

// List available tools
const tools = await mcp.listTools();
tools.forEach(tool => {
  console.log(`- ${tool.name}: ${tool.description}`);
});

// Get MCP schema
const schema = await mcp.getSchema();

// Execute a tool
const result = await mcp.executeTool('get_market', {
  market_id: 'upp:kalshi:MELON-240301'
});
console.log('Tool result:', result);

// Get agent card for A2A integration
const agentCard = await client.getAgentCard();
```

## API Reference

### UppClient

Main REST API client with methods for all endpoints.

#### Configuration

```typescript
new UppClient({
  baseUrl: 'http://localhost:8080', // required
  apiKey: 'your-api-key',           // optional
  timeout: 30000,                   // optional, milliseconds
  fetch: customFetchImpl             // optional, for Node.js environments
})
```

#### Public Methods

**Discovery:**
- `getWellKnown()` — Get well-known endpoint info
- `listProviders()` — List all available providers
- `getManifest(provider)` — Get provider manifest
- `negotiate(provider)` — Negotiate capabilities
- `checkProviderHealth(provider)` — Check provider health
- `checkAllProviderHealth()` — Check all providers

**Markets:**
- `listMarkets(options?)` — List markets with optional filters
- `searchMarkets(query, options?)` — Search markets
- `getMarket(marketId)` — Get specific market
- `getOrderbook(marketId, options?)` — Get market orderbook
- `getMergedOrderbook(marketId, options?)` — Get cross-provider orderbook
- `listCategories()` — List market categories
- `getResolution(marketId)` — Get market resolution
- `listResolutions()` — List all resolutions

**Trading (requires API key):**
- `createOrder(request)` — Create a new order
- `listOrders(options?)` — List all orders
- `getOrder(orderId, provider?)` — Get specific order
- `cancelOrder(orderId, provider?)` — Cancel an order
- `cancelAllOrders(provider, marketId?)` — Cancel all orders
- `estimateOrder(request)` — Estimate order cost
- `listTrades(options?)` — List all trades

**Portfolio (requires API key):**
- `listPositions(provider?)` — List all positions
- `getPortfolioSummary(provider?)` — Get portfolio summary
- `listPortfolioBalances(provider?)` — Get portfolio balances

**Infrastructure:**
- `health()` — Check gateway health
- `ready()` — Check gateway readiness
- `metrics()` — Get Prometheus metrics

**MCP:**
- `listMcpTools()` — List available MCP tools
- `getMcpSchema()` — Get MCP schema
- `executeMcpTool(tool, params)` — Execute MCP tool
- `getAgentCard()` — Get agent card for A2A integration

### UppWebSocket

Real-time WebSocket client for market subscriptions.

#### Configuration

```typescript
new UppWebSocket({
  url: 'ws://localhost:8080/upp/v1/ws', // required
  reconnect: {                            // optional
    enabled: true,
    maxAttempts: 10,
    initialDelayMs: 1000,
    maxDelayMs: 30000,
    backoffMultiplier: 2
  },
  heartbeatInterval: 30000,               // optional, milliseconds
  WebSocket: customWebSocketImpl           // optional, for custom implementations
})
```

#### Methods

- `on(callbacks)` — Register event callbacks
- `connect()` — Connect to WebSocket server
- `disconnect()` — Close WebSocket connection
- `subscribePrices(marketIds, intervalMs?)` — Subscribe to price updates
- `subscribeOrderbook(marketIds, depth?, intervalMs?)` — Subscribe to orderbook updates
- `unsubscribe(channel, marketIds)` — Unsubscribe from channel
- `getMarket(marketId)` — Get market (one-off request)
- `isConnected()` — Check connection state
- `getSubscriptions()` — Get current subscriptions

#### Callbacks

```typescript
ws.on({
  onConnect: () => { /* connected */ },
  onDisconnect: () => { /* disconnected */ },
  onPrice: (update) => { /* price update */ },
  onOrderbook: (update) => { /* orderbook update */ },
  onError: (error) => { /* error */ }
})
```

### McpHelper

Helpers for working with MCP tools.

#### Methods

- `listTools()` — List all available tools
- `getSchema()` — Get MCP schema
- `executeTool(tool, params)` — Execute a tool
- `findTool(name)` — Find tool by name
- `getToolSchema(toolName)` — Get tool's input schema
- `listToolNames()` — List all tool names

## Error Handling

The client throws `UppApiError` for API errors:

```typescript
import { UppClient, UppApiError } from '@upp/sdk';

const client = new UppClient({ baseUrl: 'http://localhost:8080' });

try {
  const market = await client.getMarket('invalid-market-id');
} catch (error) {
  if (error instanceof UppApiError) {
    console.log(`Error: ${error.code} - ${error.message}`);
    console.log(`Status: ${error.status}`);
    console.log(`Details:`, error.details);
  } else {
    throw error;
  }
}
```

## Type Safety

All API responses and requests are fully typed. TypeScript will catch type errors at compile time:

```typescript
// This will fail at compile time - invalid market status
const markets = await client.listMarkets({
  status: 'invalid' // TS Error: Type '"invalid"' is not assignable to type 'MarketStatus'
});

// This is correct
const markets = await client.listMarkets({
  status: 'open'
});
```

## Building

```bash
npm install
npm run build
```

Output is compiled to the `dist` directory.

## License

Apache-2.0
