#!/bin/bash
set -e

# UPP Gateway k6 Load Test Runner
# This script orchestrates the load testing scenarios for the UPP gateway

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_URL="${BASE_URL:-http://localhost:8080}"
K6_BIN="${K6_BIN:-k6}"
RESULTS_DIR="${SCRIPT_DIR}/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Create results directory
mkdir -p "$RESULTS_DIR"

# Function to print colored output
print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# Function to check if k6 is installed
check_k6_installed() {
    print_header "Checking k6 Installation"

    if ! command -v "$K6_BIN" &> /dev/null; then
        print_error "k6 not found at $K6_BIN"
        echo "Please install k6 from: https://k6.io/docs/getting-started/installation/"
        exit 1
    fi

    K6_VERSION=$($K6_BIN version)
    print_success "k6 is installed: $K6_VERSION"
}

# Function to check gateway connectivity
check_gateway() {
    print_header "Checking Gateway Connectivity"

    if ! curl -sf --max-time 5 "${BASE_URL}/health" > /dev/null 2>&1; then
        print_error "Gateway not responding at ${BASE_URL}"
        echo "Please ensure the gateway is running on localhost:8080"
        exit 1
    fi

    print_success "Gateway is responding at ${BASE_URL}"
}

# Function to run a single scenario
run_scenario() {
    local scenario_name=$1
    local script_file=$2
    local output_file="${RESULTS_DIR}/${scenario_name}_${TIMESTAMP}.json"
    local summary_file="${RESULTS_DIR}/${scenario_name}_${TIMESTAMP}_summary.txt"

    print_header "Running Scenario: ${scenario_name}"

    # Run the test
    if SCENARIO="$scenario_name" BASE_URL="$BASE_URL" $K6_BIN run \
        --out json="$output_file" \
        --summary-export="$output_file" \
        "$script_file" 2>&1 | tee "$summary_file"; then
        print_success "Scenario '${scenario_name}' completed successfully"
        return 0
    else
        print_error "Scenario '${scenario_name}' failed"
        return 1
    fi
}

# Function to run REST API tests
run_rest_tests() {
    print_header "REST API Load Testing"

    local rest_script="${SCRIPT_DIR}/k6-rest.js"

    if [ ! -f "$rest_script" ]; then
        print_error "REST test script not found: $rest_script"
        return 1
    fi

    # Run smoke test
    print_header "Running REST Smoke Test"
    if SCENARIO="smoke" BASE_URL="$BASE_URL" $K6_BIN run \
        --vus 1 --duration 30s \
        "$rest_script" 2>&1 | tee "${RESULTS_DIR}/rest_smoke_${TIMESTAMP}.txt"; then
        print_success "REST Smoke test passed"
    else
        print_error "REST Smoke test failed"
        return 1
    fi

    sleep 5

    # Run load test
    print_header "Running REST Load Test"
    if SCENARIO="load" BASE_URL="$BASE_URL" $K6_BIN run \
        "$rest_script" 2>&1 | tee "${RESULTS_DIR}/rest_load_${TIMESTAMP}.txt"; then
        print_success "REST Load test passed"
    else
        print_warning "REST Load test encountered issues (may be acceptable)"
    fi

    sleep 5

    # Run stress test
    print_header "Running REST Stress Test"
    if SCENARIO="stress" BASE_URL="$BASE_URL" $K6_BIN run \
        "$rest_script" 2>&1 | tee "${RESULTS_DIR}/rest_stress_${TIMESTAMP}.txt"; then
        print_success "REST Stress test passed"
    else
        print_warning "REST Stress test encountered issues (may be acceptable)"
    fi
}

# Function to run WebSocket tests
run_websocket_tests() {
    print_header "WebSocket Load Testing"

    local ws_script="${SCRIPT_DIR}/k6-websocket.js"

    if [ ! -f "$ws_script" ]; then
        print_error "WebSocket test script not found: $ws_script"
        return 1
    fi

    # Run sustained test
    print_header "Running WebSocket Sustained Test"
    if BASE_URL="$BASE_URL" $K6_BIN run \
        "$ws_script" 2>&1 | tee "${RESULTS_DIR}/ws_sustained_${TIMESTAMP}.txt"; then
        print_success "WebSocket Sustained test passed"
    else
        print_warning "WebSocket Sustained test encountered issues"
    fi
}

# Function to run spike and soak tests
run_spike_soak_tests() {
    print_header "Spike and Soak Testing"

    local spike_script="${SCRIPT_DIR}/k6-spike.js"

    if [ ! -f "$spike_script" ]; then
        print_error "Spike test script not found: $spike_script"
        return 1
    fi

    # Run soak test
    print_header "Running Soak Test"
    if SCENARIO="soak" BASE_URL="$BASE_URL" $K6_BIN run \
        "$spike_script" 2>&1 | tee "${RESULTS_DIR}/soak_${TIMESTAMP}.txt"; then
        print_success "Soak test passed"
    else
        print_warning "Soak test encountered issues"
    fi

    sleep 10

    # Run spike test
    print_header "Running Spike Test"
    if SCENARIO="spike" BASE_URL="$BASE_URL" $K6_BIN run \
        "$spike_script" 2>&1 | tee "${RESULTS_DIR}/spike_${TIMESTAMP}.txt"; then
        print_success "Spike test passed"
    else
        print_warning "Spike test encountered issues"
    fi
}

# Function to display results summary
display_summary() {
    print_header "Test Summary"

    echo "Test results saved to: $RESULTS_DIR"
    echo ""
    echo "Files generated:"
    ls -lh "$RESULTS_DIR"/*_${TIMESTAMP}* 2>/dev/null | awk '{print "  " $9}'
}

# Function to show usage
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

OPTIONS:
    --scenario SCENARIO    Run only a specific scenario
                          Available: smoke, load, stress, spike, soak, ws-sustained, ws-spike
    --url URL              Gateway URL (default: http://localhost:8080)
    --help                 Show this help message

EXAMPLES:
    # Run all tests
    $0

    # Run only smoke test
    $0 --scenario smoke

    # Run load test against custom URL
    $0 --scenario load --url http://gateway.example.com:8080

    # Run WebSocket tests
    $0 --scenario ws-sustained
EOF
}

# Parse command line arguments
SCENARIO=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --scenario)
            SCENARIO="$2"
            shift 2
            ;;
        --url)
            BASE_URL="$2"
            shift 2
            ;;
        --help)
            show_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Main execution
main() {
    print_header "UPP Gateway Load Testing Suite"
    echo "Gateway URL: $BASE_URL"
    echo "Results directory: $RESULTS_DIR"
    echo ""

    # Perform pre-flight checks
    check_k6_installed
    echo ""
    check_gateway
    echo ""

    if [ -n "$SCENARIO" ]; then
        # Run specific scenario
        case $SCENARIO in
            smoke|load|stress|spike)
                run_rest_tests
                ;;
            ws-sustained|ws-spike)
                run_websocket_tests
                ;;
            soak)
                run_spike_soak_tests
                ;;
            *)
                print_error "Unknown scenario: $SCENARIO"
                show_usage
                exit 1
                ;;
        esac
    else
        # Run all tests
        run_rest_tests
        echo ""
        run_websocket_tests
        echo ""
        run_spike_soak_tests
    fi

    echo ""
    display_summary
    echo ""
    print_success "Load testing completed!"
}

# Run main
main "$@"
