# Getting Started

Welcome! This section will get you up and running with UPP in minutes.

## What You'll Learn

- **Quickstart** — Spin up the local development stack and run your first query
- **Installation** — Build from source, use Docker, or install pre-built binaries
- **Next Steps** — Choose your integration path (REST API, gRPC, Rust SDK, or CLI)

## Prerequisites

Before you begin, ensure you have:

- **Docker & Docker Compose** — For the local development stack
- **Rust 1.70+** — If building from source (install via [rustup](https://rustup.rs/))
- **Git** — To clone the repository
- **A prediction market account** (optional) — Kalshi, Polymarket, or Opinion.trade API credentials to trade (not required for demo queries)

## Quick Overview

UPP provides multiple ways to interact with prediction markets:

| Interface | Best For | Latency |
|-----------|----------|---------|
| **REST API** | Web applications, general-purpose clients | 50-200ms |
| **gRPC** | High-performance backends, service-to-service | 10-50ms |
| **Rust SDK** | Native Rust applications, lowest overhead | <10ms |
| **CLI** | Ad-hoc queries, scripts, investigation | 100-500ms |
| **WebSocket** | Real-time market feeds, subscriptions | 10-100ms |

## Choose Your Path

**Just want to see it work?** → Start with [Quickstart](quickstart.md)

**Building a new application?** → Read [Installation](installation.md), then:
- REST API users → See [REST API Reference](../api/rest.md)
- Rust developers → See [Rust SDK Guide](../sdk/rust.md)
- Ops/DevOps teams → See [Deployment](../operations/deployment.md)

**Curious about how it works?** → Read [Architecture Overview](../architecture/overview.md)

## Development Environment

The simplest way to experiment with UPP is the local development stack:

```bash
docker-compose up -d
```

This brings up:
- **Gateway** — Main UPP server on `http://localhost:8080`
- **gRPC Server** — On port `50051`
- **Redis** — Caching and rate limiting
- **Prometheus** — Metrics collection
- **Grafana** — Visualization dashboards
- **Jaeger** — Distributed tracing

All services are pre-configured and talk to each other automatically. You don't need to touch the exchanges—the local stack uses mock providers that return realistic data.

Ready? Let's go to the [Quickstart](quickstart.md).
