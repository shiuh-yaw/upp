# Deployment Guide

This page covers deploying UPP to various environments from development to production.

## Docker Compose (Development)

Perfect for local development with all services:

```yaml
# docker-compose.yml (already in repo)
version: '3.8'

services:
  gateway:
    build:
      context: .
      dockerfile: Dockerfile.gateway
    ports:
      - "8080:8080"
      - "50051:50051"
    environment:
      RUST_LOG: info
      REDIS_URL: redis://redis:6379
    depends_on:
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/v1/health"]
      interval: 10s
      timeout: 5s
      retries: 3

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 3

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin
    volumes:
      - grafana-storage:/var/lib/grafana

  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"
      - "6831:6831/udp"

volumes:
  grafana-storage:
```

Run it:

```bash
docker-compose up -d
docker-compose logs -f gateway
docker-compose ps
```

## Single Container (Simple Production)

For small deployments on a single machine.

### Build Docker Image

```bash
# In repository root
docker build -f Dockerfile.gateway -t upp:latest .
```

Or use pre-built image:

```bash
docker pull ghcr.io/universal-prediction-protocol/gateway:latest
docker tag ghcr.io/universal-prediction-protocol/gateway:latest upp:latest
```

### Run Container

```bash
docker run -d \
  --name upp-gateway \
  -p 8080:8080 \
  -p 50051:50051 \
  -e RUST_LOG=info \
  -e REDIS_URL=redis://redis.example.com:6379 \
  -e KALSHI_API_KEY=your_key \
  -e POLYMARKET_PRIVATE_KEY=0x... \
  --restart always \
  upp:latest
```

### Health Check

```bash
curl http://localhost:8080/api/v1/health
```

### View Logs

```bash
docker logs -f upp-gateway
docker logs --tail 100 upp-gateway
```

### Stop & Remove

```bash
docker stop upp-gateway
docker rm upp-gateway
```

## Kubernetes (High Availability)

For production with multiple instances and automatic scaling.

### Namespace & RBAC

```yaml
# k8s/namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: upp

---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: upp-gateway
  namespace: upp
```

### Deployment

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: upp-gateway
  namespace: upp
spec:
  replicas: 3
  selector:
    matchLabels:
      app: upp-gateway
  template:
    metadata:
      labels:
        app: upp-gateway
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
    spec:
      serviceAccountName: upp-gateway
      containers:
      - name: gateway
        image: ghcr.io/universal-prediction-protocol/gateway:latest
        imagePullPolicy: IfNotPresent
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 50051
          name: grpc
        env:
        - name: RUST_LOG
          value: "info"
        - name: REDIS_URL
          value: "redis://upp-redis:6379"
        - name: SERVER_HOST
          value: "0.0.0.0"
        - name: SERVER_PORT
          value: "8080"
        - name: GRPC_PORT
          value: "50051"
        - name: KALSHI_API_KEY
          valueFrom:
            secretKeyRef:
              name: upp-secrets
              key: kalshi-api-key
        - name: POLYMARKET_PRIVATE_KEY
          valueFrom:
            secretKeyRef:
              name: upp-secrets
              key: polymarket-private-key
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /api/v1/health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /api/v1/health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
```

### Service

```yaml
# k8s/service.yaml
apiVersion: v1
kind: Service
metadata:
  name: upp-gateway
  namespace: upp
spec:
  type: LoadBalancer
  selector:
    app: upp-gateway
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: grpc
    port: 50051
    targetPort: 50051
```

### ConfigMap

```yaml
# k8s/configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: upp-config
  namespace: upp
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
    scrape_configs:
    - job_name: 'upp'
      static_configs:
      - targets: ['localhost:8080']
```

### Secrets

```bash
# Create secrets for sensitive data
kubectl create secret generic upp-secrets \
  --from-literal=kalshi-api-key=YOUR_KEY \
  --from-literal=polymarket-private-key=0x... \
  -n upp
```

### Deploy

```bash
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml

# Verify
kubectl -n upp get pods
kubectl -n upp get svc

# View logs
kubectl -n upp logs -f deployment/upp-gateway
```

## Cloud Deployments

### AWS ECS

Create an ECS task definition:

```json
{
  "family": "upp-gateway",
  "containerDefinitions": [
    {
      "name": "gateway",
      "image": "ghcr.io/universal-prediction-protocol/gateway:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "hostPort": 8080,
          "protocol": "tcp"
        },
        {
          "containerPort": 50051,
          "hostPort": 50051,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "RUST_LOG",
          "value": "info"
        },
        {
          "name": "REDIS_URL",
          "value": "redis://elasticache-endpoint:6379"
        }
      ],
      "secrets": [
        {
          "name": "KALSHI_API_KEY",
          "valueFrom": "arn:aws:secretsmanager:..."
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/upp-gateway",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      }
    }
  ]
}
```

### Google Cloud Run

```bash
# Build and push image
gcloud builds submit --tag gcr.io/YOUR_PROJECT/upp-gateway

