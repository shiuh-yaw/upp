# Gateway Internals

The gateway is the heart of UPP—a high-performance Axum-based server that routes requests, manages caching, enforces rate limits, and orchestrates provider adapters.

## Gateway Architecture

```
┌─────────────────────────────────────────────┐
│   Axum Application                          │
├─────────────────────────────────────────────┤
│   Router                                    │
│   └─ /api/v1/health → health_handler       │
│   └─ /api/v1/markets → markets_handler     │
│   └─ /api/v1/orders → orders_handler       │
│   └─ /api/v1/portfolio → portfolio_handler │
│   └─ /feed → websocket_handler             │
│                                             │
│   Middleware Stack                          │
│   ├─ Tracing (Jaeger)                      │
│   ├─ Authentication                        │
│   ├─ Request logging                       │
│   ├─ Error handling                        │
│   └─ CORS                                  │
│                                             │
│   Shared State                              │
│   ├─ Redis client                          │
│   ├─ Adapter registry                      │
│   ├─ Rate limiter                          │
│   └─ WebSocket subscriptions               │
└─────────────────────────────────────────────┘
```

## Request Router

The router maps HTTP paths to handler functions using Axum's macro-based routing:

```rust
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health endpoints
        .route("/api/v1/health", get(health_handler))

        // Markets endpoints
        .route("/api/v1/markets", get(list_markets_handler))
        .route("/api/v1/markets/:id", get(get_market_handler))
        .route("/api/v1/markets/search", get(search_markets_handler))

        // Orders endpoints
        .route("/api/v1/orders", get(list_orders_handler))
        .route("/api/v1/orders", post(place_order_handler))
        .route("/api/v1/orders/:id", get(get_order_handler))
        .route("/api/v1/orders/:id", delete(cancel_order_handler))

        // Portfolio endpoints
        .route("/api/v1/portfolio", get(portfolio_handler))
        .route("/api/v1/portfolio/positions", get(positions_handler))

        // Arbitrage endpoints
        .route("/api/v1/arbitrage/opportunities", get(arbitrage_handler))

        // Backtest endpoints
        .route("/api/v1/backtest", post(backtest_handler))

        // WebSocket
        .route("/api/v1/feed", get(websocket_handler))

        // gRPC reflection
        .route("/grpc.reflection.v1.ServerReflection/Server", get(grpc_handler))

        .with_state(state)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10MB max
        .fallback(not_found)
}
```

## Middleware Stack

Middleware executes in order for each request:

### 1. Request ID & Tracing

Assigns a unique ID and creates a Jaeger span:

```rust
pub async fn tracing_middleware(
    req: Request,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %req.method(),
        uri = %req.uri(),
    );

    let _guard = span.enter();
    let response = next.run(req).await;

    tracing::info!("request completed");
    response
}
```

### 2. Authentication

Validates API keys or OAuth signatures:

```rust
pub async fn auth_middleware(
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Verify token
    let token = auth_header.strip_prefix("Bearer ").ok_or(StatusCode::UNAUTHORIZED)?;
    let identity = verify_token(token).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add to request extensions
    req.extensions_mut().insert(identity);

    Ok(next.run(req).await)
}
```

### 3. Request Logging

Logs key information about each request:

```rust
pub async fn logging_middleware(
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    let elapsed = start.elapsed();
    tracing::info!(
        method = %method,
        uri = %uri,
        status = response.status().as_u16(),
        duration_ms = elapsed.as_millis(),
    );

    response
}
```

### 4. Error Handling

Catches panics and converts errors to JSON responses:

```rust
pub async fn error_handling_middleware(
    req: Request,
    next: Next,
) -> Result<Response, JsonRejection> {
    match catch_unwind(AssertUnwindSafe(|| { next.run(req) })) {
        Ok(response) => Ok(response),
        Err(_) => {
            tracing::error!("request panicked");
            Err(JsonRejection::from(
                ApiError::InternalServerError("internal server error".into())
            ))
        }
    }
}
```

## Cache Management

The gateway uses Redis for distributed caching:

### Cache Keys

Keys are hierarchical and include all filter parameters:

```
markets:{provider}:{filters_hash}
markets:polymarket:fbe8c2d4

orders:{provider}:{user_id}
orders:kalshi:user123

portfolio:{provider}:{user_id}
portfolio:polymarket:user456

health
backtest:{query_hash}
```

### TTL Configuration

```rust
const CACHE_DEFAULTS: &[(Pattern, Duration)] = &[
    (Pattern::HealthCheck, Duration::from_secs(5)),
    (Pattern::Markets, Duration::from_secs(60)),
    (Pattern::Orders, Duration::from_secs(10)),
    (Pattern::Portfolio, Duration::from_secs(30)),
    (Pattern::Backtest, Duration::MAX), // Never expires
];
```

### Cache Invalidation

Triggered by:

1. **Time-based** — TTL expiration (automatic)
2. **Event-based** — After place_order() or cancel_order(), invalidate related portfolios
3. **Manual** — DELETE /api/v1/cache/{key}

