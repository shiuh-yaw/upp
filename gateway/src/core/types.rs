// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Core UPP types — Rust representations of the protocol data models.
// These are the in-memory types used by the gateway. They map 1:1 to
// the Protobuf definitions but use Rust-native types for ergonomics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Universal Market ID ─────────────────────────────────────

/// Universal Market Identifier: "upp:{provider}:{native_id}"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct UniversalMarketId {
    pub provider: String,
    pub native_id: String,
}

impl UniversalMarketId {
    pub fn new(provider: &str, native_id: &str) -> Self {
        Self {
            provider: provider.to_string(),
            native_id: native_id.to_string(),
        }
    }

    /// Format as "upp:{provider}:{native_id}"
    pub fn to_full_id(&self) -> String {
        format!("upp:{}:{}", self.provider, self.native_id)
    }

    /// Parse from "upp:{provider}:{native_id}" format
    pub fn parse(full_id: &str) -> Option<Self> {
        let parts: Vec<&str> = full_id.splitn(3, ':').collect();
        if parts.len() == 3 && parts[0] == "upp" {
            Some(Self {
                provider: parts[1].to_string(),
                native_id: parts[2].to_string(),
            })
        } else {
            None
        }
    }
}

impl std::fmt::Display for UniversalMarketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "upp:{}:{}", self.provider, self.native_id)
    }
}

// ─── Market ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: UniversalMarketId,
    pub event: Event,
    pub market_type: MarketType,
    pub outcomes: Vec<Outcome>,
    pub pricing: MarketPricing,
    pub volume: MarketVolume,
    pub lifecycle: MarketLifecycle,
    pub rules: MarketRules,
    pub regulatory: MarketRegulatory,
    #[serde(default)]
    pub provider_metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub image_url: Option<String>,
    pub series_id: Option<String>,
    pub series_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub id: String,
    pub label: String,
    pub token_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketType {
    Binary,
    Categorical,
    Scalar,
}

// ─── Pricing ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPricing {
    pub last_price: HashMap<String, String>,
    pub best_bid: HashMap<String, String>,
    pub best_ask: HashMap<String, String>,
    pub mid_price: HashMap<String, String>,
    pub spread: HashMap<String, String>,
    pub tick_size: String,
    pub currency: String,
    pub min_order_size: i32,
    pub max_order_size: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketVolume {
    pub total_volume: String,
    pub volume_24h: String,
    pub volume_7d: Option<String>,
    pub open_interest: String,
    pub num_traders: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketLifecycle {
    pub status: MarketStatus,
    pub created_at: DateTime<Utc>,
    pub opens_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub resolution_source: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketStatus {
    Pending,
    Open,
    Halted,
    Closed,
    Resolved,
    Disputed,
    Voided,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRules {
    pub allowed_order_types: Vec<OrderType>,
    pub allowed_tif: Vec<TimeInForce>,
    pub allows_short_selling: bool,
    pub allows_partial_fill: bool,
    pub maker_fee_rate: String,
    pub taker_fee_rate: String,
    pub max_position_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRegulatory {
    pub jurisdiction: String,
    pub compliant: bool,
    pub eligible_regions: Vec<String>,
    pub restricted_regions: Vec<String>,
    pub regulator: String,
    pub license_type: String,
    pub contract_type: String,
    pub required_kyc: KycLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KycLevel {
    None,
    Basic,
    Enhanced,
    Institutional,
}

// ─── Order ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub provider_order_id: String,
    pub client_order_id: Option<String>,
    pub market_id: UniversalMarketId,
    pub outcome_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub tif: TimeInForce,
    pub price: Option<String>,
    pub quantity: i64,
    pub filled_quantity: i64,
    pub average_fill_price: Option<String>,
    pub remaining_quantity: i64,
    pub status: OrderStatus,
    pub fees: OrderFees,
    pub cost_basis: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Gtd,
    Fok,
    Ioc,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderFees {
    pub maker_fee: String,
    pub taker_fee: String,
    pub total_fee: String,
}

// ─── Position ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: UniversalMarketId,
    pub outcome_id: String,
    pub quantity: i64,
    pub average_entry_price: String,
    pub current_price: String,
    pub cost_basis: String,
    pub current_value: String,
    pub unrealized_pnl: String,
    pub realized_pnl: String,
    pub status: PositionStatus,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub market_title: String,
    pub market_status: MarketStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PositionStatus {
    Open,
    Closed,
    Settled,
    Expired,
}

// ─── Trade ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub order_id: String,
    pub market_id: UniversalMarketId,
    pub outcome_id: String,
    pub side: Side,
    pub price: String,
    pub quantity: i64,
    pub notional: String,
    pub role: TradeRole,
    pub fees: OrderFees,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TradeRole {
    Maker,
    Taker,
}

// ─── Pagination ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaginationRequest {
    pub limit: Option<i32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationResponse {
    pub cursor: String,
    pub has_more: bool,
    pub total: i32,
}

// ─── Error ───────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UppError {
    pub code: String,
    pub message: String,
    pub provider: Option<String>,
    pub request_id: String,
    #[serde(default)]
    pub details: HashMap<String, String>,
}

#[allow(dead_code)]
impl UppError {
    pub fn not_found(msg: &str) -> Self {
        Self {
            code: "NOT_FOUND".to_string(),
            message: msg.to_string(),
            provider: None,
            request_id: uuid::Uuid::new_v4().to_string(),
            details: HashMap::new(),
        }
    }

    pub fn internal(msg: &str) -> Self {
        Self {
            code: "INTERNAL".to_string(),
            message: msg.to_string(),
            provider: None,
            request_id: uuid::Uuid::new_v4().to_string(),
            details: HashMap::new(),
        }
    }
}
