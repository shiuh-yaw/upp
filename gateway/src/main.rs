// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// UPP Gateway — High-performance protocol gateway.
//
// Routes requests from Player Surfaces to Prediction Providers,
// handling protocol translation, caching, rate limiting, and
// real-time WebSocket fan-out.

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer, compression::CompressionLayer};
use tracing::{info, warn};

mod adapters;
mod core;
mod gen;
mod handlers;
mod middleware;
mod transport;

use crate::core::{config::GatewayConfig, registry::ProviderRegistry, cache::MarketCache, storage::StorageBackend};
use crate::core::storage;
use crate::core::hardening::{CircuitBreakerRegistry, CircuitBreakerConfig, ConfigValidator};
use crate::core::arbitrage::ArbitrageScanner;
use crate::core::price_index::PriceIndex;
use crate::core::smart_router::SmartRouter;
use crate::transport::websocket::WebSocketManager;
use crate::transport::live_feed::LiveFeedManager;
use crate::core::historical::IngestionPipeline;
use crate::transport::grpc::GrpcState;

// ─── Error Helpers ──────────────────────────────────────────

pub fn upp_error(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "code": code,
            "message": message,
            "request_id": uuid::Uuid::new_v4().to_string(),
        }
    })
}

pub fn internal_error(e: &anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
    warn!("Internal error: {:#}", e);
    (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INTERNAL", &e.to_string())))
}

pub fn not_found(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(upp_error("NOT_FOUND", msg)))
}

pub fn bad_request(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", msg)))
}

// ─── Shared Application State ───────────────────────────────

use crate::middleware::auth::{AuthState, ApiKeyManager};
use crate::middleware::rate_limit::{RateLimitState, extract_client_key, classify_endpoint};
use std::sync::atomic::{AtomicU64, Ordering};

/// Request counters for Prometheus-style metrics.
pub struct Metrics {
    pub requests_total: AtomicU64,
    pub requests_ok: AtomicU64,
    pub requests_err: AtomicU64,
    pub requests_rate_limited: AtomicU64,
    pub ws_connections: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_ok: AtomicU64::new(0),
            requests_err: AtomicU64::new(0),
            requests_rate_limited: AtomicU64::new(0),
            ws_connections: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<ProviderRegistry>,
    pub cache: Arc<MarketCache>,
    pub ws_manager: Arc<WebSocketManager>,
    pub config: Arc<GatewayConfig>,
    pub rate_limiter: Arc<RateLimitState>,
    pub auth: Arc<AuthState>,
    pub metrics: Arc<Metrics>,
    pub circuit_breakers: Arc<CircuitBreakerRegistry>,
    pub storage: Arc<dyn StorageBackend>,
    pub arbitrage_scanner: Arc<ArbitrageScanner>,
    pub price_index: Arc<PriceIndex>,
    pub smart_router: Arc<SmartRouter>,
    pub live_feed: Arc<LiveFeedManager>,
    pub ingestion: Arc<IngestionPipeline>,
    pub api_keys: Arc<ApiKeyManager>,
}

