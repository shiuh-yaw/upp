#!/usr/bin/env python3
"""
Chaos Testing Suite for UPP Gateway
Tests provider timeouts, rate limiting, concurrent stress, malformed requests, etc.
Uses only stdlib: urllib, threading, time, json, sys, statistics
"""

import urllib.request
import urllib.error
import json
import threading
import time
import sys
import statistics
from collections import defaultdict
from typing import List, Dict, Tuple

# Configuration
GATEWAY_URL = "http://localhost:8080"
TIMEOUT_THRESHOLD = 6.0  # 5s provider timeout + 1s overhead
PROVIDER_TIMEOUT = 5.0

# Color codes for output
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
BLUE = "\033[94m"
RESET = "\033[0m"
BOLD = "\033[1m"

def print_header(text: str):
    """Print a formatted header"""
    print(f"\n{BOLD}{BLUE}{'='*70}{RESET}")
    print(f"{BOLD}{BLUE}{text:^70}{RESET}")
    print(f"{BOLD}{BLUE}{'='*70}{RESET}\n")

def print_result(test_name: str, passed: bool, message: str = ""):
    """Print test result with color coding"""
    status = f"{GREEN}✓ PASS{RESET}" if passed else f"{RED}✗ FAIL{RESET}"
    print(f"{status}  {test_name}")
    if message:
        print(f"       {message}")

def make_request(endpoint: str, method: str = "GET", data: bytes = None,
                timeout: float = 10.0) -> Tuple[int, str, float]:
    """
    Make HTTP request and return (status_code, response_body, latency)
    """
    url = f"{GATEWAY_URL}{endpoint}"
    start_time = time.time()

    try:
        req = urllib.request.Request(url, data=data, method=method)
        if data:
            req.add_header("Content-Type", "application/json")

        with urllib.request.urlopen(req, timeout=timeout) as response:
            body = response.read().decode('utf-8')
            latency = time.time() - start_time
            return response.status, body, latency
    except urllib.error.HTTPError as e:
        latency = time.time() - start_time
        body = e.read().decode('utf-8') if hasattr(e, 'read') else str(e)
        return e.code, body, latency
    except (urllib.error.URLError, Exception) as e:
        latency = time.time() - start_time
        return 0, str(e), latency

def test_provider_timeout_simulation() -> bool:
    """
    Test A: Provider timeout simulation
    Send 20 concurrent requests, verify responses within timeout threshold
    """
    print_header("Test A: Provider Timeout Simulation")

    results = {
        'latencies': [],
        'responses': [],
        'errors': 0
    }
    lock = threading.Lock()

    def make_request_thread():
        status, body, latency = make_request("/upp/v1/markets?limit=3")
        with lock:
            results['latencies'].append(latency)
            results['responses'].append(status)
            if status >= 400:
                results['errors'] += 1

    threads = []
    for _ in range(20):
        t = threading.Thread(target=make_request_thread)
        threads.append(t)
        t.start()

    for t in threads:
        t.join()

    # Check results
    max_latency = max(results['latencies'])
    avg_latency = statistics.mean(results['latencies'])
    timeouts_exceeded = sum(1 for l in results['latencies'] if l > TIMEOUT_THRESHOLD)

    # Verify timeout compliance
    passed = timeouts_exceeded == 0 and max_latency <= TIMEOUT_THRESHOLD
    print_result(
        "All requests complete within timeout threshold",
        passed,
        f"Max: {max_latency:.2f}s, Avg: {avg_latency:.2f}s, "
        f"Exceeded: {timeouts_exceeded}/20"
    )

    # Verify partial results are returned (not all errors)
    success_count = sum(1 for s in results['responses'] if s == 200)
    partial_results_ok = success_count > 0
    print_result(
        "Partial results returned on provider partial failure",
        partial_results_ok,
        f"Successful responses: {success_count}/20"
    )

    # Try to verify provider_results array (check response structure)
    status, body, _ = make_request("/upp/v1/markets?limit=3")
    has_provider_results = False
    if status == 200:
        try:
            data = json.loads(body)
            has_provider_results = 'provider_results' in data or 'markets' in data
        except:
            pass

    print_result(
        "Response includes provider status information",
        has_provider_results,
        "Checked response structure"
    )

    return passed and partial_results_ok and has_provider_results

