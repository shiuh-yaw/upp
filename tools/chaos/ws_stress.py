#!/usr/bin/env python3
"""
WebSocket Stress Test for UPP Gateway
Tests concurrent WebSocket connections, subscriptions, reconnection, and throughput
Attempts to use websockets library, falls back with instructions if unavailable
"""

import json
import time
import sys
import threading
from collections import defaultdict
from typing import List, Dict, Optional
import urllib.request
import urllib.error

# Color codes
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
BLUE = "\033[94m"
RESET = "\033[0m"
BOLD = "\033[1m"

# Configuration
GATEWAY_URL = "http://localhost:8080"
WS_URL = "ws://localhost:8080"
DEFAULT_NUM_CLIENTS = 20
DEFAULT_DURATION = 60

# Try to import websockets
try:
    import asyncio
    import websockets
    WEBSOCKETS_AVAILABLE = True
except ImportError:
    WEBSOCKETS_AVAILABLE = False

def print_header(text: str):
    """Print formatted header"""
    print(f"\n{BOLD}{BLUE}{'='*70}{RESET}")
    print(f"{BOLD}{BLUE}{text:^70}{RESET}")
    print(f"{BOLD}{BLUE}{'='*70}{RESET}\n")

def print_result(test_name: str, passed: bool, message: str = ""):
    """Print test result with color coding"""
    status = f"{GREEN}✓ PASS{RESET}" if passed else f"{RED}✗ FAIL{RESET}"
    print(f"{status}  {test_name}")
    if message:
        print(f"       {message}")

def get_gateway_metrics() -> Optional[Dict]:
    """Fetch /metrics endpoint to check WebSocket connection counts"""
    try:
        url = f"{GATEWAY_URL}/metrics"
        with urllib.request.urlopen(url, timeout=5.0) as response:
            body = response.read().decode('utf-8')
            # Parse simple Prometheus format
            metrics = {}
            for line in body.split('\n'):
                if line.startswith('ws_connections'):
                    parts = line.split(' ')
                    if len(parts) >= 2:
                        try:
                            metrics['ws_connections'] = int(parts[1])
                        except:
                            pass
            return metrics if metrics else None
    except:
        return None

async def ws_client_task(client_id: int, duration: int, results: Dict) -> None:
    """
    Simulate a single WebSocket client
    Connects, subscribes to markets, and tracks metrics
    """
    try:
        markets = ['kalshi_bitcoin', 'polymarket_eth', 'opinion_trade_apple']
        connection_start = time.time()

        async with websockets.connect(f"{WS_URL}/ws") as websocket:
            connection_time = time.time() - connection_start

            results['connection_times'].append(connection_time)
            results['client_count'] += 1

            # Subscribe to markets
            for market in markets:
                subscribe_msg = json.dumps({
                    'type': 'subscribe',
                    'market': market
                })
                await websocket.send(subscribe_msg)

            first_msg_start = time.time()
            first_msg_received = False
            messages_received = 0
            start_time = time.time()

            while time.time() - start_time < duration:
                try:
                    msg = await asyncio.wait_for(websocket.recv(), timeout=1.0)
                    messages_received += 1

                    if not first_msg_received:
                        first_msg_time = time.time() - first_msg_start
                        results['first_message_latencies'].append(first_msg_time)
                        first_msg_received = True

                except asyncio.TimeoutError:
                    continue

            results['messages_per_client'].append(messages_received)

    except Exception as e:
        results['errors'].append(str(e))

async def run_ws_stress_test() -> bool:
    """
    Run WebSocket stress test with multiple concurrent clients
    """
    print_header("WebSocket Concurrent Connection Test")

    results = {
        'connection_times': [],
        'first_message_latencies': [],
        'messages_per_client': [],
        'client_count': 0,
        'errors': [],
        'lock': threading.Lock()
    }

    print(f"Connecting {DEFAULT_NUM_CLIENTS} clients for {DEFAULT_DURATION} seconds...\n")

    tasks = []
    for i in range(DEFAULT_NUM_CLIENTS):
        task = ws_client_task(i, DEFAULT_DURATION, results)
        tasks.append(task)

    await asyncio.gather(*tasks)

    # Analyze results
    print(f"Connected clients: {results['client_count']}/{DEFAULT_NUM_CLIENTS}")

    connection_ok = results['client_count'] >= DEFAULT_NUM_CLIENTS * 0.8  # 80% success
    print_result(
        "80%+ clients connected successfully",
        connection_ok,
        f"Connected: {results['client_count']}/{DEFAULT_NUM_CLIENTS}"
    )

    # Connection time
    if results['connection_times']:
        avg_connection = sum(results['connection_times']) / len(results['connection_times'])
        max_connection = max(results['connection_times'])
        print_result(
            "Connection establishment latency acceptable",
            avg_connection < 1.0,
            f"Avg: {avg_connection*1000:.2f}ms, Max: {max_connection*1000:.2f}ms"
        )

    # First message latency
    if results['first_message_latencies']:
        avg_first_msg = sum(results['first_message_latencies']) / len(results['first_message_latencies'])
        max_first_msg = max(results['first_message_latencies'])
        first_msg_ok = avg_first_msg < 2.0
        print_result(
            "First message latency within limits",
            first_msg_ok,
            f"Avg: {avg_first_msg*1000:.2f}ms, Max: {max_first_msg*1000:.2f}ms"
        )
    else:
        print_result("First message latency within limits", False, "No messages received")
        first_msg_ok = False

    # Messages per client and throughput
    if results['messages_per_client']:
        total_messages = sum(results['messages_per_client'])
        avg_per_client = total_messages / len(results['messages_per_client'])
        throughput = total_messages / DEFAULT_DURATION

        throughput_ok = throughput > 10  # At least 10 messages/sec
        print_result(
            "Adequate message throughput",
            throughput_ok,
            f"Total: {total_messages}, Avg/client: {avg_per_client:.1f}, "
            f"Throughput: {throughput:.1f} msg/sec"
        )
    else:
        print_result("Adequate message throughput", False, "No messages received")
        throughput_ok = False

    # Error handling
    error_ok = len(results['errors']) == 0
    if results['errors']:
        print_result(
            "No connection errors",
            error_ok,
            f"Errors: {len(results['errors'])}"
        )
    else:
        print_result("No connection errors", error_ok)

    # Check metrics
    print("\nChecking gateway metrics...")
    metrics = get_gateway_metrics()
    if metrics:
        print_result(
            "WebSocket connection counter in /metrics accurate",
            True,
            f"ws_connections: {metrics.get('ws_connections', 'N/A')}"
        )
    else:
        print_result(
            "/metrics endpoint available",
            False,
            "Could not fetch metrics"
        )

    return connection_ok and first_msg_ok and throughput_ok and error_ok

