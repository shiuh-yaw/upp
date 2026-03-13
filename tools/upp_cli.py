#!/usr/bin/env python3
"""
UPP CLI Demo — Query live prediction markets through the Universal Prediction Protocol.

Demonstrates the UPP normalization layer by fetching data from Kalshi and Polymarket
public APIs and transforming them into the unified UPP format.

Usage:
    pip install requests
    python tools/upp_cli.py markets              # List markets from all providers
    python tools/upp_cli.py markets --provider kalshi
    python tools/upp_cli.py markets --provider polymarket
    python tools/upp_cli.py search "bitcoin"      # Search markets
    python tools/upp_cli.py market TICKER         # Get specific market
    python tools/upp_cli.py orderbook TICKER      # Get orderbook
    python tools/upp_cli.py health                # Provider health check
    python tools/upp_cli.py providers             # List providers
"""

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from typing import Optional

try:
    import requests
except ImportError:
    print("Please install requests: pip install requests")
    sys.exit(1)

# ─── Configuration ───────────────────────────────────────────

KALSHI_BASE = "https://api.elections.kalshi.com/trade-api/v2"
POLYMARKET_GAMMA_BASE = "https://gamma-api.polymarket.com"
POLYMARKET_CLOB_BASE = "https://clob.polymarket.com"
UPP_VERSION = "2026-03-11"

# ─── UPP Normalization Functions ─────────────────────────────

def kalshi_cents_to_upp(cents) -> str:
    """Convert Kalshi cents (0-100) to UPP probability (0.00-1.00)"""
    if cents is None:
        return "0.00"
    return f"{int(cents) / 100:.2f}"


def format_volume(vol) -> str:
    """Format volume as human-readable string"""
    if vol is None:
        return "0"
    v = float(vol)
    if v >= 1_000_000:
        return f"${v / 1_000_000:.1f}M"
    elif v >= 1_000:
        return f"${v / 1_000:.1f}K"
    return f"${v:.0f}"


def normalize_kalshi_market(km) -> dict:
    """Transform a Kalshi native market into UPP format."""
    ticker = km.get("ticker", "")
    return {
        "id": {
            "provider": "kalshi.com",
            "native_id": ticker,
            "full_id": f"upp:kalshi.com:{ticker}",
        },
        "event": {
            "id": km.get("event_ticker", ticker),
            "title": km.get("title", km.get("subtitle", "Unknown")),
            "description": km.get("subtitle", ""),
            "category": km.get("category", "uncategorized"),
        },
        "market_type": "binary",
        "outcomes": [
            {"id": "yes", "label": "Yes"},
            {"id": "no", "label": "No"},
        ],
        "pricing": {
            "last_price": {
                "yes": kalshi_cents_to_upp(km.get("last_price")),
                "no": kalshi_cents_to_upp(100 - (km.get("last_price") or 0)),
            },
            "best_bid": {
                "yes": kalshi_cents_to_upp(km.get("yes_bid")),
            },
            "best_ask": {
                "yes": kalshi_cents_to_upp(km.get("yes_ask")),
            },
            "currency": "USD",
        },
        "volume": {
            "total_volume": str(km.get("volume", 0)),
            "volume_24h": str(km.get("volume_24h", 0)),
            "open_interest": str(km.get("open_interest", 0)),
        },
        "lifecycle": {
            "status": km.get("status", "unknown"),
            "closes_at": km.get("close_time", km.get("expiration_time")),
        },
    }


