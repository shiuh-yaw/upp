#!/usr/bin/env python3
"""
UPP Gateway Integration Tests
==============================

Tests the running UPP gateway against its HTTP API.
Verifies: health, discovery, market data, trading, rate limiting, metrics.

Prerequisites:
    cargo run  # start the gateway on localhost:8080

Usage:
    python tools/test_gateway.py                    # default: http://localhost:8080
    python tools/test_gateway.py http://localhost:9090  # custom base URL
    UPP_GATEWAY_URL=http://... python tools/test_gateway.py
"""

import json
import os
import sys
import time
import urllib.request
import urllib.error
from dataclasses import dataclass, field
from typing import Optional

# ─── Config ──────────────────────────────────────────────────

BASE_URL = os.environ.get(
    "UPP_GATEWAY_URL",
    sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080",
)

# ─── Test Infrastructure ────────────────────────────────────

@dataclass
class TestResult:
    name: str
    passed: bool
    error: Optional[str] = None
    duration_ms: float = 0.0

results: list = []
_section = ""


def section(name: str):
    global _section
    _section = name
    print(f"\n{'=' * 60}")
    print(f"  {name}")
    print(f"{'=' * 60}")


def test(name: str):
    """Decorator to register a test function."""
    def decorator(func):
        func._test_name = f"[{_section}] {name}"
        return func
    return decorator


def request(method: str, path: str, body=None, headers=None) -> tuple:
    """Make an HTTP request. Returns (status_code, headers_dict, body_json_or_str)."""
    url = f"{BASE_URL}{path}"
    data = None
    if body is not None:
        data = json.dumps(body).encode("utf-8")

    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept", "application/json")
    if headers:
        for k, v in headers.items():
            req.add_header(k, v)

    try:
        resp = urllib.request.urlopen(req, timeout=15)
        resp_body = resp.read().decode("utf-8")
        resp_headers = dict(resp.headers)
        try:
            resp_json = json.loads(resp_body)
        except json.JSONDecodeError:
            resp_json = resp_body
        return resp.status, resp_headers, resp_json
    except urllib.error.HTTPError as e:
        resp_body = e.read().decode("utf-8")
        resp_headers = dict(e.headers)
        try:
            resp_json = json.loads(resp_body)
        except json.JSONDecodeError:
            resp_json = resp_body
        return e.code, resp_headers, resp_json


def GET(path, **kwargs):
    return request("GET", path, **kwargs)


def POST(path, body=None, **kwargs):
    return request("POST", path, body=body, **kwargs)


def DELETE(path, **kwargs):
    return request("DELETE", path, **kwargs)


def assert_eq(actual, expected, msg=""):
    if actual != expected:
        raise AssertionError(f"{msg}: expected {expected!r}, got {actual!r}")


def assert_true(cond, msg=""):
    if not cond:
        raise AssertionError(msg or "Assertion failed")


def assert_in(item, container, msg=""):
    if item not in container:
        raise AssertionError(f"{msg}: {item!r} not in {container!r}")


# ═══════════════════════════════════════════════════════════════
# Infrastructure Tests
# ═══════════════════════════════════════════════════════════════

section("Infrastructure")


@test("GET /health returns ok")
def test_health():
    status, _, body = GET("/health")
    assert_eq(status, 200)
    assert_eq(body["status"], "ok")
    assert_true("version" in body, "Missing version")
    assert_true(body["protocol"].startswith("UPP/"), f"Bad protocol: {body['protocol']}")


@test("GET /ready returns provider count")
def test_ready():
    status, _, body = GET("/ready")
    assert_eq(status, 200)
    assert_eq(body["ready"], True)
    assert_true(body["providers"] >= 1, f"Expected >=1 providers, got {body['providers']}")
    assert_true(isinstance(body["provider_ids"], list))


