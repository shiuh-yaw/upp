# UPP CLI Examples

Comprehensive examples for common workflows with the UPP Gateway CLI.

## Setup

First, configure the gateway URL and API key:

```bash
upp config set-url http://localhost:9090
upp config set-key your-api-key
upp config show
```

## Basic Operations

### Check Gateway Health

```bash
upp health
upp health --json
```

Output:
```
Gateway Health
──────────────
Status  │ healthy
Uptime  │ 48h 23m
Version │ 1.2.3
```

## Market Operations

### List All Markets

```bash
upp markets list
upp markets list --limit 100
```

### Filter Markets by Provider

```bash
upp markets list --provider kalshi
upp markets list --provider polymarket
upp markets list --provider manifold
```

### Filter Markets by Status

```bash
upp markets list --status open
upp markets list --status closed
upp markets list --provider kalshi --status open
```

### Get Market Details

```bash
upp markets get 0xabc123...
upp markets get 0xabc123... --json
```

Shows detailed information:
```
Market: 0xabc123...
───────────────────
ID       │ 0xabc123...
Name     │ Bitcoin Price > $100k
Provider │ kalshi
Status   │ open
Price    │ $0.65
Volume   │ 5234.50
Created  │ 2024-01-15T10:30:00Z
```

### Search Markets

```bash
upp markets search "bitcoin"
upp markets search "ethereum price prediction"
upp markets search "election 2024" --limit 50
```

## Order Management

### List Your Orders

```bash
upp orders list
upp orders list --limit 50
upp orders list --provider kalshi --status open
```

Output shows:
```
Orders
──────────────────────────────────────────────────
ID                  │ Market            │ Side │ Status  │ Price │ Qty  │ Filled
0x1234567890abcdef  │ 0xmarket123...    │ buy  │ open    │ $0.55 │ 10.0 │ 5.0
0x2345678901bcdef   │ 0xmarket456...    │ sell │ filled  │ $0.75 │ 20.0 │ 20.0
```

### Create an Order

Simple market order:
```bash
upp orders create --market 0xmarket123... --side buy --price 0.55 --quantity 10
```

Short position:
```bash
upp orders create --market 0xmarket456... --side sell --price 0.75 --quantity 20
```

Response:
```
Order created: 0x1234567890abcdef

Order ID │ 0x1234567890abcdef
Status   │ open
Market   │ 0xmarket123...
Side     │ buy
Price    │ $0.55
Quantity │ 10.00
```

### Get Order Details

```bash
upp orders get 0x1234567890abcdef
upp orders get 0x1234567890abcdef --json
```

### Cancel an Order

```bash
upp orders cancel 0x1234567890abcdef
```

### Cancel All Orders

```bash
upp orders cancel-all
```

Response:
```
Cancelled 5 orders
```

## Trade Tracking

### View Recent Trades

```bash
upp trades list
upp trades list --limit 100
```

Output:
```
Trades
──────────────────────────────────────────────
ID           │ Order ID            │ Side  │ Price  │ Quantity │ Time
0xtr1...     │ 0x1234567890abcdef  │ buy   │ $0.54  │ 5.0      │ 2024-01-15 14:23:45
0xtr2...     │ 0x2345678901bcdef   │ sell  │ $0.76  │ 20.0     │ 2024-01-15 14:22:10
```

## Portfolio Management

### View Current Positions

```bash
upp portfolio positions
```

Shows what you own:
```
Positions
────────────────────────────────────────────────
Market                │ Outcome  │ Quantity │ Avg Price │ Value
0xmarket123...        │ yes      │ 10.0     │ $0.55     │ $5.50
0xmarket456...        │ no       │ -20.0    │ $0.75     │ -$15.00
```

### Portfolio Summary

```bash
upp portfolio summary
```

Quick overview:
```
Portfolio Summary
─────────────────
Total Value  │ $1,234.56
Cash         │ $500.00
Invested     │ $734.56
P&L          │ $123.45
P&L %        │ 11.12%
```

### Full Portfolio Analytics

```bash
upp portfolio analytics
```

Detailed metrics:
```
Portfolio Analytics
───────────────────
Total Value  │ $1,234.56
Cash         │ $500.00
Invested     │ $734.56
P&L          │ $123.45
P&L %        │ 11.12%
Volatility   │ 15.23%
Sharpe Ratio │ 1.45
Max Drawdown │ -8.32%
```

### Account Balances

```bash
upp portfolio balances
```

Shows available funds:
```
Account Balances
──────────────────────────────────
Symbol │ Available │ Reserved │ Total
USDC   │ $500.00   │ $0.00    │ $500.00
ETH    │ 0.50      │ 0.00     │ 0.50
DAI    │ 1000.00   │ 0.00     │ 1000.00
```

## Arbitrage Opportunities

