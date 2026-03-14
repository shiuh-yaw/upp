#!/bin/bash
# Copyright 2026 Universal Prediction Protocol Authors
# SPDX-License-Identifier: Apache-2.0
#
# Docker smoke test for the UPP Gateway
# Tests basic endpoints and ensures the service is functional
#
# Usage: ./scripts/docker-smoke-test.sh
# Run from the UPP project root
#
# Note: chmod +x scripts/docker-smoke-test.sh to make executable

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Test results
declare -a TEST_RESULTS

# Cleanup function — ensure docker compose down runs on exit or failure
cleanup() {
    echo ""
    echo "Cleaning up Docker containers..."
    docker compose down -v || true
}

# Set up trap to ensure cleanup on exit or failure
trap cleanup EXIT

# Helper function to run a test
run_test() {
    local test_name="$1"
    local method="$2"
    local endpoint="$3"
    local expected_status="$4"
    local expected_key="${5:-}"

    TESTS_RUN=$((TESTS_RUN + 1))

    echo -n "Testing $test_name ... "

    local url="http://localhost:8080$endpoint"
    local response

    # Make the request and capture response
    response=$(curl -sf "$url" -w "\n%{http_code}" 2>/dev/null || true)

    # Split response and status code
    local status_code=$(echo "$response" | tail -n1)
    local body=$(echo "$response" | head -n-1)

    # Check status code
    if [ "$status_code" != "$expected_status" ]; then
        echo -e "${RED}FAIL${NC} (expected $expected_status, got $status_code)"
        TEST_RESULTS+=("$test_name: FAIL (status code)")
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi

    # Check response body if expected_key is provided
    if [ -n "$expected_key" ]; then
        if ! echo "$body" | grep -q "$expected_key"; then
            echo -e "${RED}FAIL${NC} (missing key: $expected_key)"
            TEST_RESULTS+=("$test_name: FAIL (missing key)")
            TESTS_FAILED=$((TESTS_FAILED + 1))
            return 1
        fi
    fi

    echo -e "${GREEN}PASS${NC}"
    TEST_RESULTS+=("$test_name: PASS")
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
}

# Main script
echo "=========================================="
echo "UPP Gateway Docker Smoke Test"
echo "=========================================="
echo ""

# Step 1: Start Redis
echo "Starting Redis..."
docker compose up -d redis || {
    echo -e "${RED}Failed to start Redis${NC}"
    exit 1
}

# Step 2: Wait for Redis to be healthy
echo "Waiting for Redis to be healthy..."
RETRY_COUNT=0
MAX_RETRIES=30
while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if docker compose exec redis redis-cli ping > /dev/null 2>&1; then
        echo -e "${GREEN}Redis is healthy${NC}"
        break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    sleep 1
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
    echo -e "${RED}Redis failed to become healthy${NC}"
    exit 1
fi

# Step 3: Build gateway Docker image
echo ""
echo "Building gateway Docker image..."
docker compose build gateway || {
    echo -e "${RED}Failed to build gateway image${NC}"
    exit 1
}

# Step 4: Start the gateway
echo "Starting gateway..."
docker compose up -d gateway || {
    echo -e "${RED}Failed to start gateway${NC}"
    exit 1
}

# Step 5: Wait for gateway to be healthy
echo "Waiting for gateway to be healthy..."
RETRY_COUNT=0
MAX_RETRIES=30
while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -sf http://localhost:8080/health > /dev/null 2>&1; then
        echo -e "${GREEN}Gateway is healthy${NC}"
        break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    sleep 1
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
    echo -e "${RED}Gateway failed to become healthy${NC}"
    exit 1
fi

# Step 6: Run smoke tests
echo ""
echo "Running smoke tests..."
echo "=========================================="

# Test 1: Health endpoint
run_test "GET /health" "GET" "/health" "200" "status"

# Test 2: Readiness endpoint
run_test "GET /ready" "GET" "/ready" "200" "ready"

# Test 3: Metrics endpoint
run_test "GET /metrics" "GET" "/metrics" "200"

# Test 4: Markets endpoint
run_test "GET /upp/v1/markets" "GET" "/upp/v1/markets" "200" "markets"

# Test 5: Discovery manifest endpoint
run_test "GET /upp/v1/discovery/manifest" "GET" "/upp/v1/discovery/manifest/kalshi" "200"

# Test 6: Status endpoint (HTML dashboard)
run_test "GET /status" "GET" "/status" "200"

# Print summary
echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo "Total tests run: $TESTS_RUN"
echo -e "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests failed: ${RED}$TESTS_FAILED${NC}"

# Print individual results
if [ ${#TEST_RESULTS[@]} -gt 0 ]; then
    echo ""
    echo "Results:"
    for result in "${TEST_RESULTS[@]}"; do
        if [[ "$result" == *"PASS"* ]]; then
            echo -e "  ${GREEN}✓${NC} $result"
        else
            echo -e "  ${RED}✗${NC} $result"
        fi
    done
fi

echo ""

# Exit with appropriate code
if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
