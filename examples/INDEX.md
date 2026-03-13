# UPP Gateway Examples - Complete Index

## Directory Contents

```
examples/
├── README.md                    # Comprehensive usage guide
├── OVERVIEW.md                  # Quick reference and architecture
├── INDEX.md                     # This file
│
├── arbitrage_scanner.py         # Cross-provider arbitrage detector
├── portfolio_rebalancer.py      # Portfolio rebalancing suggestion engine
├── market_monitor.py            # Real-time market dashboard
└── mcp_agent_demo.py            # AI agent MCP tool demonstration
```

## File Sizes & Line Counts

| File | Lines | Size | Purpose |
|------|-------|------|---------|
| arbitrage_scanner.py | 329 | 13 KB | Arbitrage opportunity detection |
| portfolio_rebalancer.py | 360 | 12 KB | Portfolio allocation management |
| market_monitor.py | 327 | 11 KB | Live market dashboard |
| mcp_agent_demo.py | 343 | 12 KB | Agent workflow demonstration |
| README.md | 500+ | 15 KB | Full API reference & guide |
| OVERVIEW.md | 300+ | 8 KB | Quick start & architecture |
| **Total** | **1,300+** | **65+ KB** | **4 complete bot applications** |

## Quick Navigation

### To Get Started
1. Read: **OVERVIEW.md** (5 min)
2. Read: **README.md** Prerequisites section (2 min)
3. Run: `python3 market_monitor.py --top 5 --static` (1 min)

### To Understand Each Example

**Arbitrage Scanner**
- File: `arbitrage_scanner.py`
- Purpose: Find profitable spreads between providers
- Key class: `ArbitrageScanner`
- Key method: `scan_once()`, `monitor()`
- Read: README.md section "1. Arbitrage Scanner"
- Quick start: `python3 arbitrage_scanner.py --monitor --interval 30`

**Portfolio Rebalancer**
- File: `portfolio_rebalancer.py`
- Purpose: Suggest trades to match target allocation
- Key class: `PortfolioMonitor`
- Key method: `calculate_rebalancing_trades()`
- Read: README.md section "2. Portfolio Rebalancer"
- Quick start: `python3 portfolio_rebalancer.py --target target.json`

**Market Monitor**
- File: `market_monitor.py`
- Purpose: Display live market prices in terminal
- Key class: `MarketMonitor`
- Key method: `display_dashboard()`
- Read: README.md section "3. Market Monitor"
- Quick start: `python3 market_monitor.py --top 10`

**MCP Agent Demo**
- File: `mcp_agent_demo.py`
- Purpose: Show AI agent interaction with UPP
- Key class: `AgentDemo`
- Key method: `run_analysis()`
- Read: README.md section "4. MCP Agent Demo"
- Quick start: `python3 mcp_agent_demo.py --topic "bitcoin"`

### To Learn the API

See **README.md** sections:
- "API Reference" — All endpoint documentation
- "Market Object Structure" — JSON schema for markets
- "Common Patterns" — Error handling, retry logic, colors

### To Extend & Customize

See **OVERVIEW.md** sections:
- "Common Extensions" — Database, email, webhooks, metrics
- "Next Steps" — How to modify for your use case

See **README.md** sections:
- "Extending the Examples" — Adding providers, categories
- "Performance Tuning" — Optimization strategies

## Code Structure

All examples follow this pattern:

```python
#!/usr/bin/env python3
"""Docstring explaining purpose"""

# Imports (stdlib only)
import argparse
import json
import sys
# ...

# Color definitions
class Colors:
    GREEN = "\033[92m"
    # ...

# Data classes
@dataclass
class SomeData:
    field1: str
    field2: float

# Client class (API wrapper)
class UPPClient:
    def _request(self, method, path, data=None):
        """Make HTTP request"""
    def get_markets(self, ...):
        """Get markets from gateway"""

# Main logic class
class MainLogic:
    def __init__(self, base_url=...):
        self.client = UPPClient(base_url)
    
    def analyze(self, params):
        """Main analysis method"""
    
    def display_results(self, results):
        """Pretty-print results"""

# CLI Entry point
def main():
    parser = argparse.ArgumentParser(...)
    # ... argument parsing
    logic = MainLogic(args.url)
    logic.analyze(...)

if __name__ == "__main__":
    main()
```

