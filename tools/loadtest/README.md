# UPP Gateway k6 Load Testing Suite

Comprehensive load testing suite for the Universal Prediction Protocol (UPP) prediction market aggregation gateway. This suite includes REST API, WebSocket, spike, and soak tests using k6.

## Quick Start

### Installation

1. **Install k6**

   On macOS with Homebrew:
   ```bash
   brew install k6
   ```

   On Linux (Ubuntu/Debian):
   ```bash
   sudo apt-get update
   sudo apt-get install k6
   ```

   On Windows with Chocolatey:
   ```bash
   choco install k6
   ```

   Or visit [k6 installation guide](https://k6.io/docs/getting-started/installation/) for other methods.

2. **Verify installation**
   ```bash
   k6 version
   ```

### Running the Tests

**Start the UPP Gateway** (ensure it's running on `localhost:8080` and `localhost:50051`)

**Run all tests**
```bash
./run-all.sh
```

**Run specific scenario**
```bash
./run-all.sh --scenario smoke
./run-all.sh --scenario load
./run-all.sh --scenario stress
./run-all.sh --scenario spike
./run-all.sh --scenario soak
./run-all.sh --scenario ws-sustained
./run-all.sh --scenario ws-spike
```

**Run against custom gateway URL**
```bash
./run-all.sh --url http://gateway.example.com:8080
```

## Test Scenarios

### REST API Tests (`k6-rest.js`)

**Smoke Test** (baseline, minimal load)
- Duration: 30 seconds
- Virtual Users (VUs): 1
- Purpose: Verify basic functionality and establish baseline latency
- Endpoints tested: All REST endpoints
- Success criteria: All endpoints respond with <100ms for static endpoints

**Load Test** (realistic production load)
- Duration: 6 minutes total
  - Ramp up: 2 minutes (0→50 VUs)
  - Sustain: 3 minutes (constant 50 VUs)
  - Ramp down: 1 minute (50→0 VUs)
- Purpose: Verify gateway handles sustained production-like load
- Success criteria:
  - Market endpoints: p95 < 2000ms
  - Static endpoints: p95 < 100ms
  - Error rate: < 5%

**Stress Test** (push to limits)
- Duration: 2 minutes
- Virtual Users (VUs): 100 (constant)
- Purpose: Find the breaking point and performance degradation
- Success criteria:
  - Error rate: < 5%
  - System stays responsive

**Spike Test** (sudden traffic surge)
- Duration: ~50 seconds
  - Spike: 0→200 VUs in 10s
  - Sustain: 30 seconds at 200 VUs
  - Recovery: 10 seconds (200→0 VUs)
- Purpose: Test gateway resilience to sudden load increases
- Success criteria:
  - Graceful degradation
  - Recovery within 30 seconds of spike end

### WebSocket Tests (`k6-websocket.js`)

**Sustained Test**
- Duration: 2 minutes
- Concurrent connections: 10
- Purpose: Verify steady-state WebSocket functionality
- Features:
  - Subscribes to 5 market price updates
  - Subscribes to 3 orderbook updates
  - Sends periodic pings and verifies pongs
  - Tracks message receive rate and latency

**Spike Test**
- Duration: ~45 seconds
  - Spike: 0→50 concurrent connections in 5s
  - Sustain: 30 seconds
  - Recovery: 10 seconds (50→0)
- Purpose: Test WebSocket handling under connection surge
- Metrics tracked:
  - Connection establishment time
  - Message latency from subscription to first data
  - Reconnection success rate

### Spike and Soak Tests (`k6-spike.js`)

**Soak Test** (extended endurance)
- Duration: 10 minutes
- Virtual Users (VUs): 20 (constant)
- Purpose: Detect memory leaks and stability issues over time
- Checks performed:
  - Memory usage tracking every 30 seconds
  - Memory leak detection (>25% increase flagged)
  - Endpoint availability and response time consistency
  - Error rate stability

**Spike Test** (extreme load)
- Duration: ~40 seconds
  - Spike: 0→500 VUs in 5s
  - Sustain: 30 seconds at 500 VUs
  - Recovery: 5 seconds (500→0 VUs)
- Purpose: Test recovery and graceful degradation under extreme load
- Metrics:
  - Error rate spike tracking
  - Recovery time measurement
  - Request counter and memory monitoring

## Endpoints Tested

### REST Endpoints

| Endpoint | Method | Purpose | Latency SLA |
|----------|--------|---------|------------|
| `/health` | GET | Gateway health check | p95 < 100ms |
| `/upp/v1/markets` | GET | List active markets | p95 < 2000ms |
| `/upp/v1/markets/search` | GET | Search markets by query | p95 < 2000ms |
| `/upp/v1/markets/categories` | GET | List market categories | p95 < 500ms |
| `/upp/v1/mcp/execute` | POST | Execute MCP tools | p95 < 2500ms |
| `/upp/v1/orders/estimate` | POST | Estimate order costs | p95 < 500ms |
| `/.well-known/agent.json` | GET | Agent configuration | p95 < 100ms |
| `/upp/v1/mcp/tools` | GET | List available MCP tools | p95 < 100ms |

### WebSocket Endpoint

| Endpoint | Purpose |
|----------|---------|
| `ws://localhost:8080/upp/v1/ws` | Price updates, orderbook updates, subscriptions |

### Monitoring Endpoint

| Endpoint | Purpose |
|----------|---------|
| `/metrics` | Prometheus-format metrics for memory, connections, requests |

## Performance Thresholds

### Response Time (Latency)

| Category | p95 | p99 |
|----------|-----|-----|
| Health / Static | <100ms | <200ms |
| Market APIs | <2000ms | <3000ms |
| Compute (execute) | <2500ms | <4000ms |
| WebSocket connection | <2000ms | <4000ms |
| WebSocket ping-pong | <500ms | <1000ms |

### Error Rates

| Scenario | Threshold | Rationale |
|----------|-----------|-----------|
| Smoke | <1% | Should be perfect with 1 VU |
| Load | <5% | Normal operation allows minor errors |
| Stress | <5% | Acceptable during stress testing |
| Spike | <15% | Graceful degradation expected |
| Soak | <10% | Extended run may accumulate minor issues |

### Recovery

| Metric | Threshold |
|--------|-----------|
| Recovery time after spike | <5000ms |
| Memory leak detection | >25% increase flagged |

## Custom Metrics

### REST Tests

- **`health_latency`**: Response time for health check endpoint
- **`market_latency`**: Response time for market list endpoint
- **`search_latency`**: Response time for market search
- **`categories_latency`**: Response time for categories endpoint
- **`execute_latency`**: Response time for MCP execute
- **`estimate_latency`**: Response time for order estimate
- **`static_latency`**: Response time for static endpoints
- **`error_rate`**: HTTP error rate (non-2xx responses)
- **`cache_hit_rate`**: Cache hit ratio based on response headers

### WebSocket Tests

- **`ws_connection_time`**: Time to establish WebSocket connection
- **`ws_message_latency`**: Latency from subscription to first message
- **`ws_messages_per_second`**: Message throughput per connection
- **`ws_reconnect_count`**: Successful reconnection attempts
- **`ws_ping_pong_latency`**: Round-trip latency for ping-pong
- **`ws_connection_errors`**: Failed connection attempts
- **`ws_message_errors`**: Malformed or unparseable messages

### Spike/Soak Tests

- **`soak_error_rate`**: Error rate during soak test
- **`spike_error_rate`**: Error rate during spike test
- **`recovery_time_ms`**: Time to recover after spike
- **`memory_usage_mb`**: Process memory usage
- **`ws_connection_count`**: Active WebSocket connections
- **`memory_leak_detected`**: Flag for potential memory leaks

## Interpreting Results

### Success Criteria

A test run is considered successful when:

1. **All thresholds pass**: k6 evaluates thresholds and reports pass/fail
2. **No critical errors**: HTTP 5xx errors or connection failures
3. **Latency within SLA**: p95 and p99 latencies meet thresholds
4. **Error rate acceptable**: Depends on scenario (0-15%)
5. **Recovery verified**: System returns to normal after load

### Reading k6 Output

```
scenarios: (100.00%) 1 scenario, 50 max VUs, 6m30s max duration (incl. graceful stop)
  ✓ load

    ✓ health status is 200
    ✓ health response has content
    ✓ health responds quickly

    checks.....................: 95.21% ✓ 3000 ✗ 145
    data_received..............: 2.5 MB ✓
    data_sent..................: 1.2 MB ✓
    dropped_iterations.........: 0 ✓
    errors......................: 0 ✓
    http_req_blocked...........: avg=45µs min=10µs max=2.3ms p(90)=87µs p(95)=120µs
    http_req_connecting........: avg=8µs min=0s max=1.2ms p(90)=12µs p(95)=20µs
    http_req_duration..........: avg=125ms min=15ms max=2.5s p(90)=200ms p(95)=400ms
    http_req_failed............: 1.20% ✗ 60
    http_req_receiving.........: avg=5ms min=1ms max=50ms p(90)=8ms p(95)=12ms
    http_req_sending...........: avg=1ms min=0s max=10ms p(90)=2ms p(95)=3ms
    http_req_tls_handshaking...: avg=0s min=0s max=0s p(90)=0s p(95)=0s
    http_req_waiting...........: avg=119ms min=10ms max=2.4s p(90)=190ms p(95)=385ms
    http_requests..............: 5000 ✓
    iteration_duration.........: avg=4.5s min=2.1s max=12.3s p(90)=6.2s p(95)=7.1s
    iterations.................: 5000 ✓
    vus..........................: 1 min, 50 max
    vus_max......................: 50 ✓

PASSED [ 95%] ✓ 3000
FAILED [  5%] ✗ 145
```

Key metrics to focus on:

- **Checks**: Should be >95% passing
- **http_req_failed**: Should be <5% (varies by scenario)
- **p(95) latency**: Compare to SLA thresholds
- **data_received/sent**: Verify no unexpected data sizes
- **errors**: Should be 0 or very low

### Common Issues and Solutions

**High Error Rate (>5%)**
- Check gateway logs for errors
- Verify backend provider connectivity (Kalshi, Polymarket, etc.)
- Check for rate limiting from external APIs
- Increase timeout values if experiencing network delays

**Slow Response Times (p95 > SLA)**
- Check gateway resource usage (CPU, memory)
- Look for slow external API calls
- Check network latency to prediction market providers
- Enable caching if not already enabled

**Memory Leaks Detected**
- Check for connection pool leaks
- Review log output for connection closure errors
- Monitor WebSocket connection cleanup
- Restart gateway if leak persists

**WebSocket Connection Failures**
- Verify WebSocket handler is enabled in gateway
- Check for connection limits in load balancer
- Review WebSocket heartbeat/timeout settings
- Ensure gateway has sufficient file descriptor limits

## Running Custom Scenarios

### Modify VU Count

Edit the scenario in the test file:

```javascript
load: {
  executor: 'ramping-vus',
  stages: [
    { duration: '2m', target: 100 },  // Change to 100 VUs
    { duration: '3m', target: 100 },
    { duration: '1m', target: 0 },
  ],
}
```

### Test Specific Endpoints

Use the `--scenario` flag or modify endpoint tests in the script.

### Increase Test Duration

Modify stage durations in the scenarios object.

### Add Custom Checks

Add checks in the test groups:

```javascript
check(res, {
  'custom check': (r) => r.body.includes('expected_text'),
});
```

## Performance Baseline

Typical performance baselines (from reference implementation):

| Scenario | Endpoint | p95 Latency | p99 Latency | Error Rate |
|----------|----------|-------------|-------------|-----------|
| Smoke | Health | 12ms | 18ms | 0% |
| Smoke | Markets | 85ms | 120ms | 0% |
| Load | Health | 15ms | 25ms | 0% |
| Load | Markets | 380ms | 650ms | <1% |
| Load | Search | 420ms | 750ms | <1% |
| Stress | Health | 45ms | 100ms | 1-2% |
| Stress | Markets | 1200ms | 1800ms | 3-4% |
| Spike | Any | variable | variable | 5-10% |
| Soak | All | stable | stable | <1% |

## Advanced Usage

### Collecting Metrics for Analysis

```bash
# Export results to JSON
k6 run --out json=results.json k6-rest.js

# Use cloud integration
k6 run --cloud k6-rest.js

# Export summary
k6 run --summary-export=summary.json k6-rest.js
```

### Profiling with Flame Graphs

```bash
# Requires k6 with profiling extension
k6 run --profiling-enabled k6-rest.js
```

### Distributed Testing

For load from multiple machines:

```bash
# On cloud (requires k6 account)
k6 run --cloud --vus 1000 --duration 5m k6-rest.js
```

## Troubleshooting

### Gateway Connection Issues

```bash
# Test connectivity manually
curl -v http://localhost:8080/health

# Check if port is listening
netstat -tlnp | grep 8080
```

### High False Failure Rate

- Increase timeout values
- Check gateway resource usage
- Verify no rate limiting from providers
- Check network connectivity to providers

### Inconsistent Results

- Run multiple times to get average performance
- Ensure no other load on the system
- Check for background processes
- Verify stable network connectivity

## Architecture

```
loadtest/
├── k6-rest.js          # REST API load tests
├── k6-websocket.js     # WebSocket stress tests
├── k6-spike.js         # Spike and soak tests
├── run-all.sh          # Test orchestration script
├── README.md           # This file
└── results/            # Test results (generated)
    ├── rest_smoke_*.txt
    ├── rest_load_*.txt
    ├── rest_stress_*.txt
    ├── ws_sustained_*.txt
    ├── spike_*.txt
    └── soak_*.txt
```

## References

- [k6 Documentation](https://k6.io/docs/)
- [k6 HTTP Module](https://k6.io/docs/javascript-api/k6-http/)
- [k6 WebSocket Module](https://k6.io/docs/javascript-api/k6-ws/)
- [k6 Metrics](https://k6.io/docs/javascript-api/k6-metrics/)
- [k6 Checks](https://k6.io/docs/javascript-api/k6/#checks)
- [UPP Gateway Documentation](../README.md)

## License

Same as UPP Gateway project
