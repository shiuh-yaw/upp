# UPP Gateway Example Applications

This directory contains real-world bot applications demonstrating usage of the UPP (Universal Prediction Platform) gateway REST API and WebSocket endpoints.

All examples use only Python standard library (except where noted) and require the UPP gateway running on `localhost:8080`.

## Prerequisites

- Python 3.9+
- UPP gateway running at `http://localhost:8080`
- All scripts are executable: `python script_name.py [options]`

## Examples

### 1. Arbitrage Scanner (`arbitrage_scanner.py`)

**Purpose:** Identifies profitable arbitrage opportunities between different prediction market providers.

**How it works:**
- Scans markets from multiple providers (Kalshi, Polymarket, etc.)
- Groups markets by event title
- Compares best_bid on one provider vs best_ask on another
- Calculates arbitrage spread and theoretical profit after fees (0.2% per side)
- Displays opportunities in priority order

**Key features:**
- Continuous monitoring mode with configurable interval
- Filter by market category (politics, sports, crypto, etc.)
- Minimum spread threshold to ignore marginal opportunities
- Colored output (green = profitable, yellow = marginal)
- Pretty-printed table format for easy analysis

**Usage:**

```bash
# Single scan for current opportunities
python arbitrage_scanner.py

# Monitor continuously every 30 seconds
python arbitrage_scanner.py --monitor --interval 30

# Filter by category, show only 2%+ spreads
python arbitrage_scanner.py --monitor --category politics --min-spread 0.02

# Use custom gateway URL
python arbitrage_scanner.py --url http://localhost:9000 --monitor
```

**Example output:**
```
Opportunity #1
  Event: Trump re-election 2024
  Outcome: Yes
  Buy on polymarket: $0.6500
  Sell on kalshi: $0.7200
  Spread: 10.77%
  Profit after fees: 10.37%
```

**Key endpoints used:**
- `GET /upp/v1/markets?provider=X&limit=N` — List markets by provider
- `GET /upp/v1/markets/{market_id}/orderbook/merged` — Compare cross-provider prices

---

### 2. Portfolio Rebalancer (`portfolio_rebalancer.py`)

**Purpose:** Monitors portfolio allocation across providers and suggests rebalancing trades.

**How it works:**
- Scans markets across all providers
- Calculates current allocation by category (politics, sports, crypto, etc.)
- Compares to target allocation from JSON file
- Suggests minimum trades needed to rebalance
- Optionally executes trades (with `--execute` flag)

**Key features:**
- Target allocation from JSON file
- ASCII pie chart showing current allocation
- Drift calculation (how far from target)
- Maximum trade size limit
- Dry-run mode (default) vs execute mode
- Pretty-printed rebalancing plan with estimated costs

**Usage:**

```bash
# Create target allocation file
cat > target.json << 'EOF'
{
  "politics": 0.3,
  "sports": 0.3,
  "crypto": 0.4
}
EOF

# Show current allocation
python portfolio_rebalancer.py --target target.json

# Show plan and execute (with confirmation)
python portfolio_rebalancer.py --target target.json --execute

# Limit single trade size to $500
python portfolio_rebalancer.py --target target.json --execute --max-trade-size 500
```

**Example output:**
```
Current Allocation:
  crypto         ████████████████░░░░░░░░░░░░░░░░  40.0%
  politics       ███████████░░░░░░░░░░░░░░░░░░░░░░  30.0%
  sports         ███████████░░░░░░░░░░░░░░░░░░░░░░  30.0%

Rebalancing Plan (2 trades):

Direction   Category        Quantity   Est. Cost    Reason
sell        crypto            200.00   $10,000.00   Rebalance crypto from 40.0% to 35.0%
buy         politics          100.00   $5,000.00    Rebalance politics from 25.0% to 30.0%
```

**Key endpoints used:**
- `GET /upp/v1/markets?limit=N` — Get all markets
- `POST /upp/v1/orders/estimate` — Estimate trade costs

---

### 3. Market Monitor (`market_monitor.py`)

**Purpose:** Real-time terminal dashboard showing live market prices and changes.

**How it works:**
- Fetches top markets by volume
- Uses ANSI escape codes for colored terminal output
- Updates display at 1Hz with live price changes
- Shows: title, yes/no prices, 24h volume, spreads
- Color-codes price movements (green = up, red = down)
- Graceful handling of disconnections

**Key features:**
- Live-updating terminal dashboard
- Color-coded price changes
- Configurable number of markets to display
- Option to monitor specific market IDs
- Static view mode (non-updating)
- Keyboard interrupt handling

**Usage:**

