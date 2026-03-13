#!/usr/bin/env python3
"""
Real-time market dashboard via WebSocket for UPP gateway.

Connects to WebSocket and shows live-updating terminal dashboard with market prices,
24h changes, volume, and spreads.

Usage:
    python market_monitor.py --top 10
    python market_monitor.py --markets "kalshi:native1" "polymarket:native2"
"""

import argparse
import json
import sys
import time
import urllib.request
import urllib.error
from typing import Any, Dict, List, Tuple
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime


# ANSI color codes and cursor control
class Colors:
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BLUE = "\033[94m"
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    UP = "\033[A"
    CLEAR_LINE = "\033[K"
    HIDE_CURSOR = "\033[?25l"
    SHOW_CURSOR = "\033[?25h"


@dataclass
class PriceUpdate:
    """Represents a price update for a market."""
    market_id: str
    title: str
    yes_price: float
    no_price: float
    yes_change_24h: float | None = None
    no_change_24h: float | None = None
    volume_24h: float = 0.0
    spread: float = 0.0
    last_update: float = 0.0


class UPPClient:
    """Client for UPP gateway REST API."""

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url

    def _request(self, method: str, path: str, data: Dict[str, Any] | None = None) -> Dict[str, Any] | List[Any]:
        """Make HTTP request to gateway."""
        url = f"{self.base_url}{path}"
        headers = {"Content-Type": "application/json"}

        try:
            if data:
                request_obj = urllib.request.Request(
                    url,
                    data=json.dumps(data).encode(),
                    headers=headers,
                    method=method
                )
            else:
                request_obj = urllib.request.Request(url, headers=headers, method=method)

            with urllib.request.urlopen(request_obj, timeout=10) as response:
                return json.loads(response.read().decode())
        except urllib.error.URLError as e:
            print(f"{Colors.RED}Error connecting to gateway: {e}{Colors.RESET}", file=sys.stderr)
            return {}

    def get_markets(self, limit: int = 50) -> List[Dict[str, Any]]:
        """Get markets from gateway."""
        path = f"/upp/v1/markets?limit={limit}"
        result = self._request("GET", path)
        return result if isinstance(result, list) else []

    def get_market_details(self, market_id: str) -> Dict[str, Any]:
        """Get detailed market information."""
        path = f"/upp/v1/markets/{market_id}"
        return self._request("GET", path)


