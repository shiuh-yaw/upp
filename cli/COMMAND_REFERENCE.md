# UPP CLI - Complete Command Reference

Quick reference for all available commands with parameters and examples.

## Global Flags

```
--url <URL>         Override gateway URL (default: from config)
--api-key <KEY>     Override API key (default: from config)
--json              Output as JSON instead of formatted tables
```

## Command Categories

### Health & Status (1 command)

#### upp health
Check gateway status and connectivity.

```bash
upp health
upp health --json
upp health --url http://staging:9090
```

**Output**: Status, uptime, version

---

### Markets (3 commands)

#### upp markets list
List markets with optional filtering.

```bash
upp markets list
upp markets list --limit 50
upp markets list --provider kalshi
upp markets list --status open
upp markets list --provider kalshi --status open --limit 100
```

**Parameters**:
- `--provider <STRING>` - Filter by provider (optional)
- `--status <STRING>` - Filter by status: open, closed, etc. (optional)
- `--limit <NUMBER>` - Max results, default 20

**Output**: Table with ID, name, provider, status, price, outcome

---

#### upp markets get <market_id>
Get detailed information about a specific market.

```bash
upp markets get 0xabc123...
upp markets get 0xabc123... --json
```

**Parameters**:
- `<market_id>` - The market identifier (required)

**Output**: ID, name, provider, status, price, volume, created timestamp

---

#### upp markets search <query>
Search for markets by name or description.

```bash
upp markets search "bitcoin"
upp markets search "ethereum prediction"
upp markets search "election" --limit 100
```

**Parameters**:
- `<query>` - Search term (required)
- `--limit <NUMBER>` - Max results, default 20

**Output**: Table of matching markets

---

### Orders (5 commands)

#### upp orders list
List your orders with optional filtering.

```bash
upp orders list
upp orders list --limit 100
upp orders list --status open
upp orders list --provider kalshi --status open
```

**Parameters**:
- `--provider <STRING>` - Filter by provider (optional)
- `--status <STRING>` - Filter by status: open, filled, cancelled (optional)
- `--limit <NUMBER>` - Max results, default 20

**Output**: Table with ID, market, side, status, price, quantity, filled

---

#### upp orders create
Create a new buy or sell order.

```bash
upp orders create --market 0xmkt123 --side buy --price 0.55 --quantity 10
upp orders create --market 0xmkt456 --side sell --price 0.75 --quantity 20
```

**Parameters**:
- `--market <STRING>` - Market ID (required)
- `--side <STRING>` - buy or sell (required)
- `--price <FLOAT>` - Price (required)
- `--quantity <FLOAT>` - Quantity to trade (required)

**Output**: Order ID, status, market, side, price, quantity

---

#### upp orders get <order_id>
Get detailed information about a specific order.

```bash
upp orders get 0xorder123
upp orders get 0xorder123 --json
```

**Parameters**:
- `<order_id>` - The order identifier (required)

**Output**: Order details including status, prices, filled amount, timestamp

---

#### upp orders cancel <order_id>
Cancel an open order.

```bash
upp orders cancel 0xorder123
upp orders cancel 0xorder123 --json
```

**Parameters**:
- `<order_id>` - The order to cancel (required)

**Output**: Confirmation message

---

#### upp orders cancel-all
Cancel all open orders immediately.

```bash
upp orders cancel-all
upp orders cancel-all --json
```

**Output**: Number of cancelled orders

---

### Trades (1 command)

#### upp trades list
View your recent trades and executions.

```bash
upp trades list
upp trades list --limit 100
upp trades list --limit 500
```

**Parameters**:
- `--limit <NUMBER>` - Max results, default 20

**Output**: Table with trade ID, order ID, side, price, quantity, timestamp

---

### Portfolio Management (4 commands)

#### upp portfolio positions
View your current open positions.

```bash
upp portfolio positions
upp portfolio positions --json
```

**Output**: Table with market, outcome, quantity, average price, value

---

#### upp portfolio summary
Quick overview of portfolio performance.

```bash
upp portfolio summary
upp portfolio summary --json
```

**Output**: Total value, cash, invested, P&L, P&L percentage

---

#### upp portfolio analytics
Detailed portfolio analytics with risk metrics.

```bash
upp portfolio analytics
upp portfolio analytics --json
```

**Output**: All summary fields plus volatility, Sharpe ratio, max drawdown

---

#### upp portfolio balances
Account balances by currency/asset.

```bash
upp portfolio balances
upp portfolio balances --json
```

**Output**: Table with symbol, available, reserved, total balance

---

### Arbitrage (2 commands)

#### upp arbitrage list
Find arbitrage opportunities across markets.

```bash
upp arbitrage list
upp arbitrage list --json
```

**Output**: Table with opportunity ID, market, profit amount, profit %, status

---

#### upp arbitrage summary
Statistical summary of arbitrage opportunities.

```bash
upp arbitrage summary
upp arbitrage summary --json
```

**Output**: Total count, average profit %, max profit %, active trades

---

### Technical Analysis (1 command)

#### upp candles <market_id>
Get OHLCV candle data for a market.

```bash
upp candles 0xmkt123
upp candles 0xmkt123 --resolution 5m
upp candles 0xmkt123 --resolution 1d --limit 30
upp candles 0xmkt123 --outcome yes --limit 100
```

**Parameters**:
- `<market_id>` - Market identifier (required)
- `--outcome <STRING>` - Filter by outcome (optional)
- `--resolution <STRING>` - Timeframe: 1m, 5m, 15m, 1h, 4h, 1d (default: 1h)
- `--limit <NUMBER>` - Number of candles, default 100

**Output**: Table with timestamp, open, high, low, close, volume

---

### Backtesting (3 commands)

#### upp backtest strategies
List available backtesting strategies.