// ─── Main ───────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "upp_gateway=info,tower_http=debug".into()),
        )
        .json()
        .init();

    let config = Arc::new(GatewayConfig::load()?);
    info!(version = env!("CARGO_PKG_VERSION"), "Starting UPP Gateway");

    // Validate configuration at startup
    ConfigValidator::validate_all(&config).await?;

    let registry = Arc::new(ProviderRegistry::new(&config).await?);
    let cache = Arc::new(MarketCache::new(&config));
    let ws_manager = Arc::new(WebSocketManager::new(registry.clone(), config.clone()));

    // Rate limiter — token bucket per client with configurable multi-tier support
    let rate_limit_config = config.rate_limit_config();
    let rate_limiter = Arc::new(RateLimitState::new(rate_limit_config));
    rate_limiter.start_cleanup();
    info!(
        "Rate limiter active: Light({}/{}), Standard({}/{}), Heavy({}/{}), WebSocket({}/{})",
        config.rate_limit_light_burst, config.rate_limit_light_rps,
        config.rate_limit_standard_burst, config.rate_limit_standard_rps,
        config.rate_limit_heavy_burst, config.rate_limit_heavy_rps,
        config.rate_limit_ws_burst, config.rate_limit_ws_rps
    );

    // Auth — dev mode (pass-through) by default
    let auth = Arc::new(AuthState::dev_mode());
    info!("Auth: dev mode (all requests pass through)");

    // Metrics counters
    let metrics = Arc::new(Metrics::new());

    // Circuit breaker registry — per-provider state management
    let circuit_breakers = Arc::new(CircuitBreakerRegistry::new(CircuitBreakerConfig::default()));
    info!(
        "Circuit breaker initialized: {} failures to trip, {} second recovery timeout",
        5, 30
    );

    // Start WebSocket price poller (1s interval)
    ws_manager.start_price_poller(1000);

    // Start WebSocket orderbook poller (2s interval)
    ws_manager.start_orderbook_poller(2000);

    // Initialize persistent storage (in-memory by default, Redis if configured)
    let storage = storage::create_storage(config.redis_url.as_deref()).await?;

    // Arbitrage scanner — 0.5% min spread, 2% estimated fee per side
    let arbitrage_scanner = Arc::new(ArbitrageScanner::new(0.5, 0.02));
    info!("Arbitrage scanner initialized (min spread: 0.5%, fee estimate: 2%)");

    // Price indexer — time-series candle aggregation from WebSocket feed
    let price_index = Arc::new(PriceIndex::new());
    info!("Price indexer initialized (resolutions: 1m, 5m, 1h, 1d)");

    // Smart order router — cross-provider optimal routing
    let smart_router = Arc::new(SmartRouter::new(0.02));
    info!("Smart order router initialized (default fee rate: 2%)");

    // Live feed manager — persistent WebSocket connections to providers
    let live_feed = crate::transport::live_feed::start_live_feeds(ws_manager.clone());
    info!("Live feed manager started (providers: kalshi, polymarket, opinion)");

    // Historical data ingestion pipeline — mock data sources for dev
    let ingestion = crate::core::historical::create_dev_pipeline(price_index.clone());
    crate::core::historical::start_ingestion_pipeline(ingestion.clone(), 60);
    info!("Historical ingestion pipeline started (interval: 60 min)");

    // API key manager — in-memory key store for dev mode
    let api_keys = Arc::new(ApiKeyManager::new());
    info!("API key manager initialized");

    let state = AppState {
        registry: registry.clone(),
        cache: cache.clone(),
        ws_manager: ws_manager.clone(),
        config: config.clone(),
        rate_limiter,
        auth,
        metrics,
        circuit_breakers,
        storage,
        arbitrage_scanner: arbitrage_scanner.clone(),
        price_index: price_index.clone(),
        smart_router: smart_router.clone(),
        live_feed: live_feed.clone(),
        ingestion: ingestion.clone(),
        api_keys: api_keys.clone(),
    };

    // Start the background arbitrage scanner (every 5 seconds)
    crate::core::arbitrage::start_arbitrage_scanner(
        arbitrage_scanner,
        registry.clone(),
        ws_manager.clone(),
        5000,
    );

    // Start the price indexer (polls every 5 seconds, ingests into candle series)
    crate::core::price_index::start_price_indexer(
        price_index,
        ws_manager.clone(),
        5000,
    );

    // Start gRPC server on port 50051 (background task)
    let grpc_state = GrpcState {
        registry: registry.clone(),
        cache: cache.clone(),
        config: config.clone(),
    };
    let grpc_port = std::env::var("UPP_GRPC_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50051u16);

    tokio::spawn(async move {
        if let Err(e) = crate::transport::grpc::start_grpc_server(grpc_state, grpc_port).await {
            tracing::error!("gRPC server error: {}", e);
        }
    });

    let app = build_router(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(address = %addr, "UPP Gateway listening (REST + gRPC:{grpc_port})");

    // Setup graceful shutdown signal handler (SIGINT/SIGTERM)
    let shutdown_signal = async {
        use tokio::signal;
        let _ = signal::ctrl_c().await;
        info!("Received shutdown signal, initiating graceful shutdown...");
    };

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    info!("UPP Gateway shut down gracefully");
    Ok(())
}

// ─── Router ─────────────────────────────────────────────────

fn build_router(state: AppState) -> Router {
    // Public routes — no auth required, rate-limited
    let public = Router::new()
        .route("/upp/v1/discovery/manifest/:provider", get(handlers::discovery::get_manifest))
        .route("/upp/v1/discovery/providers", get(handlers::discovery::list_providers))
        .route("/upp/v1/discovery/negotiate", post(handlers::discovery::negotiate))
        .route("/upp/v1/discovery/health/:provider", get(handlers::discovery::health_check))
        .route("/upp/v1/discovery/health", get(handlers::discovery::health_check_all))
        .route("/upp/v1/markets", get(handlers::markets::list_markets))
        .route("/upp/v1/markets/search", get(handlers::markets::search_markets))
        .route("/upp/v1/markets/:market_id", get(handlers::markets::get_market))
        .route("/upp/v1/markets/:market_id/orderbook", get(handlers::markets::get_orderbook))
        .route("/upp/v1/markets/:market_id/orderbook/merged", get(handlers::markets::get_merged_orderbook))
        .route("/upp/v1/markets/categories", get(handlers::markets::list_categories))
        // Arbitrage endpoints
        .route("/upp/v1/arbitrage", get(handlers::arbitrage::list_opportunities))
        .route("/upp/v1/arbitrage/summary", get(handlers::arbitrage::get_summary))
        .route("/upp/v1/arbitrage/history", get(handlers::arbitrage::get_history))
        // Price history / candlestick endpoints
        .route("/upp/v1/markets/:market_id/candles", get(handlers::price_history::get_candles))
        .route("/upp/v1/markets/:market_id/candles/latest", get(handlers::price_history::get_latest_candle))
        .route("/upp/v1/price-index/stats", get(handlers::price_history::get_stats))
        .route("/upp/v1/resolutions/:market_id", get(handlers::resolution::get_resolution))
        .route("/upp/v1/resolutions", get(handlers::resolution::list_resolutions))
        .route("/upp/v1/settlement/instruments", get(handlers::settlement::list_instruments))
        .route("/upp/v1/settlement/handlers", get(handlers::settlement::list_handlers))
        .route("/.well-known/upp", get(handlers::discovery::well_known))
        // MCP (Model Context Protocol) & A2A Integration
        .route("/upp/v1/mcp/tools", get(handlers::mcp::list_tools))
        .route("/upp/v1/mcp/execute", post(handlers::mcp::execute_tool))
        .route("/upp/v1/mcp/schema", get(handlers::mcp::get_schema))
        .route("/.well-known/agent.json", get(handlers::mcp::get_agent_card))
        // Live feed status
        .route("/upp/v1/feeds/status", get(handlers::live_feed::feed_status))
        .route("/upp/v1/feeds/stats", get(handlers::live_feed::feed_stats))
        // Backtesting
        .route("/upp/v1/backtest/strategies", get(handlers::backtest::list_strategies))
        .route("/upp/v1/backtest/run", post(handlers::backtest::run_backtest))
        .route("/upp/v1/backtest/compare", post(handlers::backtest::compare_strategies))
        // Historical ingestion
        .route("/upp/v1/ingestion/stats", get(handlers::ingestion::stats))
        .route("/upp/v1/ingestion/ingest", post(handlers::ingestion::ingest_market))
        .route("/upp/v1/ingestion/ingest-recent", post(handlers::ingestion::ingest_recent))
        // Provider status
        .route("/upp/v1/status", get(handlers::status::status_json));

    // Auth-required routes — trading & portfolio
    let protected = Router::new()
        .route("/upp/v1/orders", post(handlers::trading::create_order))
        .route("/upp/v1/orders", get(handlers::trading::list_orders))
        .route("/upp/v1/orders/:order_id", get(handlers::trading::get_order))
        .route("/upp/v1/orders/:order_id", delete(handlers::trading::cancel_order))
        .route("/upp/v1/orders/cancel-all", post(handlers::trading::cancel_all_orders))
        .route("/upp/v1/orders/estimate", post(handlers::trading::estimate_order))
        .route("/upp/v1/trades", get(handlers::trading::list_trades))
        .route("/upp/v1/portfolio/positions", get(handlers::portfolio::list_positions))
        .route("/upp/v1/portfolio/summary", get(handlers::portfolio::get_summary))
        .route("/upp/v1/portfolio/balances", get(handlers::portfolio::list_balances))
        // Portfolio analytics
        .route("/upp/v1/portfolio/analytics", get(handlers::portfolio::get_analytics))
        // Smart order routing
        .route("/upp/v1/orders/route", post(handlers::smart_routing::compute_route))
        .route("/upp/v1/orders/route/execute", post(handlers::smart_routing::execute_route))
        .route("/upp/v1/orders/route/stats", get(handlers::smart_routing::get_stats))
        // Live feed subscription management
        .route("/upp/v1/feeds/subscribe", post(handlers::live_feed::subscribe_markets))
        // API key management
        .route("/upp/v1/auth/keys", post(handlers::auth_mgmt::create_key))
        .route("/upp/v1/auth/keys", get(handlers::auth_mgmt::list_keys))
        .route("/upp/v1/auth/keys/revoke", post(handlers::auth_mgmt::revoke_key))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware_auth_check,
        ));

    // Infra routes — health, metrics, WebSocket
    let infra = Router::new()
        .route("/health", get(handlers::health::health))
        .route("/ready", get(handlers::health::ready))
        .route("/metrics", get(handlers::health::metrics_handler))
        .route("/upp/v1/ws", get(handlers::websocket::ws_upgrade))
        .route("/dashboard", get(handlers::dashboard::serve_dashboard))
        .route("/status", get(handlers::status::status_page))
        .route("/docs", get(handlers::docs::swagger_ui))
        .route("/openapi.json", get(handlers::docs::openapi_spec));

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(infra)
        // Global middleware — rate limit + metrics counting on every request
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware_rate_limit,
        ))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ─── Middleware Functions ──────────────────────────────────