def normalize_polymarket_market(pm) -> dict:
    """Transform a Polymarket Gamma market into UPP format."""
    condition_id = pm.get("condition_id", pm.get("conditionId", ""))

    # Parse JSON-encoded string arrays
    def parse_json_str(s):
        if isinstance(s, str):
            try:
                return json.loads(s)
            except:
                return []
        return s if isinstance(s, list) else []

    outcomes_raw = parse_json_str(pm.get("outcomes", '["Yes","No"]'))
    prices_raw = parse_json_str(
        pm.get("outcomePrices", pm.get("outcome_prices", "[]"))
    )
    tokens_raw = parse_json_str(
        pm.get("clobTokenIds", pm.get("clob_token_ids", "[]"))
    )

    outcomes = []
    last_price = {}
    for i, label in enumerate(outcomes_raw):
        oid = label.lower()
        outcomes.append({
            "id": oid,
            "label": label,
            "token_id": tokens_raw[i] if i < len(tokens_raw) else None,
        })
        if i < len(prices_raw):
            last_price[oid] = prices_raw[i]

    return {
        "id": {
            "provider": "polymarket.com",
            "native_id": condition_id,
            "full_id": f"upp:polymarket.com:{condition_id}",
        },
        "event": {
            "id": pm.get("question_id", pm.get("questionId", condition_id)),
            "title": pm.get("question", "Unknown"),
            "description": pm.get("description", "")[:200],
            "category": "uncategorized",
        },
        "market_type": "binary" if len(outcomes) == 2 else "categorical",
        "outcomes": outcomes,
        "pricing": {
            "last_price": last_price,
            "best_bid": {"yes": str(pm.get("bestBid", pm.get("best_bid", "")))} if pm.get("bestBid") or pm.get("best_bid") else {},
            "best_ask": {"yes": str(pm.get("bestAsk", pm.get("best_ask", "")))} if pm.get("bestAsk") or pm.get("best_ask") else {},
            "currency": "USDC",
        },
        "volume": {
            "total_volume": str(pm.get("volume", 0)),
            "volume_24h": str(pm.get("volume24hr", pm.get("volume_24hr", 0))),
            "open_interest": str(pm.get("open_interest", 0)),
        },
        "lifecycle": {
            "status": "closed" if pm.get("closed") else ("open" if pm.get("active") else "pending"),
            "closes_at": pm.get("endDateIso", pm.get("end_date_iso")),
        },
    }


# ─── API Fetchers ────────────────────────────────────────────

def fetch_kalshi_markets(limit=10, status="open") -> list:
    """Fetch markets from Kalshi public API."""
    try:
        url = f"{KALSHI_BASE}/markets?limit={limit}&status={status}"
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()
        data = resp.json()
        return [normalize_kalshi_market(m) for m in data.get("markets", [])]
    except Exception as e:
        print(f"  ⚠ Kalshi API error: {e}")
        return []


def fetch_polymarket_markets(limit=10) -> list:
    """Fetch markets from Polymarket Gamma API."""
    try:
        url = f"{POLYMARKET_GAMMA_BASE}/markets?limit={limit}&active=true&closed=false&order=volume24hr&ascending=false"
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()
        markets = resp.json()
        if isinstance(markets, list):
            return [normalize_polymarket_market(m) for m in markets if m.get("condition_id") or m.get("conditionId")]
        return []
    except Exception as e:
        print(f"  ⚠ Polymarket API error: {e}")
        return []


def fetch_kalshi_market(ticker: str) -> Optional[dict]:
    """Fetch a single market from Kalshi."""
    try:
        url = f"{KALSHI_BASE}/markets/{ticker}"
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()
        data = resp.json()
        return normalize_kalshi_market(data.get("market", data))
    except Exception as e:
        print(f"  ⚠ Kalshi API error: {e}")
        return None


def search_kalshi_markets(query: str, limit=10) -> list:
    """Search Kalshi markets."""
    try:
        # Kalshi doesn't have a text search endpoint; filter client-side
        all_markets = fetch_kalshi_markets(limit=100)
        query_lower = query.lower()
        return [m for m in all_markets if query_lower in m["event"]["title"].lower()][:limit]
    except Exception as e:
        print(f"  ⚠ Kalshi search error: {e}")
        return []


def search_polymarket_markets(query: str, limit=10) -> list:
    """Search Polymarket markets."""
    try:
        from urllib.parse import quote
        url = f"{POLYMARKET_GAMMA_BASE}/markets?_q={quote(query)}&limit={limit}&active=true&closed=false"
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()
        markets = resp.json()
        if isinstance(markets, list):
            return [normalize_polymarket_market(m) for m in markets if m.get("condition_id") or m.get("conditionId")]
        return []
    except Exception as e:
        print(f"  ⚠ Polymarket search error: {e}")
        return []


