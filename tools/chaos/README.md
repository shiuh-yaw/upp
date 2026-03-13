# UPP Gateway Chaos Testing & Latency Benchmarking Suite

A comprehensive testing suite for the UPP (Universal Prediction Platform) gateway, including chaos testing, latency benchmarking, and WebSocket stress testing. All tools use Python standard library (+ optional websockets) for maximum compatibility.

## Overview

The suite consists of four main components:

1. **chaos_test.py** — Chaos testing harness with 6 test categories
2. **latency_bench.py** — Latency benchmarking with SLA verification
3. **ws_stress.py** — WebSocket stress and reconnection testing
4. **run-all.sh** — Master runner script for sequential test execution

## Prerequisites

- Python 3.7+
- UPP gateway running on `localhost:8080`
- Optional: `websockets` library for WebSocket tests (fallback provided if missing)

### Installation

```bash
# Standard library only (required)
# No additional packages needed for chaos_test.py and latency_bench.py

# For WebSocket testing (optional)
pip install websockets
```

## Gateway Setup

Before running tests, ensure the gateway is running:

```bash
python3 -m upp.gateway  # or however your gateway starts
```

Verify it's running:

```bash
curl http://localhost:8080/health
# Expected: {"status": "ok"}
```

## Quick Start

### Run All Tests

```bash
./run-all.sh
```

This executes all tests sequentially and reports overall pass/fail.

### Run Individual Tests

```bash
# Chaos testing
python3 chaos_test.py

# Latency benchmarking
python3 latency_bench.py

# WebSocket stress testing
python3 ws_stress.py
```

## Test Details

### 1. Chaos Testing (chaos_test.py)

Tests the gateway's resilience under adverse conditions using only stdlib (`urllib`, `threading`, `time`, `json`, `statistics`).

#### Test A: Provider Timeout Simulation
- Sends 20 concurrent requests to `/upp/v1/markets?limit=3`
- Verifies all responses complete within **6 seconds** (5s provider timeout + 1s overhead)
- Checks that partial results are returned even if some providers fail
- Validates `provider_results` array structure in responses

**Why it matters:** Providers (kalshi.com, polymarket.com, opinion.trade) are queried in parallel with 5s timeouts. The gateway should aggregate available results and return them promptly even if some providers are slow.

#### Test B: Rate Limit Exhaustion
- Fires 100 rapid requests from the same client
- Tracks when 429 (Too Many Requests) responses begin
- Verifies `Retry-After` header presence
- Confirms service recovers after backoff

**Why it matters:** Rate limiting protects the gateway from abuse. This test ensures limits are enforced and clients can properly implement backoff.

#### Test C: Concurrent Connection Stress
- Launches 50 concurrent threads hitting different endpoints:
  - `/health`
  - `/upp/v1/markets`
  - `/upp/v1/markets/search?q=bitcoin`
- Tracks per-endpoint latency (min, max, p50, p95, p99)
- Runs for 30 seconds
- Verifies zero 5xx server errors

**Why it matters:** Simulates realistic production load. Measures how the gateway behaves under concurrent access patterns.

#### Test D: Large Payload Handling
- POSTs 1MB JSON body to `/upp/v1/mcp/execute`
- POSTs 512KB string to `/upp/v1/orders`
- Verifies gateway returns 400/413/414 (not 500 or crash)

**Why it matters:** Malicious or misconfigured clients may send oversized payloads. Gateway should gracefully reject them without crashing.

#### Test E: Malformed Request Handling
- Invalid JSON bodies: `{invalid json}`
- Missing required fields in order submissions
- SQL injection attempts in query parameters: `?q=' OR '1'='1`
- Verifies structured error responses, no panics

**Why it matters:** Ensures the gateway is robust against malformed input and injection attacks.

#### Test F: Rapid Reconnect
- Hits `/health` endpoint 1000 times sequentially
- Tracks full latency distribution (min, p50, p95, p99, max)
- Reports error rate

**Why it matters:** Health checks are used for monitoring and load balancing. Gateway must respond reliably under rapid repeated access.

