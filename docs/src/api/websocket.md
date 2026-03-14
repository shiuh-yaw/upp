# WebSocket Protocol

Real-time subscriptions for market data, order updates, and portfolio changes. Lower latency than polling with continuous updates.

## Connection

**URL:** `ws://localhost:8080/api/v1/feed` (or `wss://` for TLS)

**Protocol:** WebSocket over HTTP/1.1 upgrade

### JavaScript Example

```javascript
const ws = new WebSocket('ws://localhost:8080/api/v1/feed');

ws.onopen = () => {
  console.log('Connected');

  // Authenticate
  ws.send(JSON.stringify({
    action: 'authenticate',
    token: 'api_key_here'
  }));

  // Subscribe to markets
  ws.send(JSON.stringify({
    action: 'subscribe',
    channel: 'markets:polymarket'
  }));
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  console.log('Received:', message);
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = () => {
  console.log('Disconnected');
};
```

### Rust Example

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://localhost:8080/api/v1/feed";
    let (ws, _) = connect_async(url).await?;

    let (mut write, mut read) = ws.split();

    // Authenticate
    let auth = json!({
        "action": "authenticate",
        "token": "api_key"
    });
    write.send(Message::Text(auth.to_string())).await?;

    // Subscribe
    let subscribe = json!({
        "action": "subscribe",
        "channel": "markets:polymarket"
    });
    write.send(Message::Text(subscribe.to_string())).await?;

    // Receive updates
    while let Some(msg) = read.next().await {
        if let Ok(Message::Text(text)) = msg {
            if let Ok(update) = serde_json::from_str::<Update>(&text) {
                println!("Update: {:?}", update);
            }
        }
    }

    Ok(())
}
```

## Message Format

All WebSocket messages are JSON objects with an `action` field and additional context:

### Request Format

```json
{
  "action": "subscribe|unsubscribe|authenticate|ping",
  "channel": "markets:polymarket|orders:user123|portfolio:user123",
  "token": "api_key_here"
}
```

### Response Format

```json
{
  "type": "connection|update|error|pong",
  "channel": "markets:polymarket",
  "data": {...},
  "timestamp": "2026-03-14T12:34:56.000Z"
}
```

## Channels

Subscribe to real-time data streams.

### Markets Channel

Stream market updates (prices, volume) in real-time.

**Subscribe:**

```json
{
  "action": "subscribe",
  "channel": "markets:polymarket"
}
```

Response on subscription:

```json
{
  "type": "connection",
  "channel": "markets:polymarket",
  "data": {
    "status": "connected",
    "message": "Subscribed to markets:polymarket"
  }
}
```

**Updates:**

Market prices and volume update every 5 seconds:

```json
{
  "type": "update",
  "channel": "markets:polymarket",
  "data": {
    "market_id": "0x1234...abcd",
    "title": "Will ETH exceed $5000 by Q2 2026?",
    "outcomes": [
      {
        "id": "0",
        "name": "Yes",
        "price": 0.72,
        "price_change": 0.01,
        "volume": 125000,
        "volume_change": 25000
      },
      {
        "id": "1",
        "name": "No",
        "price": 0.28,
        "price_change": -0.01,
        "volume": 875000,
        "volume_change": 175000
      }
    ],
    "timestamp": "2026-03-14T12:35:00.000Z"
  }
}
```

**Channel Variants:**

```
markets:polymarket        # Polymarket markets
markets:kalshi            # Kalshi markets
markets:opinion_trade     # Opinion.trade markets
markets:all               # All providers
markets:polymarket:active # Polymarket active markets only
```

### Orders Channel

Stream user's order updates (fills, cancellations).

**Subscribe:**

```json
{
  "action": "subscribe",
  "channel": "orders:user123"
}
```

**Updates:**

Triggered when orders change:

```json
{
  "type": "update",
  "channel": "orders:user123",
  "data": {
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
    "event": "partial_fill",
    "timestamp": "2026-03-14T12:34:30.000Z"
  }
}
```

**Events:**

- `order_placed` — New order created
- `partial_fill` — Order partially executed
- `full_fill` — Order completely executed
- `order_cancelled` — Order cancelled

### Portfolio Channel

Stream portfolio changes (balance, positions).

**Subscribe:**

```json
{
  "action": "subscribe",
  "channel": "portfolio:user123"
}
```

**Updates:**

Triggered after order fills or cancellations:

```json
{
  "type": "update",
  "channel": "portfolio:user123",
  "data": {
    "cash_balance": 2500,
    "total_value": 7200,
    "positions_changed": [
      {
        "market_id": "0x1234...abcd",
        "outcome": "Yes",
        "quantity": 75,
        "entry_price": 0.65,
        "current_price": 0.72,
        "pnl": 525,
        "change_type": "quantity_increased"
      }
    ],
    "total_pnl": 575,
    "timestamp": "2026-03-14T12:34:30.000Z"
  }
}
```

**Channel Variants:**

```
portfolio:user123         # Specific user portfolio
portfolio:all             # All positions (requires admin auth)
```

### Arbitrage Channel

Stream detected arbitrage opportunities as they appear.

**Subscribe:**

```json
{
  "action": "subscribe",
  "channel": "arbitrage"
}
```

**Updates:**

Sent when profitable arbitrage detected:

```json
{
  "type": "update",
  "channel": "arbitrage",
  "data": {
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
    "potential_profit": 3000,
    "timestamp": "2026-03-14T12:34:56.000Z"
  }
}
```

**Options:**

```json
{
  "action": "subscribe",
  "channel": "arbitrage",
  "options": {
    "min_spread": 0.05,
    "min_profit": 1000
  }
}
```

## Control Messages

### Authenticate

Send API key for secured channels (orders, portfolio).

**Request:**

```json
{
  "action": "authenticate",
  "token": "api_key_here"
}
```

**Response:**

```json
{
  "type": "connection",
  "data": {
    "status": "authenticated",
    "user_id": "user123",
    "message": "Authentication successful"
  }
}
```

### Ping/Pong

Heartbeat to maintain connection.

**Client sends:**

```json
{
  "action": "ping"
}
```

**Server responds:**

```json
{
  "type": "pong",
  "timestamp": "2026-03-14T12:34:56.000Z"
}
```

Sent automatically every 30 seconds if no activity.

### Unsubscribe

Stop receiving updates from a channel.

**Request:**

```json
{
  "action": "unsubscribe",
  "channel": "markets:polymarket"
}
```

**Response:**

```json
{
  "type": "connection",
  "channel": "markets:polymarket",
  "data": {
    "status": "unsubscribed",
    "message": "Unsubscribed from markets:polymarket"
  }
}
```

## Error Handling

### Authentication Error

```json
{
  "type": "error",
  "data": {
    "code": "UNAUTHENTICATED",
    "message": "Invalid or missing authentication token"
  }
}
```

### Invalid Channel

```json
{
  "type": "error",
  "data": {
    "code": "INVALID_CHANNEL",
    "message": "Unknown channel: markets:invalid"
  }
}
```

### Rate Limit

```json
{
  "type": "error",
  "data": {
    "code": "RATE_LIMITED",
    "message": "Too many subscriptions (max: 100)"
  }
}
```

## Best Practices

### 1. Handle Disconnections

Implement exponential backoff reconnection:

```javascript
let retries = 0;
const maxRetries = 5;