## Gateway Connection Reference

**Default URL:** `http://localhost:8080`

**Override with:**
```bash
python3 script.py --url http://custom-gateway:8080
```

**Test connectivity:**
```bash
curl http://localhost:8080/upp/v1/markets?limit=1
```

## Examples by Use Case

### I want to find profitable trades
→ Use **arbitrage_scanner.py**
```bash
python3 arbitrage_scanner.py --monitor --min-spread 0.02
```

### I want to track my portfolio allocation
→ Use **portfolio_rebalancer.py**
```bash
python3 portfolio_rebalancer.py --target target.json --execute
```

### I want to watch live market prices
→ Use **market_monitor.py**
```bash
python3 market_monitor.py --top 15
```

### I want to understand MCP integration
→ Use **mcp_agent_demo.py**
```bash
python3 mcp_agent_demo.py --interactive
```

### I want to build something new
→ Copy any example and:
1. Modify the `UPPClient` class for custom API calls
2. Modify the main logic class for custom analysis
3. Update `main()` for your CLI interface
4. Keep the error handling and color output patterns

## Dependencies

**Required:**
- Python 3.9+
- UPP gateway running on accessible URL

**Included (stdlib):**
- `argparse` — CLI argument parsing
- `json` — JSON encoding/decoding
- `urllib` — HTTP requests
- `time` — Delays and timestamps
- `sys` — System utilities
- `dataclasses` — Data containers
- `collections` — Data structures
- `typing` — Type hints

**Optional (with fallback):**
- `websockets` — WebSocket connections (can fallback to polling)

## Testing

All scripts validate:
```bash
# Syntax check
python3 -m py_compile *.py

# Test import
python3 -c "import arbitrage_scanner"

# Test CLI help
python3 arbitrage_scanner.py --help

# Test gateway connection
curl http://localhost:8080/upp/v1/markets?limit=1
```

## Error Messages & Solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `Connection refused` | Gateway not running | Start UPP gateway on port 8080 |
| `No markets found` | Provider credentials missing | Check gateway config |
| `Invalid market_id` | Malformed market ID | Use format `provider:native_id` |
| `Timeout` | Slow network | Increase timeout or reduce batch size |
| `JSON decode error` | Invalid response | Check gateway is running correct version |

## Performance Notes

**Optimal update intervals:**
- Arbitrage scanner: 30+ seconds
- Portfolio rebalancer: 60+ seconds
- Market monitor: 1 Hz (1 second)
- MCP agent: Sequential (500ms+ between steps)

**Throughput:**
- ~100 markets per scan (arbitrage)
- 1000+ markets per portfolio analysis
- 10+ markets live monitoring
- 5 MCP tools in sequence

## Contributing

To add a new example:

1. Create `new_bot.py` following the pattern above
2. Add docstring explaining purpose
3. Implement `UPPClient` subclass if needed
4. Implement main logic class with `analyze()` and `display()` methods
5. Add CLI via `argparse`
6. Test with `python3 -m py_compile new_bot.py`
7. Add entry in README.md "Examples" section

## License & Support

These examples are provided for demonstration.

For support:
1. Check README.md troubleshooting section
2. Verify gateway connectivity: `curl http://localhost:8080/upp/v1/health`
3. Check gateway logs for errors
4. Review market data availability from providers

## Version Info

- Created: March 2026
- Python: 3.9+
- UPP Gateway: Latest
- Status: Production-ready

---

**Start here:** Read OVERVIEW.md, then pick an example that matches your use case!