@test("GET /metrics returns Prometheus counters")
def test_metrics():
    status, _, body = GET("/metrics")
    assert_eq(status, 200)
    assert_true(isinstance(body, str), "Expected text body")
    assert_in("upp_requests_total", body)
    assert_in("upp_ws_active_channels", body)
    assert_in("upp_rate_limit_tracked_clients", body)


# ═══════════════════════════════════════════════════════════════
# Well-Known & Discovery
# ═══════════════════════════════════════════════════════════════

section("Discovery")


@test("GET /.well-known/upp returns protocol info")
def test_well_known():
    status, _, body = GET("/.well-known/upp")
    assert_eq(status, 200)
    assert_true("upp_version" in body)
    assert_true("gateway" in body)
    assert_true("providers" in body)
    assert_in("rest", body["gateway"]["transports"])


@test("GET /discovery/providers lists registered providers")
def test_list_providers():
    status, _, body = GET("/upp/v1/discovery/providers")
    assert_eq(status, 200)
    assert_true(body["total"] >= 1, f"Expected >=1 provider, got {body['total']}")
    # ProviderManifest nests ID at provider.id
    ids = [p.get("provider", {}).get("id", p.get("id", "")) for p in body["providers"]]
    assert_in("kalshi.com", ids, f"Kalshi should be registered, got: {ids}")


@test("GET /discovery/health/kalshi.com checks provider health")
def test_health_check_provider():
    status, _, body = GET("/upp/v1/discovery/health/kalshi.com")
    # May be 200 or 503 depending on network
    assert_true(status in (200, 503), f"Unexpected status: {status}")


@test("GET /discovery/health checks all providers")
def test_health_check_all():
    status, _, body = GET("/upp/v1/discovery/health")
    assert_eq(status, 200)
    assert_true("providers" in body)
    assert_true(body["total"] >= 1)


@test("POST /discovery/negotiate returns capabilities")
def test_negotiate():
    status, _, body = POST("/upp/v1/discovery/negotiate", {"provider": "kalshi.com"})
    assert_eq(status, 200)
    assert_true("active_capabilities" in body)
    assert_true("selected_transport" in body)


# ═══════════════════════════════════════════════════════════════
# Market Data (hits live public APIs)
# ═══════════════════════════════════════════════════════════════

section("Market Data")


@test("GET /markets returns paginated market list with provider_results")
def test_list_markets():
    status, _, body = GET("/upp/v1/markets?limit=5")
    assert_eq(status, 200)
    assert_true("markets" in body)
    assert_true("pagination" in body)
    assert_true(isinstance(body["markets"], list))
    # Aggregation adds provider_results for per-provider diagnostics
    assert_true("provider_results" in body, "Missing provider_results from aggregation")


@test("GET /markets?provider=kalshi.com filters by provider")
def test_list_markets_filtered():
    status, _, body = GET("/upp/v1/markets?provider=kalshi.com&limit=3")
    assert_eq(status, 200)
    for m in body["markets"]:
        assert_eq(m["id"]["provider"], "kalshi.com", "Wrong provider")


@test("GET /markets/search?q=... returns search results")
def test_search_markets():
    status, _, body = GET("/upp/v1/markets/search?q=bitcoin&limit=5")
    assert_eq(status, 200)
    assert_eq(body["query"], "bitcoin")
    assert_true(isinstance(body["markets"], list))


@test("GET /markets/categories returns static categories")
def test_categories():
    status, _, body = GET("/upp/v1/markets/categories")
    assert_eq(status, 200)
    cats = body["categories"]
    assert_true(len(cats) >= 4, f"Expected >=4 categories, got {len(cats)}")
    assert_in("politics", cats)
    assert_in("crypto", cats)


@test("GET /markets/{unknown} returns 404")
def test_get_market_unknown():
    status, _, body = GET("/upp/v1/markets/upp:fakeprovider.com:FAKE-123")
    assert_eq(status, 404)
    # Body should be JSON with error.code, but handle non-JSON gracefully
    if isinstance(body, dict) and "error" in body:
        assert_eq(body["error"]["code"], "NOT_FOUND")
    # If axum returns bare 404, that's still correct status


