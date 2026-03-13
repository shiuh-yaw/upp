#!/usr/bin/env python3
"""
MCP tool usage demonstration for UPP gateway.

Shows how an AI agent would interact with UPP via MCP (Model Context Protocol).
Demonstrates: market search, details retrieval, orderbook analysis, trade estimation,
and analysis summarization.

Usage:
    python mcp_agent_demo.py --topic "bitcoin price"
    python mcp_agent_demo.py --topic "us election 2024" --interactive
"""

import argparse
import json
import sys
import time
import urllib.request
import urllib.error
from typing import Any, Dict, List
from dataclasses import dataclass


# ANSI color codes
class Colors:
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BLUE = "\033[94m"
    CYAN = "\033[96m"
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"


@dataclass
class AgentStep:
    """Represents a step in agent reasoning."""
    step_num: int
    action: str
    tool: str
    params: Dict[str, Any]
    result: Dict[str, Any] | List[Any]
    reasoning: str


class MCPClient:
    """Client for MCP tool execution via UPP gateway."""

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url
        self.steps: List[AgentStep] = []
        self.step_count = 0

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
            print(f"{Colors.RED}Error: {e}{Colors.RESET}", file=sys.stderr)
            return {}

    def execute_tool(self, tool: str, params: Dict[str, Any], reasoning: str = "") -> Dict[str, Any] | List[Any]:
        """Execute an MCP tool via gateway."""
        self.step_count += 1

        data = {
            "tool": tool,
            "params": params
        }

        result = self._request("POST", "/upp/v1/mcp/execute", data)

        step = AgentStep(
            step_num=self.step_count,
            action=f"Execute {tool}",
            tool=tool,
            params=params,
            result=result,
            reasoning=reasoning
        )
        self.steps.append(step)

        return result

    def _print_step_header(self, step: AgentStep) -> None:
        """Print step header."""
        print(f"\n{Colors.BOLD}Step {step.step_num}: {step.action}{Colors.RESET}")
        if step.reasoning:
            print(f"{Colors.DIM}Reasoning: {step.reasoning}{Colors.RESET}")

    def _print_tool_call(self, step: AgentStep) -> None:
        """Print tool call with parameters."""
        print(f"\n{Colors.CYAN}→ Tool Call:{Colors.RESET}")
        print(f"  Tool: {Colors.BOLD}{step.tool}{Colors.RESET}")
        print(f"  Params: {json.dumps(step.params, indent=2)}")

    def _print_result(self, step: AgentStep) -> None:
        """Print tool result."""
        print(f"\n{Colors.CYAN}← Result:{Colors.RESET}")

        if isinstance(step.result, list):
            print(f"  Found {len(step.result)} items:")
            for i, item in enumerate(step.result[:5]):  # Show first 5
                if isinstance(item, dict):
                    title = item.get("event", {}).get("title", item.get("id", "Unknown"))
                    print(f"    [{i+1}] {title}")
                else:
                    print(f"    [{i+1}] {str(item)[:60]}")
            if len(step.result) > 5:
                print(f"    ... and {len(step.result) - 5} more")
        elif isinstance(step.result, dict):
            # Pretty print key fields
            keys_to_show = ["id", "title", "description", "best_bid", "best_ask",
                           "last_price", "status", "provider"]
            shown = {}
            for key in keys_to_show:
                if key in step.result:
                    shown[key] = step.result[key]

            if shown:
                print(f"  {json.dumps(shown, indent=2)}")
            else:
                print(f"  {json.dumps(step.result, indent=2)[:200]}...")
        else:
            print(f"  {step.result}")

    def print_step(self, step: AgentStep) -> None:
        """Pretty-print a step with tool call and result."""
        self._print_step_header(step)
        self._print_tool_call(step)
        self._print_result(step)

    def print_summary(self) -> None:
        """Print summary of agent reasoning."""
        if not self.steps:
            return

        print(f"\n{Colors.BOLD}Analysis Summary{Colors.RESET}")
        print(f"Completed {len(self.steps)} steps\n")

        summary_text = self._generate_summary()
        print(summary_text)

    def _generate_summary(self) -> str:
        """Generate a summary of the analysis."""
        if not self.steps:
            return "No analysis performed."

        lines = []

        # Search results
        search_step = next((s for s in self.steps if s.tool == "search_markets"), None)
        if search_step and isinstance(search_step.result, list):
            lines.append(f"Found {len(search_step.result)} relevant markets for the topic.")

        # Market details
        details_step = next((s for s in self.steps if s.tool == "get_market_details"), None)
        if details_step and isinstance(details_step.result, dict):
            market = details_step.result
            event = market.get("event", {})
            title = event.get("title", "Unknown")
            category = event.get("category", "Unknown")
            lines.append(f"Analyzed market: {Colors.BOLD}{title}{Colors.RESET} ({category})")

            outcomes = market.get("outcomes", [])
            if outcomes:
                outcome_labels = [o.get("label", "?") for o in outcomes]
                lines.append(f"Possible outcomes: {', '.join(outcome_labels)}")

        # Orderbook analysis
        orderbook_step = next((s for s in self.steps if s.tool == "get_orderbook"), None)
        if orderbook_step and isinstance(orderbook_step.result, dict):
            pricing = orderbook_step.result.get("pricing", {})
            if pricing:
                bid = pricing.get("best_bid", {})
                ask = pricing.get("best_ask", {})
                spread = ask.get("yes") - bid.get("yes") if isinstance(bid, dict) and isinstance(ask, dict) else None
                if spread:
                    lines.append(f"Market spread: {spread:.4f}")

        # Estimation
        estimate_step = next((s for s in self.steps if s.tool == "estimate_order"), None)
        if estimate_step and isinstance(estimate_step.result, dict):
            cost = estimate_step.result.get("estimated_cost", 0)
            lines.append(f"To buy 100 shares: estimated cost ${cost:.2f}")

        return "\n".join(lines)


