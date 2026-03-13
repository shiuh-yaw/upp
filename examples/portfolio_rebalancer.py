#!/usr/bin/env python3
"""
Portfolio monitoring and rebalancing bot for UPP gateway.

Monitors portfolio positions across all providers and suggests rebalancing trades
to match target allocations by category.

Usage:
    python portfolio_rebalancer.py --target target.json --dry-run
    python portfolio_rebalancer.py --target target.json --execute --max-trade-size 100
"""

import argparse
import json
import sys
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
    DIM = "\033[2m"


@dataclass
class Position:
    """Represents a position in a market."""
    provider: str
    market_id: str
    market_title: str
    category: str
    outcome: str
    quantity: float
    entry_price: float
    current_price: float
    value: float


@dataclass
class RebalanceTrade:
    """Represents a suggested rebalancing trade."""
    direction: str  # "buy" or "sell"
    category: str
    market_id: str
    market_title: str
    quantity: float
    estimated_price: float
    estimated_cost: float
    reason: str


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

    def get_markets(self, provider: str | None = None, limit: int = 200) -> List[Dict[str, Any]]:
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

    def estimate_order(self, market_id: str, side: str, quantity: float) -> Dict[str, Any]:
        """Estimate order cost."""
        path = "/upp/v1/orders/estimate"
        data = {
            "market_id": market_id,
            "side": side,
            "quantity": quantity
        }
        return self._request("POST", path, data)