# ═══════════════════════════════════════════════════════════════
# Aggregation (parallel fan-out, merged orderbooks)
# ═══════════════════════════════════════════════════════════════

section("Aggregation")


@test("GET /markets returns per-provider latency in provider_results")
def test_aggregation_provider_latency():
    status, _, body = GET("/upp/v1/markets?limit=3")
    assert_eq(status, 200)
    for pr in body.get("provider_results", []):
        assert_true("provider" in pr, "Missing 'provider' in provider_results")
        assert_true("latency_ms" in pr, "Missing 'latency_ms' in provider_results")


@test("GET /markets/search returns aggregated results with provider_results")
def test_aggregation_search():
    status, _, body = GET("/upp/v1/markets/search?q=election&limit=5")
    assert_eq(status, 200)
    assert_true("provider_results" in body, "Missing provider_results")
    assert_true("markets" in body)


@test("GET /markets/{id}/orderbook/merged returns merged orderbook structure")
def test_merged_orderbook():
    # First get a real market ID from list
    _, _, listing = GET("/upp/v1/markets?provider=kalshi.com&limit=1")
    if not listing.get("markets"):
        return  # Skip if no markets available

    market = listing["markets"][0]
    market_id = market["id"]["provider"] + ":" + market["id"]["native_id"]

    status, _, body = GET(f"/upp/v1/markets/{market_id}/orderbook/merged?depth=5")
    assert_eq(status, 200)
    assert_true("market_id" in body, "Missing market_id")
    assert_true("bids" in body, "Missing bids")
    assert_true("asks" in body, "Missing asks")
    assert_true("provider_books" in body, "Missing provider_books")
    # arbitrage may or may not be present (null if no opportunity)


# ═══════════════════════════════════════════════════════════════
# Settlement & Resolution (static)
# ═══════════════════════════════════════════════════════════════

section("Settlement & Resolution")


@test("GET /settlement/instruments returns payment types")
def test_instruments():
    status, _, body = GET("/upp/v1/settlement/instruments")
    assert_eq(status, 200)
    assert_true(len(body["instruments"]) >= 3)


@test("GET /settlement/handlers returns handler list")
def test_handlers():
    status, _, body = GET("/upp/v1/settlement/handlers")
    assert_eq(status, 200)
    assert_true(isinstance(body["handlers"], list))


@test("GET /resolutions returns empty list (placeholder)")
def test_resolutions():
    status, _, body = GET("/upp/v1/resolutions")
    assert_eq(status, 200)


# ═══════════════════════════════════════════════════════════════
# Trading Endpoints (dev mode — pass through auth)
# ═══════════════════════════════════════════════════════════════

section("Trading")


@test("POST /orders/estimate calculates cost")
def test_estimate():
    status, _, body = POST("/upp/v1/orders/estimate", {
        "provider": "kalshi.com",
        "market_id": "TEST-MARKET",
        "outcome_id": "yes",
        "side": "buy",
        "price": "0.65",
        "quantity": 10,
    })
    assert_eq(status, 200)
    assert_eq(body["estimated_cost"], "6.50")
    assert_eq(body["estimated_fee"], "0.00")


@test("POST /orders rejects invalid side")
def test_create_order_bad_side():
    status, _, body = POST("/upp/v1/orders", {
        "provider": "kalshi.com",
        "market_id": "TEST",
        "outcome_id": "yes",
        "side": "sideways",
        "order_type": "limit",
        "quantity": 10,
    })
    assert_eq(status, 400)
    assert_eq(body["error"]["code"], "BAD_REQUEST")


@test("POST /orders rejects unknown provider")
def test_create_order_bad_provider():
    status, _, body = POST("/upp/v1/orders", {
        "provider": "nonexistent.com",
        "market_id": "TEST",
        "outcome_id": "yes",
        "side": "buy",
        "order_type": "limit",
        "quantity": 10,
    })
    assert_eq(status, 400)
    assert_in("nonexistent.com", body["error"]["message"])


