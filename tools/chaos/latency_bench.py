#!/usr/bin/env python3
"""
Latency Benchmarking Suite for UPP Gateway
Measures min, max, mean, p50, p90, p95, p99, stddev for all endpoints
Compares against SLA thresholds with color-coded pass/fail
Uses only stdlib: urllib, time, json, sys, statistics
"""

import urllib.request
import urllib.error
import json
import time
import sys
import statistics
from typing import List, Dict, Tuple

# Configuration
GATEWAY_URL = "http://localhost:8080"
DEFAULT_ITERATIONS = 100

# SLA Thresholds (in seconds)
SLA_THRESHOLDS = {
    '/health': 0.010,                           # 10ms
    '/upp/v1/markets': 3.0,                     # 3s (market data)
    '/upp/v1/markets/search': 3.0,              # 3s (market data)
    '/upp/v1/mcp/tools': 0.050,                 # 50ms (static)
    '/upp/v1/mcp/execute': 1.0,                 # 1s (computation)
    '/.well-known/agent.json': 0.050,           # 50ms (static)
}

# Color codes
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
BLUE = "\033[94m"
RESET = "\033[0m"
BOLD = "\033[1m"

def print_header(text: str):
    """Print formatted header"""
    print(f"\n{BOLD}{BLUE}{'='*90}{RESET}")
    print(f"{BOLD}{BLUE}{text:^90}{RESET}")
    print(f"{BOLD}{BLUE}{'='*90}{RESET}\n")

def make_request(endpoint: str, method: str = "GET", body: dict = None) -> Tuple[int, float]:
    """Make HTTP request and return (status_code, latency)"""
    url = f"{GATEWAY_URL}{endpoint}"
    start_time = time.time()

    try:
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            req = urllib.request.Request(url, data=data, method=method)
            req.add_header("Content-Type", "application/json")
        else:
            req = urllib.request.Request(url, method=method)
        with urllib.request.urlopen(req, timeout=30.0) as response:
            response.read()  # Consume response
            latency = time.time() - start_time
            return response.status, latency
    except urllib.error.HTTPError as e:
        latency = time.time() - start_time
        return e.code, latency
    except (urllib.error.URLError, Exception):
        latency = time.time() - start_time
        return 0, latency

def get_sla_threshold(endpoint: str) -> float:
    """Get SLA threshold for an endpoint, stripping query parameters for lookup."""
    # Exact match first
    if endpoint in SLA_THRESHOLDS:
        return SLA_THRESHOLDS[endpoint]
    # Strip query params and try again
    base_path = endpoint.split('?')[0]
    if base_path in SLA_THRESHOLDS:
        return SLA_THRESHOLDS[base_path]
    return 5.0  # Default 5s SLA for unknown endpoints


def benchmark_endpoint(endpoint: str, iterations: int, method: str = "GET",
                       body: dict = None, pace_ms: int = 10) -> Dict:
    """Benchmark an endpoint with optional pacing between requests."""
    latencies = []
    success_count = 0
    error_count = 0

    label = f"{method} {endpoint}"
    print(f"  {YELLOW}Benchmarking {label}...{RESET}", end='', flush=True)

    for i in range(iterations):
        status, latency = make_request(endpoint, method=method, body=body)
        if 200 <= status < 300:
            latencies.append(latency)
            success_count += 1
        else:
            error_count += 1
        # Small pause between requests to avoid rate-limit false negatives
        if pace_ms > 0 and i < iterations - 1:
            time.sleep(pace_ms / 1000.0)

    print(f" {GREEN}✓{RESET}")

    sla_threshold = get_sla_threshold(endpoint)

    if not latencies:
        return {
            'endpoint': label,
            'iterations': iterations,
            'success': success_count,
            'errors': error_count,
            'min': 0,
            'max': 0,
            'mean': 0,
            'median': 0,
            'p90': 0,
            'p95': 0,
            'p99': 0,
            'stddev': 0,
            'sla_threshold': sla_threshold,
            'sla_pass': False
        }

    # Sort for percentile calculations
    latencies.sort()

    min_lat = min(latencies)
    max_lat = max(latencies)
    mean_lat = statistics.mean(latencies)
    median_lat = statistics.median(latencies)
    p90 = latencies[int(len(latencies) * 0.90)]
    p95 = latencies[int(len(latencies) * 0.95)]
    p99 = latencies[int(len(latencies) * 0.99)]
    stddev = statistics.stdev(latencies) if len(latencies) > 1 else 0

    sla_pass = p95 <= sla_threshold  # Use P95 for SLA

    return {
        'endpoint': label,
        'iterations': iterations,
        'success': success_count,
        'errors': error_count,
        'min': min_lat,
        'max': max_lat,
        'mean': mean_lat,
        'median': median_lat,
        'p90': p90,
        'p95': p95,
        'p99': p99,
        'stddev': stddev,
        'sla_threshold': sla_threshold,
        'sla_pass': sla_pass
    }

def format_latency(ms: float) -> str:
    """Format latency with appropriate units"""
    if ms < 0.001:
        return f"{ms*1e6:.1f}µs"
    elif ms < 1.0:
        return f"{ms*1000:.2f}ms"
    else:
        return f"{ms:.3f}s"