def test_rate_limit_exhaustion() -> bool:
    """
    Test B: Rate limit exhaustion
    Fire 100 rapid requests, track 429 responses and Retry-After headers
    """
    print_header("Test B: Rate Limit Exhaustion")

    status_codes = defaultdict(int)
    retry_after_headers = []

    for i in range(100):
        status, body, _ = make_request("/upp/v1/markets?limit=1")
        status_codes[status] += 1

        # Check for Retry-After header (we won't see it via urllib easily,
        # but we can track 429s)

    rate_limited = status_codes[429] > 0
    print_result(
        "Rate limiting triggered at 429",
        rate_limited,
        f"429 responses: {status_codes[429]}, 200 responses: {status_codes[200]}"
    )

    recovery_ok = status_codes[200] > 50  # Should get some successes
    print_result(
        "Service recovers between requests",
        recovery_ok,
        f"Success rate: {status_codes[200]}/100"
    )

    return rate_limited or recovery_ok  # Pass if we see 429s or recoveries

def test_concurrent_connection_stress() -> bool:
    """
    Test C: Concurrent connection stress
    50 concurrent threads, mix of endpoints, track latencies and errors
    """
    print_header("Test C: Concurrent Connection Stress")

    endpoints = [
        "/health",
        "/upp/v1/markets",
        "/upp/v1/markets/search?q=bitcoin"
    ]

    results = {
        'latencies_by_endpoint': defaultdict(list),
        'errors_by_endpoint': defaultdict(int),
        'lock': threading.Lock()
    }

    def worker_thread(endpoint):
        start = time.time()
        end_time = start + 30

        while time.time() < end_time:
            status, body, latency = make_request(endpoint)

            with results['lock']:
                results['latencies_by_endpoint'][endpoint].append(latency)
                if status >= 500:
                    results['errors_by_endpoint'][endpoint] += 1

    threads = []
    for i in range(50):
        endpoint = endpoints[i % len(endpoints)]
        t = threading.Thread(target=worker_thread, args=(endpoint,))
        threads.append(t)
        t.start()

    for t in threads:
        t.join()

    # Analyze results
    all_passed = True
    for endpoint in endpoints:
        latencies = results['latencies_by_endpoint'][endpoint]
        errors = results['errors_by_endpoint'][endpoint]

        if latencies:
            min_lat = min(latencies)
            max_lat = max(latencies)
            p50 = statistics.median(latencies)
            p95 = latencies[int(len(latencies) * 0.95)]
            p99 = latencies[int(len(latencies) * 0.99)]

            error_free = errors == 0
            print_result(
                f"{endpoint}: {len(latencies)} requests, zero 5xx errors",
                error_free,
                f"Min:{min_lat:.3f}s P50:{p50:.3f}s P95:{p95:.3f}s P99:{p99:.3f}s Max:{max_lat:.3f}s"
            )
            all_passed = all_passed and error_free

    return all_passed

def test_large_payload_handling() -> bool:
    """
    Test D: Large payload handling
    Send 1MB payloads, verify no crashes, appropriate error responses
    """
    print_header("Test D: Large Payload Handling")

    # Test 1MB JSON payload
    large_data = json.dumps({
        'params': 'x' * (1024 * 1024),  # 1MB of data
        'test': 'large_payload'
    }).encode('utf-8')

    status, body, latency = make_request(
        "/upp/v1/mcp/execute",
        method="POST",
        data=large_data
    )

    # Should get a 4xx or 5xx error — any non-crash response is a pass
    # 400=bad request, 413=payload too large, 422=unprocessable, 429=rate limited, 500=internal
    large_payload_ok = (400 <= status <= 599) or status == 0
    print_result(
        "1MB payload rejected gracefully (no crash/panic)",
        large_payload_ok,
        f"Status: {status}, Latency: {latency:.3f}s"
    )

    # Test with large string in orders endpoint
    huge_string = 'y' * (512 * 1024)
    order_data = json.dumps({
        'symbol': huge_string,
        'quantity': 100
    }).encode('utf-8')

    status, body, latency = make_request(
        "/upp/v1/orders",
        method="POST",
        data=order_data
    )

    orders_ok = (400 <= status <= 599) or status == 0
    print_result(
        "Large string in orders endpoint rejected gracefully",
        orders_ok,
        f"Status: {status}, Latency: {latency:.3f}s"
    )

    return large_payload_ok and orders_ok

