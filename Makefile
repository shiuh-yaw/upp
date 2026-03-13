# UPP — Universal Prediction Protocol
# Build, test, and generate commands.

# Ensure Cargo (Rust) is on PATH when running gateway targets
export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: all proto lint breaking gen test gateway clean integration-test smoke-test

# ── Full build pipeline ──────────────────────────────────────
all: lint proto gen test

# ── Protobuf ─────────────────────────────────────────────────

# Lint .proto files with Buf
lint:
	cd proto && buf lint

# Check for breaking changes against main branch
breaking:
	cd proto && buf breaking --against '.git#branch=main'

# Generate code from .proto files (Rust, TS, Go, Python, JSON Schema, OpenAPI)
proto:
	cd proto && buf generate

# ── Gateway (Rust) ───────────────────────────────────────────

# Build the gateway binary
gateway:
	cd gateway && cargo build --release

# Run the gateway in development mode
gateway-dev:
	cd gateway && RUST_LOG=upp_gateway=debug,tower_http=debug cargo run

# Run gateway tests
gateway-test:
	cd gateway && cargo test

# Run gateway benchmarks
gateway-bench:
	cd gateway && cargo bench

# Format Rust code
gateway-fmt:
	cd gateway && cargo fmt

# Lint Rust code
gateway-lint:
	cd gateway && cargo clippy -- -D warnings

# ── Conformance Tests ────────────────────────────────────────

# Install conformance test dependencies
conformance-install:
	cd conformance && npm install

# Run all conformance tests
test: conformance-install
	cd conformance && npm test

# Run schema conformance tests only
test-schema:
	cd conformance && npm run test:schema

# Run behavioral conformance tests only
test-behavior:
	cd conformance && npm run test:behavior

# Validate golden fixtures against schemas
validate-fixtures:
	cd conformance && npm run validate:fixtures

# ── SDKs ─────────────────────────────────────────────────────

# Build TypeScript SDK
sdk-ts:
	cd sdk/typescript && npm install && npm run build

# Build Python SDK
sdk-py:
	cd sdk/python && pip install -e .

# ── Docker ───────────────────────────────────────────────────

# Build gateway Docker image
docker-build:
	docker build -t upp-gateway:latest -f gateway/Dockerfile .

# Run gateway with Docker Compose (includes Redis)
docker-up:
	docker compose up -d

docker-down:
	docker compose down

# ── Tools ──────────────────────────────────────────────────

# Run the UPP CLI demo (queries live public APIs)
cli:
	python3 tools/upp_cli.py $(ARGS)

# List markets from all providers
cli-markets:
	python3 tools/upp_cli.py markets

# Search markets
cli-search:
	python3 tools/upp_cli.py search "$(Q)"

# Health check all providers
cli-health:
	python3 tools/upp_cli.py health

# Dump UPP JSON from all providers
cli-dump:
	python3 tools/upp_cli.py dump --limit 20

# Start the mock server for local dev (trading, portfolio)
mock-server:
	python3 tools/mock_server.py

# Run integration tests against a running gateway
integration-test:
	python3 tools/test_gateway.py $(UPP_GATEWAY_URL)

# Quick smoke test (start gateway, run tests, stop)
smoke-test: gateway
	@echo "Starting gateway..."
	cd gateway && cargo run &
	@sleep 3
	python3 tools/test_gateway.py || true
	@kill %1 2>/dev/null || true

# ── Docker (with profiles) ──────────────────────────────────

# Start with mock server for testing
docker-mock:
	docker compose --profile mock up -d

# Start with monitoring (Prometheus + Grafana)
docker-monitoring:
	docker compose --profile monitoring up -d

# ── Cleanup ──────────────────────────────────────────────────
clean:
	cd gateway && cargo clean
	rm -rf schemas/json/*.json schemas/openapi/*.json
	rm -rf sdk/typescript/src/gen sdk/python/src/gen
	rm -rf gen/