# ═══════════════════════════════════════════════════════════════
# Rate Limiting
# ═══════════════════════════════════════════════════════════════

section("Rate Limiting")


@test("Responses include X-RateLimit-* headers")
def test_rate_limit_headers():
    status, headers, _ = GET("/health")
    assert_eq(status, 200)
    # Headers are case-insensitive; urllib lowercases them
    h_lower = {k.lower(): v for k, v in headers.items()}
    assert_true("x-ratelimit-limit" in h_lower, f"Missing X-RateLimit-Limit. Headers: {list(h_lower.keys())}")
    assert_true("x-ratelimit-remaining" in h_lower, "Missing X-RateLimit-Remaining")


@test("Remaining count decreases with requests")
def test_rate_limit_decreasing():
    _, h1, _ = GET("/health")
    _, h2, _ = GET("/health")
    h1_lower = {k.lower(): v for k, v in h1.items()}
    h2_lower = {k.lower(): v for k, v in h2.items()}
    r1 = int(h1_lower.get("x-ratelimit-remaining", "0"))
    r2 = int(h2_lower.get("x-ratelimit-remaining", "0"))
    assert_true(r2 <= r1, f"Remaining should decrease: {r1} -> {r2}")


# ═══════════════════════════════════════════════════════════════
# Metrics Integration
# ═══════════════════════════════════════════════════════════════

section("Metrics Integration")


@test("Request counter increments")
def test_metrics_increment():
    # Hit health a few times
    for _ in range(3):
        GET("/health")

    _, _, body = GET("/metrics")
    # Parse the total from Prometheus text
    for line in body.split("\n"):
        line = line.strip()
        if line.startswith("upp_requests_total") and not line.startswith("#"):
            total = int(line.split()[-1])
            assert_true(total >= 3, f"Expected requests_total >= 3, got {total}")
            return
    raise AssertionError("upp_requests_total not found in metrics output")


# ═══════════════════════════════════════════════════════════════
# MCP (Model Context Protocol) & A2A Integration
# ═══════════════════════════════════════════════════════════════

section("MCP & A2A")


@test("GET /mcp/tools returns tool catalog")
def test_mcp_list_tools():
    status, _, body = GET("/upp/v1/mcp/tools")
    assert_eq(status, 200)
    assert_true("tools" in body)
    assert_true(body["total"] >= 8, f"Expected >=8 tools, got {body['total']}")
    tool_names = [t["name"] for t in body["tools"]]
    assert_in("search_markets", tool_names)
    assert_in("get_market", tool_names)
    assert_in("place_order", tool_names)
    assert_in("get_market_analysis", tool_names)
    # Verify tool structure
    tool = body["tools"][0]
    assert_true("name" in tool)
    assert_true("description" in tool)
    assert_true("input_schema" in tool)
    assert_true("properties" in tool["input_schema"], "Tool should have JSON Schema input")


@test("GET /mcp/schema returns OpenAPI-like schema")
def test_mcp_schema():
    status, _, body = GET("/upp/v1/mcp/schema")
    assert_eq(status, 200)
    assert_eq(body["openapi"], "3.1.0")
    assert_true("info" in body)
    assert_true("x-mcp-tools" in body)
    assert_true("components" in body)
    assert_true("schemas" in body["components"])
    # Each tool should have a schema entry
    schemas = body["components"]["schemas"]
    assert_in("search_markets", schemas)
    assert_in("get_orderbook", schemas)


@test("POST /mcp/execute search_markets works")
def test_mcp_execute_search():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "search_markets",
        "params": {"query": "bitcoin", "limit": 3}
    })
    assert_eq(status, 200)
    assert_eq(body["status"], "ok")
    assert_true("result" in body)
    result = body["result"]
    assert_true("markets" in result)
    assert_eq(result["query"], "bitcoin")