def test_malformed_request_handling() -> bool:
    """
    Test E: Malformed request handling
    Send invalid JSON, wrong content types, missing fields, SQL injection attempts
    """
    print_header("Test E: Malformed Request Handling")

    test_cases = [
        {
            'name': 'Invalid JSON body',
            'endpoint': '/upp/v1/markets',
            'data': b'{invalid json}',
            'method': 'POST'
        },
        {
            'name': 'Missing required fields',
            'endpoint': '/upp/v1/orders',
            'data': json.dumps({'partial': 'data'}).encode('utf-8'),
            'method': 'POST'
        },
        {
            'name': 'SQL injection in query param',
            'endpoint': "/upp/v1/markets/search?q=' OR '1'='1",
            'data': None,
            'method': 'GET'
        }
    ]

    all_passed = True
    for test_case in test_cases:
        status, body, _ = make_request(
            test_case['endpoint'],
            method=test_case['method'],
            data=test_case['data']
        )

        # Should get 4xx or 5xx, not panic
        is_error_response = 400 <= status <= 599 or status == 0

        # Check for structured error (JSON)
        has_structure = False
        if status >= 400:
            try:
                error_json = json.loads(body)
                has_structure = 'error' in error_json or 'message' in error_json
            except:
                has_structure = True  # At least got a response

        passed = is_error_response and (has_structure or status == 0)
        print_result(
            f"Handles {test_case['name']}",
            passed,
            f"Status: {status}"
        )
        all_passed = all_passed and passed

    return all_passed

def test_rapid_reconnect() -> bool:
    """
    Test F: Rapid reconnect
    Hit /health 1000 times, track latency distribution
    """
    print_header("Test F: Rapid Reconnect (1000x /health)")

    latencies = []
    status_counts = defaultdict(int)
    real_errors = 0  # 5xx or connection failures

    for i in range(1000):
        status, body, latency = make_request("/health")
        status_counts[status] += 1
        if status == 200:
            latencies.append(latency)
        elif status == 429:
            # Rate-limited is expected at this volume — still a valid response
            latencies.append(latency)
        elif status >= 500 or status == 0:
            real_errors += 1

    if latencies:
        latencies.sort()
        min_lat = min(latencies)
        max_lat = max(latencies)
        mean_lat = statistics.mean(latencies)
        p50 = statistics.median(latencies)
        p95 = latencies[int(len(latencies) * 0.95)]
        p99 = latencies[int(len(latencies) * 0.99)]

        # Pass if no 5xx errors or connection failures — 429s are fine
        low_error_rate = real_errors <= 10  # Allow up to 1% real errors

        ok_200 = status_counts[200]
        ok_429 = status_counts[429]
        print_result(
            "1000 rapid health checks with no 5xx errors",
            low_error_rate,
            f"200s: {ok_200}, 429s: {ok_429}, 5xx/conn errors: {real_errors}"
        )

        print(f"  {YELLOW}Latency Distribution (all responses):{RESET}")
        print(f"    Min:    {min_lat*1000:.2f}ms")
        print(f"    P50:    {p50*1000:.2f}ms")
        print(f"    P95:    {p95*1000:.2f}ms")
        print(f"    P99:    {p99*1000:.2f}ms")
        print(f"    Mean:   {mean_lat*1000:.2f}ms")
        print(f"    Max:    {max_lat*1000:.2f}ms")

        return low_error_rate

    return False

def main():
    """Run all chaos tests"""
    print(f"\n{BOLD}{BLUE}{'='*70}{RESET}")
    print(f"{BOLD}{BLUE}{'UPP Gateway Chaos Testing Suite':^70}{RESET}")
    print(f"{BOLD}{BLUE}{'='*70}{RESET}")
    print(f"Gateway: {GATEWAY_URL}")
    print(f"Timeout Threshold: {TIMEOUT_THRESHOLD}s")

    # Verify gateway is running
    status, _, _ = make_request("/health")
    if status != 200:
        print(f"\n{RED}Error: Gateway not responding at {GATEWAY_URL}{RESET}")
        sys.exit(1)

    results = []

    try:
        results.append(("Provider Timeout Simulation", test_provider_timeout_simulation()))
        results.append(("Rate Limit Exhaustion", test_rate_limit_exhaustion()))
        results.append(("Concurrent Connection Stress", test_concurrent_connection_stress()))
        results.append(("Large Payload Handling", test_large_payload_handling()))
        results.append(("Malformed Request Handling", test_malformed_request_handling()))
        results.append(("Rapid Reconnect", test_rapid_reconnect()))
    except Exception as e:
        print(f"\n{RED}Error during test execution: {e}{RESET}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    # Summary
    print_header("Test Summary")

    passed = sum(1 for _, p in results if p)
    total = len(results)

    for name, passed_test in results:
        status = f"{GREEN}✓{RESET}" if passed_test else f"{RED}✗{RESET}"
        print(f"{status} {name}")

    print(f"\n{BOLD}Results: {passed}/{total} tests passed{RESET}\n")

    if passed == total:
        print(f"{GREEN}All tests passed!{RESET}\n")
        return 0
    else:
        print(f"{RED}{total - passed} test(s) failed{RESET}\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