```rust
pub async fn place_order_handler(
    State(state): State<AppState>,
    Json(order): Json<OrderRequest>,
) -> Result<Json<OrderResponse>, ApiError> {
    let response = state.adapter.place_order(order.clone()).await?;

    // Invalidate caches
    state.cache.delete("orders:*:*").await.ok(); // All orders
    state.cache.delete("portfolio:*:*").await.ok(); // All portfolios

    Ok(Json(response))
}
```

## Rate Limiting

Per-client and per-provider rate limiting protects against abuse:

### Configuration

```rust
pub struct RateLimitConfig {
    pub per_client_qps: u32,      // 10 requests per second
    pub per_client_burst: u32,    // Allow 20 in a burst
    pub per_provider_qps: u32,    // 100 requests per second
    pub per_provider_burst: u32,  // Allow 200 in a burst
}
```

### Implementation (Token Bucket)

```rust
pub struct RateLimiter {
    tokens: Arc<Mutex<f64>>,
    refill_rate: f64,           // tokens per second
    max_tokens: f64,            // burst capacity
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    pub async fn check(&self) -> Result<(), Duration> {
        let mut tokens = self.tokens.lock().await;
        let mut last = self.last_refill.lock().await;

        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(*last).as_secs_f64();
        *tokens = ((*tokens + self.refill_rate * elapsed).min(self.max_tokens)).max(0.0);
        *last = now;

        if *tokens >= 1.0 {
            *tokens -= 1.0;
            Ok(())
        } else {
            // Calculate how long until next token available
            let wait_time = Duration::from_secs_f64(1.0 / self.refill_rate);
            Err(wait_time)
        }
    }
}
```

### Middleware Integration

```rust
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get client IP
    let ip = addr.ip().to_string();

    // Check rate limit
    match state.rate_limiter.check(&ip).await {
        Ok(_) => Ok(next.run(req).await),
        Err(backoff) => {
            let retry_after = backoff.as_secs().to_string();
            let mut response = StatusCode::TOO_MANY_REQUESTS.into_response();
            response.headers_mut().insert(
                "Retry-After",
                retry_after.parse().unwrap(),
            );
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}
```

## Provider Adapters

The gateway holds a registry of provider adapters:

```rust
pub struct AdapterRegistry {
    adapters: HashMap<String, Arc<dyn ProviderAdapter>>,
}

impl AdapterRegistry {
    pub fn get(&self, provider: &str) -> Option<Arc<dyn ProviderAdapter>> {
        self.adapters.get(provider).cloned()
    }

    pub async fn list_all_markets(&self) -> Result<Vec<Market>> {
        // Query all adapters in parallel
        let futures = self.adapters.values().map(|adapter| {
            adapter.get_markets(MarketFilter::default())
        });

        let results = futures::future::join_all(futures).await;
        let all_markets: Vec<Market> = results
            .into_iter()
            .filter_map(|r| r.ok())
            .flat_map(|markets| markets)
            .collect();

        Ok(all_markets)
    }
}
```

## WebSocket Handler

Manages real-time subscriptions and fan-out:

```rust
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let subscription_id = Uuid::new_v4();

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(cmd) = serde_json::from_str::<SubscriptionCommand>(&text) {
                    match cmd.action.as_str() {
                        "subscribe" => {
                            // Register subscription
                            state.subscriptions.register(
                                subscription_id,
                                cmd.channel.clone(),
                                sender.clone(),
                            ).await;

                            // Start background poller if needed
                            if !state.subscriptions.has_poller(&cmd.channel).await {
                                start_poller(cmd.channel, state.clone()).await;
                            }
                        }
                        "unsubscribe" => {
                            state.subscriptions.unregister(subscription_id, &cmd.channel).await;
                        }
                        _ => {
                            let _ = sender.send(error_message("unknown action")).await;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
        }
    }
}
```

## Health Check Endpoint

Provides system health and provider status:

```rust
pub async fn health_handler(
    State(state): State<AppState>,
) -> Json<HealthResponse> {
    let mut provider_health = HashMap::new();

    for (name, adapter) in &state.adapters.adapters {
        let start = Instant::now();
        let status = match adapter.health_check().await {
            Ok(_) => "up",
            Err(_) => "down",
        };

        provider_health.insert(name.clone(), ProviderStatus {
            status: status.to_string(),
            latency_ms: start.elapsed().as_millis() as u64,
            last_sync: Utc::now(),
        });
    }

    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
        providers: provider_health,
        uptime_seconds: state.start_time.elapsed().as_secs(),
    })
}
```

## Error Responses

All errors follow a consistent JSON format:

```json
{
  "error": {
    "code": "INVALID_PROVIDER",
    "message": "Unknown provider: 'invalid'",
    "details": {
      "available_providers": ["polymarket", "kalshi", "opinion_trade"]
    }
  },
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2026-03-14T12:34:56Z"
}
```

## Performance Tuning

### Tokio Runtime Configuration

```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(num_cpus::get())
    .thread_name("upp-worker")
    .enable_all()
    .build()?;
```

### Connection Pooling

```rust
let client = HttpClient::builder()
    .pool_max_idle_per_host(10)
    .timeout(Duration::from_secs(30))
    .build()?;
```

### Memory Tuning

- Increase Redis memory for more cache
- Tune WebSocket message queue sizes
- Monitor adapter-local cache sizes