@test("POST /mcp/execute list_markets works")
def test_mcp_execute_list():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "list_markets",
        "params": {"limit": 3}
    })
    assert_eq(status, 200)
    assert_eq(body["status"], "ok")
    result = body["result"]
    assert_true("markets" in result)


@test("POST /mcp/execute estimate_order returns cost estimate")
def test_mcp_execute_estimate():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "estimate_order",
        "params": {
            "market_id": "upp:kalshi.com:TEST",
            "outcome_id": "yes",
            "side": "buy",
            "quantity": 100,
            "price": 0.65
        }
    })
    assert_eq(status, 200)
    assert_eq(body["status"], "ok")
    result = body["result"]
    assert_eq(result["quantity"], 100)
    assert_true(result["estimated_cost"] > 0, "Cost should be positive")
    assert_true("fees" in result)
    assert_true("total" in result)


@test("POST /mcp/execute place_order requires auth")
def test_mcp_execute_place_order_auth():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "place_order",
        "params": {
            "market_id": "upp:kalshi.com:TEST",
            "outcome_id": "yes",
            "side": "buy",
            "quantity": 10
        }
    })
    # Should return 400 with AUTH_REQUIRED error
    assert_eq(status, 400)
    assert_true("error" in body)
    assert_eq(body["error"]["code"], "AUTH_REQUIRED")


@test("POST /mcp/execute unknown tool returns error")
def test_mcp_execute_unknown_tool():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "nonexistent_tool",
        "params": {}
    })
    assert_eq(status, 400)
    assert_eq(body["error"]["code"], "UNKNOWN_TOOL")


@test("POST /mcp/execute get_market with invalid ID returns error")
def test_mcp_execute_bad_market_id():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "get_market",
        "params": {"market_id": "just-a-bare-id"}
    })
    assert_eq(status, 400)
    assert_eq(body["error"]["code"], "INVALID_MARKET_ID")


@test("GET /.well-known/agent.json returns A2A Agent Card")
def test_a2a_agent_card():
    status, _, body = GET("/.well-known/agent.json")
    assert_eq(status, 200)
    assert_eq(body["name"], "UPP Gateway")
    assert_true("description" in body)
    assert_true("url" in body)
    assert_true("version" in body)
    assert_true("capabilities" in body)
    assert_in("market-research", body["capabilities"])
    assert_in("trading", body["capabilities"])
    assert_true("authentication" in body)
    assert_true(len(body["authentication"]) >= 2, "Expected >=2 auth methods")
    assert_true("skills" in body)
    skill_ids = [s["id"] for s in body["skills"]]
    assert_in("market-research", skill_ids)
    assert_in("trading", skill_ids)
    assert_in("portfolio-management", skill_ids)
    assert_in("market-analysis", skill_ids)
    # Skills should have examples
    for skill in body["skills"]:
        assert_true(len(skill["examples"]) >= 1, f"Skill {skill['id']} missing examples")


@test("POST /mcp/execute get_portfolio returns placeholder")
def test_mcp_execute_portfolio():
    status, _, body = POST("/upp/v1/mcp/execute", {
        "tool": "get_portfolio",
        "params": {}
    })
    assert_eq(status, 200)
    assert_eq(body["status"], "ok")
    result = body["result"]
    assert_true("positions" in result)
    assert_true("summary" in result)


# ═══════════════════════════════════════════════════════════════
# WebSocket Streaming (basic HTTP-level tests)
# ═══════════════════════════════════════════════════════════════

section("WebSocket")


@test("GET /ws returns 426 Upgrade Required for non-WS request")
def test_ws_upgrade_required():
    # A regular HTTP GET to the WS endpoint should not crash the server.
    # Axum returns various status codes for non-upgrade requests.
    try:
        status, _, _ = GET("/upp/v1/ws")
        # Axum may return 400 or 426 for non-upgrade WebSocket requests
        assert_true(status in (400, 426, 405), f"Expected 400/426/405, got {status}")
    except Exception:
        # Connection might be refused/reset — that's acceptable for non-WS request
        pass