### Find Arbitrage Opportunities

```bash
upp arbitrage list
```

View profitable mismatches:
```
Arbitrage Opportunities
──────────────────────────────────────────────────
ID      │ Market            │ Profit  │ Profit % │ Status
arb1234 │ 0xmarket123...    │ $2.50   │ 4.55%    │ available
arb5678 │ 0xmarket456...    │ $1.20   │ 2.17%    │ available
```

### Arbitrage Summary

```bash
upp arbitrage summary
```

Statistical view:
```
Arbitrage Summary
────────────────────────────────
Total Opportunities │ 12
Average Profit %    │ 3.24%
Max Profit %        │ 5.67%
Active Trades       │ 3
```

## Technical Analysis - Candles

### Get Recent Candles

```bash
upp candles 0xmarket123...
```

1-hour candles (default):
```
Candles
────────────────────────────────────────────────────
Time                 │ Open  │ High  │ Low   │ Close │ Volume
2024-01-15 15:00:00  │ $0.54 │ $0.57 │ $0.54 │ $0.56 │ 1500
2024-01-15 14:00:00  │ $0.52 │ $0.55 │ $0.51 │ $0.54 │ 1200
2024-01-15 13:00:00  │ $0.50 │ $0.53 │ $0.50 │ $0.52 │ 900
```

### Get 5-Minute Candles

```bash
upp candles 0xmarket123... --resolution 5m --limit 20
```

### Get Daily Candles

```bash
upp candles 0xmarket123... --resolution 1d --limit 30
```

### Filter by Outcome

```bash
upp candles 0xmarket123... --outcome yes
```

## Backtesting Strategies

### List Available Strategies

```bash
upp backtest strategies
```

Output:
```
Available Strategies
────────────────────────────────────────────────
Name              │ Description                    │ Parameters
mean_reversion    │ Mean reversion strategy        │ lookback, threshold
momentum          │ Momentum-based strategy        │ period, signal_period
rsi               │ RSI-based strategy             │ period, oversold
bollinger_bands   │ Bollinger Bands strategy       │ period, std_dev
```

### Run Simple Backtest

```bash
upp backtest run --strategy mean_reversion --market 0xmarket123...
```

Results:
```
Backtest Results
────────────────
Strategy     │ mean_reversion
Market       │ 0xmarket123...
Return       │ 15.32%
Sharpe Ratio │ 1.42
Max Drawdown │ -5.20%
Win Rate     │ 62.34%
Trades       │ 45
```

### Backtest with Parameters

```bash
upp backtest run \
  --strategy momentum \
  --market 0xmarket123... \
  --params period=14,signal_period=9
```

### Compare Multiple Strategies

```bash
upp backtest compare \
  --market 0xmarket123... \
  --strategies mean_reversion,momentum,rsi
```

Comparison table:
```
Strategy Comparison
─────────────────────────────────────────────────
Strategy           │ Return  │ Sharpe │ Max DD │ Win Rate │ Trades
mean_reversion     │ 15.32%  │ 1.42   │ -5.20% │ 62.34%   │ 45
momentum           │ 12.15%  │ 1.18   │ -7.50% │ 58.92%   │ 38
rsi                │ 8.45%   │ 0.92   │ -9.30% │ 54.21%   │ 52
```

## Data Feed Management

### Check Feed Status

```bash
upp feeds status
```

Connection status:
```
Feed Status
──────────────────────────────────────────────────
Feed           │ Status     │ Connected │ Last Update
kalshi         │ healthy    │ Yes       │ 2024-01-15 14:23:45
polymarket     │ healthy    │ Yes       │ 2024-01-15 14:23:42
manifold       │ healthy    │ Yes       │ 2024-01-15 14:23:43
alternative    │ degraded   │ Yes       │ 2024-01-15 14:22:00
```

### Feed Statistics

```bash
upp feeds stats
```

Performance metrics:
```
Feed Statistics
───────────────────────────────────────────────────
Feed           │ Messages/sec │ Errors │ Latency (ms) │ Uptime
kalshi         │ 125.45       │ 0      │ 45.2         │ 99.99%
polymarket     │ 98.32        │ 2      │ 52.1         │ 99.98%
manifold       │ 75.18        │ 0      │ 38.5         │ 99.99%
alternative    │ 12.50        │ 145    │ 234.7        │ 98.50%
```

## Route Computation and Execution

### Compute Execution Route

Find optimal split across venues:

```bash
upp route compute --market 0xmarket123... --side buy --quantity 100
```

Output:
```
Route Computation
────────────────────────────
Market        │ 0xmarket123...
Side          │ buy
Quantity      │ 100.00
Best Price    │ $0.55
Total Cost    │ $55.00
Legs          │ 2

Route Legs
──────────────────────────────────────────────
Provider   │ Quantity │ Price  │ Cost
kalshi     │ 60.00    │ $0.54  │ $32.40
polymarket │ 40.00    │ $0.56  │ $22.40
```