```bash
# Show top 10 markets by volume with live updates
python market_monitor.py --top 10

# Show specific markets
python market_monitor.py --markets "kalshi:native1" "polymarket:native2"

# Static view (no updates)
python market_monitor.py --top 15 --static

# Use custom gateway
python market_monitor.py --url http://localhost:9000 --top 5
```

**Example output:**
```
Market Monitor Dashboard
Updated: 2024-03-13 14:30:45 | Updates: 127
────────────────────────────────────────────────────────────────────────────
Market                                   YES          NO           Spread     24h Vol      Status
────────────────────────────────────────────────────────────────────────────
Trump 2024 Re-election                   $0.6432      $0.3568      0.0864     12450.23     active
Federal Reserve Rate Decision            $0.7821      $0.2179      0.0642      8923.10     active
Bitcoin above $50k by June               $0.5234      $0.4766      0.0468      5612.34     active
────────────────────────────────────────────────────────────────────────────
```

**Key endpoints used:**
- `GET /upp/v1/markets?limit=N` — List top markets
- `GET /upp/v1/markets/{market_id}` — Get market details
- `WS ws://localhost:8080/upp/v1/ws` — WebSocket for live updates (polling fallback)

---

### 4. MCP Agent Demo (`mcp_agent_demo.py`)

**Purpose:** Demonstrates how an AI agent would use MCP (Model Context Protocol) tools to analyze markets.

**How it works:**
- Simulates an agent analysis workflow:
  1. Search for markets about a topic
  2. Fetch detailed market information
  3. Analyze order book depth and liquidity
  4. Estimate trade cost
  5. Summarize findings
- Each step shows the tool call and result
- Interactive mode allows user queries

**Key features:**
- Simulated agent reasoning with color-coded steps
- Tool call/result pretty-printing
- Interactive query mode
- Multi-step market analysis workflow
- Natural language queries about markets

**Usage:**

```bash
# Analyze a specific topic
python mcp_agent_demo.py --topic "bitcoin price"

# Interactive mode (user provides queries)
python mcp_agent_demo.py --interactive

# Specify gateway URL
python mcp_agent_demo.py --topic "US election 2024" --url http://localhost:9000

# Default analysis (bitcoin)
python mcp_agent_demo.py
```

**Example output:**
```
UPP Market Analysis Agent
Topic: bitcoin price prediction

────────────────────────────────────────────────────────────────────────────

Step 1: Searching for markets...
Reasoning: User asked about 'bitcoin price prediction', so search for related prediction markets

→ Tool Call:
  Tool: search_markets
  Params: {"query": "bitcoin price prediction"}

← Result:
  Found 5 items:
    [1] Bitcoin above $50k by June 2024
    [2] Bitcoin dominance over 50% by 2024
    [3] BTC/USD price target 2024
    [4] Cryptocurrency market cap $2T by year-end
    [5] Ethereum outperformance vs Bitcoin

Step 2: Getting market details...
→ Tool Call:
  Tool: get_market_details
  Params: {"market_id": "kalshi:btc_50k_2024"}

← Result:
  Found market: Bitcoin above $50k by June 2024
  Possible outcomes: Yes, No
  Market spread: 0.0234

Summary
─────────────────────────────────────────────────────────────────────────────
Found 5 relevant markets for the topic.
Analyzed market: Bitcoin above $50k by June 2024 (crypto)
Possible outcomes: Yes, No
Market spread: 0.0234
To buy 100 shares: estimated cost $5234.20
```

**Key endpoints used:**
- `POST /upp/v1/mcp/execute` — Execute any MCP tool
  - `search_markets` — Search for markets by keyword
  - `get_market_details` — Get full market information
  - `get_orderbook` — Analyze order book depth
  - `estimate_order` — Calculate trade cost

---

## API Reference

### REST Endpoints

```
GET /upp/v1/markets?provider=X&limit=N
  List markets from specific provider with limit

GET /upp/v1/markets/search?q=X
  Search markets by query string

GET /upp/v1/markets/{market_id}
  Get detailed market information

GET /upp/v1/markets/{market_id}/orderbook?depth=N
  Get orderbook with specified depth

GET /upp/v1/markets/{market_id}/orderbook/merged?depth=N
  Get merged orderbook across all providers

POST /upp/v1/orders/estimate
  Estimate order cost
  Body: {"market_id": "...", "side": "buy|sell", "quantity": N}

POST /upp/v1/mcp/execute
  Execute MCP tool
  Body: {"tool": "...", "params": {...}}
```

### WebSocket

```
WS ws://localhost:8080/upp/v1/ws
  Subscribe to live price updates (JSON-RPC 2.0)

  Messages:
    {"method": "subscribe_prices", "params": ["market_id1", "market_id2"]}
    {"method": "subscribe_orderbook", "params": {"market_id": "...", "depth": 5}}
    {"method": "ping"}
```