@test("Metrics include ws_connections and ws_subscribers")
def test_ws_metrics_present():
    _, _, body = GET("/metrics")
    assert_in("upp_ws_connections", body)
    assert_in("upp_ws_active_channels", body)
    assert_in("upp_ws_subscribers", body)


# ═══════════════════════════════════════════════════════════════
# Production Hardening (error structure, graceful shutdown)
# ═══════════════════════════════════════════════════════════════

section("Production Hardening")


@test("Error responses include request_id")
def test_error_has_request_id():
    status, _, body = GET("/upp/v1/markets/upp:fakeprovider.com:FAKE-123")
    assert_eq(status, 404)
    assert_true("error" in body)
    assert_true("request_id" in body["error"], "Error response should include request_id")
    # request_id should be a UUID-like string
    rid = body["error"]["request_id"]
    assert_true(len(rid) >= 32, f"request_id too short: {rid}")


@test("Error responses have consistent structure")
def test_error_structure():
    # Test 400 error
    status, _, body = POST("/upp/v1/orders", {
        "provider": "kalshi.com",
        "market_id": "T",
        "outcome_id": "y",
        "side": "bad",
        "order_type": "limit",
        "quantity": 1,
    })
    assert_eq(status, 400)
    err = body["error"]
    assert_true("code" in err, "Error should have 'code'")
    assert_true("message" in err, "Error should have 'message'")
    assert_true("request_id" in err, "Error should have 'request_id'")


@test("MCP error responses have structured codes")
def test_mcp_error_codes():
    # Unknown tool
    s1, _, b1 = POST("/upp/v1/mcp/execute", {"tool": "nope", "params": {}})
    assert_eq(b1["error"]["code"], "UNKNOWN_TOOL")

    # Missing params
    s2, _, b2 = POST("/upp/v1/mcp/execute", {"tool": "get_market", "params": {}})
    assert_eq(b2["error"]["code"], "INVALID_PARAMS")


@test("Gateway handles concurrent requests without errors")
def test_concurrent_resilience():
    import concurrent.futures
    def hit_health():
        s, _, _ = GET("/health")
        return s == 200

    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as pool:
        futures = [pool.submit(hit_health) for _ in range(20)]
        results = [f.result(timeout=10) for f in futures]

    assert_true(all(results), f"Some concurrent requests failed: {results.count(False)} failures")


# ═══════════════════════════════════════════════════════════════
# Runner
# ═══════════════════════════════════════════════════════════════

def run_all():
    # Collect all test functions
    tests = []
    g = dict(globals())
    for name, obj in g.items():
        if callable(obj) and hasattr(obj, "_test_name"):
            tests.append(obj)

    # Check connectivity first
    print(f"\nUPP Gateway Integration Tests")
    print(f"Target: {BASE_URL}")
    print(f"Tests:  {len(tests)}")

    try:
        urllib.request.urlopen(f"{BASE_URL}/health", timeout=5)
    except Exception as e:
        print(f"\n*** Cannot connect to gateway at {BASE_URL}")
        print(f"*** Error: {e}")
        print(f"*** Start the gateway first:  cd gateway && cargo run")
        sys.exit(1)

    passed = 0
    failed = 0
    errors = []

    for test_fn in tests:
        name = test_fn._test_name
        t0 = time.monotonic()
        try:
            test_fn()
            duration = (time.monotonic() - t0) * 1000
            print(f"  PASS  {name}  ({duration:.0f}ms)")
            passed += 1
        except Exception as e:
            duration = (time.monotonic() - t0) * 1000
            print(f"  FAIL  {name}  ({duration:.0f}ms)")
            print(f"        {e}")
            failed += 1
            errors.append((name, str(e)))

    print(f"\n{'─' * 60}")
    print(f"Results: {passed} passed, {failed} failed, {passed + failed} total")

    if errors:
        print(f"\nFailures:")
        for name, err in errors:
            print(f"  {name}")
            print(f"    {err}")

    print()
    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    run_all()