### Execute a Route

```bash
upp route execute '{
  "market_id": "0xmarket123...",
  "side": "buy",
  "quantity": 100,
  "legs": [
    {"provider": "kalshi", "quantity": 60, "price": 0.54},
    {"provider": "polymarket", "quantity": 40, "price": 0.56}
  ]
}'
```

Confirmation:
```
Route executed successfully

Execution ID  │ exec_0x1234567...
Status        │ filled
Total Cost    │ $55.00
Timestamp     │ 2024-01-15 14:25:33
```

## Advanced Workflows

### Complete Trade Entry to Exit

```bash
# 1. Find market
upp markets search "ethereum" --limit 5

# 2. Create position
upp orders create \
  --market 0xmarket123... \
  --side buy \
  --price 0.55 \
  --quantity 50

# 3. Monitor position
upp portfolio positions

# 4. Exit position
upp orders create \
  --market 0xmarket123... \
  --side sell \
  --price 0.75 \
  --quantity 50

# 5. Review trades
upp trades list --limit 10
```

### Backtest and Deploy Strategy

```bash
# 1. List strategies
upp backtest strategies

# 2. Test on market
upp backtest run \
  --strategy momentum \
  --market 0xmarket123... \
  --params period=14,signal_period=9

# 3. Compare alternatives
upp backtest compare \
  --market 0xmarket123... \
  --strategies momentum,mean_reversion,rsi

# 4. Monitor live execution
upp orders list --status open
upp trades list --limit 50
```

### Portfolio Rebalancing

```bash
# 1. View current allocation
upp portfolio positions
upp portfolio summary

# 2. Identify over/under-weighted positions
# (manual analysis)

# 3. Execute trades
upp orders create --market market1 --side buy --price 0.55 --quantity 20
upp orders create --market market2 --side sell --price 0.75 --quantity 30

# 4. Verify new allocation
upp portfolio positions
upp portfolio summary
```

### Arbitrage Execution

```bash
# 1. Find opportunities
upp arbitrage list

# 2. Compute routes for highest profit
upp route compute --market arb_market_1 --side buy --quantity 100
upp route compute --market arb_market_2 --side sell --quantity 100

# 3. Execute both routes
upp route execute '{"market_id":"arb_market_1",...}'
upp route execute '{"market_id":"arb_market_2",...}'

# 4. Monitor execution
upp trades list --limit 5

# 5. Check summary
upp arbitrage summary
```

## JSON Output for Scripting

All commands support `--json` for programmatic use:

```bash
# Parse with jq
upp markets list --json | jq '.markets[0]'

# Store for processing
upp portfolio summary --json > portfolio.json

# Pipeline example
upp arbitrage list --json | jq '.opportunities | max_by(.profit_percentage)'
```

## Override Gateway Settings

Connect to different gateway:

```bash
upp health --url http://staging.example.com:9090

upp orders list --url http://testnet:9090 --api-key test-key
```

## Common Patterns

### Monitor Market in Real-time

```bash
while true; do
  clear
  upp markets get 0xmarket123...
  sleep 5
done
```

### Track All Open Orders

```bash
watch -n 5 'upp orders list --status open'
```

### Export Portfolio to CSV

```bash
upp portfolio positions --json | jq -r '.positions[] | [.market_id, .outcome, .quantity, .value] | @csv'
```

### Compare Strategy Performance

```bash
# Run multiple backtests and export
for strategy in mean_reversion momentum rsi; do
  upp backtest run \
    --strategy $strategy \
    --market 0xmarket123... \
    --json >> results.json
done
```

## Tips and Tricks

1. **Save config once**: Set gateway URL and key once, reuse across commands
2. **Use JSON for automation**: Pipe `--json` output to jq, Python, or other tools
3. **Monitor feeds**: Regular `upp feeds stats` checks ensure data quality
4. **Test backtests**: Validate strategy assumptions before live trading
5. **Verify routes**: Review computed routes before execution with `--json`
6. **Use limits**: Start with `--limit 10` for large result sets
7. **Combine filters**: Use `--provider` and `--status` together for precise queries

## Troubleshooting

### Connection Failed

```bash
# Check gateway is running
upp health

# Verify URL configuration
upp config show

# Override temporarily
upp health --url http://different-host:9090
```

### No Results

```bash
# Check with higher limit
upp markets list --limit 100

# Verify filters
upp markets list --provider kalshi  # check provider name

# Search instead of list
upp markets search "keywords"
```

### JSON Parse Errors

```bash
# View raw JSON to diagnose
upp <command> --json

# Common issues:
# - Missing fields (use .field? for optional)
# - Different data types (check numeric vs string)
```