/// Global rate limiter — runs on every request.
async fn middleware_rate_limit(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let key = extract_client_key(req.headers());
    let tier = classify_endpoint(req.uri().path());

    let result = state.rate_limiter.check(&key, tier);
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

    if !result.allowed {
        state.metrics.requests_rate_limited.fetch_add(1, Ordering::Relaxed);
        warn!(client = %key, "Rate limited");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("X-RateLimit-Limit", result.limit.to_string()),
                ("X-RateLimit-Remaining", "0".to_string()),
                ("Retry-After", (result.retry_after.ceil() as u64).to_string()),
            ],
            Json(upp_error("RATE_LIMITED", "Too many requests")),
        ).into_response();
    }

    let mut response = next.run(req).await;

    // Inject rate limit headers
    let headers = response.headers_mut();
    if let Ok(v) = result.limit.to_string().parse() {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = result.remaining.to_string().parse() {
        headers.insert("X-RateLimit-Remaining", v);
    }

    response
}

/// Auth check — runs only on protected (trading/portfolio) routes.
async fn middleware_auth_check(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    use crate::middleware::auth::AuthResult;

    let path = req.uri().path().to_string();
    let result = state.auth.authenticate(req.headers(), &path);

    match result {
        AuthResult::Authenticated(_) | AuthResult::Public => {
            next.run(req).await
        }
        AuthResult::Unauthorized(msg) => {
            (
                StatusCode::UNAUTHORIZED,
                [("WWW-Authenticate", "Bearer, ApiKey")],
                Json(upp_error("UNAUTHORIZED", &msg)),
            ).into_response()
        }
        AuthResult::Forbidden(msg) => {
            (StatusCode::FORBIDDEN, Json(upp_error("FORBIDDEN", &msg))).into_response()
        }
    }
}