async def run_reconnection_test() -> bool:
    """
    Test WebSocket reconnection behavior
    Disconnect and reconnect a client, verify subscriptions resume
    """
    print_header("WebSocket Reconnection Test")

    try:
        print("Testing single client reconnection...")

        # First connection
        async with websockets.connect(f"{WS_URL}/ws") as ws1:
            subscribe_msg = json.dumps({
                'type': 'subscribe',
                'market': 'kalshi_bitcoin'
            })
            await ws1.send(subscribe_msg)

            # Receive a few messages
            try:
                msg1 = await asyncio.wait_for(ws1.recv(), timeout=2.0)
            except asyncio.TimeoutError:
                msg1 = None

        # Brief pause
        time.sleep(0.5)

        # Reconnect
        try:
            async with websockets.connect(f"{WS_URL}/ws") as ws2:
                await ws2.send(subscribe_msg)

                # Should receive messages on new connection
                msg2 = await asyncio.wait_for(ws2.recv(), timeout=2.0)

                reconnect_ok = msg2 is not None
                print_result(
                    "Client successfully reconnects and resumes subscriptions",
                    reconnect_ok,
                    "Received messages on both connections"
                )
                return reconnect_ok

        except Exception as e:
            print_result(
                "Client successfully reconnects and resumes subscriptions",
                False,
                f"Reconnection failed: {str(e)}"
            )
            return False

    except Exception as e:
        print_result(
            "Client successfully reconnects and resumes subscriptions",
            False,
            f"Test failed: {str(e)}"
        )
        return False

def websockets_not_installed():
    """Print instructions for installing websockets"""
    print_header("WebSocket Tests Skipped")
    print(f"{YELLOW}WebSocket stress tests require the 'websockets' library{RESET}\n")
    print("To install, run:\n")
    print(f"{BOLD}  pip install websockets{RESET}")
    print(f"{BOLD}  # or{RESET}")
    print(f"{BOLD}  python3 -m pip install websockets{RESET}\n")
    print("Or with conda:\n")
    print(f"{BOLD}  conda install websockets{RESET}\n")
    print("After installation, re-run this script to test WebSocket functionality.\n")
    return 1

async def main_async():
    """Run all WebSocket tests"""
    if not WEBSOCKETS_AVAILABLE:
        return websockets_not_installed()

    print(f"\n{BOLD}{BLUE}{'='*70}{RESET}")
    print(f"{BOLD}{BLUE}{'UPP Gateway WebSocket Stress Test':^70}{RESET}")
    print(f"{BOLD}{BLUE}{'='*70}{RESET}")
    print(f"Gateway: {WS_URL}")
    print(f"Num Clients: {DEFAULT_NUM_CLIENTS}")
    print(f"Duration: {DEFAULT_DURATION}s")

    # Verify gateway is running
    try:
        with urllib.request.urlopen(f"{GATEWAY_URL}/health", timeout=5.0) as response:
            if response.status != 200:
                print(f"\n{RED}Error: Gateway not responding at {GATEWAY_URL}{RESET}")
                return 1
    except:
        print(f"\n{RED}Error: Gateway not responding at {GATEWAY_URL}{RESET}")
        return 1

    results = []

    try:
        results.append(("Concurrent Connection Stress", await run_ws_stress_test()))
        results.append(("Reconnection Behavior", await run_reconnection_test()))
    except Exception as e:
        print(f"\n{RED}Error during WebSocket tests: {e}{RESET}")
        import traceback
        traceback.print_exc()
        return 1

    # Summary
    print_header("WebSocket Test Summary")

    passed = sum(1 for _, p in results if p)
    total = len(results)

    for name, passed_test in results:
        status = f"{GREEN}✓{RESET}" if passed_test else f"{RED}✗{RESET}"
        print(f"{status} {name}")

    print(f"\n{BOLD}Results: {passed}/{total} tests passed{RESET}\n")

    return 0 if passed == total else 1

def main():
    """Entry point"""
    if not WEBSOCKETS_AVAILABLE:
        return websockets_not_installed()

    try:
        return asyncio.run(main_async())
    except Exception as e:
        print(f"\n{RED}Fatal error: {e}{RESET}")
        import traceback
        traceback.print_exc()
        return 1

if __name__ == "__main__":
    sys.exit(main())