#### Output Example

```
======================================================================
                    Test A: Provider Timeout Simulation
======================================================================

✓ PASS  All requests complete within timeout threshold
         Max: 5.32s, Avg: 3.41s, Exceeded: 0/20
✓ PASS  Partial results returned on provider partial failure
         Successful responses: 18/20
✓ PASS  Response includes provider status information
         Checked response structure

======================================================================
                              Test Summary
======================================================================

✓ Provider Timeout Simulation
✓ Rate Limit Exhaustion
✓ Concurrent Connection Stress
✓ Large Payload Handling
✓ Malformed Request Handling
✓ Rapid Reconnect

Results: 6/6 tests passed
```

### 2. Latency Benchmarking (latency_bench.py)

Measures response time distributions across all gateway endpoints with SLA verification. Default: 100 iterations per endpoint.

#### Endpoints Tested

| Endpoint | Iterations | SLA Threshold | Category |
|----------|-----------|---------------|----------|
| `/health` | 100 | 10ms | Health Check |
| `/upp/v1/markets?limit=5` | 100 | 3s | Market Data |
| `/upp/v1/markets/search?q=bitcoin` | 100 | 3s | Market Data |
| `/upp/v1/mcp/tools` | 100 | 50ms | Static |
| `/upp/v1/mcp/execute` | 100 | 1s | Computation |
| `/.well-known/agent.json` | 100 | 50ms | Static |

#### SLA Thresholds

- **Health checks**: < 10ms (must be fast for load balancer health checks)
- **Static endpoints**: < 50ms (agent.json, tools list)
- **Market data**: < 3s (3 providers × 5s timeout, aggregation overhead)
- **Computation**: < 1s (MCP tool execution)

#### Metrics Calculated

For each endpoint:
- **Min, Max, Mean** — Absolute bounds and average
- **P50, P90, P95, P99** — Percentile distribution
- **StdDev** — Latency consistency
- **SLA Pass/Fail** — Based on P95 ≤ threshold

#### Output Format

```
Latency Benchmark Results (N=100)

Endpoint                            Min          P95          Max          SLA          Status
------------------------------------------------------------------------------------------------------
/health                             1.23ms       3.45ms       8.91ms       10.00ms      ✓ PASS
/upp/v1/markets                     245.12ms     2.34s        2.89s        3.00s        ✓ PASS
/upp/v1/markets/search?q=bitcoin    342.56ms     2.11s        2.78s        3.00s        ✓ PASS
/upp/v1/mcp/tools                   5.23ms       23.45ms      45.67ms      50.00ms      ✓ PASS
/upp/v1/mcp/execute                 123.45ms     523.34ms     891.23ms     1.00s        ✓ PASS
/.well-known/agent.json             2.34ms       12.34ms      28.56ms      50.00ms      ✓ PASS

SLA Results: 6/6 endpoints passed
```

#### JSON Report

A `latency_report.json` file is automatically generated with all metrics:

```json
{
  "timestamp": "2026-03-13 14:32:45",
  "gateway": "http://localhost:8080",
  "iterations": 100,
  "results": [
    {
      "endpoint": "/health",
      "latencies": {
        "min_ms": 1.23,
        "p50_ms": 2.34,
        "p95_ms": 3.45,
        "p99_ms": 4.56,
        "max_ms": 8.91,
        "mean_ms": 2.78,
        "stddev_ms": 1.23
      },
      "sla": {
        "threshold_ms": 10.0,
        "pass": true
      },
      "requests": {
        "successful": 100,
        "errors": 0,
        "total": 100
      }
    }
  ]
}
```

### 3. WebSocket Stress Testing (ws_stress.py)

Tests concurrent WebSocket connections, subscriptions, and reconnection behavior. **Requires `websockets` library** (gracefully skipped if not installed).