def fetch_polymarket_orderbook(token_id: str) -> Optional[dict]:
    """Fetch orderbook from Polymarket CLOB."""
    try:
        url = f"{POLYMARKET_CLOB_BASE}/book?token_id={token_id}"
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()
        return resp.json()
    except Exception as e:
        print(f"  ⚠ Polymarket CLOB error: {e}")
        return None


def check_provider_health(name: str, url: str) -> dict:
    """Check if a provider API is reachable."""
    start = time.time()
    try:
        resp = requests.get(url, timeout=5)
        latency = int((time.time() - start) * 1000)
        return {
            "provider": name,
            "healthy": resp.status_code < 500,
            "status_code": resp.status_code,
            "latency_ms": latency,
        }
    except Exception as e:
        return {
            "provider": name,
            "healthy": False,
            "error": str(e),
            "latency_ms": 0,
        }


# ─── Display Functions ───────────────────────────────────────

def print_market_table(markets: list):
    """Print markets in a formatted table."""
    if not markets:
        print("  No markets found.")
        return

    # Header
    print(f"\n  {'#':<3} {'Provider':<15} {'Title':<50} {'YES':>6} {'NO':>6} {'Vol 24h':>10} {'Status':<8}")
    print(f"  {'─'*3} {'─'*15} {'─'*50} {'─'*6} {'─'*6} {'─'*10} {'─'*8}")

    for i, m in enumerate(markets, 1):
        provider = m["id"]["provider"].replace(".com", "").replace(".trade", "")
        title = m["event"]["title"][:48]
        yes = m["pricing"]["last_price"].get("yes", "—")
        no = m["pricing"]["last_price"].get("no", "—")
        vol = format_volume(m["volume"].get("volume_24h", "0"))
        status = m["lifecycle"]["status"]

        # Color the price based on confidence
        print(f"  {i:<3} {provider:<15} {title:<50} {yes:>6} {no:>6} {vol:>10} {status:<8}")

    print(f"\n  Total: {len(markets)} markets")


def print_market_detail(m: dict):
    """Print detailed market info."""
    print(f"\n  ┌─ {m['event']['title']}")
    print(f"  │")
    print(f"  │  UPP ID:    {m['id']['full_id']}")
    print(f"  │  Provider:  {m['id']['provider']}")
    print(f"  │  Native ID: {m['id']['native_id']}")
    print(f"  │  Type:      {m['market_type']}")
    print(f"  │  Status:    {m['lifecycle']['status']}")
    if m['lifecycle'].get('closes_at'):
        print(f"  │  Closes:    {m['lifecycle']['closes_at']}")
    print(f"  │")
    print(f"  │  Pricing:")
    for outcome in m.get("outcomes", []):
        oid = outcome["id"]
        price = m["pricing"]["last_price"].get(oid, "—")
        pct = f"{float(price)*100:.0f}%" if price != "—" else "—"
        print(f"  │    {outcome['label']:<8} {price:>6}  ({pct})")
    print(f"  │")
    print(f"  │  Volume:")
    print(f"  │    24h:   {format_volume(m['volume'].get('volume_24h', '0'))}")
    print(f"  │    Total: {format_volume(m['volume'].get('total_volume', '0'))}")
    print(f"  │    OI:    {format_volume(m['volume'].get('open_interest', '0'))}")
    print(f"  └─")


def print_orderbook(book: dict, outcome_label="YES"):
    """Print orderbook in a visual format."""
    bids = book.get("bids", [])[:10]
    asks = book.get("asks", [])[:10]

    print(f"\n  ┌─ Orderbook: {outcome_label}")
    print(f"  │")
    print(f"  │  {'BIDS':<25} │ {'ASKS':<25}")
    print(f"  │  {'Price':>8} {'Size':>8} {'──────':>8} │ {'Price':>8} {'Size':>8}")
    print(f"  │  {'─'*25} │ {'─'*25}")

    max_rows = max(len(bids), len(asks))
    for i in range(min(max_rows, 10)):
        bid_str = ""
        ask_str = ""
        if i < len(bids):
            bid_str = f"  {bids[i].get('price', ''):>8} {bids[i].get('size', ''):>8}"
        else:
            bid_str = f"  {'':>8} {'':>8}"
        if i < len(asks):
            ask_str = f"  {asks[i].get('price', ''):>8} {asks[i].get('size', ''):>8}"
        else:
            ask_str = f"  {'':>8} {'':>8}"
        print(f"  │{bid_str:25} │{ask_str:25}")

    print(f"  └─")


