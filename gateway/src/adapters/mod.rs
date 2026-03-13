// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Provider adapter trait and implementations.
//
// The UppProvider trait defines the contract every prediction market
// provider must implement. The gateway routes requests to the correct
// adapter based on the provider field in the request.

pub mod kalshi;
pub mod polymarket;
pub mod opinion;

use crate::core::types::*;
use anyhow::Result;
use async_trait::async_trait;
use std::pin::Pin;
use futures::Stream;

/// Stream of real-time price updates from a provider.
pub type PriceStream = Pin<Box<dyn Stream<Item = PriceUpdate> + Send>>;

/// Stream of real-time orderbook updates from a provider.
pub type OrderBookStream = Pin<Box<dyn Stream<Item = OrderBookUpdate> + Send>>;

/// A real-time price update event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PriceUpdate {
    pub market_id: UniversalMarketId,
    pub prices: std::collections::HashMap<String, String>, // outcome_id -> price
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A real-time orderbook update event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderBookUpdate {
    pub market_id: UniversalMarketId,
    pub outcome_id: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderBookLevel {
    pub price: String,
    pub quantity: i64,
}

// ─── Provider Trait ──────────────────────────────────────────

/// The core adapter trait. Each prediction market provider implements this.
///
/// Design principles:
/// - All methods return UPP-normalized types (not provider-native types)
/// - All prices are decimal strings in [0.00, 1.00] probability format
/// - All IDs are UPP universal IDs
/// - Adapters handle provider-specific auth, rate limiting, and quirks
#[async_trait]
pub trait UppProvider: Send + Sync {
    /// Provider identifier (e.g. "kalshi.com")
    fn provider_id(&self) -> &str;

    /// Provider display name
    fn provider_name(&self) -> &str;

    /// Get the provider's capability manifest.
    fn manifest(&self) -> ProviderManifest;

    // ── Markets Capability ──────────────────────────────────

    /// List markets with optional filtering.
    async fn list_markets(&self, filter: MarketFilter) -> Result<MarketPage>;

    /// Get a single market by native ID.
    async fn get_market(&self, native_id: &str) -> Result<Market>;

    /// Search markets by query string.
    async fn search_markets(&self, query: &str, filter: MarketFilter) -> Result<MarketPage>;

    /// Get the current orderbook for a market outcome.
    async fn get_orderbook(&self, native_id: &str, outcome_id: Option<&str>, depth: i32) -> Result<Vec<OrderBookSnapshot>>;

    // ── Trading Capability ──────────────────────────────────

    /// Place a new order.
    async fn create_order(&self, req: CreateOrderRequest) -> Result<Order>;

    /// Cancel an existing order.
    async fn cancel_order(&self, provider_order_id: &str) -> Result<Order>;

    /// Cancel all open orders, optionally for a specific market.
    async fn cancel_all_orders(&self, market_native_id: Option<&str>) -> Result<Vec<String>>;

    /// Get a specific order by provider order ID.
    async fn get_order(&self, provider_order_id: &str) -> Result<Order>;

    /// List orders with filtering.
    async fn list_orders(&self, filter: OrderFilter) -> Result<OrderPage>;

    /// List executed trades.
    async fn list_trades(&self, filter: TradeFilter) -> Result<TradePage>;

    // ── Portfolio Capability ────────────────────────────────

    /// Get current positions.
    async fn get_positions(&self) -> Result<Vec<Position>>;

    /// Get account balances.
    async fn get_balances(&self) -> Result<Vec<Balance>>;

    /// Get trade history.
    async fn get_trade_history(&self, filter: TradeFilter) -> Result<Vec<Trade>>;

    // ── Streaming (Optional) ────────────────────────────────

    /// Subscribe to real-time price updates for specified markets.
    /// Returns None if provider doesn't support streaming.
    async fn subscribe_prices(&self, _market_ids: Vec<String>) -> Option<PriceStream> {
        None // Default: not supported
    }

    /// Subscribe to real-time orderbook updates.
    async fn subscribe_orderbook(&self, _market_ids: Vec<String>) -> Option<OrderBookStream> {
        None
    }

    // ── Normalization Helpers ────────────────────────────────

    /// Convert a provider-native price to UPP probability format [0.00, 1.00]
    fn normalize_price(&self, raw_price: &str) -> Result<String>;

    /// Convert a UPP probability to provider-native price format
    fn denormalize_price(&self, probability: &str) -> Result<String>;

    /// Generate UPP universal ID from native ID
    fn to_universal_id(&self, native_id: &str) -> UniversalMarketId {
        UniversalMarketId::new(self.provider_id(), native_id)
    }

    // ── Health ──────────────────────────────────────────────

    /// Check if the provider API is healthy and reachable.
    async fn health_check(&self) -> Result<ProviderHealth>;
}

// ─── Supporting Types ────────────────────────────────────────

/// Provider capability manifest (JSON-serializable for /.well-known/upp)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderManifest {
    pub upp_version: String,
    pub provider: ProviderInfo,
    pub capabilities: Vec<CapabilityDeclaration>,
    pub transport: TransportInfo,
    pub authentication: Vec<String>,
    pub rate_limits: Option<RateLimitInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub id: String,
    pub provider_type: String,
    pub jurisdictions: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapabilityDeclaration {
    pub name: String,
    pub version: String,
    pub operations: Vec<String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransportInfo {
    pub rest_base_url: Option<String>,
    pub websocket_url: Option<String>,
    pub grpc_endpoint: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RateLimitInfo {
    pub requests_per_second: i32,
    pub requests_per_minute: i32,
    pub tier: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Balance {
    pub provider: String,
    pub instrument_type: String,
    pub available: String,
    pub reserved: String,
    pub total: String,
    pub currency: String,
}

/// Market listing filter.
#[derive(Debug, Clone, Default)]
pub struct MarketFilter {
    pub provider: Option<String>,
    pub category: Option<String>,
    pub status: Option<MarketStatus>,
    pub market_type: Option<MarketType>,
    pub tags: Vec<String>,
    pub sort_by: Option<String>,
    pub pagination: PaginationRequest,
}

/// Paginated market result.
#[derive(Debug, Clone)]
pub struct MarketPage {
    pub markets: Vec<Market>,
    pub pagination: PaginationResponse,
}

/// Orderbook snapshot for one outcome.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderBookSnapshot {
    pub outcome_id: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub asks_computed: bool,
}

/// Order creation request.
#[derive(Debug, Clone)]
pub struct CreateOrderRequest {
    pub market_native_id: String,
    pub outcome_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub tif: TimeInForce,
    pub price: Option<String>,
    pub quantity: i64,
    pub client_order_id: Option<String>,
}

/// Order listing filter.
#[derive(Debug, Clone, Default)]
pub struct OrderFilter {
    pub market_id: Option<String>,
    pub status: Option<OrderStatus>,
    pub side: Option<Side>,
    pub pagination: PaginationRequest,
}

/// Paginated order result.
#[derive(Debug, Clone)]
pub struct OrderPage {
    pub orders: Vec<Order>,
    pub pagination: PaginationResponse,
}

/// Trade listing filter.
#[derive(Debug, Clone, Default)]
pub struct TradeFilter {
    pub market_id: Option<String>,
    pub order_id: Option<String>,
    pub pagination: PaginationRequest,
}

/// Paginated trade result.
#[derive(Debug, Clone)]
pub struct TradePage {
    pub trades: Vec<Trade>,
    pub pagination: PaginationResponse,
}

/// Provider health status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderHealth {
    pub provider: String,
    pub healthy: bool,
    pub status: String,
    pub latency_ms: u64,
}