#### Test A: Concurrent Connection Stress
- Connects N clients simultaneously (default: 20)
- Each subscribes to 3 markets for real-time prices
- Measures:
  - **Connection time**: Time to establish WebSocket handshake
  - **First message latency**: Time to receive first price update
  - **Messages per client**: Total messages received during test
  - **Throughput**: Total messages/sec across all clients
- Runs for configurable duration (default: 60s)

#### Test B: Reconnection Behavior
- Single client connects and subscribes to a market
- Deliberately disconnects
- Reconnects and resubscribes
- Verifies messages are received on both connections
- Confirms subscriptions properly resume

#### Metrics

- **Connection success rate**: 80%+ of clients should connect
- **Connection latency**: Average < 1000ms
- **First message latency**: Average < 2000ms
- **Throughput**: Minimum 10 messages/sec across all clients
- **Error rate**: Zero connection errors
- **Metrics accuracy**: `/metrics` endpoint reports correct `ws_connections` count

#### Output Example

```
======================================================================
                  WebSocket Concurrent Connection Test
======================================================================

Connecting 20 clients for 60 seconds...

✓ PASS  80%+ clients connected successfully
         Connected: 18/20
✓ PASS  Connection establishment latency acceptable
         Avg: 234.56ms, Max: 1203.45ms
✓ PASS  First message latency within limits
         Avg: 456.78ms, Max: 2100.34ms
✓ PASS  Adequate message throughput
         Total: 3456, Avg/client: 172.8, Throughput: 57.6 msg/sec
✓ PASS  No connection errors

======================================================================
                     WebSocket Test Summary
======================================================================

✓ Concurrent Connection Stress
✓ Reconnection Behavior

Results: 2/2 tests passed
```

#### Installation Notes

If WebSocket tests are skipped:

```
WebSocket stress tests require the 'websockets' library

To install, run:

  pip install websockets
  # or
  python3 -m pip install websockets

Or with conda:

  conda install websockets

After installation, re-run this script to test WebSocket functionality.
```

### 4. Master Runner (run-all.sh)

Executes all tests sequentially and provides a summary report.

```bash
./run-all.sh
```

**Output:**

```
======================================================================
              UPP Gateway Chaos Testing Suite - Full Run
======================================================================

Executing all chaos and benchmark tests sequentially...

======================================================================
                           SUMMARY
======================================================================

✓ Chaos Testing Harness
✓ Latency Benchmarking
✓ WebSocket Stress Test

Results: 3 passed, 0 failed

All tests passed!
```

**Exit codes:**
- `0` — All tests passed
- `1` — One or more tests failed

## Configuration

### Adjusting Test Parameters

Edit the top of each script to customize:

**chaos_test.py:**
```python
GATEWAY_URL = "http://localhost:8080"
TIMEOUT_THRESHOLD = 6.0        # seconds
PROVIDER_TIMEOUT = 5.0         # seconds
```

**latency_bench.py:**
```python
GATEWAY_URL = "http://localhost:8080"
DEFAULT_ITERATIONS = 100       # per endpoint

SLA_THRESHOLDS = {
    '/health': 0.010,          # 10ms
    '/upp/v1/markets': 3.0,    # 3 seconds
    # ... more thresholds
}
```

**ws_stress.py:**
```python
GATEWAY_URL = "http://localhost:8080"
WS_URL = "ws://localhost:8080"
DEFAULT_NUM_CLIENTS = 20       # concurrent clients
DEFAULT_DURATION = 60          # seconds
```

## Expected Results

### Passing Gateway

All tests should pass when the gateway:

1. **Completes concurrent market requests within 6 seconds** (provider aggregation latency)
2. **Implements rate limiting** (429 responses after threshold)
3. **Handles 50 concurrent connections** without errors
4. **Rejects oversized/malformed requests** with appropriate 4xx responses
5. **Health checks consistently under 10ms**
6. **Market data endpoints under 3s (P95)**
7. **WebSockets support 20+ concurrent subscriptions** with < 2s first message latency

### Interpreting Failures

