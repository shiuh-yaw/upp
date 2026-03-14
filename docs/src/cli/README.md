# Command-Line Interface

The UPP CLI tool provides quick access to market data, order management, and portfolio operations without writing code.

## Installation

### From Pre-built Binaries

Download from [GitHub Releases](https://github.com/universal-prediction-protocol/upp/releases):

```bash
# macOS (Apple Silicon)
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/upp-cli-darwin-arm64
chmod +x upp-cli-darwin-arm64
sudo mv upp-cli-darwin-arm64 /usr/local/bin/upp

# Linux
wget https://github.com/universal-prediction-protocol/upp/releases/download/v0.1.0/upp-cli-linux-x86_64
chmod +x upp-cli-linux-x86_64
sudo mv upp-cli-linux-x86_64 /usr/local/bin/upp

# Verify installation
upp --version
```

### From Source

```bash
cargo build --release -p cli
sudo cp ./target/release/upp-cli /usr/local/bin/upp
```

### From Cargo

```bash
cargo install upp-cli
```

## Configuration

### Environment Variables

```bash
export UPP_API_KEY=your_api_key
export UPP_GATEWAY_URL=http://localhost:8080  # Optional, defaults to localhost:8080
export UPP_OUTPUT_FORMAT=json                 # json, table, csv
```

### Config File

Create `~/.upp/config.toml`:

```toml
[default]
gateway_url = "http://localhost:8080"
api_key = "your_api_key"
output_format = "table"

[dev]
gateway_url = "http://localhost:8080"
output_format = "json"

[prod]
gateway_url = "https://api.upp.example.com"
output_format = "table"
```

Use with `--profile`:

```bash
upp --profile prod markets list
```

## Commands

### Help

```bash
upp help
upp help markets
upp markets list --help
```

### Health Check

```bash
upp health
```

Returns provider status and uptime.

### Markets

#### List Markets

```bash
# Get 10 Polymarket markets
upp markets list --provider polymarket --limit 10

# Get with specific category
upp markets list --provider kalshi --category politics

# Get all markets from all providers
upp markets list --all-providers

# Export to CSV
upp markets list --provider polymarket --output csv > markets.csv

# Export to JSON
upp markets list --provider polymarket --output json | jq .
```

**Output (table format):**

```
ID                  | Title                                    | Provider   | Liquidity   | Volume 24h
0x1234...abcd      | Will ETH exceed $5000 by Q2 2026?        | polymarket | 1,250,000   | 875,000
0x5678...efgh      | Will Trump win 2028 election?            | kalshi     | 2,100,000   | 1,500,000
```

#### Search Markets

```bash
upp markets search "ethereum"
upp markets search "ethereum" --limit 20
upp markets search "ethereum" --output json
```

#### Get Market Details

```bash
upp markets get 0x1234...abcd

# Pretty print
upp markets get 0x1234...abcd --pretty

# JSON only
upp markets get 0x1234...abcd --output json
```

**Output:**

```
Market: 0x1234...abcd
  Title: Will ETH exceed $5000 by Q2 2026?
  Provider: polymarket
  Status: active
  Liquidity: $1,250,000
  Volume 24h: $875,000
  Created: 2026-01-15 08:30:00
  Expires: 2026-06-30 23:59:59

Outcomes:
  Yes: $0.72 (probability: 72%)
  No:  $0.28 (probability: 28%)
```

### Orders

#### List Orders

```bash
# Get open orders
upp orders list --status open

# Get from specific provider
upp orders list --provider polymarket

# Get filled orders
upp orders list --status filled

# All orders (default)
upp orders list
```

**Output:**

```
Order ID    | Market ID         | Side | Outcome | Price | Qty | Filled | Status
order_12345 | 0x1234...abcd     | BUY  | Yes     | 0.72  | 100 | 100    | FILLED
order_12346 | 0x5678...efgh     | SELL | Yes     | 0.68  | 50  | 25     | PARTIAL
```

#### Get Order Details

```bash
upp orders get order_12345
```

#### Place Order

```bash
# Buy 100 shares at 0.72
upp orders place \
  --market 0x1234...abcd \
  --side BUY \
  --outcome Yes \
  --price 0.72 \
  --quantity 100 \
  --provider polymarket

# Interactive (prompts for values)
upp orders place --interactive
```

**Output:**

```
Placing order...
Order placed: order_12347
Status: OPEN
Filled: 0 / 100
Price: 0.72
```

#### Cancel Order

```bash
upp orders cancel order_12345
upp orders cancel order_12346 order_12347  # Multiple
```

### Portfolio

#### View Portfolio

```bash
upp portfolio view
upp portfolio view --output json
upp portfolio view --output csv > portfolio.csv
```

**Output:**

```
Portfolio Summary
  Cash Balance: $2,500.00
  Total Value: $7,500.00
  Total P&L: $600.00 (8.0%)

Positions:
  Market                              | Outcome | Qty | Entry | Current | P&L    | P&L %
  0x1234...abcd                       | Yes     | 100 | 0.65  | 0.72    | $700   | 10.7%
  0x5678...efgh                       | No      | 500 | 0.40  | 0.38    | -$100  | -5.0%
```

#### List Positions

```bash
# All positions
upp portfolio positions

# By provider
upp portfolio positions --provider polymarket

# Grouped by outcome
upp portfolio positions --group-by outcome

# Sorted by P&L
upp portfolio positions --sort pnl --reverse
```

### Arbitrage

#### Find Opportunities

```bash
# Spread > 5%
upp arbitrage find --min-spread 0.05

# Spread > 5% with profit > $1000
upp arbitrage find --min-spread 0.05 --min-profit 1000

# Limit results
upp arbitrage find --min-spread 0.05 --limit 20
```

**Output:**

```
Arbitrage Opportunities
Market                                      | Outcome | Buy @ | Buy Exchange | Sell @ | Sell Exchange | Spread | Max Vol | Profit
Will ETH exceed $5000 by Q2 2026?          | Yes     | 0.68  | Kalshi       | 0.74   | Polymarket    | 8.8%   | 50k    | $3,000
```

### Backtest

#### Run Backtest

```bash
# Create trades file (JSON)
cat > trades.json << 'EOF'
{
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
}
EOF

# Run backtest
upp backtest run --file trades.json

# Output results to file
upp backtest run --file trades.json --output results.json
```

**Output:**

```
Backtest Results: 0x1234...abcd
  Period: 2025-09-01 to 2026-03-14
  Initial Balance: $10,000.00
  Final Balance: $12,200.00
  Total P&L: $2,200.00 (22.0%)

Performance Metrics:
  Max Drawdown: -8.0%
  Sharpe Ratio: 1.45
  Win Rate: 100%

Trade Details:
  2025-10-01: BUY 1000 @ 0.50 | Balance: $9,500
  2026-01-15: SELL 1000 @ 0.72 | Balance: $12,200 | P&L: $220
```

## Output Formats

### Table (Default)

Human-readable columns:

```bash
upp markets list --output table
```

### JSON

Complete data structure:

```bash
upp markets list --output json | jq .
```

### CSV

Spreadsheet-friendly:

```bash
upp markets list --output csv > markets.csv
```

### Pretty JSON

Indented and colorized:

```bash
upp markets list --output json --pretty
```

## Filtering & Sorting

### Filtering

```bash
# By status
upp markets list --status active

# By category
upp markets list --category politics

# By provider
upp orders list --provider polymarket

# By multiple criteria
upp portfolio positions --provider polymarket --sort pnl
```

### Sorting

```bash
# Sort by liquidity (ascending)
upp markets list --sort liquidity

# Sort by volume descending
upp markets list --sort volume --reverse

# Available sort keys: id, title, provider, liquidity, volume, created, expires
```

## Advanced Usage

### Piping

```bash
# Get market IDs
upp markets list --output json | jq '.markets[].id' | \
  xargs -I {} upp markets get {} --output json

# Get markets with volume > $100k
upp markets list --output json | \
  jq '.markets[] | select(.volume_24h > 100000)'
```

### Scripting

```bash
#!/bin/bash
# Monitor portfolio every 30 seconds

while true; do
  echo "=== $(date) ==="
  upp portfolio view --output json | jq '.portfolio | {balance: .cash_balance, pnl: .total_pnl}'
  sleep 30
done
```

### Cron Jobs

```bash
# Export portfolio daily at 9 AM
0 9 * * * upp portfolio view --output csv >> ~/portfolio-history.csv

# Check for arbitrage opportunities every 5 minutes
*/5 * * * * upp arbitrage find --min-spread 0.05 | mail -s "Arbitrage Alert" user@example.com
```

## Troubleshooting

### "Connection refused"

```
Error: failed to connect to http://localhost:8080
```

Ensure gateway is running:
```bash
docker-compose up -d
```

Or specify correct address:
```bash
upp --gateway-url http://192.168.1.100:8080 health
```

### "Unauthorized"

```
Error: 401 Unauthorized
```

Set API key:
```bash
export UPP_API_KEY=your_api_key
upp portfolio view
```

### "Command not found"

```
bash: upp: command not found
```

Check installation:
```bash
which upp
# If empty, reinstall and ensure /usr/local/bin is in PATH
echo $PATH
```

## See Also

- [REST API Reference](../api/rest.md) — Programmatic access
- [Rust SDK Guide](../sdk/rust.md) — Type-safe client library
- [Installation Guide](../getting-started/installation.md) — Installation methods