class AgentDemo:
    """Demonstrates AI agent interaction with UPP."""

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.client = MCPClient(base_url)

    def run_analysis(self, topic: str) -> None:
        """Run a full market analysis for a topic."""
        print(f"{Colors.BOLD}UPP Market Analysis Agent{Colors.RESET}")
        print(f"Topic: {Colors.CYAN}{topic}{Colors.RESET}\n")
        print("-" * 80)

        # Step 1: Search for markets
        print(f"\n{Colors.YELLOW}Step 1: Searching for markets...{Colors.RESET}")
        search_result = self.client.execute_tool(
            "search_markets",
            {"query": topic},
            reasoning=f"User asked about '{topic}', so search for related prediction markets"
        )
        self.client.print_step(self.client.steps[-1])

        if not search_result or (isinstance(search_result, list) and not search_result):
            print(f"{Colors.RED}No markets found.{Colors.RESET}")
            return

        # Get first market ID
        if isinstance(search_result, list) and search_result:
            first_market = search_result[0]
            market_id = first_market.get("id", {})
            if isinstance(market_id, dict):
                market_id = f"{market_id.get('provider', 'unknown')}:{market_id.get('native_id', 'unknown')}"
            elif isinstance(market_id, str):
                pass
            else:
                market_id = str(market_id)
        else:
            print(f"{Colors.RED}Invalid search result format.{Colors.RESET}")
            return

        time.sleep(0.5)

        # Step 2: Get market details
        print(f"\n{Colors.YELLOW}Step 2: Getting market details...{Colors.RESET}")
        details_result = self.client.execute_tool(
            "get_market_details",
            {"market_id": market_id},
            reasoning=f"Get full details of market {market_id} to understand outcomes and pricing"
        )
        self.client.print_step(self.client.steps[-1])

        time.sleep(0.5)

        # Step 3: Analyze orderbook
        print(f"\n{Colors.YELLOW}Step 3: Analyzing orderbook...{Colors.RESET}")
        orderbook_result = self.client.execute_tool(
            "get_orderbook",
            {"market_id": market_id, "depth": 5},
            reasoning=f"Check order book depth to assess liquidity and market sentiment"
        )
        self.client.print_step(self.client.steps[-1])

        time.sleep(0.5)

        # Step 4: Estimate a trade
        print(f"\n{Colors.YELLOW}Step 4: Estimating trade...{Colors.RESET}")
        estimate_result = self.client.execute_tool(
            "estimate_order",
            {"market_id": market_id, "side": "buy", "quantity": 100},
            reasoning=f"Calculate cost to buy 100 shares to determine if trade is economical"
        )
        self.client.print_step(self.client.steps[-1])

        time.sleep(0.5)

        # Print summary
        print(f"\n{Colors.BOLD}Summary{Colors.RESET}")
        print("-" * 80)
        self.client.print_summary()

    def interactive_mode(self) -> None:
        """Run interactive mode where user provides queries."""
        print(f"{Colors.BOLD}UPP Market Analysis Agent (Interactive){Colors.RESET}")
        print(f"{Colors.DIM}Type 'quit' to exit{Colors.RESET}\n")

        while True:
            try:
                topic = input(f"{Colors.CYAN}Query: {Colors.RESET}").strip()
                if topic.lower() == "quit":
                    break
                if not topic:
                    continue

                print()
                self.run_analysis(topic)
                print("\n" + "-" * 80 + "\n")

            except KeyboardInterrupt:
                print(f"\n{Colors.BLUE}Exiting...{Colors.RESET}")
                break
            except Exception as e:
                print(f"{Colors.RED}Error: {e}{Colors.RESET}", file=sys.stderr)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="MCP tool usage demonstration for UPP gateway"
    )
    parser.add_argument(
        "--url",
        default="http://localhost:8080",
        help="UPP gateway URL (default: http://localhost:8080)"
    )
    parser.add_argument(
        "--topic",
        help="Market topic to analyze (e.g., 'bitcoin price', 'US election 2024')"
    )
    parser.add_argument(
        "--interactive",
        action="store_true",
        help="Run in interactive mode"
    )

    args = parser.parse_args()

    demo = AgentDemo(base_url=args.url)

    if args.interactive:
        demo.interactive_mode()
    elif args.topic:
        demo.run_analysis(args.topic)
    else:
        # Default analysis
        print(f"{Colors.YELLOW}No topic specified. Running default analysis...{Colors.RESET}\n")
        demo.run_analysis("bitcoin price prediction")


if __name__ == "__main__":
    main()