| Failure | Likely Cause | Action |
|---------|--------------|--------|
| Timeout tests fail | Providers slow or unreachable | Check provider URLs, network connectivity |
| Rate limit tests fail | Rate limiting disabled | Enable rate limiting middleware |
| Latency SLA failures | Gateway overloaded or slow storage | Profile gateway, check resource usage |
| WebSocket tests fail | WebSocket handler issue | Check WebSocket implementation, inspect logs |
| Large payload tests fail | Request size limits too small | Verify request body size limit config |

## Troubleshooting

### "Gateway not responding at localhost:8080"

```bash
# Check if gateway is running
curl http://localhost:8080/health

# If not running, start it
python3 -m upp.gateway

# Check if running on different port
curl http://localhost:8000/health  # or whatever port
# Then update GATEWAY_URL in test scripts
```

### WebSocket tests skipped

```bash
# Install websockets library
pip install websockets

# Re-run tests
python3 ws_stress.py
```

### Tests hang or timeout

```bash
# Gateway may be unresponsive
pkill -f upp.gateway  # kill any hung processes

# Restart
python3 -m upp.gateway

# Check logs for errors
journalctl -f  # if running as service
```

### Rate limit tests never see 429

```bash
# Rate limiting may be disabled
# Check gateway configuration for rate limit settings
# Ensure REDIS or in-memory rate limit store is working
```

## Performance Tuning

### Running in parallel (faster, but less isolation)

Modify `run-all.sh`:

```bash
# Instead of sequential:
run_test "Chaos Testing" "chaos_test.py" &
run_test "Latency Bench" "latency_bench.py" &
run_test "WebSocket" "ws_stress.py" &

wait  # Wait for all background jobs
```

### Reduced test load (faster baseline tests)

Edit test scripts:

**chaos_test.py:**
```python
def test_provider_timeout_simulation():
    # Change 20 to 5
    threads = []
    for _ in range(5):  # was 20
        # ...
```

**latency_bench.py:**
```python
DEFAULT_ITERATIONS = 10  # was 100
```

**ws_stress.py:**
```python
DEFAULT_NUM_CLIENTS = 5     # was 20
DEFAULT_DURATION = 30       # was 60
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Gateway Chaos Tests

on: [push, pull_request]

jobs:
  chaos-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions/setup-python@v2
        with:
          python-version: '3.9'

      - name: Start Gateway
        run: python3 -m upp.gateway &

      - name: Wait for Gateway
        run: sleep 2 && curl http://localhost:8080/health

      - name: Install WebSocket Library
        run: pip install websockets

      - name: Run Chaos Tests
        run: ./tools/chaos/run-all.sh
```

## Output Files

After running tests:

- **latency_report.json** — Detailed latency metrics in JSON format
- **Console output** — Colored pass/fail results with detailed metrics

## Performance Benchmarks

### Expected Latencies (Healthy Gateway)

These are typical ranges on a modern system with 3 providers available:

| Endpoint | P50 | P95 | P99 |
|----------|-----|-----|-----|
| /health | 1-5ms | 5-10ms | 10-20ms |
| /markets | 500-1000ms | 1.5-2.5s | 2.5-3s |
| /markets/search | 600-1200ms | 1.8-2.8s | 2.8-3s |
| /mcp/tools | 5-15ms | 20-40ms | 40-50ms |
| /mcp/execute | 200-500ms | 500-900ms | 900ms-1s |
| /.well-known/agent.json | 1-5ms | 10-30ms | 30-50ms |

## File Structure

```
chaos/
├── chaos_test.py           # 300 lines - Chaos testing harness
├── latency_bench.py        # 200 lines - Latency benchmarking
├── ws_stress.py            # 200 lines - WebSocket stress testing
├── run-all.sh              # Master runner script
├── README.md               # This file
└── latency_report.json     # Generated after latency_bench.py runs
```

## License

Same as UPP gateway project

## Support

For issues or questions:

1. Check this README's troubleshooting section
2. Review test output for specific failure messages
3. Check gateway logs: `journalctl -u upp-gateway -f`
4. Verify gateway is running: `curl http://localhost:8080/health`
