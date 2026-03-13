# UPP Gateway Examples - Overview

This directory contains 5 production-ready example applications demonstrating real-world usage of the UPP (Universal Prediction Platform) gateway.

## Files Created

### Core Examples (4 Python scripts)

1. **arbitrage_scanner.py** (329 lines)
   - Cross-provider arbitrage opportunity detector
   - Scans Kalshi, Polymarket, and other providers for profitable spreads
   - Features: continuous monitoring, category filtering, fee calculations
   - Key endpoints: `/upp/v1/markets`, `/upp/v1/markets/{id}/orderbook/merged`

2. **portfolio_rebalancer.py** (360 lines)
   - Portfolio allocation monitoring and rebalancing suggestion engine
   - Calculates drift from target allocation and suggests corrective trades
   - Features: ASCII pie charts, dry-run/execute modes, trade size limits
   - Key endpoints: `/upp/v1/markets`, `/upp/v1/orders/estimate`

3. **market_monitor.py** (327 lines)
   - Real-time market dashboard with live price updates
   - Displays prices, 24h volume, spreads with color-coded changes
   - Features: top N markets by volume, specific market tracking, static view
   - Key endpoints: `/upp/v1/markets`, `/upp/v1/markets/{id}`

4. **mcp_agent_demo.py** (343 lines)
   - AI agent interaction demonstration with MCP tool calls
   - Shows multi-step market analysis workflow
   - Features: tool execution visualization, interactive mode, reasoning display
   - Key endpoint: `POST /upp/v1/mcp/execute`

### Documentation

5. **README.md** (500+ lines)
   - Comprehensive usage guide for all examples
   - API reference with endpoint documentation
   - Market object structure and JSON schemas
   - Troubleshooting, patterns, and extension guide

6. **OVERVIEW.md** (this file)
   - Quick reference overview

## Quick Start

### Prerequisites
```bash
# Python 3.9+
python3 --version

# UPP gateway running on localhost:8080
curl http://localhost:8080/upp/v1/markets?limit=1
```

### Running Examples

```bash
# Arbitrage scanning (single scan)
python3 arbitrage_scanner.py

# Continuous arbitrage monitoring
python3 arbitrage_scanner.py --monitor --interval 30 --min-spread 0.02

# Portfolio analysis
echo '{"politics": 0.4, "sports": 0.3, "crypto": 0.3}' > target.json
python3 portfolio_rebalancer.py --target target.json

# Live market dashboard
python3 market_monitor.py --top 10

# AI agent analysis
python3 mcp_agent_demo.py --topic "bitcoin price"
python3 mcp_agent_demo.py --interactive
```

## Architecture

All scripts follow a consistent design:

```
┌─────────────────────────────────────┐
│   Example Script                    │
├─────────────────────────────────────┤
│ • argparse CLI interface            │
│ • Color-coded terminal output       │
│ • Graceful error handling           │
│ • UPPClient wrapper class           │
│ • Domain-specific analysis logic    │
└──────────────────┬──────────────────┘
                   │
                   ▼
         ┌─────────────────────┐
         │  UPPClient Class    │
         ├─────────────────────┤
         │ • HTTP requests     │
         │ • JSON parsing      │
         │ • Error handling    │
         └──────────────┬──────┘
                        │
                        ▼
        ┌───────────────────────────┐
        │ UPP Gateway               │
        │ (localhost:8080)          │
        ├───────────────────────────┤
        │ REST API endpoints        │
        │ WebSocket (ws://)         │
        │ MCP tool execution        │
        └───────────────────────────┘
```

## Key Concepts

### Market ID Format
- `provider:native_id` (e.g., "kalshi:btc_50k_2024")
- Or as object: `{"provider": "kalshi", "native_id": "btc_50k_2024"}`

### Pricing Model
Each market tracks outcomes (typically Yes/No) with:
- `last_price`: Most recent execution price
- `best_bid`: Highest buy order
- `best_ask`: Lowest sell order
- `spread`: Difference between best_ask and best_bid

