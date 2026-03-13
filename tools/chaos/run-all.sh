#!/bin/bash
# Run all chaos tests sequentially
# Exit code 0 if all pass, 1 if any fail

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHAOS_DIR="$SCRIPT_DIR"

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RESET='\033[0m'
BOLD='\033[1m'

# Counters
PASSED=0
FAILED=0
RESULTS=()

# Function to run a test
run_test() {
    local test_name="$1"
    local test_script="$2"

    echo ""
    echo -e "${BOLD}${BLUE}========================================================================${RESET}"
    echo -e "${BOLD}${BLUE}Running: $test_name${RESET}"
    echo -e "${BOLD}${BLUE}========================================================================${RESET}"

    if python3 "$CHAOS_DIR/$test_script"; then
        RESULTS+=("${GREEN}✓${RESET} $test_name")
        ((PASSED++))
    else
        RESULTS+=("${RED}✗${RESET} $test_name")
        ((FAILED++))
    fi
}

# Print header
echo ""
echo -e "${BOLD}${BLUE}========================================================================${RESET}"
echo -e "${BOLD}${BLUE}              UPP Gateway Chaos Testing Suite - Full Run${RESET}"
echo -e "${BOLD}${BLUE}========================================================================${RESET}"
echo ""
echo "Executing all chaos and benchmark tests sequentially..."
echo ""

# Run all tests
run_test "Chaos Testing Harness" "chaos_test.py"
run_test "Latency Benchmarking" "latency_bench.py"
run_test "WebSocket Stress Test" "ws_stress.py"

# Print summary
echo ""
echo -e "${BOLD}${BLUE}========================================================================${RESET}"
echo -e "${BOLD}${BLUE}                           SUMMARY${RESET}"
echo -e "${BOLD}${BLUE}========================================================================${RESET}"
echo ""

for result in "${RESULTS[@]}"; do
    echo -e "$result"
done

echo ""
echo -e "${BOLD}Results: ${GREEN}${PASSED} passed${RESET}, ${RED}${FAILED} failed${RESET}${RESET}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${RESET}"
    echo ""
    exit 0
else
    echo -e "${RED}${FAILED} test(s) failed${RESET}"
    echo ""
    exit 1
fi
