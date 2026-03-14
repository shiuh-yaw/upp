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
    extract::{Path, Query, State},
    extract::ws::WebSocketUpgrade,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer, compression::CompressionLayer};
use tracing::{info, warn, debug};

mod adapters;
mod core;
mod gen;
mod middleware;
mod transport;

use crate::core::{config::GatewayConfig, registry::ProviderRegistry, cache::MarketCache, storage::StorageBackend};
use crate::core::storage;
use crate::core::hardening::{CircuitBreakerRegistry, CircuitBreakerConfig, ConfigValidator};
use crate::core::arbitrage::ArbitrageScanner;
use crate::core::price_index::PriceIndex;
use crate::core::smart_router::SmartRouter;
use crate::core::types::*;
use crate::adapters::MarketFilter;
use crate::transport::websocket::WebSocketManager;
use crate::transport::live_feed::LiveFeedManager;
use crate::core::historical::IngestionPipeline;
use crate::transport::grpc::GrpcState;

// ─── Error Helpers ──────────────────────────────────────────

fn upp_error(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "code": code,
            "message": message,
            "request_id": uuid::Uuid::new_v4().to_string(),
        }
    })
}

fn internal_error(e: &anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
    warn!("Internal error: {:#}", e);
    (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INTERNAL", &e.to_string())))
}

fn not_found(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(upp_error("NOT_FOUND", msg)))
}