# ─── Commands ────────────────────────────────────────────────

def cmd_markets(args):
    """List markets from providers."""
    print(f"\n  UPP Market Discovery (v{UPP_VERSION})")
    print(f"  {'═'*70}")

    all_markets = []

    if not args.provider or args.provider == "kalshi":
        print(f"\n  Fetching from Kalshi...")
        all_markets.extend(fetch_kalshi_markets(limit=args.limit))

    if not args.provider or args.provider == "polymarket":
        print(f"  Fetching from Polymarket...")
        all_markets.extend(fetch_polymarket_markets(limit=args.limit))

    print_market_table(all_markets)


def cmd_search(args):
    """Search markets across providers."""
    print(f"\n  UPP Search: \"{args.query}\"")
    print(f"  {'═'*70}")

    results = []

    if not args.provider or args.provider == "kalshi":
        print(f"  Searching Kalshi...")
        results.extend(search_kalshi_markets(args.query, limit=args.limit))

    if not args.provider or args.provider == "polymarket":
        print(f"  Searching Polymarket...")
        results.extend(search_polymarket_markets(args.query, limit=args.limit))

    print_market_table(results)


def cmd_market(args):
    """Get detailed market info."""
    ticker = args.ticker
    print(f"\n  UPP Market Detail: {ticker}")
    print(f"  {'═'*70}")

    # Try Kalshi first (tickers are usually uppercase)
    m = fetch_kalshi_market(ticker)
    if m:
        print_market_detail(m)
        if args.json:
            print(f"\n  Raw UPP JSON:")
            print(json.dumps(m, indent=2))
        return

    print(f"  Market '{ticker}' not found on any provider.")


def cmd_orderbook(args):
    """Get orderbook for a market."""
    # For Polymarket, token_id is needed
    token_id = args.token_id
    print(f"\n  UPP Orderbook: token {token_id[:16]}...")
    print(f"  {'═'*70}")

    book = fetch_polymarket_orderbook(token_id)
    if book:
        print_orderbook(book)
        if args.json:
            print(f"\n  Raw CLOB response:")
            print(json.dumps(book, indent=2))
    else:
        print(f"  Could not fetch orderbook for token {token_id}")


def cmd_health(args):
    """Check provider health."""
    print(f"\n  UPP Provider Health Check")
    print(f"  {'═'*70}")

    checks = [
        ("Kalshi", f"{KALSHI_BASE}/markets?limit=1"),
        ("Polymarket Gamma", f"{POLYMARKET_GAMMA_BASE}/markets?limit=1"),
        ("Polymarket CLOB", f"{POLYMARKET_CLOB_BASE}/time"),
    ]

    for name, url in checks:
        result = check_provider_health(name, url)
        status = "✓" if result["healthy"] else "✗"
        latency = f"{result['latency_ms']}ms"
        code = result.get("status_code", result.get("error", "?"))
        print(f"  {status} {name:<25} {code:<6} {latency:>6}")


def cmd_providers(args):
    """List known providers."""
    print(f"\n  UPP Provider Registry (v{UPP_VERSION})")
    print(f"  {'═'*70}")
    print()

    providers = [
        {
            "id": "kalshi.com",
            "name": "Kalshi",
            "type": "CFTC-regulated exchange",
            "auth_public": "None (market data)",
            "auth_trading": "RSA-PSS signed API key",
            "currency": "USD",
            "capabilities": ["markets", "trading", "portfolio", "resolution"],
        },
        {
            "id": "polymarket.com",
            "name": "Polymarket",
            "type": "Decentralized (Polygon)",
            "auth_public": "None (market data)",
            "auth_trading": "Ethereum wallet (EIP-712)",
            "currency": "USDC",
            "capabilities": ["markets", "trading", "portfolio"],
        },
        {
            "id": "opinion.trade",
            "name": "Opinion",
            "type": "Decentralized (BNB Chain)",
            "auth_public": "API key required",
            "auth_trading": "CLOB SDK (EIP-712)",
            "currency": "USDC",
            "capabilities": ["markets", "trading", "resolution"],
        },
    ]

    for p in providers:
        print(f"  ┌─ {p['name']} ({p['id']})")
        print(f"  │  Type:          {p['type']}")
        print(f"  │  Public auth:   {p['auth_public']}")
        print(f"  │  Trading auth:  {p['auth_trading']}")
        print(f"  │  Currency:      {p['currency']}")
        print(f"  │  Capabilities:  {', '.join(p['capabilities'])}")
        print(f"  └─\n")


