# Operations & Deployment

This section covers everything needed to deploy, monitor, and operate UPP in production environments.

## Quick Overview

Operations involves:

1. **Deployment** — Getting UPP running in your infrastructure
2. **Monitoring** — Tracking health, performance, and errors
3. **Configuration** — Tuning for your specific needs

## Prerequisites

Before deploying to production:

- Docker & Docker Compose (for containerization)
- Kubernetes (optional, for orchestration)
- Prometheus (for metrics collection)
- Redis (for distributed caching)
- At least one prediction market API key (Kalshi, Polymarket, or Opinion.trade)

## Deployment Strategies

| Strategy | Best For | Complexity |
|----------|----------|-----------|
| Docker Compose | Local development, small deployments | Low |
| Single Container | Small production, single machine | Low |
| Kubernetes | High availability, multiple machines | High |
| Cloud-managed | Hands-off, scalable | Medium |

## Key Concepts

### Statelessness

The gateway is stateless except for WebSocket subscriptions. This enables:

- Horizontal scaling (add more instances)
- Load balancing (distribute requests)
- Zero-downtime deployments (rolling updates)

### External Dependencies

```
UPP Gateway
  ├─ Redis (distributed cache)
  ├─ Polymarket API (external)
  ├─ Kalshi API (external)
  └─ Opinion.trade API (external)
```

Redis is the only required dependency. Exchange APIs are called on-demand.

### Configuration

Controlled via environment variables or config files:

```bash
export RUST_LOG=info
export REDIS_URL=redis://localhost:6379
export KALSHI_API_KEY=...
export POLYMARKET_PRIVATE_KEY=0x...
```

## Pages in This Section

- **[Deployment](deployment.md)** — Docker, Kubernetes, cloud platforms
- **[Monitoring & Observability](monitoring.md)** — Prometheus, Grafana, Jaeger, logging
- **[Configuration](configuration.md)** — Environment variables, config files, secrets management

## Common Tasks

### Start Local Development

```bash
docker-compose up -d
```

### Deploy to Production

See [Deployment](deployment.md).

### Monitor System Health

See [Monitoring](monitoring.md).

### Configure Providers

See [Configuration](configuration.md).

## High Availability Checklist

For production deployments:

- [ ] Multiple gateway instances behind load balancer
- [ ] Redis cluster or managed Redis service
- [ ] Automated health checks and alerts
- [ ] Graceful shutdown on updates
- [ ] Structured logging to centralized system
- [ ] Metrics collection with Prometheus
- [ ] Distributed tracing with Jaeger
- [ ] Disaster recovery / backup plan

## Next Steps

- New to operations? Start with [Deployment](deployment.md)
- Need to monitor? See [Monitoring](monitoring.md)
- Tuning performance? Read [Configuration](configuration.md)