### Arbitrage Mechanism
```
Buy at provider A's ask (lower price)
    ↓
Sell at provider B's bid (higher price)
    ↓
Profit = (bid - ask) - (2 × fee_rate)
```

### Portfolio Allocation
Current allocation = sum of position values by category / total portfolio value
Target allocation = JSON file with category weights
Drift = target - current (positive = underweight, negative = overweight)

## Features by Example

| Feature | Arbitrage | Portfolio | Monitor | MCP Agent |
|---------|-----------|-----------|---------|-----------|
| Market search | ✓ | - | ✓ | ✓ |
| Orderbook analysis | ✓ | - | ✓ | ✓ |
| Real-time updates | ✓ | - | ✓ | - |
| Trade execution | - | ✓ | - | - |
| Portfolio tracking | - | ✓ | - | - |
| MCP tools | - | - | - | ✓ |
| Interactive mode | - | - | - | ✓ |
| Monitoring loop | ✓ | - | ✓ | - |

## Code Quality

- **100% stdlib Python** (except websockets with fallback)
- **Type hints** throughout for clarity
- **Docstrings** on all classes and methods
- **Error handling** with user-friendly messages
- **Color output** for terminal clarity
- **Graceful shutdown** on interrupt (Ctrl+C)
- **Configurable via CLI** with argparse

## Testing Checklist

- [x] All scripts compile without syntax errors
- [x] Proper imports (stdlib only)
- [x] CLI argument parsing works
- [x] Color output classes defined
- [x] Error handling implemented
- [x] Type hints present
- [x] Docstrings complete
- [x] Main entry points defined

## File Statistics

```
arbitrage_scanner.py     329 lines    13 KB
portfolio_rebalancer.py  360 lines    12 KB
market_monitor.py        327 lines    11 KB
mcp_agent_demo.py        343 lines    12 KB
README.md                500+ lines   15 KB
────────────────────────────────────────
Total                    1300+ lines   65+ KB
```

## Gateway Endpoints Used

### Markets API
- `GET /upp/v1/markets` — List markets
- `GET /upp/v1/markets/search` — Search markets
- `GET /upp/v1/markets/{id}` — Market details
- `GET /upp/v1/markets/{id}/orderbook` — Single provider orderbook
- `GET /upp/v1/markets/{id}/orderbook/merged` — Cross-provider comparison

### Trading API
- `POST /upp/v1/orders/estimate` — Estimate order cost

### MCP API
- `POST /upp/v1/mcp/execute` — Execute any MCP tool

### WebSocket
- `WS ws://localhost:8080/upp/v1/ws` — Live price/orderbook updates

## Next Steps

1. **Run a single example** to verify gateway connectivity
2. **Modify parameters** to match your use case
3. **Add custom logic** for your specific needs
4. **Extend with new features** using the patterns shown
5. **Deploy as microservices** for production use

## Common Extensions

### Add Database Logging
```python
import sqlite3
conn = sqlite3.connect('opportunities.db')
# Log opportunities for analysis
```

### Add Email Alerts
```python
import smtplib
if profit > threshold:
    send_email("High arbitrage opportunity detected")
```

### Add Webhook Notifications
```python
import urllib.request
if arbitrage_found:
    urllib.request.urlopen("https://hook.example.com", data=json.dumps(opp))
```

### Add Prometheus Metrics
```python
from prometheus_client import Counter, start_http_server
arbitrage_count = Counter('arbitrage_opportunities', 'Found opportunities')
```

## Troubleshooting

See README.md for detailed troubleshooting guide.

Common issues:
- **Connection refused**: Gateway not running
- **No markets found**: Provider credentials missing
- **Slow responses**: Network latency or gateway overload
- **Missing prices**: Market not actively trading

## Resources

- UPP Gateway documentation: See `/sessions/.../upp/`
- Python docs: https://docs.python.org/3/
- Market data format: See README.md for JSON schemas