def cmd_dump(args):
    """Dump raw UPP JSON for a provider's markets."""
    print(f"  Fetching raw UPP data from {args.provider or 'all providers'}...")

    all_markets = []
    if not args.provider or args.provider == "kalshi":
        all_markets.extend(fetch_kalshi_markets(limit=args.limit))
    if not args.provider or args.provider == "polymarket":
        all_markets.extend(fetch_polymarket_markets(limit=args.limit))

    output = {
        "upp_version": UPP_VERSION,
        "fetched_at": datetime.now(timezone.utc).isoformat(),
        "markets": all_markets,
        "total": len(all_markets),
    }
    print(json.dumps(output, indent=2))


# ─── Main ────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="UPP CLI — Universal Prediction Protocol demo client",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python upp_cli.py markets                    List markets from all providers
  python upp_cli.py markets --provider kalshi  List only Kalshi markets
  python upp_cli.py search "bitcoin"           Search all providers
  python upp_cli.py market PRES-2028-DEM       Get Kalshi market detail
  python upp_cli.py health                     Check provider health
  python upp_cli.py dump --json > markets.json Export UPP JSON
""",
    )

    subparsers = parser.add_subparsers(dest="command", help="Command to run")

    # markets
    p_markets = subparsers.add_parser("markets", help="List markets")
    p_markets.add_argument("--provider", "-p", choices=["kalshi", "polymarket", "opinion"])
    p_markets.add_argument("--limit", "-n", type=int, default=10)
    p_markets.set_defaults(func=cmd_markets)

    # search
    p_search = subparsers.add_parser("search", help="Search markets")
    p_search.add_argument("query", help="Search query")
    p_search.add_argument("--provider", "-p", choices=["kalshi", "polymarket", "opinion"])
    p_search.add_argument("--limit", "-n", type=int, default=10)
    p_search.set_defaults(func=cmd_search)

    # market detail
    p_market = subparsers.add_parser("market", help="Get market detail")
    p_market.add_argument("ticker", help="Market ticker or condition ID")
    p_market.add_argument("--json", action="store_true", help="Show raw JSON")
    p_market.set_defaults(func=cmd_market)

    # orderbook
    p_book = subparsers.add_parser("orderbook", help="Get orderbook (Polymarket)")
    p_book.add_argument("token_id", help="CLOB token ID")
    p_book.add_argument("--json", action="store_true", help="Show raw JSON")
    p_book.set_defaults(func=cmd_orderbook)

    # health
    p_health = subparsers.add_parser("health", help="Check provider health")
    p_health.set_defaults(func=cmd_health)

    # providers
    p_providers = subparsers.add_parser("providers", help="List providers")
    p_providers.set_defaults(func=cmd_providers)

    # dump
    p_dump = subparsers.add_parser("dump", help="Dump raw UPP JSON")
    p_dump.add_argument("--provider", "-p", choices=["kalshi", "polymarket", "opinion"])
    p_dump.add_argument("--limit", "-n", type=int, default=20)
    p_dump.add_argument("--json", action="store_true", default=True)
    p_dump.set_defaults(func=cmd_dump)

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    print(f"\n  ╔═══════════════════════════════════════════════╗")
    print(f"  ║   UPP — Universal Prediction Protocol CLI     ║")
    print(f"  ║   Version: {UPP_VERSION}                        ║")
    print(f"  ╚═══════════════════════════════════════════════╝")

    args.func(args)
    print()


if __name__ == "__main__":
    main()
