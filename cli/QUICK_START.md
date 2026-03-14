# UPP CLI - Quick Start Guide

Get up and running with the UPP Gateway CLI in 5 minutes.

## Installation

### Build from Source
```bash
cd /sessions/stoic-compassionate-turing/mnt/outputs/upp/cli
cargo build --release
```

The binary will be at: `target/release/upp`

### Install Globally
```bash
cargo install --path .
# Binary installed to ~/.cargo/bin/upp
# Add ~/.cargo/bin to PATH if not already
```

## Initial Setup

### 1. Set Gateway URL
```bash
upp config set-url http://localhost:9090
```

### 2. (Optional) Set API Key
```bash
upp config set-key your-api-key-here
```

### 3. Verify Configuration
```bash
upp config show
```

### 4. Test Connection
```bash
upp health
```

You should see: `Status | healthy`

## 5-Minute Walkthrough

### Check Markets
```bash
# List all markets
upp markets list

# Filter by provider
upp markets list --provider kalshi

# Search for specific market
upp markets search "bitcoin"
```

### Create an Order
```bash
# Find a market ID from the list above, then:
upp orders create --market 0xMARKET_ID --side buy --price 0.55 --quantity 10
```

### Check Your Portfolio
```bash
# View positions
upp portfolio positions

# Quick summary
upp portfolio summary

# Full analytics
upp portfolio analytics
```

### View Recent Trades
```bash
upp trades list
```

### Get Candle Data
```bash
upp candles 0xMARKET_ID --resolution 1h --limit 50
```

### Test Strategy
```bash
# See available strategies
upp backtest strategies

# Run backtest
upp backtest run --strategy mean_reversion --market 0xMARKET_ID
```

## Common Commands

| Task | Command |
|------|---------|
| Check health | `upp health` |
| List markets | `upp markets list` |
| Create order | `upp orders create --market ID --side buy --price 0.55 --quantity 10` |
| List orders | `upp orders list` |
| Cancel order | `upp orders cancel 0xORDER_ID` |
| View positions | `upp portfolio positions` |
| Portfolio summary | `upp portfolio summary` |
| Find arbitrage | `upp arbitrage list` |
| Backtest strategy | `upp backtest run --strategy mean_reversion --market ID` |
| Compute route | `upp route compute --market ID --side buy --quantity 100` |

## Global Flags

Use these with any command:

```bash
# Output as JSON instead of table
upp markets list --json

# Use different gateway
upp health --url http://staging:9090

# Override API key
upp portfolio positions --api-key different-key
```

## View Help

For any command, add `--help`:

```bash
upp --help
upp markets --help
upp orders create --help
```

## Output Modes

### Pretty Tables (Default)
```bash
upp portfolio summary

Portfolio Summary
─────────────────
Total Value  │ $1,234.56
Cash         │ $500.00
Invested     │ $734.56
P&L          │ $123.45
P&L %        │ 11.12%
```

### JSON (for scripting)
```bash
upp portfolio summary --json

{
  "total_value": 1234.56,
  "cash": 500.00,
  "invested": 734.56,
  "pnl": 123.45,
  "pnl_percentage": 11.12
}
```

## Scripting with JSON

Combine with `jq` for powerful queries:

```bash
# Get first market
upp markets list --json | jq '.markets[0]'

# Extract all market IDs
upp markets list --json | jq -r '.markets[].id'

# Find highest price market
upp markets list --json | jq '.markets | max_by(.price)'

# Count markets
upp markets list --json | jq '.markets | length'
```

## Next Steps

1. **Read More**: Check out `EXAMPLES.md` for detailed workflows
2. **Command Reference**: See `COMMAND_REFERENCE.md` for all 28 commands
3. **Advanced Use**: Explore backtesting, arbitrage, route computation
4. **Integration**: Build scripts that use `--json` output

## Troubleshooting

### Connection Failed
```bash
# Verify gateway is running
upp health

# Check configuration
upp config show

# Test with explicit URL
upp health --url http://localhost:9090
```

### No Results
```bash
# Increase limit
upp markets list --limit 100

# Try different filter
upp markets list --provider kalshi

# Search instead
upp markets search "your query"
```

### JSON Parsing Error
```bash
# View raw JSON to debug
upp <command> --json

# Use jq to validate
upp <command> --json | jq
```

## Performance Tips

1. **Use filters**: `--provider`, `--status` reduce data
2. **Set limits**: Start with `--limit 20`, increase if needed
3. **JSON mode**: Faster for large datasets
4. **Direct queries**: Search specific markets instead of listing all

## Security Notes

- API key stored in `~/.upp/config.toml` (user-readable)
- Use `--api-key` flag for sensitive operations
- Don't commit config files to version control
- Secure your gateway credentials

## All 28 Commands at a Glance

**Health**: health

**Markets**: markets list, markets get, markets search

**Orders**: orders list, orders create, orders get, orders cancel, orders cancel-all

**Trades**: trades list

**Portfolio**: portfolio positions, portfolio summary, portfolio analytics, portfolio balances

**Arbitrage**: arbitrage list, arbitrage summary

**Technical**: candles

**Backtesting**: backtest strategies, backtest run, backtest compare

**Feeds**: feeds status, feeds stats

**Routes**: route compute, route execute

**Config**: config set-url, config set-key, config show

## Documentation Map

| Document | Purpose | When to Use |
|----------|---------|-------------|
| QUICK_START.md | This file | First time setup |
| README.md | Getting started | Understanding the tool |
| EXAMPLES.md | Real workflows | Learning by example |
| COMMAND_REFERENCE.md | All commands | Looking up specific commands |
| PROJECT_SUMMARY.md | Technical details | Understanding architecture |
| FILE_MANIFEST.txt | File inventory | Verifying installation |

## One-Liners

```bash
# Monitor orders live
watch -n 5 'upp orders list --status open'

# Export portfolio to CSV
upp portfolio positions --json | jq -r '.positions[] | [.market_id, .outcome, .quantity] | @csv'

# Find arbitrage opportunities
upp arbitrage list --json | jq '.opportunities | sort_by(-.profit_percentage) | .[0]'

# List all open markets
upp markets list --json | jq '.markets[] | select(.status=="open") | .name'

# Get latest trades
upp trades list --limit 10 --json | jq '.trades[0]'

# Compare strategies
upp backtest compare --market MARKET_ID --strategies mean_reversion,momentum --json | jq '.results | sort_by(-.return)'

# Check feed health
upp feeds status --json | jq '.feeds[] | select(.status != "healthy")'
```

## Getting Help

1. **Built-in help**: `upp --help`, `upp <command> --help`
2. **Documentation**: See README.md, EXAMPLES.md, COMMAND_REFERENCE.md
3. **Issues**: Review error messages - they're descriptive
4. **JSON debugging**: Use `--json` flag to see raw data

## What's Next?

1. Set up config (above)
2. Run `upp health` to verify
3. Try `upp markets list` to explore
4. Create your first order with `upp orders create`
5. Monitor portfolio with `upp portfolio positions`
6. Explore advanced features (backtesting, arbitrage)

Enjoy using the UPP CLI!
