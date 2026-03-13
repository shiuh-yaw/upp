# Quick Start Guide

## 60-Second Setup

### 1. Start Gateway

```bash
python3 -m upp.gateway
# Gateway now listening on http://localhost:8080
```

### 2. Install WebSocket Support (Optional)

```bash
pip install websockets
```

### 3. Run All Tests

```bash
cd /path/to/chaos
./run-all.sh
```

## What You'll Get

### Colored Output
- **✓ GREEN** = Test passed
- **✗ RED** = Test failed

### Three Test Suites

#### 1. Chaos Testing (2-3 minutes)
Tests gateway resilience:
- Provider timeout simulation (20 concurrent requests)
- Rate limit exhaustion (100 rapid requests)
- Concurrent connection stress (50 threads × 30 seconds)
- Large payload handling (1MB+ bodies)
- Malformed request handling (SQL injection, bad JSON)
- Rapid reconnect health checks (1000 sequential)

#### 2. Latency Benchmarking (1-2 minutes)
Measures SLA compliance:
- `/health` — Should be < 10ms
- `/markets` — Should be < 3s (P95)
- `/markets/search` — Should be < 3s (P95)
- `/mcp/tools` — Should be < 50ms (P95)
- `/mcp/execute` — Should be < 1s (P95)
- `/.well-known/agent.json` — Should be < 50ms (P95)

Auto-generates `latency_report.json` with detailed metrics.

#### 3. WebSocket Stress Testing (1-2 minutes)
Tests real-time subscriptions:
- 20 concurrent clients connecting
- Each subscribes to 3 markets
- Measures connection time, first message latency, throughput
- Tests reconnection behavior

## Individual Commands

```bash
# Just chaos testing
python3 chaos_test.py

# Just latency benchmarking
python3 latency_bench.py

# Just WebSocket tests
python3 ws_stress.py
```

## Expected Results

### Passing Tests Look Like:

```
✓ PASS  All requests complete within timeout threshold
         Max: 5.32s, Avg: 3.41s, Exceeded: 0/20
✓ PASS  Partial results returned on provider partial failure
         Successful responses: 18/20
```

### Failing Tests Look Like:

```
✗ FAIL  All requests complete within timeout threshold
         Max: 7.12s, Avg: 4.51s, Exceeded: 5/20
```

## Common Issues

### Gateway not running?
```bash
curl http://localhost:8080/health
```

### WebSocket tests skip?
```bash
pip install websockets
```

### Port already in use?
```bash
# Change GATEWAY_URL in scripts to your port
# Edit: GATEWAY_URL = "http://localhost:8081"
```

### Tests hanging?
```bash
# Kill and restart gateway
pkill -f upp.gateway
python3 -m upp.gateway
```

## Exit Codes

```bash
./run-all.sh
echo $?  # 0 = all pass, 1 = any fail
```

## Output Files

After running:
- `latency_report.json` — JSON metrics (for parsing, CI/CD integration)

## Typical Run Times

- **Chaos Testing**: 2-3 minutes
- **Latency Benchmarking**: 1-2 minutes (100 iterations per endpoint)
- **WebSocket Tests**: 1-2 minutes (60s test duration)
- **Total**: 4-7 minutes for full suite

## Gateway Configuration Checklist

Before running tests, ensure:

- [ ] Gateway running on `localhost:8080`
- [ ] All 3 providers configured (kalshi.com, polymarket.com, opinion.trade)
- [ ] Provider timeout set to 5 seconds
- [ ] Rate limiting enabled (for rate limit tests)
- [ ] WebSocket endpoint available at `/ws` (for WS tests)
- [ ] `/metrics` endpoint available (for monitoring)

## Interpreting Results

| All Pass | Status |
|----------|--------|
| ✓✓✓ | Gateway production-ready |
| ✓✓✗ | Gateway OK, WS tests skipped (websockets not installed) |
| ✓✗✓ | Latency SLA issues, investigate |
| ✗✓✓ | Chaos handling issues, check error responses |
| ✗✗✗ | Gateway needs debugging, check logs |

## Next Steps

1. **Review Results**: Check console output and `latency_report.json`
2. **Address Failures**: Use README.md troubleshooting section
3. **Optimize**: Tune gateway config based on bottlenecks
4. **Monitor**: Run regularly in CI/CD pipeline
5. **Trending**: Track `latency_report.json` over time for performance regression

## More Info

See **README.md** for detailed documentation on each test.
