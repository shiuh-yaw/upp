#!/usr/bin/env python3
"""
Cross-provider arbitrage scanner for UPP gateway.

Scans for the same event across multiple providers (Kalshi, Polymarket) and
identifies arbitrage opportunities where the best_bid on one provider exceeds
the best_ask on another.

Usage:
    python arbitrage_scanner.py --category politics --min-spread 2.0
    python arbitrage_scanner.py --monitor --interval 30
"""

import argparse
import json
import sys
import time
import urllib.request
import urllib.error
from typing import Any, Dict, List, Tuple
from dataclasses import dataclass
from collections import defaultdict


# ANSI color codes
class Colors:
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BLUE = "\033[94m"
    RESET = "\033[0m"
    BOLD = "\033[1m"


@dataclass
class ArbitrageOpportunity:
    """Represents an arbitrage opportunity between two providers."""
    event_title: str
    event_id: str
    provider_a: str
    provider_b: str
    market_id_a: str
    market_id_b: str
    outcome: str
    bid_price: float
    bid_provider: str
    ask_price: float
    ask_provider: str
    spread: float
    profit_after_fees: float
    fee_rate: float = 0.002  # 0.2% per side


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

    def get_markets(self, provider: str | None = None, limit: int = 100) -> List[Dict[str, Any]]:
        """Get markets from gateway."""
        path = "/upp/v1/markets"
        params = []
        if provider:
            params.append(f"provider={provider}")
        if limit:
            params.append(f"limit={limit}")
        if params:
            path += "?" + "&".join(params)

        result = self._request("GET", path)
        return result if isinstance(result, list) else []

    def get_merged_orderbook(self, market_id: str, depth: int = 5) -> Dict[str, Any]:
        """Get merged orderbook across providers."""
        path = f"/upp/v1/markets/{market_id}/orderbook/merged?depth={depth}"
        return self._request("GET", path)

    def get_market_details(self, market_id: str) -> Dict[str, Any]:
        """Get detailed market information."""
        path = f"/upp/v1/markets/{market_id}"
        return self._request("GET", path)