class PortfolioMonitor:
    """Monitors and analyzes portfolio across providers."""

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.client = UPPClient(base_url)
        self.positions: List[Position] = []

    def _safe_float(self, value: Any) -> float:
        """Safely convert value to float."""
        if value is None:
            return 0.0
        try:
            return float(value)
        except (ValueError, TypeError):
            return 0.0

    def _get_current_price(self, market: Dict[str, Any]) -> float:
        """Extract current price from market data."""
        pricing = market.get("pricing", {})
        last_price = pricing.get("last_price", {})

        if isinstance(last_price, dict):
            # Take average of yes/no if available
            values = [self._safe_float(v) for v in last_price.values()]
            return sum(values) / len(values) if values else 0.5
        else:
            return self._safe_float(last_price)

    def scan_portfolio(self) -> Tuple[List[Position], Dict[str, float]]:
        """
        Scan all markets to build portfolio view.
        In a real system, this would query actual positions from users.
        """
        all_markets = self.client.get_markets(limit=200)

        positions_by_category = defaultdict(float)
        total_value = 0.0

        for market in all_markets:
            category = market.get("event", {}).get("category", "other")
            price = self._get_current_price(market)
            # Simulate position: random allocation
            quantity = 10.0
            value = quantity * price

            positions_by_category[category] += value
            total_value += value

        # Calculate allocation percentages
        allocation = {}
        if total_value > 0:
            for category, value in positions_by_category.items():
                allocation[category] = value / total_value
        else:
            allocation = {cat: 0.0 for cat in positions_by_category}

        return list(positions_by_category.items()), allocation

    def _print_ascii_pie_chart(self, allocation: Dict[str, float], width: int = 40) -> None:
        """Print ASCII pie chart of allocation."""
        if not allocation:
            print("No positions")
            return

        print("\nCurrent Allocation:")
        sorted_alloc = sorted(allocation.items(), key=lambda x: x[1], reverse=True)

        for category, pct in sorted_alloc:
            bar_width = int(width * pct)
            bar = "█" * bar_width
            print(f"  {category:15} {bar:40} {pct*100:5.1f}%")

    def calculate_rebalancing_trades(
        self,
        current_allocation: Dict[str, float],
        target_allocation: Dict[str, float],
        max_trade_size: float | None = None,
        portfolio_value: float = 10000.0
    ) -> List[RebalanceTrade]:
        """Calculate trades needed to rebalance to target allocation."""
        trades = []

        # Normalize allocations
        all_categories = set(current_allocation.keys()) | set(target_allocation.keys())
        current = {cat: current_allocation.get(cat, 0.0) for cat in all_categories}
        target = {cat: target_allocation.get(cat, 0.0) for cat in all_categories}

        # Calculate drift
        drifts = []
        for category in all_categories:
            drift = target[category] - current[category]
            if abs(drift) > 0.01:  # Only rebalance if drift > 1%
                drifts.append((category, drift))

        # Sort by absolute drift (largest first)
        drifts.sort(key=lambda x: abs(x[1]), reverse=True)

        # Create trades
        for category, drift in drifts:
            direction = "buy" if drift > 0 else "sell"
            quantity = abs(drift) * portfolio_value / 100.0  # Assume ~100 per share

            if max_trade_size and quantity > max_trade_size:
                quantity = max_trade_size

            trade = RebalanceTrade(
                direction=direction,
                category=category,
                market_id=f"category:{category}",
                market_title=f"Top {category} market",
                quantity=quantity,
                estimated_price=50.0,  # Simulated
                estimated_cost=quantity * 50.0,
                reason=f"Rebalance {category} from {current[category]*100:.1f}% to {target[category]*100:.1f}%"
            )
            trades.append(trade)

        return trades

    def _print_rebalancing_plan(self, trades: List[RebalanceTrade]) -> None:
        """Pretty-print rebalancing plan as table."""
        if not trades:
            print("Portfolio is already balanced.")
            return

        print(f"\n{Colors.BOLD}Rebalancing Plan ({len(trades)} trades):{Colors.RESET}\n")
        print(f"{'Direction':<8} {'Category':<15} {'Quantity':<10} {'Est. Cost':<12} {'Reason':<30}")
        print("-" * 80)

        total_cost = 0.0
        for trade in trades:
            color = Colors.GREEN if trade.direction == "buy" else Colors.RED
            direction_str = f"{color}{trade.direction.upper()}{Colors.RESET}"
            cost_str = f"${trade.estimated_cost:>10.2f}"

            print(
                f"{direction_str:<8} {trade.category:<15} {trade.quantity:>9.2f} "
                f"{cost_str:<12} {trade.reason:<30}"
            )
            total_cost += trade.estimated_cost

        print("-" * 80)
        print(f"Total estimated cost: ${total_cost:,.2f}")

    def execute_trades(self, trades: List[RebalanceTrade]) -> None:
        """Execute rebalancing trades (simulated)."""
        print(f"\n{Colors.BLUE}Executing {len(trades)} trades...{Colors.RESET}\n")

        for i, trade in enumerate(trades, 1):
            status = f"{Colors.GREEN}✓{Colors.RESET}"
            print(
                f"{status} [{i}/{len(trades)}] {trade.direction.upper()} {trade.quantity:.2f} "
                f"in {trade.category} @ ${trade.estimated_price:.2f}"
            )
            # In real system, would execute via API
            # self.client.execute_trade(trade)

        print(f"\n{Colors.BLUE}All trades executed.{Colors.RESET}")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Portfolio monitoring and rebalancing bot"
    )
    parser.add_argument(
        "--url",
        default="http://localhost:8080",
        help="UPP gateway URL (default: http://localhost:8080)"
    )
    parser.add_argument(
        "--target",
        required=True,
        help="JSON file with target allocations (e.g., {\"politics\": 0.3, \"sports\": 0.3})"
    )
    parser.add_argument(
        "--max-trade-size",
        type=float,
        help="Maximum single trade size in dollars"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        default=True,
        help="Show plan without executing (default)"
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Execute rebalancing trades"
    )

    args = parser.parse_args()

    # Load target allocation
    try:
        with open(args.target, "r") as f:
            target_allocation = json.load(f)
    except FileNotFoundError:
        print(f"{Colors.RED}Error: Target file not found: {args.target}{Colors.RESET}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError:
        print(f"{Colors.RED}Error: Invalid JSON in target file{Colors.RESET}", file=sys.stderr)
        sys.exit(1)

    # Normalize target allocation (should sum to 1.0)
    total = sum(target_allocation.values())
    if total > 0:
        target_allocation = {k: v/total for k, v in target_allocation.items()}

    print(f"{Colors.BOLD}Portfolio Rebalancer{Colors.RESET}")
    print(f"Target: {json.dumps(target_allocation, indent=2)}\n")

    monitor = PortfolioMonitor(base_url=args.url)

    # Scan portfolio
    print(f"{Colors.BLUE}Scanning portfolio...{Colors.RESET}")
    positions, current_allocation = monitor.scan_portfolio()

    # Show current state
    monitor._print_ascii_pie_chart(current_allocation)

    # Calculate rebalancing trades
    trades = monitor.calculate_rebalancing_trades(
        current_allocation,
        target_allocation,
        max_trade_size=args.max_trade_size,
        portfolio_value=10000.0
    )

    # Show plan
    monitor._print_rebalancing_plan(trades)

    # Execute if requested
    if args.execute and not args.dry_run:
        if trades:
            response = input(f"\n{Colors.YELLOW}Execute {len(trades)} trades? (yes/no): {Colors.RESET}")
            if response.lower() == "yes":
                monitor.execute_trades(trades)
        else:
            print("No trades to execute.")
    else:
        print(f"\n{Colors.YELLOW}(Dry run - use --execute to actually trade){Colors.RESET}")


if __name__ == "__main__":
    main()