```bash
upp backtest strategies
upp backtest strategies --json
```

**Output**: Table with strategy name, description, available parameters

---

#### upp backtest run
Run a backtest of a specific strategy on a market.

```bash
upp backtest run --strategy mean_reversion --market 0xmkt123
upp backtest run --strategy momentum --market 0xmkt456 --params period=14,signal=9
upp backtest run --strategy rsi --market 0xmkt789 --params period=14,oversold=30
```

**Parameters**:
- `--strategy <STRING>` - Strategy name (required)
- `--market <STRING>` - Market ID (required)
- `--params <STRING>` - Strategy parameters as key=val,key=val (optional)

**Output**: Return %, Sharpe ratio, max drawdown, win rate, trade count

---

#### upp backtest compare
Compare multiple strategies on the same market.

```bash
upp backtest compare --market 0xmkt123 --strategies mean_reversion,momentum
upp backtest compare --market 0xmkt456 --strategies rsi,bollinger_bands,macd
```

**Parameters**:
- `--market <STRING>` - Market ID (required)
- `--strategies <STRING>` - Comma-separated strategy names (required)

**Output**: Comparison table with return, Sharpe, max DD, win rate for each

---

### Data Feeds (2 commands)

#### upp feeds status
Check status of all data feeds.

```bash
upp feeds status
upp feeds status --json
```

**Output**: Table with feed name, status, connected, last update time

---

#### upp feeds stats
Get performance statistics for data feeds.

```bash
upp feeds stats
upp feeds stats --json
```

**Output**: Table with messages/sec, error count, latency, uptime for each feed

---

### Route Computation (2 commands)

#### upp route compute
Compute optimal execution route across venues.

```bash
upp route compute --market 0xmkt123 --side buy --quantity 100
upp route compute --market 0xmkt456 --side sell --quantity 50
```

**Parameters**:
- `--market <STRING>` - Market ID (required)
- `--side <STRING>` - buy or sell (required)
- `--quantity <FLOAT>` - Quantity to route (required)

**Output**: Best price, total cost, number of legs, route legs table

---

#### upp route execute
Execute a previously computed route.

```bash
upp route execute '{"market_id":"0xmkt123","side":"buy",...}'
upp route execute @route.json  # from file
```

**Parameters**:
- `<route_json>` - Route JSON string or file (required)

**Output**: Execution ID, status, total cost, timestamp

---

### Configuration (3 commands)

#### upp config set-url <url>
Set the gateway URL in configuration.

```bash
upp config set-url http://localhost:9090
upp config set-url http://staging.example.com:9090
```

**Parameters**:
- `<url>` - Gateway URL (required)

**Output**: Confirmation message

---

#### upp config set-key <key>
Set the API key in configuration.

```bash
upp config set-key your-api-key
upp config set-key sk_prod_abc123xyz
```

**Parameters**:
- `<key>` - API key (required)

**Output**: Confirmation message

---

#### upp config show
Display current configuration.

```bash
upp config show
```

**Output**: Current gateway URL and API key (masked)

---

## Parameter Types

### STRING
Text parameters:
```bash
--provider kalshi
--market 0xabc123...
--strategy mean_reversion
```

### NUMBER (u32, u64)
Integer parameters:
```bash
--limit 50
```

### FLOAT (f64)
Decimal parameters:
```bash
--price 0.55
--quantity 10.5
```

## Output Formatting

### Default (Formatted Tables)
Human-readable colored tables with proper alignment.

```
Orders
──────────────────────────────────────────────────
ID          │ Market         │ Side │ Price │ Qty
0xorder123  │ 0xmkt456...    │ buy  │ $0.55 │ 10
```

### JSON Mode (--json flag)
Raw JSON output for scripting and automation.

```json
{
  "orders": [
    {
      "id": "0xorder123",
      "market": "0xmkt456...",
      "side": "buy",
      "price": 0.55,
      "quantity": 10
    }
  ]
}
```

## Error Handling

All commands return appropriate error messages:

```bash
# Connection error
upp health
> Error: Failed to connect to gateway

# Invalid parameters
upp orders create --market abc --side invalid --price x --quantity y
> Error: invalid side or price format

# Resource not found
upp markets get invalid_id
> Error: Market not found or gateway error
```

## Common Workflows

### Get Started
```bash
upp config set-url http://localhost:9090
upp config show
upp health
```

### Trading
```bash
upp markets list --status open
upp markets get MARKET_ID
upp orders create --market MARKET_ID --side buy --price 0.55 --quantity 10
upp portfolio positions
```

### Analysis
```bash
upp candles MARKET_ID --resolution 1h --limit 50
upp portfolio analytics
upp arbitrage list
```

### Strategy Testing
```bash
upp backtest strategies
upp backtest run --strategy mean_reversion --market MARKET_ID
upp backtest compare --market MARKET_ID --strategies rsi,momentum
```

## Tips

1. **Always set config first**: `upp config set-url ...` then all commands work
2. **Use JSON for piping**: `upp markets list --json | jq '.markets[0]'`
3. **Increase limits for large datasets**: `upp trades list --limit 500`
4. **Use filters to narrow results**: `upp markets list --provider kalshi --status open`
5. **Check feed stats**: `upp feeds stats` to verify data quality
6. **Combine flags**: `--provider X --status Y --limit Z` all work together
7. **Override settings**: `--url` and `--api-key` override config for one command
8. **Monitor in loops**: `watch -n 5 'upp orders list --status open'`

## Summary

- **28 total commands** organized in 11 categories
- **Global flags**: `--url`, `--api-key`, `--json`
- **Common parameters**: Filtering, pagination, output format
- **Consistent interface**: Subcommands with required and optional arguments
- **Full documentation**: See README.md and EXAMPLES.md for detailed usage