### Market Object Structure

```json
{
  "id": {
    "provider": "kalshi|polymarket|...",
    "native_id": "market_identifier"
  },
  "event": {
    "title": "Market title",
    "description": "Detailed description",
    "category": "politics|sports|crypto|..."
  },
  "outcomes": [
    {"id": "outcome_1", "label": "Yes"},
    {"id": "outcome_2", "label": "No"}
  ],
  "pricing": {
    "last_price": {"outcome_1": "0.65", "outcome_2": "0.35"},
    "best_bid": {"outcome_1": "0.64", "outcome_2": "0.34"},
    "best_ask": {"outcome_1": "0.66", "outcome_2": "0.36"},
    "spread": "0.02"
  },
  "volume": {
    "total_volume": 50000.0,
    "volume_24h": 12500.0,
    "open_interest": 8750.0
  },
  "lifecycle": {
    "status": "active|closed|suspended"
  }
}
```

---

## Common Patterns

### Error Handling

All scripts handle gateway connection failures gracefully:

```python
except urllib.error.URLError as e:
    print(f"Error connecting to gateway: {e}")
    # Retry or exit cleanly
```

### Retry Logic

For production use, consider adding retry logic with exponential backoff:

```python
import time
max_retries = 3
for attempt in range(max_retries):
    try:
        result = client.get_markets()
        break
    except Exception as e:
        if attempt == max_retries - 1:
            raise
        time.sleep(2 ** attempt)
```

### Color Output

All examples use ANSI escape codes for colored terminal output. To disable colors:

```python
# In any script, set:
class Colors:
    GREEN = ""
    RED = ""
    # ... etc
```

---

## Gateway Connection

By default, all examples connect to `http://localhost:8080`. To use a different URL:

```bash
python arbitrage_scanner.py --url http://gateway.example.com:8080
python portfolio_rebalancer.py --url http://192.168.1.100:8080
python market_monitor.py --url http://gateway:8080
python mcp_agent_demo.py --url http://remote-gateway:9000
```

---

## Rate Limiting

The gateway may implement rate limits. Recommended practices:

1. **Arbitrage scanner**: 30s+ intervals between full scans
2. **Portfolio rebalancer**: Run once per minute or less
3. **Market monitor**: 1Hz update rate for dashboard
4. **MCP agent**: Sequential requests with 500ms+ delays between steps

---

## Testing

Test gateway connectivity:

```bash
curl http://localhost:8080/upp/v1/markets?limit=1
```

Quick test of each example:

```bash
# Test arbitrage scanner (single scan)
python arbitrage_scanner.py

# Test portfolio rebalancer
echo '{"politics": 0.5, "sports": 0.5}' > /tmp/test_target.json
python portfolio_rebalancer.py --target /tmp/test_target.json

# Test market monitor (static view)
python market_monitor.py --top 5 --static

# Test MCP agent
python mcp_agent_demo.py --topic "test"
```

---

## Extending the Examples

### Adding New Providers

To add a new provider to the arbitrage scanner:

1. Update the client to fetch from the new provider
2. The grouping by event title handles cross-provider matching automatically
3. Add provider name to category filters if needed

### Custom Market Categories

Modify the category filter in any script to match your use case:

```python
# In arbitrage_scanner.py
CATEGORIES = ["politics", "sports", "crypto", "commodities", "custom"]
```

### Performance Tuning

For high-volume monitoring:

1. Increase `--interval` in arbitrage scanner
2. Use `--max-trade-size` in portfolio rebalancer to limit work
3. Reduce `--top` in market monitor for fewer markets
4. Cache market details in MCP agent for repeated queries

---

## Troubleshooting

### Gateway Connection Refused

```
Error connecting to gateway: [Errno 111] Connection refused
```

Solution: Ensure UPP gateway is running on `localhost:8080`

```bash
curl http://localhost:8080/upp/v1/markets?limit=1
```

### No Markets Found

The gateway may be running but without market data loaded. Check:
- Gateway logs for indexing status
- Market provider API credentials
- Gateway configuration

### Slow Response Times

- Check network connectivity
- Verify gateway isn't under high load
- Consider reducing batch size (`--limit`)
- Add retry delays for throttled responses

### Price Data Missing

- Verify market lifecycle status is "active"
- Check if pricing info is available for all outcomes
- Some markets may not have live pricing

---

## License

These examples are provided as-is for demonstration purposes.

## Support

For issues, questions, or contributions:

1. Check the UPP gateway logs
2. Review endpoint response formats
3. Verify market data is available from providers
4. Check network connectivity to gateway