class ArbitrageScanner:
    """Scans for arbitrage opportunities across providers."""

    def __init__(self, base_url: str = "http://localhost:8080", fee_rate: float = 0.002):
        self.client = UPPClient(base_url)
        self.fee_rate = fee_rate

    def _safe_float(self, value: Any) -> float:
        """Safely convert value to float."""
        if value is None:
            return 0.0
        try:
            return float(value)
        except (ValueError, TypeError):
            return 0.0

    def _group_markets_by_event(self, markets: List[Dict[str, Any]]) -> Dict[str, List[Dict[str, Any]]]:
        """Group markets by event title to find same event across providers."""
        grouped = defaultdict(list)
        for market in markets:
            event = market.get("event", {})
            title = event.get("title", "")
            if title:
                grouped[title].append(market)
        return grouped

    def _extract_price_from_pricing(self, pricing: Dict[str, Any] | Any, outcome_id: str) -> Tuple[float, float]:
        """Extract best_bid and best_ask for outcome."""
        if not isinstance(pricing, dict):
            return 0.0, 1.0

        best_bid = self._safe_float(pricing.get("best_bid", {}).get(outcome_id, 0.0)
                                   if isinstance(pricing.get("best_bid"), dict)
                                   else pricing.get("best_bid", 0.0))
        best_ask = self._safe_float(pricing.get("best_ask", {}).get(outcome_id, 1.0)
                                   if isinstance(pricing.get("best_ask"), dict)
                                   else pricing.get("best_ask", 1.0))

        return best_bid, best_ask

    def scan_once(self, category: str | None = None, min_spread: float = 0.02) -> List[ArbitrageOpportunity]:
        """Perform a single scan for arbitrage opportunities."""
        opportunities = []

        # Get markets from all providers
        kalshi_markets = self.client.get_markets(provider="kalshi", limit=100)
        polymarket_markets = self.client.get_markets(provider="polymarket", limit=100)

        # Combine and group by event
        all_markets = kalshi_markets + polymarket_markets
        grouped = self._group_markets_by_event(all_markets)

        # Find opportunities in markets that exist on multiple providers
        for event_title, markets in grouped.items():
            if len(markets) < 2:
                continue

            # Filter by category if specified
            if category:
                event_cat = markets[0].get("event", {}).get("category", "")
                if event_cat.lower() != category.lower():
                    continue

            # Compare outcomes across providers
            providers_by_outcome = defaultdict(dict)
            for market in markets:
                provider = market.get("id", {}).get("provider", "")
                outcomes = market.get("outcomes", [])
                pricing = market.get("pricing", {})

                for outcome in outcomes:
                    outcome_id = outcome.get("id", "")
                    outcome_label = outcome.get("label", "")
                    bid, ask = self._extract_price_from_pricing(pricing, outcome_id)

                    if outcome_label not in providers_by_outcome:
                        providers_by_outcome[outcome_label] = {}
                    providers_by_outcome[outcome_label][provider] = {
                        "market_id": market.get("id", {}),
                        "bid": bid,
                        "ask": ask
                    }

            # Find arbitrage spreads
            for outcome_label, providers in providers_by_outcome.items():
                provider_list = list(providers.keys())
                if len(provider_list) < 2:
                    continue

                for i, prov_a in enumerate(provider_list):
                    for prov_b in provider_list[i+1:]:
                        bid_a = providers[prov_a]["bid"]
                        ask_a = providers[prov_a]["ask"]
                        bid_b = providers[prov_b]["bid"]
                        ask_b = providers[prov_b]["ask"]

                        # Check for arbitrage: buy at ask_a, sell at bid_b
                        if ask_a < bid_b:
                            spread = (bid_b - ask_a) / ask_a if ask_a > 0 else 0
                            profit_after_fees = spread - (2 * self.fee_rate)  # 2 sides

                            if spread >= min_spread:
                                event_id = markets[0].get("id", {})
                                opp = ArbitrageOpportunity(
                                    event_title=event_title,
                                    event_id=json.dumps(event_id),
                                    provider_a=prov_a,
                                    provider_b=prov_b,
                                    market_id_a=json.dumps(providers[prov_a]["market_id"]),
                                    market_id_b=json.dumps(providers[prov_b]["market_id"]),
                                    outcome=outcome_label,
                                    bid_price=bid_b,
                                    bid_provider=prov_b,
                                    ask_price=ask_a,
                                    ask_provider=prov_a,
                                    spread=spread,
                                    profit_after_fees=profit_after_fees
                                )
                                opportunities.append(opp)

        return opportunities

    def _print_opportunity(self, opp: ArbitrageOpportunity, index: int) -> None:
        """Pretty-print an opportunity."""
        color = Colors.GREEN if opp.profit_after_fees > 0 else Colors.YELLOW

        print(f"\n{color}Opportunity #{index}{Colors.RESET}")
        print(f"  Event: {Colors.BOLD}{opp.event_title}{Colors.RESET}")
        print(f"  Outcome: {opp.outcome}")
        print(f"  Buy on {opp.ask_provider}: ${opp.ask_price:.4f}")
        print(f"  Sell on {opp.bid_provider}: ${opp.bid_price:.4f}")
        print(f"  Spread: {color}{opp.spread*100:.2f}%{Colors.RESET}")
        print(f"  Profit after fees: {color}{opp.profit_after_fees*100:.2f}%{Colors.RESET}")

    def monitor(self, interval: int = 30, category: str | None = None, min_spread: float = 0.02) -> None:
        """Continuously scan for arbitrage opportunities."""
        print(f"{Colors.BLUE}Starting arbitrage scanner (interval: {interval}s){Colors.RESET}")
        print(f"Looking for spreads >= {min_spread*100:.1f}%\n")

        iteration = 0
        while True:
            try:
                iteration += 1
                timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
                print(f"{Colors.BOLD}[{timestamp}] Scan #{iteration}{Colors.RESET}")

                opportunities = self.scan_once(category=category, min_spread=min_spread)

                if opportunities:
                    print(f"Found {len(opportunities)} opportunity(ies):\n")
                    for idx, opp in enumerate(opportunities, 1):
                        self._print_opportunity(opp, idx)
                else:
                    print("No arbitrage opportunities found.")

                print(f"\nNext scan in {interval}s...")
                time.sleep(interval)
            except KeyboardInterrupt:
                print(f"\n{Colors.BLUE}Shutting down...{Colors.RESET}")
                break
            except Exception as e:
                print(f"{Colors.RED}Error during scan: {e}{Colors.RESET}", file=sys.stderr)
                time.sleep(interval)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Cross-provider arbitrage scanner for UPP gateway"
    )
    parser.add_argument(
        "--url",
        default="http://localhost:8080",
        help="UPP gateway URL (default: http://localhost:8080)"
    )
    parser.add_argument(
        "--monitor",
        action="store_true",
        help="Run continuous monitoring (default: single scan)"
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=30,
        help="Scan interval in seconds (default: 30)"
    )
    parser.add_argument(
        "--category",
        help="Filter by market category (e.g., politics, sports, crypto)"
    )
    parser.add_argument(
        "--min-spread",
        type=float,
        default=0.02,
        help="Minimum profitable spread as decimal (default: 0.02 = 2%%)"
    )

    args = parser.parse_args()

    scanner = ArbitrageScanner(base_url=args.url, fee_rate=0.002)

    if args.monitor:
        scanner.monitor(
            interval=args.interval,
            category=args.category,
            min_spread=args.min_spread
        )
    else:
        opportunities = scanner.scan_once(
            category=args.category,
            min_spread=args.min_spread
        )

        if opportunities:
            print(f"Found {len(opportunities)} opportunity(ies):\n")
            for idx, opp in enumerate(opportunities, 1):
                scanner._print_opportunity(opp, idx)
        else:
            print("No arbitrage opportunities found.")


if __name__ == "__main__":
    main()