# Deploy
gcloud run deploy upp-gateway \
  --image gcr.io/YOUR_PROJECT/upp-gateway \
  --platform managed \
  --region us-central1 \
  --memory 512Mi \
  --set-env-vars REDIS_URL=redis://127.0.0.1:6379 \
  --set-env-vars RUST_LOG=info
```

### Heroku

```bash
# Create Procfile
echo "web: ./target/release/gateway" > Procfile

# Deploy
heroku create upp-gateway
git push heroku main

# View logs
heroku logs -t
```

## Environment Configuration

### Required Variables

```bash
# Redis connection
export REDIS_URL=redis://localhost:6379

# Logging
export RUST_LOG=info

# Server config
export SERVER_HOST=0.0.0.0
export SERVER_PORT=8080
export GRPC_PORT=50051
```

### Optional API Keys

```bash
# Kalshi (API key + secret)
export KALSHI_API_KEY=your_key
export KALSHI_API_SECRET=your_secret

# Polymarket (ECDSA private key, hex format)
export POLYMARKET_PRIVATE_KEY=0x1234567890abcdef...

# Opinion.trade
export OPINION_TRADE_API_KEY=your_key
```

### Performance Tuning

```bash
# Cache configuration
export CACHE_TTL_SECONDS=300
export CACHE_MAX_SIZE=1000

# Connection pooling
export HTTP_POOL_SIZE=50
export REDIS_POOL_SIZE=10

# Rate limiting
export RATE_LIMIT_PER_SECOND=10
export RATE_LIMIT_BURST=20

# WebSocket
export WS_MAX_CONNECTIONS=1000
export WS_MESSAGE_QUEUE_SIZE=100
```

### Observability

```bash
# Jaeger tracing
export JAEGER_AGENT_HOST=localhost
export JAEGER_AGENT_PORT=6831
export JAEGER_SAMPLER_TYPE=const
export JAEGER_SAMPLER_PARAM=1

# Prometheus metrics
export PROMETHEUS_PUSH_ADDR=http://prometheus:9090
```

## Rolling Updates

Update without downtime:

```bash
# Kubernetes
kubectl set image deployment/upp-gateway \
  gateway=ghcr.io/universal-prediction-protocol/gateway:v0.2.0 \
  -n upp

# Monitor rollout
kubectl rollout status deployment/upp-gateway -n upp
```

## Health Checks

### Liveness Probe

Restart if unhealthy:

```bash
curl -f http://localhost:8080/api/v1/health || exit 1
```

### Readiness Probe

Mark as not-ready if can't serve traffic:

```bash
# Check all providers are up
curl -f http://localhost:8080/api/v1/health | jq '.providers | all(.status == "up")'
```

## Graceful Shutdown

The gateway handles shutdown signals:

```bash
# Send SIGTERM
docker stop upp-gateway  # Waits 10 seconds before killing

# Or
kill -TERM <pid>
```

The gateway will:
1. Stop accepting new connections
2. Wait for in-flight requests to complete
3. Close WebSocket subscriptions
4. Exit cleanly

Configure timeout:

```bash
docker run -d \
  --stop-signal SIGTERM \
  --stop-timeout 30 \
  upp:latest
```

## Scaling

### Horizontal Scaling

Add more instances behind a load balancer:

```
Load Balancer
  ├─ Gateway Instance 1
  ├─ Gateway Instance 2
  └─ Gateway Instance 3
    ↓
  Shared Redis Cache
```

### Vertical Scaling

Increase resources on a single instance:

```bash
# Docker
docker update --memory 1g --cpus 2 upp-gateway

# Kubernetes
kubectl set resources deployment upp-gateway \
  -n upp \
  --limits=cpu=2,memory=1Gi \
  --requests=cpu=1,memory=512Mi
```

## Backup & Disaster Recovery

### Redis Backup

```bash
# Automatic snapshots
redis-cli BGSAVE

# Manual export
redis-cli --rdb /backup/redis-dump.rdb

# Restore
redis-cli --pipe < /backup/redis-dump.rdb
```

### Configuration Backup

```bash
# Backup env vars / config
docker inspect upp-gateway | jq '.Config.Env' > backup-env.json

# Backup secrets (Kubernetes)
kubectl -n upp get secrets -o yaml > backup-secrets.yaml
```

## Monitoring Deployment Health

See [Monitoring & Observability](monitoring.md) for detailed setup.

Quick checks:

```bash
# API availability
curl -f http://localhost:8080/api/v1/health

# Metrics
curl http://localhost:9090/metrics | grep upp

# Logs
docker logs upp-gateway

# Jaeger traces
curl http://localhost:16686/api/traces
```