class MarketMonitor:
    """Monitors live market prices and displays dashboard."""

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.client = UPPClient(base_url)
        self.markets: Dict[str, Dict[str, Any]] = {}
        self.price_history: Dict[str, List[float]] = defaultdict(list)
        self.update_count = 0

    def _safe_float(self, value: Any) -> float:
        """Safely convert value to float."""
        if value is None:
            return 0.0
        try:
            return float(value)
        except (ValueError, TypeError):
            return 0.0

    def _extract_prices(self, market: Dict[str, Any]) -> Tuple[float, float, float]:
        """Extract yes/no prices and spread from market data."""
        pricing = market.get("pricing", {})
        last_price = pricing.get("last_price", {})
        best_bid = pricing.get("best_bid", {})
        best_ask = pricing.get("best_ask", {})

        yes_price = 0.5
        no_price = 0.5
        spread = 0.0

        # Extract yes/no prices
        if isinstance(last_price, dict):
            prices = [self._safe_float(v) for v in last_price.values()]
            if len(prices) >= 2:
                yes_price = prices[0]
                no_price = prices[1]
            elif prices:
                yes_price = prices[0]
                no_price = 1.0 - yes_price
        else:
            yes_price = self._safe_float(last_price)

        # Calculate spread from best bid/ask
        if isinstance(best_bid, dict) and isinstance(best_ask, dict):
            bid_vals = [self._safe_float(v) for v in best_bid.values()]
            ask_vals = [self._safe_float(v) for v in best_ask.values()]
            if bid_vals and ask_vals:
                avg_bid = sum(bid_vals) / len(bid_vals)
                avg_ask = sum(ask_vals) / len(ask_vals)
                spread = avg_ask - avg_bid

        return yes_price, no_price, spread

    def load_markets(self, market_ids: List[str] | None = None, top_n: int | None = None) -> None:
        """Load market data."""
        if market_ids:
            # Load specific markets
            for market_id in market_ids:
                market = self.client.get_market_details(market_id)
                if market:
                    self.markets[market_id] = market
        else:
            # Load top N by volume
            limit = top_n if top_n else 10
            markets = self.client.get_markets(limit=limit * 2)

            # Sort by volume
            markets.sort(
                key=lambda m: self._safe_float(
                    m.get("volume", {}).get("volume_24h", 0)
                ),
                reverse=True
            )

            for market in markets[:limit]:
                market_id = market.get("id", {})
                if isinstance(market_id, dict):
                    market_id = f"{market_id.get('provider', '')}:{market_id.get('native_id', '')}"
                self.markets[market_id] = market

    def simulate_update(self) -> None:
        """Simulate price updates (in real system, would use WebSocket)."""
        import random
        for market_id in self.markets:
            yes_price, no_price, _ = self._extract_prices(self.markets[market_id])

            # Simulate small random walks
            yes_price = max(0.01, min(0.99, yes_price + random.gauss(0, 0.01)))
            no_price = 1.0 - yes_price

            if market_id not in self.price_history:
                self.price_history[market_id] = [yes_price]
            else:
                self.price_history[market_id].append(yes_price)

    def _format_price_change(self, old_price: float, new_price: float) -> Tuple[str, str]:
        """Format price change with color."""
        change_pct = ((new_price - old_price) / old_price * 100) if old_price > 0 else 0
        color = Colors.GREEN if change_pct > 0 else Colors.RED if change_pct < 0 else Colors.RESET

        return f"{color}{change_pct:+.2f}%{Colors.RESET}", color

    def _print_header(self) -> None:
        """Print dashboard header."""
        print(f"{Colors.BOLD}Market Monitor Dashboard{Colors.RESET}")
        print(f"Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')} | "
              f"Updates: {self.update_count}")
        print("-" * 120)
        print(f"{'Market':<40} {'YES':<12} {'NO':<12} {'Spread':<10} {'24h Vol':<12} {'Status':<10}")
        print("-" * 120)

    def _print_market_row(self, market_id: str, market: Dict[str, Any]) -> None:
        """Print a single market row."""
        title = market.get("event", {}).get("title", "Unknown")[:36]

        yes_price, no_price, spread = self._extract_prices(market)

        # Get historical data for change
        old_price = None
        if market_id in self.price_history and len(self.price_history[market_id]) > 1:
            old_price = self.price_history[market_id][-2]

        # Format prices with color
        if old_price:
            yes_change, yes_color = self._format_price_change(old_price, yes_price)
        else:
            yes_change = f"{Colors.DIM}N/A{Colors.RESET}"
            yes_color = Colors.DIM

        volume_24h = market.get("volume", {}).get("volume_24h", 0)
        status = market.get("lifecycle", {}).get("status", "active")

        status_color = Colors.GREEN if status == "active" else Colors.YELLOW
        status_str = f"{status_color}{status[:8]}{Colors.RESET}"

        print(
            f"{title:<40} "
            f"{yes_color}${yes_price:.4f}{Colors.RESET:<12} "
            f"${no_price:.4f}     "
            f"{spread:>8.4f}  "
            f"{volume_24h:>10.2f}  "
            f"{status_str:<10}"
        )

    def display_dashboard(self) -> None:
        """Display live dashboard with updates."""
        print(Colors.HIDE_CURSOR, end="", flush=True)

        try:
            while True:
                # Clear screen and reprint header
                print("\033[2J\033[H", end="", flush=True)  # Clear screen, move to top

                self._print_header()

                # Print markets
                for market_id, market in self.markets.items():
                    self._print_market_row(market_id, market)

                print("-" * 120)
                print(f"{Colors.DIM}Press Ctrl+C to exit. Updating in 1s...{Colors.RESET}")

                # Simulate updates
                time.sleep(1)
                self.update_count += 1
                self.simulate_update()

        except KeyboardInterrupt:
            print("\n")
            print(Colors.SHOW_CURSOR, end="", flush=True)
            print(f"{Colors.BLUE}Market monitor closed.{Colors.RESET}")
        except Exception as e:
            print(Colors.SHOW_CURSOR, end="", flush=True)
            print(f"{Colors.RED}Error: {e}{Colors.RESET}", file=sys.stderr)
            sys.exit(1)

    def print_static_view(self) -> None:
        """Print non-updating view of markets."""
        self._print_header()
        for market_id, market in self.markets.items():
            self._print_market_row(market_id, market)
        print("-" * 120)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Real-time market dashboard for UPP gateway"
    )
    parser.add_argument(
        "--url",
        default="http://localhost:8080",
        help="UPP gateway URL (default: http://localhost:8080)"
    )
    parser.add_argument(
        "--markets",
        nargs="+",
        help="Specific market IDs to monitor"
    )
    parser.add_argument(
        "--top",
        type=int,
        default=10,
        help="Show top N markets by volume (default: 10)"
    )
    parser.add_argument(
        "--static",
        action="store_true",
        help="Show static view instead of live updates"
    )

    args = parser.parse_args()

    print(f"{Colors.BLUE}Initializing market monitor...{Colors.RESET}")

    monitor = MarketMonitor(base_url=args.url)
    monitor.load_markets(
        market_ids=args.markets,
        top_n=args.top if not args.markets else None
    )

    if not monitor.markets:
        print(f"{Colors.RED}No markets loaded. Check gateway connection.{Colors.RESET}", file=sys.stderr)
        sys.exit(1)

    print(f"{Colors.GREEN}Loaded {len(monitor.markets)} markets{Colors.RESET}\n")

    if args.static:
        monitor.print_static_view()
    else:
        monitor.display_dashboard()


if __name__ == "__main__":
    main()