function connect() {
  ws = new WebSocket('ws://localhost:8080/api/v1/feed');

  ws.onclose = () => {
    if (retries < maxRetries) {
      const delay = Math.pow(2, retries) * 1000;
      console.log(`Reconnecting in ${delay}ms...`);
      setTimeout(connect, delay);
      retries++;
    } else {
      console.error('Max reconnection attempts reached');
    }
  };

  ws.onopen = () => {
    retries = 0;  // Reset on success
  };
}
```

### 2. Buffer Updates During Disconnection

Store updates and reconcile on reconnect:

```javascript
let updateBuffer = [];

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);

  if (isDisconnected) {
    updateBuffer.push(message);
  } else {
    processUpdate(message);
  }
};
```

### 3. Limit Subscriptions

Connection has a limit of 100 concurrent subscriptions:

```javascript
// Good
ws.send(JSON.stringify({
  action: 'subscribe',
  channel: 'markets:polymarket'
}));

// Bad - might fail if you have many subscriptions
for (let i = 0; i < 200; i++) {
  ws.send(JSON.stringify({
    action: 'subscribe',
    channel: `markets:id_${i}`
  }));
}
```

### 4. Monitor Connection Health

Check if connection is alive:

```javascript
let lastPongTime = Date.now();

setInterval(() => {
  const timeSinceLastPong = Date.now() - lastPongTime;
  if (timeSinceLastPong > 120000) {  // 2 minutes
    console.error('Connection stale, reconnecting...');
    ws.close();
    connect();
  }
}, 30000);

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.type === 'pong') {
    lastPongTime = Date.now();
  }
};
```

### 5. Update Reconciliation

Markets channel sends full snapshots every 30 seconds. Track locally:

```javascript
let marketCache = new Map();

function handleMarketUpdate(data) {
  const key = `${data.market_id}:${data.provider}`;

  if (marketCache.has(key)) {
    // Update existing
    const existing = marketCache.get(key);
    existing.outcomes = data.outcomes;
    existing.timestamp = data.timestamp;
  } else {
    // New market
    marketCache.set(key, data);
  }

  renderMarkets(marketCache);
}
```

## Performance Characteristics

| Metric | Value |
|--------|-------|
| Connection latency | <100ms |
| Update frequency | 5-30s (configurable) |
| Max subscriptions | 100 per connection |
| Message throughput | 1000+ msg/sec |
| Typical message size | 200-500 bytes |
| Memory per connection | ~5MB |

## Debugging

Enable verbose logging:

```javascript
const ws = new WebSocket('ws://localhost:8080/api/v1/feed');

const origSend = ws.send;
ws.send = function(data) {
  console.log('WS SEND:', data);
  return origSend.call(this, data);
};

ws.onmessage = (event) => {
  console.log('WS RECV:', event.data);
};
```

Or use browser DevTools:

```javascript
// Open DevTools console
fetch('http://localhost:9090/api/v1/query', {
  method: 'POST',
  body: JSON.stringify({
    query: 'upp_websocket_connections'
  })
}).then(r => r.json()).then(console.log);
```