def print_benchmark_table(results: List[Dict]):
    """Print formatted benchmark results table"""
    print(f"\n{BOLD}Latency Benchmark Results (N={DEFAULT_ITERATIONS}){RESET}\n")

    # Table header
    print(f"{'Endpoint':<35} {'Min':<12} {'P95':<12} {'Max':<12} {'SLA':<12} {'Status':<10}")
    print("-" * 95)

    # Table rows
    for result in results:
        endpoint = result['endpoint']
        min_lat = format_latency(result['min'])
        p95_lat = format_latency(result['p95'])
        max_lat = format_latency(result['max'])
        sla_threshold = format_latency(result['sla_threshold'])

        if result['sla_pass']:
            status_str = f"{GREEN}✓ PASS{RESET}"
        else:
            status_str = f"{RED}✗ FAIL{RESET}"

        print(f"{endpoint:<35} {min_lat:<12} {p95_lat:<12} {max_lat:<12} {sla_threshold:<12} {status_str:<10}")

    # Detailed stats
    print(f"\n{BOLD}Detailed Statistics{RESET}\n")

    for result in results:
        endpoint = result['endpoint']
        print(f"{BOLD}{endpoint}{RESET}")

        status = f"{GREEN}SLA: PASS{RESET}" if result['sla_pass'] else f"{RED}SLA: FAIL{RESET}"
        print(f"  {status} (threshold: {format_latency(result['sla_threshold'])}, p95: {format_latency(result['p95'])})")

        print(f"  Requests: {result['success']}/{result['iterations']} successful")

        print(f"  Min:      {format_latency(result['min'])}")
        print(f"  P50:      {format_latency(result['median'])}")
        print(f"  P90:      {format_latency(result['p90'])}")
        print(f"  P95:      {format_latency(result['p95'])}")
        print(f"  P99:      {format_latency(result['p99'])}")
        print(f"  Mean:     {format_latency(result['mean'])}")
        print(f"  Max:      {format_latency(result['max'])}")
        print(f"  StdDev:   {format_latency(result['stddev'])}")
        print()

def output_json_report(results: List[Dict], filename: str = "latency_report.json"):
    """Output results as JSON"""
    json_data = {
        'timestamp': time.strftime('%Y-%m-%d %H:%M:%S'),
        'gateway': GATEWAY_URL,
        'iterations': DEFAULT_ITERATIONS,
        'results': []
    }

    for result in results:
        json_data['results'].append({
            'endpoint': result['endpoint'],
            'latencies': {
                'min_ms': result['min'] * 1000,
                'p50_ms': result['median'] * 1000,
                'p90_ms': result['p90'] * 1000,
                'p95_ms': result['p95'] * 1000,
                'p99_ms': result['p99'] * 1000,
                'max_ms': result['max'] * 1000,
                'mean_ms': result['mean'] * 1000,
                'stddev_ms': result['stddev'] * 1000,
            },
            'sla': {
                'threshold_ms': result['sla_threshold'] * 1000,
                'pass': result['sla_pass']
            },
            'requests': {
                'successful': result['success'],
                'errors': result['errors'],
                'total': result['iterations']
            }
        })

    with open(filename, 'w') as f:
        json.dump(json_data, f, indent=2)

    print(f"JSON report written to {BOLD}{filename}{RESET}")

def main():
    """Run latency benchmarks"""
    print(f"\n{BOLD}{BLUE}{'='*90}{RESET}")
    print(f"{BOLD}{BLUE}{'UPP Gateway Latency Benchmarking Suite':^90}{RESET}")
    print(f"{BOLD}{BLUE}{'='*90}{RESET}")
    print(f"Gateway: {GATEWAY_URL}")
    print(f"Iterations per endpoint: {DEFAULT_ITERATIONS}")

    # Verify gateway is running
    status, _ = make_request("/health")
    if status != 200:
        print(f"\n{RED}Error: Gateway not responding at {GATEWAY_URL}{RESET}")
        sys.exit(1)

    # Define endpoints to benchmark: (endpoint, method, body_or_None)
    endpoints = [
        ('/health', 'GET', None),
        ('/upp/v1/markets?limit=5', 'GET', None),
        ('/upp/v1/markets/search?q=bitcoin', 'GET', None),
        ('/upp/v1/mcp/tools', 'GET', None),
        ('/upp/v1/mcp/execute', 'POST', {
            "tool": "list_markets",
            "params": {"provider": "kalshi.com", "limit": 5}
        }),
        ('/.well-known/agent.json', 'GET', None),
    ]

    print_header("Running Latency Benchmarks")

    results = []
    try:
        for endpoint, method, body in endpoints:
            result = benchmark_endpoint(endpoint, DEFAULT_ITERATIONS,
                                        method=method, body=body)
            results.append(result)
    except Exception as e:
        print(f"\n{RED}Error during benchmarking: {e}{RESET}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    # Print results
    print_benchmark_table(results)

    # Summary
    print_header("Benchmark Summary")

    passed_sla = sum(1 for r in results if r['sla_pass'])
    total = len(results)

    for result in results:
        status = f"{GREEN}✓{RESET}" if result['sla_pass'] else f"{RED}✗{RESET}"
        print(f"{status} {result['endpoint']:<35} P95: {format_latency(result['p95']):<12} SLA: {format_latency(result['sla_threshold'])}")

    print(f"\n{BOLD}SLA Results: {passed_sla}/{total} endpoints passed{RESET}\n")

    # Output JSON report
    output_json_report(results)

    # Exit code
    if passed_sla == total:
        print(f"{GREEN}All endpoints meet SLA thresholds!{RESET}\n")
        return 0
    else:
        print(f"{RED}{total - passed_sla} endpoint(s) failed SLA{RESET}\n")
        return 1

if __name__ == "__main__":
    sys.exit(main())