fn bad_request(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
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
        .route("/upp/v1/ingestion/ingest-recent", post(handlers::ingestion::ingest_recent));

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
        .route("/dashboard", get(handlers::dashboard::serve_dashboard));

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

    let (allowed, remaining, limit, retry_after) = state.rate_limiter.check(&key, tier);
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

    if !allowed {
        state.metrics.requests_rate_limited.fetch_add(1, Ordering::Relaxed);
        warn!(client = %key, "Rate limited");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("X-RateLimit-Limit", limit.to_string()),
                ("X-RateLimit-Remaining", "0".to_string()),
                ("Retry-After", (retry_after.ceil() as u64).to_string()),
            ],
            Json(upp_error("RATE_LIMITED", "Too many requests")),
        ).into_response();
    }

    let mut response = next.run(req).await;

    // Inject rate limit headers
    let headers = response.headers_mut();
    if let Ok(v) = limit.to_string().parse() {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = remaining.to_string().parse() {
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

// ═════════════════════════════════════════════════════════════
// Handler Modules — Wired to Provider Registry
// ═════════════════════════════════════════════════════════════

mod handlers {

    // ── Discovery ────────────────────────────────────────────
    pub mod discovery {
        use super::super::*;

        pub async fn get_manifest(
            State(state): State<AppState>,
            Path(provider): Path<String>,
        ) -> impl IntoResponse {
            match state.registry.get_manifest(&provider).await {
                Ok(manifest) => Json(manifest).into_response(),
                Err(e) => not_found(&e.to_string()).into_response(),
            }
        }

        pub async fn list_providers(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let manifests = state.registry.list_providers().await;
            Json(serde_json::json!({
                "providers": manifests,
                "total": manifests.len(),
            }))
        }

        pub async fn negotiate(
            State(state): State<AppState>,
            Json(req): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let provider_id = req.get("provider").and_then(|v| v.as_str()).unwrap_or("");
            match state.registry.get_manifest(provider_id).await {
                Ok(manifest) => Json(serde_json::json!({
                    "active_capabilities": manifest.capabilities,
                    "selected_transport": "rest",
                    "selected_auth": manifest.authentication.first().unwrap_or(&"none".to_string()),
                })).into_response(),
                Err(e) => not_found(&e.to_string()).into_response(),
            }
        }

        pub async fn health_check(
            State(state): State<AppState>,
            Path(provider): Path<String>,
        ) -> impl IntoResponse {
            match state.registry.health_check(&provider).await {
                Ok(health) => Json(health).into_response(),
                Err(e) => (StatusCode::SERVICE_UNAVAILABLE, Json(upp_error("PROVIDER_ERROR", &e.to_string()))).into_response(),
            }
        }

        pub async fn health_check_all(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let results = state.registry.health_check_all().await;
            Json(serde_json::json!({
                "providers": results,
                "total": results.len(),
            }))
        }

        pub async fn well_known(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let providers = state.registry.list_providers().await;
            Json(serde_json::json!({
                "upp_version": "2026-03-11",
                "gateway": {
                    "version": env!("CARGO_PKG_VERSION"),
                    "transports": ["rest", "websocket"],
                },
                "providers": providers,
            }))
        }
    }

    // ── Markets ──────────────────────────────────────────────
    pub mod markets {
        use super::super::*;

        #[derive(Debug, Deserialize, Default)]
        pub struct ListMarketsParams {
            pub provider: Option<String>,
            pub status: Option<String>,
            pub category: Option<String>,
            pub market_type: Option<String>,
            pub sort_by: Option<String>,
            pub limit: Option<i32>,
            pub cursor: Option<String>,
        }

        pub async fn list_markets(
            State(state): State<AppState>,
            Query(params): Query<ListMarketsParams>,
        ) -> impl IntoResponse {
            let filter = MarketFilter {
                provider: params.provider.clone(),
                category: params.category,
                status: params.status.as_deref().map(parse_status),
                market_type: params.market_type.as_deref().map(parse_market_type),
                sort_by: params.sort_by,
                pagination: PaginationRequest {
                    limit: params.limit.or(Some(20)),
                    cursor: params.cursor,
                },
                ..Default::default()
            };

            let provider_ids = params.provider.map(|p| vec![p]);

            // Use parallel aggregation across providers
            let agg = crate::core::aggregation::parallel_list_markets(
                &state.registry, filter, provider_ids,
            ).await;

            // Cache all returned markets
            for market in &agg.markets {
                state.cache.put_market(market.id.to_full_id(), market.clone()).await;
            }

            Json(serde_json::json!({
                "markets": agg.markets,
                "pagination": {
                    "cursor": "",
                    "has_more": false,
                    "total": agg.total,
                },
                "provider_results": agg.provider_results,
                "errors": agg.errors,
            }))
        }

        #[derive(Debug, Deserialize)]
        pub struct SearchParams {
            pub q: String,
            pub provider: Option<String>,
            pub limit: Option<i32>,
            pub cursor: Option<String>,
        }

        pub async fn search_markets(
            State(state): State<AppState>,
            Query(params): Query<SearchParams>,
        ) -> impl IntoResponse {
            let filter = MarketFilter {
                provider: params.provider.clone(),
                pagination: PaginationRequest {
                    limit: params.limit.or(Some(20)),
                    cursor: params.cursor,
                },
                ..Default::default()
            };

            // Use parallel aggregation across providers
            let agg = crate::core::aggregation::parallel_search_markets(
                &state.registry, &params.q, filter,
            ).await;

            Json(serde_json::json!({
                "markets": agg.markets,
                "query": params.q,
                "pagination": {
                    "cursor": "",
                    "has_more": false,
                    "total": agg.total,
                },
                "provider_results": agg.provider_results,
                "errors": agg.errors,
            }))
        }

        pub async fn get_market(
            State(state): State<AppState>,
            Path(market_id): Path<String>,
        ) -> impl IntoResponse {
            let cache_key = if market_id.starts_with("upp:") {
                market_id.clone()
            } else {
                format!("upp:{}", market_id)
            };

            // L1 cache: in-memory MarketCache
            if let Some(cached) = state.cache.get_market(&cache_key).await {
                return Json(serde_json::to_value(&cached).unwrap()).into_response();
            }

            // L2 cache: persistent storage (Redis or in-memory storage layer)
            if let Ok(Some(stored_json)) = state.storage.get_cached_market(&cache_key).await {
                if let Ok(market) = serde_json::from_str::<Market>(&stored_json) {
                    // Promote back to L1 cache
                    state.cache.put_market(cache_key, market.clone()).await;
                    return Json(serde_json::to_value(&market).unwrap()).into_response();
                }
            }

            let (provider_id, native_id) = parse_market_id(&market_id);

            if let Some(adapter) = state.registry.get(&provider_id) {
                match adapter.get_market(&native_id).await {
                    Ok(market) => {
                        // Write to L1 cache
                        state.cache.put_market(cache_key.clone(), market.clone()).await;

                        // Write to L2 persistent cache (5 min TTL)
                        if let Ok(json) = serde_json::to_string(&market) {
                            if let Err(e) = state.storage.cache_market(&cache_key, &json, 300).await {
                                warn!("Failed to persist market cache for {}: {}", cache_key, e);
                            }
                        }

                        Json(serde_json::to_value(&market).unwrap()).into_response()
                    }
                    Err(e) => not_found(&format!("Market {} not found: {}", market_id, e)).into_response(),
                }
            } else {
                not_found(&format!("Unknown provider: {}", provider_id)).into_response()
            }
        }

        #[derive(Debug, Deserialize, Default)]
        pub struct OrderbookParams {
            pub outcome: Option<String>,
            pub depth: Option<i32>,
        }

        pub async fn get_orderbook(
            State(state): State<AppState>,
            Path(market_id): Path<String>,
            Query(params): Query<OrderbookParams>,
        ) -> impl IntoResponse {
            let (provider_id, native_id) = parse_market_id(&market_id);

            if let Some(adapter) = state.registry.get(&provider_id) {
                match adapter.get_orderbook(
                    &native_id,
                    params.outcome.as_deref(),
                    params.depth.unwrap_or(10),
                ).await {
                    Ok(snapshots) => Json(serde_json::json!({
                        "market_id": market_id,
                        "orderbook": snapshots,
                    })).into_response(),
                    Err(e) => internal_error(&e).into_response(),
                }
            } else {
                not_found(&format!("Unknown provider: {}", provider_id)).into_response()
            }
        }

        /// Merged orderbook: combines liquidity from all providers that have
        /// the same market. Detects cross-provider arbitrage opportunities.
        #[derive(Debug, Deserialize, Default)]
        pub struct MergedOrderbookParams {
            pub outcome: Option<String>,
            pub depth: Option<i32>,
        }

        pub async fn get_merged_orderbook(
            State(state): State<AppState>,
            Path(market_id): Path<String>,
            Query(params): Query<MergedOrderbookParams>,
        ) -> impl IntoResponse {
            let (primary_provider, native_id) = parse_market_id(&market_id);

            // Build a map of provider → native_market_id.
            // For now, we only have the primary provider's native ID.
            // TODO: cross-provider market matching using event similarity.
            let mut native_ids = std::collections::HashMap::new();
            native_ids.insert(primary_provider.clone(), native_id.clone());

            let mut merged = crate::core::aggregation::merged_orderbook(
                &state.registry,
                &native_ids,
                params.outcome.as_deref(),
                params.depth.unwrap_or(10),
            ).await;

            merged.market_id = market_id;

            Json(serde_json::to_value(&merged).unwrap_or_default()).into_response()
        }

        pub async fn list_categories(
            State(_state): State<AppState>,
        ) -> impl IntoResponse {
            Json(serde_json::json!({
                "categories": [
                    "politics", "crypto", "sports", "science",
                    "economics", "entertainment", "weather", "technology"
                ]
            }))
        }

        fn parse_market_id(id: &str) -> (String, String) {
            let id = id.strip_prefix("upp:").unwrap_or(id);
            if let Some(colon_pos) = id.find(':') {
                (id[..colon_pos].to_string(), id[colon_pos + 1..].to_string())
            } else if id.contains('-') && id.chars().all(|c| c.is_uppercase() || c == '-' || c.is_numeric()) {
                ("kalshi.com".to_string(), id.to_string())
            } else if id.starts_with("0x") {
                ("polymarket.com".to_string(), id.to_string())
            } else {
                ("kalshi.com".to_string(), id.to_string())
            }
        }

        fn parse_status(s: &str) -> MarketStatus {
            match s.to_lowercase().as_str() {
                "open" | "active" => MarketStatus::Open,
                "closed" => MarketStatus::Closed,
                "resolved" | "settled" => MarketStatus::Resolved,
                "halted" => MarketStatus::Halted,
                "pending" => MarketStatus::Pending,
                "voided" => MarketStatus::Voided,
                _ => MarketStatus::Open,
            }
        }

        fn parse_market_type(s: &str) -> MarketType {
            match s.to_lowercase().as_str() {
                "binary" => MarketType::Binary,
                "categorical" => MarketType::Categorical,
                "scalar" => MarketType::Scalar,
                _ => MarketType::Binary,
            }
        }
    }

    // ── Trading ──────────────────────────────────────────────
    pub mod trading {
        use super::super::*;
        use crate::adapters::CreateOrderRequest;
        use crate::core::storage::{StoredOrder, StoredTrade, OrderFilter as StorageOrderFilter, TradeFilter as StorageTradeFilter};

        /// Convert a provider Order into a StoredOrder for persistence.
        fn order_to_stored(order: &Order, provider: &str) -> StoredOrder {
            StoredOrder {
                order_id: order.id.clone(),
                provider: provider.to_string(),
                market_id: order.market_id.to_full_id(),
                outcome_id: order.outcome_id.clone(),
                side: format!("{:?}", order.side).to_lowercase(),
                price: order.price.clone().unwrap_or_default(),
                quantity: order.quantity,
                status: format!("{:?}", order.status).to_lowercase(),
                created_at: order.created_at.to_rfc3339(),
                updated_at: order.updated_at.to_rfc3339(),
                provider_order_id: Some(order.provider_order_id.clone()),
            }
        }

        /// Convert a provider Trade into a StoredTrade for persistence.
        fn trade_to_stored(trade: &Trade, provider: &str) -> StoredTrade {
            StoredTrade {
                trade_id: trade.id.clone(),
                order_id: trade.order_id.clone(),
                provider: provider.to_string(),
                market_id: trade.market_id.to_full_id(),
                side: format!("{:?}", trade.side).to_lowercase(),
                price: trade.price.clone(),
                quantity: trade.quantity,
                fee: trade.fees.total_fee.clone(),
                executed_at: trade.executed_at.to_rfc3339(),
            }
        }

        #[derive(Debug, Deserialize)]
        pub struct CreateOrderBody {
            pub provider: String,
            pub market_id: String,
            pub outcome_id: String,
            pub side: String,
            pub order_type: String,
            pub tif: Option<String>,
            pub price: Option<String>,
            pub quantity: i64,
            pub client_order_id: Option<String>,
        }

        pub async fn create_order(
            State(state): State<AppState>,
            Json(body): Json<CreateOrderBody>,
        ) -> impl IntoResponse {
            let provider = body.provider.clone();
            let Some(adapter) = state.registry.get(&provider) else {
                return bad_request(&format!("Unknown provider: {}", provider)).into_response();
            };

            let side = match body.side.to_lowercase().as_str() {
                "buy" => Side::Buy,
                "sell" => Side::Sell,
                _ => return bad_request("side must be 'buy' or 'sell'").into_response(),
            };

            let order_type = match body.order_type.to_lowercase().as_str() {
                "limit" => OrderType::Limit,
                "market" => OrderType::Market,
                _ => return bad_request("order_type must be 'limit' or 'market'").into_response(),
            };

            let tif = match body.tif.as_deref().unwrap_or("GTC").to_uppercase().as_str() {
                "GTC" => TimeInForce::Gtc,
                "FOK" => TimeInForce::Fok,
                "IOC" => TimeInForce::Ioc,
                "GTD" => TimeInForce::Gtd,
                _ => TimeInForce::Gtc,
            };

            let req = CreateOrderRequest {
                market_native_id: body.market_id,
                outcome_id: body.outcome_id,
                side,
                order_type,
                tif,
                price: body.price,
                quantity: body.quantity,
                client_order_id: body.client_order_id,
            };

            match adapter.create_order(req).await {
                Ok(order) => {
                    // Persist the order to storage
                    let stored = order_to_stored(&order, &provider);
                    if let Err(e) = state.storage.save_order(&stored).await {
                        warn!("Failed to persist order {}: {}", order.id, e);
                    }

                    (StatusCode::CREATED, Json(serde_json::to_value(&order).unwrap())).into_response()
                }
                Err(e) => internal_error(&e).into_response(),
            }
        }

        #[derive(Debug, Deserialize, Default)]
        #[allow(dead_code)]
        pub struct OrderListParams {
            pub provider: Option<String>,
            pub market_id: Option<String>,
            pub status: Option<String>,
            pub limit: Option<i32>,
            pub cursor: Option<String>,
        }

        pub async fn list_orders(
            State(state): State<AppState>,
            Query(params): Query<OrderListParams>,
        ) -> impl IntoResponse {
            // First, try to get orders from persistent storage
            let storage_filter = StorageOrderFilter {
                provider: params.provider.clone(),
                market_id: params.market_id.clone(),
                status: params.status.clone(),
                limit: params.limit.unwrap_or(50) as usize,
            };

            if let Ok(stored_orders) = state.storage.list_orders(&storage_filter).await {
                if !stored_orders.is_empty() {
                    return Json(serde_json::json!({
                        "orders": stored_orders,
                        "source": "storage",
                        "pagination": { "cursor": "", "has_more": false, "total": stored_orders.len() },
                    })).into_response();
                }
            }

            // Fall back to querying providers directly
            let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
                vec![pid.clone()]
            } else {
                state.registry.provider_ids()
            };

            let mut all_orders = Vec::new();

            for pid in &provider_ids {
                if let Some(adapter) = state.registry.get(pid) {
                    let filter = crate::adapters::OrderFilter {
                        market_id: params.market_id.clone(),
                        status: None,
                        side: None,
                        pagination: PaginationRequest {
                            limit: params.limit.or(Some(50)),
                            cursor: params.cursor.clone(),
                        },
                    };
                    match adapter.list_orders(filter).await {
                        Ok(page) => {
                            // Persist fetched orders to storage
                            for order in &page.orders {
                                let stored = order_to_stored(order, pid);
                                if let Err(e) = state.storage.save_order(&stored).await {
                                    warn!("Failed to persist order {}: {}", order.id, e);
                                }
                            }
                            all_orders.extend(page.orders);
                        }
                        Err(e) => warn!(provider = %pid, "list_orders failed: {}", e),
                    }
                }
            }

            Json(serde_json::json!({
                "orders": all_orders,
                "source": "provider",
                "pagination": { "cursor": "", "has_more": false, "total": all_orders.len() },
            })).into_response()
        }

        pub async fn get_order(
            State(state): State<AppState>,
            Path(order_id): Path<String>,
            Query(params): Query<OrderListParams>,
        ) -> impl IntoResponse {
            // Check persistent storage first
            if let Ok(Some(stored)) = state.storage.get_order(&order_id).await {
                return Json(serde_json::to_value(&stored).unwrap()).into_response();
            }

            // Fall back to provider
            let provider_id = params.provider.unwrap_or_else(|| "kalshi.com".to_string());
            if let Some(adapter) = state.registry.get(&provider_id) {
                match adapter.get_order(&order_id).await {
                    Ok(order) => {
                        // Persist to storage
                        let stored = order_to_stored(&order, &provider_id);
                        let _ = state.storage.save_order(&stored).await;

                        Json(serde_json::to_value(&order).unwrap()).into_response()
                    }
                    Err(e) => not_found(&format!("Order not found: {}", e)).into_response(),
                }
            } else {
                not_found(&format!("Unknown provider: {}", provider_id)).into_response()
            }
        }

        pub async fn cancel_order(
            State(state): State<AppState>,
            Path(order_id): Path<String>,
            Query(params): Query<OrderListParams>,
        ) -> impl IntoResponse {
            let provider_id = params.provider.unwrap_or_else(|| "kalshi.com".to_string());
            if let Some(adapter) = state.registry.get(&provider_id) {
                match adapter.cancel_order(&order_id).await {
                    Ok(order) => {
                        // Update status in storage
                        if let Err(e) = state.storage.update_order_status(&order_id, "cancelled").await {
                            warn!("Failed to update order {} status in storage: {}", order_id, e);
                        }

                        Json(serde_json::to_value(&order).unwrap()).into_response()
                    }
                    Err(e) => internal_error(&e).into_response(),
                }
            } else {
                not_found(&format!("Unknown provider: {}", provider_id)).into_response()
            }
        }

        #[derive(Debug, Deserialize)]
        pub struct CancelAllBody {
            pub provider: String,
            pub market_id: Option<String>,
        }

        pub async fn cancel_all_orders(
            State(state): State<AppState>,
            Json(body): Json<CancelAllBody>,
        ) -> impl IntoResponse {
            if let Some(adapter) = state.registry.get(&body.provider) {
                match adapter.cancel_all_orders(body.market_id.as_deref()).await {
                    Ok(cancelled) => {
                        // Update all cancelled orders in storage
                        for order_id in &cancelled {
                            if let Err(e) = state.storage.update_order_status(order_id, "cancelled").await {
                                warn!("Failed to update order {} status in storage: {}", order_id, e);
                            }
                        }

                        Json(serde_json::json!({
                            "cancelled": cancelled,
                            "count": cancelled.len(),
                        })).into_response()
                    }
                    Err(e) => internal_error(&e).into_response(),
                }
            } else {
                not_found(&format!("Unknown provider: {}", body.provider)).into_response()
            }
        }

        #[derive(Debug, Deserialize)]
        pub struct EstimateBody {
            pub provider: String,
            pub market_id: String,
            pub outcome_id: String,
            pub side: String,
            pub price: String,
            pub quantity: i64,
        }

        pub async fn estimate_order(
            State(_state): State<AppState>,
            Json(body): Json<EstimateBody>,
        ) -> impl IntoResponse {
            let price: f64 = body.price.parse().unwrap_or(0.5);
            let cost = price * body.quantity as f64;

            Json(serde_json::json!({
                "provider": body.provider,
                "market_id": body.market_id,
                "outcome_id": body.outcome_id,
                "side": body.side,
                "estimated_cost": format!("{:.2}", cost),
                "estimated_fee": "0.00",
                "estimated_total": format!("{:.2}", cost),
                "price": body.price,
                "quantity": body.quantity,
            }))
        }

        pub async fn list_trades(
            State(state): State<AppState>,
            Query(params): Query<OrderListParams>,
        ) -> impl IntoResponse {
            // First, try persistent storage
            let storage_filter = StorageTradeFilter {
                provider: params.provider.clone(),
                market_id: params.market_id.clone(),
                order_id: None,
                limit: params.limit.unwrap_or(50) as usize,
            };

            if let Ok(stored_trades) = state.storage.list_trades(&storage_filter).await {
                if !stored_trades.is_empty() {
                    return Json(serde_json::json!({
                        "trades": stored_trades,
                        "source": "storage",
                        "pagination": { "cursor": "", "has_more": false, "total": stored_trades.len() },
                    })).into_response();
                }
            }

            // Fall back to querying providers directly
            let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
                vec![pid.clone()]
            } else {
                state.registry.provider_ids()
            };

            let mut all_trades = Vec::new();

            for pid in &provider_ids {
                if let Some(adapter) = state.registry.get(pid) {
                    let filter = crate::adapters::TradeFilter {
                        market_id: params.market_id.clone(),
                        order_id: None,
                        pagination: PaginationRequest {
                            limit: params.limit.or(Some(50)),
                            cursor: params.cursor.clone(),
                        },
                    };
                    match adapter.list_trades(filter).await {
                        Ok(page) => {
                            // Persist fetched trades to storage
                            for trade in &page.trades {
                                let stored = trade_to_stored(trade, pid);
                                if let Err(e) = state.storage.save_trade(&stored).await {
                                    warn!("Failed to persist trade {}: {}", trade.id, e);
                                }
                            }
                            all_trades.extend(page.trades);
                        }
                        Err(e) => warn!(provider = %pid, "list_trades failed: {}", e),
                    }
                }
            }

            Json(serde_json::json!({
                "trades": all_trades,
                "source": "provider",
                "pagination": { "cursor": "", "has_more": false, "total": all_trades.len() },
            })).into_response()
        }
    }

    // ── Portfolio ────────────────────────────────────────────
    pub mod portfolio {
        use super::super::*;

        #[derive(Debug, Deserialize, Default)]
        pub struct PortfolioParams {
            pub provider: Option<String>,
        }

        pub async fn list_positions(
            State(state): State<AppState>,
            Query(params): Query<PortfolioParams>,
        ) -> impl IntoResponse {
            let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
                vec![pid.clone()]
            } else {
                state.registry.provider_ids()
            };

            let mut all_positions = Vec::new();
            for pid in &provider_ids {
                if let Some(adapter) = state.registry.get(pid) {
                    match adapter.get_positions().await {
                        Ok(positions) => all_positions.extend(positions),
                        Err(e) => warn!(provider = %pid, "get_positions: {}", e),
                    }
                }
            }

            Json(serde_json::json!({
                "positions": all_positions,
                "total": all_positions.len(),
            }))
        }

        pub async fn get_summary(
            State(state): State<AppState>,
            Query(params): Query<PortfolioParams>,
        ) -> impl IntoResponse {
            let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
                vec![pid.clone()]
            } else {
                state.registry.provider_ids()
            };

            let mut total_value = 0.0_f64;
            let mut total_pnl = 0.0_f64;
            let mut position_count = 0;
            let mut provider_summaries = Vec::new();

            for pid in &provider_ids {
                if let Some(adapter) = state.registry.get(pid) {
                    if let Ok(positions) = adapter.get_positions().await {
                        let prov_value: f64 = positions.iter()
                            .map(|p| p.current_value.parse::<f64>().unwrap_or(0.0)).sum();
                        let prov_pnl: f64 = positions.iter()
                            .map(|p| p.unrealized_pnl.parse::<f64>().unwrap_or(0.0)).sum();
                        total_value += prov_value;
                        total_pnl += prov_pnl;
                        position_count += positions.len();
                        provider_summaries.push(serde_json::json!({
                            "provider": pid,
                            "positions": positions.len(),
                            "value": format!("{:.2}", prov_value),
                            "unrealized_pnl": format!("{:.2}", prov_pnl),
                        }));
                    }
                }
            }

            Json(serde_json::json!({
                "total_value": format!("{:.2}", total_value),
                "unrealized_pnl": format!("{:.2}", total_pnl),
                "position_count": position_count,
                "providers": provider_summaries,
            }))
        }

        pub async fn list_balances(
            State(state): State<AppState>,
            Query(params): Query<PortfolioParams>,
        ) -> impl IntoResponse {
            let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
                vec![pid.clone()]
            } else {
                state.registry.provider_ids()
            };

            let mut all_balances = Vec::new();
            for pid in &provider_ids {
                if let Some(adapter) = state.registry.get(pid) {
                    match adapter.get_balances().await {
                        Ok(balances) => all_balances.extend(balances),
                        Err(e) => warn!(provider = %pid, "get_balances: {}", e),
                    }
                }
            }

            Json(serde_json::json!({ "balances": all_balances }))
        }

        /// GET /upp/v1/portfolio/analytics — Full portfolio analytics with risk scoring
        pub async fn get_analytics(
            State(state): State<AppState>,
            Query(params): Query<PortfolioParams>,
        ) -> impl IntoResponse {
            let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
                vec![pid.clone()]
            } else {
                state.registry.provider_ids()
            };

            let mut all_positions = Vec::new();
            let mut all_trades = Vec::new();
            let market_map = std::collections::HashMap::new();

            for pid in &provider_ids {
                if let Some(adapter) = state.registry.get(pid) {
                    if let Ok(positions) = adapter.get_positions().await {
                        all_positions.extend(positions);
                    }
                    let trade_filter = crate::adapters::TradeFilter::default();
                    if let Ok(trades) = adapter.get_trade_history(trade_filter).await {
                        all_trades.extend(trades);
                    }
                }
            }

            let analytics = crate::core::portfolio::compute_analytics(
                &all_positions,
                &all_trades,
                &market_map,
            );

            Json(serde_json::to_value(&analytics).unwrap_or_default())
        }
    }

    // ── Arbitrage ────────────────────────────────────────────
    pub mod arbitrage {
        use super::super::*;

        #[derive(Debug, Deserialize, Default)]
        pub struct ArbitrageParams {
            pub min_spread: Option<f64>,
            pub min_confidence: Option<f64>,
            pub provider: Option<String>,
            pub limit: Option<usize>,
        }

        /// GET /upp/v1/arbitrage — List active arbitrage opportunities
        pub async fn list_opportunities(
            State(state): State<AppState>,
            Query(params): Query<ArbitrageParams>,
        ) -> impl IntoResponse {
            let mut alerts = state.arbitrage_scanner.get_active_alerts();

            // Filter by min spread
            if let Some(min_spread) = params.min_spread {
                alerts.retain(|a| a.spread_pct >= min_spread);
            }

            // Filter by min confidence
            if let Some(min_conf) = params.min_confidence {
                alerts.retain(|a| a.confidence >= min_conf);
            }

            // Filter by provider (either bid or ask provider)
            if let Some(ref provider) = params.provider {
                alerts.retain(|a| &a.bid_provider == provider || &a.ask_provider == provider);
            }

            // Sort by net profit descending
            alerts.sort_by(|a, b| b.net_profit_per_contract
                .partial_cmp(&a.net_profit_per_contract)
                .unwrap_or(std::cmp::Ordering::Equal));

            // Apply limit
            let limit = params.limit.unwrap_or(50);
            alerts.truncate(limit);

            Json(serde_json::json!({
                "opportunities": alerts,
                "total": alerts.len(),
                "scanner": {
                    "scans_total": state.arbitrage_scanner.scans_total.load(Ordering::Relaxed),
                    "total_detected": state.arbitrage_scanner.opportunities_detected.load(Ordering::Relaxed),
                },
            }))
        }

        /// GET /upp/v1/arbitrage/summary — Get summary statistics
        pub async fn get_summary(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let summary = state.arbitrage_scanner.get_summary().await;
            Json(serde_json::to_value(&summary).unwrap_or_default())
        }

        /// GET /upp/v1/arbitrage/history — Get recent historical alerts
        pub async fn get_history(
            State(state): State<AppState>,
            Query(params): Query<ArbitrageParams>,
        ) -> impl IntoResponse {
            let limit = params.limit.unwrap_or(100);
            let mut history = state.arbitrage_scanner.get_history(limit).await;

            // Filter by min spread
            if let Some(min_spread) = params.min_spread {
                history.retain(|a| a.spread_pct >= min_spread);
            }

            Json(serde_json::json!({
                "history": history,
                "total": history.len(),
            }))
        }
    }

    // ── Price History / Candlesticks ────────────────────────
    pub mod price_history {
        use super::super::*;

        #[derive(Debug, Deserialize, Default)]
        pub struct CandleParams {
            pub outcome_id: Option<String>,
            pub resolution: Option<String>,
            pub from: Option<i64>,
            pub to: Option<i64>,
            pub limit: Option<usize>,
        }

        /// GET /upp/v1/markets/:market_id/candles — Get OHLCV candlestick data
        pub async fn get_candles(
            State(state): State<AppState>,
            Path(market_id): Path<String>,
            Query(params): Query<CandleParams>,
        ) -> impl IntoResponse {
            let outcome_id = params.outcome_id.as_deref().unwrap_or("yes");
            let resolution = params.resolution.as_deref()
                .and_then(crate::core::price_index::Resolution::parse)
                .unwrap_or(crate::core::price_index::Resolution::FiveMinute);
            let limit = params.limit.unwrap_or(100).min(1000);

            let candles = state.price_index.query_candles(
                &market_id,
                outcome_id,
                resolution,
                params.from,
                params.to,
                limit,
            );

            Json(serde_json::json!({
                "market_id": market_id,
                "outcome_id": outcome_id,
                "resolution": resolution,
                "candles": candles,
                "count": candles.len(),
            }))
        }

        /// GET /upp/v1/markets/:market_id/candles/latest — Get the current (incomplete) candle
        pub async fn get_latest_candle(
            State(state): State<AppState>,
            Path(market_id): Path<String>,
            Query(params): Query<CandleParams>,
        ) -> impl IntoResponse {
            let outcome_id = params.outcome_id.as_deref().unwrap_or("yes");
            let resolution = params.resolution.as_deref()
                .and_then(crate::core::price_index::Resolution::parse)
                .unwrap_or(crate::core::price_index::Resolution::OneMinute);

            let candle = state.price_index.latest_candle(&market_id, outcome_id, resolution);

            match candle {
                Some(c) => Json(serde_json::to_value(&c).unwrap_or_default()).into_response(),
                None => not_found("No candle data for this market").into_response(),
            }
        }

        /// GET /upp/v1/price-index/stats — Get price indexer statistics
        pub async fn get_stats(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let stats = state.price_index.stats();
            Json(serde_json::to_value(&stats).unwrap_or_default())
        }
    }

    // ── Smart Order Routing ─────────────────────────────────
    pub mod smart_routing {
        use super::super::*;

        #[derive(Debug, Deserialize)]
        pub struct RouteRequest {
            pub market_native_id: String,
            pub outcome_id: String,
            pub side: String,
            pub quantity: i64,
            pub strategy: Option<String>,
            pub preferred_provider: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        pub struct ExecuteRequest {
            pub market_native_id: String,
            pub outcome_id: String,
            pub side: String,
            pub quantity: i64,
            pub order_type: Option<String>,
            pub tif: Option<String>,
            pub strategy: Option<String>,
            pub preferred_provider: Option<String>,
        }

        /// POST /upp/v1/orders/route — Compute optimal routing plan (dry run)
        pub async fn compute_route(
            State(state): State<AppState>,
            Json(body): Json<RouteRequest>,
        ) -> impl IntoResponse {
            let side = match body.side.to_lowercase().as_str() {
                "buy" => Side::Buy,
                "sell" => Side::Sell,
                _ => return bad_request("Invalid side: must be 'buy' or 'sell'").into_response(),
            };

            let strategy = body.strategy.as_deref()
                .and_then(crate::core::smart_router::RoutingStrategy::parse)
                .unwrap_or(crate::core::smart_router::RoutingStrategy::SplitOptimal);

            match state.smart_router.compute_route(
                &state.registry,
                &body.market_native_id,
                &body.outcome_id,
                side,
                body.quantity,
                strategy,
                body.preferred_provider.as_deref(),
            ).await {
                Ok(plan) => Json(serde_json::to_value(&plan).unwrap_or_default()).into_response(),
                Err(e) => internal_error(&e).into_response(),
            }
        }

        /// POST /upp/v1/orders/route/execute — Compute and execute the routing plan
        pub async fn execute_route(
            State(state): State<AppState>,
            Json(body): Json<ExecuteRequest>,
        ) -> impl IntoResponse {
            let side = match body.side.to_lowercase().as_str() {
                "buy" => Side::Buy,
                "sell" => Side::Sell,
                _ => return bad_request("Invalid side").into_response(),
            };

            let order_type = match body.order_type.as_deref().unwrap_or("limit") {
                "limit" => OrderType::Limit,
                "market" => OrderType::Market,
                _ => return bad_request("Invalid order_type").into_response(),
            };

            let tif = match body.tif.as_deref().unwrap_or("GTC") {
                "GTC" => TimeInForce::Gtc,
                "GTD" => TimeInForce::Gtd,
                "FOK" => TimeInForce::Fok,
                "IOC" => TimeInForce::Ioc,
                _ => return bad_request("Invalid tif").into_response(),
            };

            let strategy = body.strategy.as_deref()
                .and_then(crate::core::smart_router::RoutingStrategy::parse)
                .unwrap_or(crate::core::smart_router::RoutingStrategy::SplitOptimal);

            // First compute the plan
            let plan = match state.smart_router.compute_route(
                &state.registry,
                &body.market_native_id,
                &body.outcome_id,
                side,
                body.quantity,
                strategy,
                body.preferred_provider.as_deref(),
            ).await {
                Ok(p) => p,
                Err(e) => return internal_error(&e).into_response(),
            };

            // Then execute it
            let results = state.smart_router.execute_plan(
                &state.registry,
                &plan,
                side,
                order_type,
                tif,
            ).await;

            Json(serde_json::json!({
                "plan": plan,
                "execution": results,
                "success_count": results.iter().filter(|r| r.status == "placed").count(),
                "total_legs": results.len(),
            })).into_response()
        }

        /// GET /upp/v1/orders/route/stats — Get smart router statistics
        pub async fn get_stats(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let stats = state.smart_router.stats();
            Json(serde_json::to_value(&stats).unwrap_or_default())
        }
    }

    // ── Resolution ───────────────────────────────────────────
    pub mod resolution {
        use super::super::*;

        pub async fn get_resolution(
            State(_state): State<AppState>,
            Path(_id): Path<String>,
        ) -> impl IntoResponse {
            Json(serde_json::json!({ "status": "not_implemented" }))
        }

        pub async fn list_resolutions(
            State(_state): State<AppState>,
        ) -> impl IntoResponse {
            Json(serde_json::json!({ "resolutions": [] }))
        }
    }

    // ── Settlement ───────────────────────────────────────────
    pub mod settlement {
        use super::super::*;

        pub async fn list_instruments(
            State(_state): State<AppState>,
        ) -> impl IntoResponse {
            Json(serde_json::json!({
                "instruments": [
                    { "type": "usd", "name": "US Dollar", "providers": ["kalshi.com"] },
                    { "type": "usdc", "name": "USDC (Polygon)", "providers": ["polymarket.com"] },
                    { "type": "usdc_bnb", "name": "USDC (BNB Chain)", "providers": ["opinion.trade"] },
                ]
            }))
        }

        pub async fn list_handlers(
            State(_state): State<AppState>,
        ) -> impl IntoResponse {
            Json(serde_json::json!({
                "handlers": [
                    { "type": "custodial_usd", "provider": "kalshi.com" },
                    { "type": "onchain_ctf", "provider": "polymarket.com" },
                    { "type": "onchain_bnb", "provider": "opinion.trade" },
                ]
            }))
        }
    }

    // ── WebSocket ────────────────────────────────────────────
    pub mod websocket {
        use super::super::*;
        use axum::extract::ws::{WebSocket, Message};
        use futures::{StreamExt, SinkExt};
        use std::collections::{HashMap, HashSet};
        use tokio::sync::mpsc;
        use tokio::sync::broadcast;

        pub async fn ws_upgrade(
            State(state): State<AppState>,
            ws: WebSocketUpgrade,
        ) -> impl IntoResponse {
            ws.on_upgrade(move |socket| handle_ws(socket, state))
        }

        /// Internal message type for the send queue
        #[derive(Debug, Clone)]
        enum SendMessage {
            /// A fan-out broadcast message
            FanOut(transport::websocket::FanOutMessage),
            /// An RPC response
            JsonRpc(serde_json::Value),
            /// Heartbeat/keepalive ping
            Heartbeat,
        }

        async fn handle_ws(socket: WebSocket, state: AppState) {
            // Increment connection counter
            state.metrics.ws_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Split the socket into sender and receiver halves
            let (ws_sender, ws_receiver) = socket.split();

            // Internal mpsc queue for all outgoing messages
            let (tx_queue, rx_queue) = mpsc::channel::<SendMessage>(256);

            // Track active subscriptions per client (channel -> market_ids)
            let subscriptions = Arc::new(tokio::sync::Mutex::new(HashMap::<String, HashSet<String>>::new()));

            info!("WebSocket client connected");

            // Spawn the receive task (handles incoming RPC messages)
            let rx_handle = {
                let state_clone = state.clone();
                let subscriptions_clone = Arc::clone(&subscriptions);
                let tx_queue_clone = tx_queue.clone();

                tokio::spawn(async move {
                    let mut receiver = ws_receiver;
                    while let Some(Ok(msg)) = receiver.next().await {
                        match msg {
                            Message::Text(text) => {
                                handle_incoming_rpc(&text, &state_clone, &subscriptions_clone, &tx_queue_clone).await;
                            }
                            Message::Ping(_data) => {
                                // Respond with pong automatically (handled by send loop)
                                let _ = tx_queue_clone.send(SendMessage::Heartbeat).await;
                            }
                            Message::Close(_) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                })
            };

            // Subscription fan-out tasks are spawned dynamically in handle_incoming_rpc
            // when clients subscribe to channels. No separate tracking task needed.

            // Spawn the send task (writes messages to the socket)
            let send_handle = {
                let ws_sender_arc = Arc::new(tokio::sync::Mutex::new(ws_sender));
                let _tx_queue_clone = tx_queue.clone();

                tokio::spawn(async move {
                    let mut rx = rx_queue;
                    while let Some(msg) = rx.recv().await {
                        let ws_msg = match msg {
                            SendMessage::FanOut(fan_out) => {
                                match serde_json::to_string(&fan_out) {
                                    Ok(json) => Message::Text(json),
                                    Err(e) => {
                                        warn!("Failed to serialize fan-out message: {}", e);
                                        continue;
                                    }
                                }
                            }
                            SendMessage::JsonRpc(value) => {
                                Message::Text(value.to_string())
                            }
                            SendMessage::Heartbeat => {
                                Message::Ping(vec![])
                            }
                        };

                        let mut sender = ws_sender_arc.lock().await;
                        if sender.send(ws_msg).await.is_err() {
                            break;
                        }
                    }
                })
            };

            // Spawn the heartbeat task (sends a ping every 30 seconds)
            let heartbeat_handle = {
                let tx_queue_clone = tx_queue.clone();

                tokio::spawn(async move {
                    let mut heartbeat_interval = tokio::time::interval(
                        std::time::Duration::from_secs(30)
                    );

                    loop {
                        heartbeat_interval.tick().await;
                        if tx_queue_clone.send(SendMessage::Heartbeat).await.is_err() {
                            break;
                        }
                    }
                })
            };

            // Wait for either RPC receiver or send task to exit
            tokio::select! {
                _ = rx_handle => {
                    info!("RPC receiver task exited");
                }
                _ = send_handle => {
                    info!("Send task exited");
                }
                _ = heartbeat_handle => {
                    info!("Heartbeat task exited");
                }
            }

            // Cleanup: unsubscribe from all channels
            let all_subs = {
                let subs = subscriptions.lock().await;
                subs.clone()
            };
            for (channel, market_ids) in all_subs {
                for market_id in market_ids {
                    state.ws_manager.unsubscribe(&channel, &market_id).await;
                }
            }

            // Decrement connection counter
            state.metrics.ws_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            info!("WebSocket client disconnected");
        }

        /// Handle incoming JSON-RPC messages
        async fn handle_incoming_rpc(
            text: &str,
            state: &AppState,
            subscriptions: &Arc<tokio::sync::Mutex<HashMap<String, HashSet<String>>>>,
            tx_queue: &mpsc::Sender<SendMessage>,
        ) {
            let msg: serde_json::Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(_) => {
                    let _ = tx_queue.send(SendMessage::JsonRpc(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": { "code": -32700, "message": "Parse error" },
                        "id": null
                    }))).await;
                    return;
                }
            };

            let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
            let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let params = msg.get("params").cloned().unwrap_or(serde_json::json!({}));

            let (result, new_task) = match method {
                "subscribe_prices" => {
                    let market_ids = params.get("market_ids")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                        .unwrap_or_default();

                    // Track subscriptions
                    {
                        let mut subs = subscriptions.lock().await;
                        let prices_subs = subs.entry("prices".to_string()).or_insert_with(HashSet::new);
                        for market_id in &market_ids {
                            prices_subs.insert(market_id.clone());
                        }
                    }

                    // Spawn fan-out tasks for each market
                    let tasks = spawn_subscription_tasks(
                        state,
                        "prices",
                        &market_ids,
                        tx_queue.clone(),
                    ).await;

                    let result = serde_json::json!({
                        "subscribed": market_ids,
                        "channel": "prices",
                        "status": "active"
                    });

                    (result, Some(tasks))
                }
                "subscribe_orderbook" => {
                    let market_ids = params.get("market_ids")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                        .unwrap_or_default();

                    // Track subscriptions
                    {
                        let mut subs = subscriptions.lock().await;
                        let orderbook_subs = subs.entry("orderbook".to_string()).or_insert_with(HashSet::new);
                        for market_id in &market_ids {
                            orderbook_subs.insert(market_id.clone());
                        }
                    }

                    // Spawn fan-out tasks for each market
                    let tasks = spawn_subscription_tasks(
                        state,
                        "orderbook",
                        &market_ids,
                        tx_queue.clone(),
                    ).await;

                    let result = serde_json::json!({
                        "subscribed": market_ids,
                        "channel": "orderbook",
                        "status": "active"
                    });

                    (result, Some(tasks))
                }
                "subscribe_arbitrage" => {
                    // Subscribe to arbitrage alerts — optionally filter by market IDs
                    let market_ids = params.get("market_ids")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                        .unwrap_or_default();

                    // If no market_ids specified, subscribe to a global "arbitrage:*" channel
                    let subscribe_ids = if market_ids.is_empty() {
                        vec!["*".to_string()]
                    } else {
                        market_ids.clone()
                    };

                    // Track subscriptions
                    {
                        let mut subs = subscriptions.lock().await;
                        let arb_subs = subs.entry("arbitrage".to_string()).or_insert_with(HashSet::new);
                        for id in &subscribe_ids {
                            arb_subs.insert(id.clone());
                        }
                    }

                    // Spawn fan-out tasks
                    let tasks = spawn_subscription_tasks(
                        state,
                        "arbitrage",
                        &subscribe_ids,
                        tx_queue.clone(),
                    ).await;

                    let result = serde_json::json!({
                        "subscribed": subscribe_ids,
                        "channel": "arbitrage",
                        "filter": if market_ids.is_empty() { "all" } else { "filtered" },
                        "status": "active"
                    });

                    (result, Some(tasks))
                }
                "unsubscribe" => {
                    let channel = params.get("channel").and_then(|v| v.as_str()).unwrap_or("");
                    let market_ids = params.get("market_ids")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>())
                        .unwrap_or_default();

                    // Remove from tracking
                    {
                        let mut subs = subscriptions.lock().await;
                        if let Some(channel_subs) = subs.get_mut(channel) {
                            for market_id in &market_ids {
                                channel_subs.remove(market_id);
                            }
                        }
                    }

                    // Unsubscribe from broadcast channels
                    for market_id in &market_ids {
                        state.ws_manager.unsubscribe(channel, market_id).await;
                    }

                    let result = serde_json::json!({
                        "status": "unsubscribed",
                        "channel": channel,
                        "market_ids": market_ids
                    });

                    (result, None)
                }
                "get_market" => {
                    let market_id = params.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
                    let cache_key = if market_id.starts_with("upp:") {
                        market_id.to_string()
                    } else {
                        format!("upp:{}", market_id)
                    };
                    let result = if let Some(market) = state.cache.get_market(&cache_key).await {
                        serde_json::to_value(&market).unwrap_or(serde_json::json!(null))
                    } else {
                        serde_json::json!({ "error": "Market not cached" })
                    };

                    (result, None)
                }
                "ping" => {
                    (serde_json::json!({ "pong": true }), None)
                }
                _ => {
                    let result = serde_json::json!({
                        "error": format!("Unknown method: {}", method),
                        "available_methods": ["subscribe_prices", "subscribe_orderbook", "subscribe_arbitrage", "unsubscribe", "get_market", "ping"]
                    });

                    (result, None)
                }
            };

            // Send the response
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "result": result,
                "id": id
            });

            let _ = tx_queue.send(SendMessage::JsonRpc(response)).await;

            // If there are new subscription tasks, we'd spawn them here
            // For now, they're spawned inside spawn_subscription_tasks
            let _ = new_task;
        }

        /// Spawn fan-out tasks for a set of market subscriptions
        async fn spawn_subscription_tasks(
            state: &AppState,
            channel: &str,
            market_ids: &[String],
            tx_queue: mpsc::Sender<SendMessage>,
        ) -> Vec<tokio::task::JoinHandle<()>> {
            let mut handles = vec![];

            for market_id in market_ids {
                let state_clone = state.clone();
                let channel_clone = channel.to_string();
                let market_id_clone = market_id.clone();
                let tx_queue_clone = tx_queue.clone();

                let handle = tokio::spawn(async move {
                    // Subscribe to the broadcast channel
                    let mut rx = state_clone.ws_manager.subscribe(
                        &channel_clone,
                        &market_id_clone,
                    ).await;

                    // Forward all messages from broadcast to the send queue
                    loop {
                        match rx.recv().await {
                            Ok(msg) => {
                                if tx_queue_clone.send(SendMessage::FanOut(msg)).await.is_err() {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                // Skip lagged messages and continue
                                debug!("Subscriber lagged on {}: {}", channel_clone, market_id_clone);
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                // Broadcast channel was closed, exit
                                break;
                            }
                        }
                    }

                    // Cleanup on task exit
                    state_clone.ws_manager.unsubscribe(&channel_clone, &market_id_clone).await;
                    debug!("Unsubscribed from {}: {}", channel_clone, market_id_clone);
                });

                handles.push(handle);
            }

            handles
        }
    }

    // ── Health ───────────────────────────────────────────────
    pub mod health {
        use super::super::*;

        pub async fn health() -> impl IntoResponse {
            Json(serde_json::json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
                "protocol": "UPP/2026-03-11",
            }))
        }

        pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
            let providers = state.registry.list_providers().await;
            Json(serde_json::json!({
                "ready": true,
                "providers": providers.len(),
                "provider_ids": state.registry.provider_ids(),
            }))
        }

        pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
            let total = state.metrics.requests_total.load(Ordering::Relaxed);
            let ok = state.metrics.requests_ok.load(Ordering::Relaxed);
            let err = state.metrics.requests_err.load(Ordering::Relaxed);
            let rl = state.metrics.requests_rate_limited.load(Ordering::Relaxed);
            let ws = state.metrics.ws_connections.load(Ordering::Relaxed);
            let ws_channels = state.ws_manager.active_channels().await;
            let ws_subs = state.ws_manager.total_subscribers().await;
            let rl_clients = state.rate_limiter.tracked_clients();

            // Storage metrics
            let stored_orders = state.storage.order_count().await.unwrap_or(0);
            let stored_trades = state.storage.trade_count().await.unwrap_or(0);

            // Arbitrage metrics
            let arb_summary = state.arbitrage_scanner.get_summary().await;
            let arb_active = arb_summary.active_opportunities;
            let arb_scans = arb_summary.total_scans;
            let arb_detected = arb_summary.total_detected;

            // Price index metrics
            let pi_stats = state.price_index.stats();

            // Router metrics
            let router_stats = state.smart_router.stats();

            // Live feed metrics
            let feed_stats = state.live_feed.stats();

            // Ingestion pipeline metrics
            let ingestion_stats = state.ingestion.stats();

            format!(
                "# HELP upp_requests_total Total requests received\n\
                 # TYPE upp_requests_total counter\n\
                 upp_requests_total {}\n\
                 # HELP upp_requests_ok Successful requests\n\
                 # TYPE upp_requests_ok counter\n\
                 upp_requests_ok {}\n\
                 # HELP upp_requests_error Failed requests\n\
                 # TYPE upp_requests_error counter\n\
                 upp_requests_error {}\n\
                 # HELP upp_requests_rate_limited Rate-limited requests\n\
                 # TYPE upp_requests_rate_limited counter\n\
                 upp_requests_rate_limited {}\n\
                 # HELP upp_ws_connections Total WebSocket connections\n\
                 # TYPE upp_ws_connections counter\n\
                 upp_ws_connections {}\n\
                 # HELP upp_ws_active_channels Active broadcast channels\n\
                 # TYPE upp_ws_active_channels gauge\n\
                 upp_ws_active_channels {}\n\
                 # HELP upp_ws_subscribers Total WebSocket subscribers\n\
                 # TYPE upp_ws_subscribers gauge\n\
                 upp_ws_subscribers {}\n\
                 # HELP upp_rate_limit_tracked_clients Tracked rate limit clients\n\
                 # TYPE upp_rate_limit_tracked_clients gauge\n\
                 upp_rate_limit_tracked_clients {}\n\
                 # HELP upp_storage_orders_total Total orders in persistent storage\n\
                 # TYPE upp_storage_orders_total gauge\n\
                 upp_storage_orders_total {}\n\
                 # HELP upp_storage_trades_total Total trades in persistent storage\n\
                 # TYPE upp_storage_trades_total gauge\n\
                 upp_storage_trades_total {}\n\
                 # HELP upp_arbitrage_scans_total Total arbitrage scans performed\n\
                 # TYPE upp_arbitrage_scans_total counter\n\
                 upp_arbitrage_scans_total {}\n\
                 # HELP upp_arbitrage_active Currently active arbitrage opportunities\n\
                 # TYPE upp_arbitrage_active gauge\n\
                 upp_arbitrage_active {}\n\
                 # HELP upp_arbitrage_detected_total Total arbitrage opportunities detected\n\
                 # TYPE upp_arbitrage_detected_total counter\n\
                 upp_arbitrage_detected_total {}\n\
                 # HELP upp_price_index_ticks_total Total price ticks ingested\n\
                 # TYPE upp_price_index_ticks_total counter\n\
                 upp_price_index_ticks_total {}\n\
                 # HELP upp_price_index_markets Markets tracked by price indexer\n\
                 # TYPE upp_price_index_markets gauge\n\
                 upp_price_index_markets {}\n\
                 # HELP upp_router_routes_computed Total routing plans computed\n\
                 # TYPE upp_router_routes_computed counter\n\
                 upp_router_routes_computed {}\n\
                 # HELP upp_router_orders_routed Total orders routed via smart router\n\
                 # TYPE upp_router_orders_routed counter\n\
                 upp_router_orders_routed {}\n\
                 # HELP upp_live_feed_messages_total Total messages received from live feeds\n\
                 # TYPE upp_live_feed_messages_total counter\n\
                 upp_live_feed_messages_total {}\n\
                 # HELP upp_live_feed_reconnects_total Total provider reconnections\n\
                 # TYPE upp_live_feed_reconnects_total counter\n\
                 upp_live_feed_reconnects_total {}\n\
                 # HELP upp_live_feed_providers Registered live feed providers\n\
                 # TYPE upp_live_feed_providers gauge\n\
                 upp_live_feed_providers {}\n\
                 # HELP upp_ingestion_ticks_total Total historical ticks ingested\n\
                 # TYPE upp_ingestion_ticks_total counter\n\
                 upp_ingestion_ticks_total {}\n\
                 # HELP upp_ingestion_markets_processed Markets processed by ingestion pipeline\n\
                 # TYPE upp_ingestion_markets_processed counter\n\
                 upp_ingestion_markets_processed {}\n\
                 # HELP upp_api_keys_total Total API keys created\n\
                 # TYPE upp_api_keys_total gauge\n\
                 upp_api_keys_total {}\n\
                 # HELP upp_api_keys_active Active (non-revoked) API keys\n\
                 # TYPE upp_api_keys_active gauge\n\
                 upp_api_keys_active {}\n",
                total, ok, err, rl, ws, ws_channels, ws_subs, rl_clients,
                stored_orders, stored_trades,
                arb_scans, arb_active, arb_detected,
                pi_stats.ticks_ingested, pi_stats.markets_tracked,
                router_stats.routes_computed, router_stats.orders_routed,
                feed_stats.messages_received_total, feed_stats.reconnects_total,
                feed_stats.providers_registered,
                ingestion_stats.ticks_ingested,
                ingestion_stats.markets_processed,
                state.api_keys.count(),
                state.api_keys.active_count(),
            )
        }
    }

    // ── MCP (Model Context Protocol) & A2A Integration ─────────
    pub mod mcp {
        use super::super::*;

        #[derive(Debug, Deserialize)]
        pub struct McpExecuteRequest {
            pub tool: String,
            pub params: serde_json::Value,
        }

        /// GET /upp/v1/mcp/tools — List all available MCP tools
        pub async fn list_tools() -> impl IntoResponse {
            let tools = crate::core::mcp::list_mcp_tools();
            Json(serde_json::json!({
                "tools": tools,
                "total": tools.len(),
                "mcp_version": "2024-11-05",
            }))
        }

        /// GET /upp/v1/mcp/schema — Return OpenAPI-like schema for all tools
        pub async fn get_schema() -> impl IntoResponse {
            let tools = crate::core::mcp::list_mcp_tools();
            let mut definitions = serde_json::Map::new();

            for tool in &tools {
                definitions.insert(tool.name.clone(), tool.input_schema.clone());
            }

            Json(serde_json::json!({
                "openapi": "3.1.0",
                "info": {
                    "title": "UPP Gateway MCP API",
                    "description": "Model Context Protocol tools for prediction market interactions",
                    "version": "2026-03-11",
                },
                "servers": [
                    {
                        "url": "/upp/v1/mcp",
                        "description": "MCP endpoint"
                    }
                ],
                "x-mcp-tools": tools,
                "components": {
                    "schemas": definitions
                }
            }))
        }

        /// POST /upp/v1/mcp/execute — Execute an MCP tool call
        pub async fn execute_tool(
            State(state): State<AppState>,
            Json(req): Json<McpExecuteRequest>,
        ) -> impl IntoResponse {
            match crate::core::mcp::execute_tool(
                &req.tool,
                req.params,
                &state.registry,
                &state.cache,
            )
            .await
            {
                Ok(result) => {
                    state.metrics.requests_ok.fetch_add(1, Ordering::Relaxed);
                    (StatusCode::OK, Json(serde_json::json!({
                        "tool": req.tool,
                        "result": result,
                        "status": "ok",
                    })))
                    .into_response()
                }
                Err(e) => {
                    state.metrics.requests_err.fetch_add(1, Ordering::Relaxed);
                    (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                        "error": {
                            "code": e.code,
                            "message": e.message,
                            "details": e.details,
                        }
                    })))
                    .into_response()
                }
            }
        }

        /// GET /.well-known/agent.json — Return A2A Agent Card
        pub async fn get_agent_card(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let gateway_url = format!(
                "http://{}:{}/upp/v1/mcp",
                state.config.host,
                state.config.port,
            );

            let card = crate::core::mcp::generate_agent_card(&gateway_url);
            Json(card)
        }
    }

    // ── Live Feed ──────────────────────────────────────────────
    pub mod live_feed {
        use super::super::*;

        /// GET /upp/v1/feeds/status — Get status of all provider WebSocket connections.
        pub async fn feed_status(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let connections = state.live_feed.status().await;
            Json(serde_json::json!({
                "connections": connections,
            }))
        }

        /// GET /upp/v1/feeds/stats — Get global live feed statistics.
        pub async fn feed_stats(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let stats = state.live_feed.stats();
            Json(serde_json::json!(stats))
        }

        /// POST /upp/v1/feeds/subscribe — Subscribe to live feed markets.
        pub async fn subscribe_markets(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let provider_id = body.get("provider_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let market_ids: Vec<String> = body.get("market_ids")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            if provider_id.is_empty() || market_ids.is_empty() {
                return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "provider_id and market_ids required"))).into_response();
            }

            state.live_feed.subscribe_markets(provider_id, market_ids.clone()).await;

            Json(serde_json::json!({
                "status": "subscribed",
                "provider_id": provider_id,
                "market_ids": market_ids,
            })).into_response()
        }
    }

    // ── Dashboard ──────────────────────────────────────────────
    pub mod dashboard {
        use axum::response::{Html, IntoResponse};

        /// GET /dashboard — Serve the monitoring dashboard.
        pub async fn serve_dashboard() -> impl IntoResponse {
            Html(include_str!("../static/dashboard.html"))
        }
    }

    // ── Auth Key Management ─────────────────────────────────────
    pub mod auth_mgmt {
        use super::super::*;
        use crate::middleware::auth::CreateApiKeyRequest;

        /// POST /upp/v1/auth/keys — Create a new API key.
        pub async fn create_key(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let client_name = body.get("client_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            let tier = body.get("tier").and_then(|v| v.as_str()).and_then(|t| {
                match t {
                    "free" => Some(crate::middleware::auth::ClientTier::Free),
                    "standard" => Some(crate::middleware::auth::ClientTier::Standard),
                    "pro" => Some(crate::middleware::auth::ClientTier::Pro),
                    "enterprise" => Some(crate::middleware::auth::ClientTier::Enterprise),
                    _ => None,
                }
            });

            let providers = body.get("providers")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

            let label = body.get("label").and_then(|v| v.as_str()).map(String::from);
            let expires_in_days = body.get("expires_in_days").and_then(|v| v.as_u64()).map(|d| d as u32);

            let req = CreateApiKeyRequest {
                client_name,
                tier,
                providers,
                label,
                expires_in_days,
            };

            let response = state.api_keys.create_key(req);
            (StatusCode::CREATED, Json(serde_json::json!({
                "key": response.key,
                "key_prefix": response.key_prefix,
                "client_id": response.client_id,
                "created_at": response.created_at,
                "expires_at": response.expires_at,
                "warning": "Store this key securely — it will not be shown again."
            })))
        }

        /// GET /upp/v1/auth/keys — List all API keys (redacted).
        pub async fn list_keys(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let keys = state.api_keys.list_keys();
            Json(serde_json::json!({
                "keys": keys,
                "total": keys.len(),
                "active": state.api_keys.active_count(),
            }))
        }

        /// POST /upp/v1/auth/keys/revoke — Revoke an API key by prefix.
        pub async fn revoke_key(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let prefix = body.get("key_prefix")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if prefix.is_empty() {
                return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                    "error": "key_prefix is required"
                }))).into_response();
            }

            let revoked = state.api_keys.revoke_by_prefix(prefix);

            if revoked {
                Json(serde_json::json!({
                    "status": "revoked",
                    "key_prefix": prefix,
                })).into_response()
            } else {
                (StatusCode::NOT_FOUND, Json(serde_json::json!({
                    "error": "Key not found",
                    "key_prefix": prefix,
                }))).into_response()
            }
        }
    }

    // ── Backtesting ────────────────────────────────────────────
    pub mod backtest {
        use super::super::*;
        use crate::core::backtest as bt;
        use std::collections::HashMap;

        /// GET /upp/v1/backtest/strategies — List available strategies.
        pub async fn list_strategies() -> impl IntoResponse {
            let strategies = bt::available_strategies();
            Json(serde_json::json!({
                "strategies": strategies,
            }))
        }

        /// POST /upp/v1/backtest/run — Run a backtest.
        pub async fn run_backtest(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let strategy_name = body.get("strategy").and_then(|v| v.as_str()).unwrap_or("");
            let market_id = body.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
            let outcome_id = body.get("outcome_id").and_then(|v| v.as_str()).unwrap_or("yes");
            let resolution_str = body.get("resolution").and_then(|v| v.as_str()).unwrap_or("1m");

            if strategy_name.is_empty() || market_id.is_empty() {
                return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "strategy and market_id required"))).into_response();
            }

            let resolution = match crate::core::price_index::Resolution::parse(resolution_str) {
                Some(r) => r,
                None => return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "Invalid resolution. Use: 1m, 5m, 1h, 1d"))).into_response(),
            };

            // Parse strategy parameters
            let params: HashMap<String, f64> = body.get("params")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                        .collect()
                })
                .unwrap_or_default();

            let mut strategy = match bt::create_strategy(strategy_name, &params) {
                Some(s) => s,
                None => return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", &format!("Unknown strategy: {}", strategy_name)))).into_response(),
            };

            // Backtest config from request body
            let config = bt::BacktestConfig {
                initial_capital: body.get("initial_capital").and_then(|v| v.as_f64()).unwrap_or(10_000.0),
                fee_rate: body.get("fee_rate").and_then(|v| v.as_f64()).unwrap_or(0.02),
                slippage_rate: body.get("slippage_rate").and_then(|v| v.as_f64()).unwrap_or(0.005),
                max_position: body.get("max_position").and_then(|v| v.as_i64()).unwrap_or(1000),
                risk_free_rate: body.get("risk_free_rate").and_then(|v| v.as_f64()).unwrap_or(0.05),
            };

            match bt::run_backtest_from_index(strategy.as_mut(), &state.price_index, market_id, outcome_id, resolution, &config) {
                Some(result) => Json(serde_json::json!(result)).into_response(),
                None => (StatusCode::NOT_FOUND, Json(upp_error("NOT_FOUND", "Insufficient candle data for backtest (need >= 2 candles)"))).into_response(),
            }
        }

        /// POST /upp/v1/backtest/compare — Compare multiple strategies on the same data.
        pub async fn compare_strategies(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let market_id = body.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
            let outcome_id = body.get("outcome_id").and_then(|v| v.as_str()).unwrap_or("yes");
            let resolution_str = body.get("resolution").and_then(|v| v.as_str()).unwrap_or("1m");
            let strategy_names: Vec<String> = body.get("strategies")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            if market_id.is_empty() || strategy_names.is_empty() {
                return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "market_id and strategies array required"))).into_response();
            }

            let resolution = match crate::core::price_index::Resolution::parse(resolution_str) {
                Some(r) => r,
                None => return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "Invalid resolution"))).into_response(),
            };

            let config = bt::BacktestConfig {
                initial_capital: body.get("initial_capital").and_then(|v| v.as_f64()).unwrap_or(10_000.0),
                fee_rate: body.get("fee_rate").and_then(|v| v.as_f64()).unwrap_or(0.02),
                slippage_rate: body.get("slippage_rate").and_then(|v| v.as_f64()).unwrap_or(0.005),
                max_position: body.get("max_position").and_then(|v| v.as_i64()).unwrap_or(1000),
                risk_free_rate: body.get("risk_free_rate").and_then(|v| v.as_f64()).unwrap_or(0.05),
            };

            let mut results = Vec::new();
            for name in &strategy_names {
                if let Some(mut strategy) = bt::create_strategy(name, &HashMap::new()) {
                    if let Some(result) = bt::run_backtest_from_index(strategy.as_mut(), &state.price_index, market_id, outcome_id, resolution, &config) {
                        results.push(result.metrics);
                    }
                }
            }

            if results.is_empty() {
                return (StatusCode::NOT_FOUND, Json(upp_error("NOT_FOUND", "No valid results — check strategies and candle data availability"))).into_response();
            }

            // Sort by total return descending
            results.sort_by(|a, b| b.total_return_pct.partial_cmp(&a.total_return_pct).unwrap_or(std::cmp::Ordering::Equal));

            Json(serde_json::json!({
                "market_id": market_id,
                "outcome_id": outcome_id,
                "resolution": resolution_str,
                "results": results,
                "best_strategy": results.first().map(|r| &r.strategy_name),
            })).into_response()
        }
    }

    // ── Historical Ingestion ──────────────────────────────────
    pub mod ingestion {
        use super::super::*;

        /// GET /upp/v1/ingestion/stats — Get ingestion pipeline stats.
        pub async fn stats(
            State(state): State<AppState>,
        ) -> impl IntoResponse {
            let s = state.ingestion.stats();
            Json(serde_json::json!(s))
        }

        /// POST /upp/v1/ingestion/ingest — Ingest historical data for a specific market.
        pub async fn ingest_market(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let provider_id = body.get("provider_id").and_then(|v| v.as_str()).unwrap_or("");
            let market_id = body.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
            let hours_back = body.get("hours_back").and_then(|v| v.as_u64()).unwrap_or(24);

            if provider_id.is_empty() || market_id.is_empty() {
                return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "provider_id and market_id required"))).into_response();
            }

            let to = chrono::Utc::now();
            let from = to - chrono::Duration::hours(hours_back as i64);

            match state.ingestion.ingest_market(provider_id, market_id, from, to).await {
                Ok(count) => Json(serde_json::json!({
                    "status": "ok",
                    "ticks_ingested": count,
                    "provider_id": provider_id,
                    "market_id": market_id,
                    "hours_back": hours_back,
                })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INGESTION_ERROR", &e.to_string()))).into_response(),
            }
        }

        /// POST /upp/v1/ingestion/ingest-recent — Bulk-ingest recent data from all providers.
        pub async fn ingest_recent(
            State(state): State<AppState>,
            Json(body): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            let hours_back = body.get("hours_back").and_then(|v| v.as_u64()).unwrap_or(1);

            match state.ingestion.ingest_all_recent(hours_back).await {
                Ok(count) => Json(serde_json::json!({
                    "status": "ok",
                    "ticks_ingested": count,
                    "hours_back": hours_back,
                })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INGESTION_ERROR", &e.to_string()))).into_response(),
            }
        }
    }
}
