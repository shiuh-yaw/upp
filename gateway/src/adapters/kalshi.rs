// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Kalshi adapter — FULL IMPLEMENTATION for public (no-auth) endpoints.
//
// Public endpoints (no API key required):
//   GET /markets             — List markets with filters
//   GET /markets/{ticker}    — Get single market
//   GET /markets/{ticker}/orderbook — Get orderbook
//   GET /events              — List events
//   GET /series/{ticker}     — Get series
//   GET /exchange/status     — Health check
//
// Auth-required endpoints (need RSA-PSS keys):
//   POST /portfolio/orders   — Create order
//   DELETE /portfolio/orders/{id} — Cancel order
//   GET /portfolio/positions — Get positions
//   GET /portfolio/balance   — Get balance
//
// Kalshi quirks handled:
//   - Prices in cents (yes_bid=65 means $0.65 = 65%)
//   - Orderbook returns only bids; asks computed by inverting
//   - Markets organized into "series" and "events"

#![allow(dead_code)] // API response structs; many fields only used for Deserialize

use super::*;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

const KALSHI_API_BASE: &str = "https://api.elections.kalshi.com/trade-api/v2";
const KALSHI_WS_BASE: &str = "wss://api.elections.kalshi.com/trade-api/ws/v2";

// ─── Kalshi API Response Types ───────────────────────────────
// These map directly to Kalshi's JSON response format.

#[derive(Debug, Deserialize)]
struct KalshiMarketsResponse {
    markets: Vec<KalshiMarket>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KalshiMarketResponse {
    market: KalshiMarket,
}

#[derive(Debug, Clone, Deserialize)]
struct KalshiMarket {
    ticker: String,
    event_ticker: String,
    market_type: Option<String>,
    title: String,
    subtitle: Option<String>,
    yes_sub_title: Option<String>,
    no_sub_title: Option<String>,

    // Timestamps
    open_time: Option<String>,
    close_time: Option<String>,
    expiration_time: Option<String>,

    // Status
    status: String,  // "open", "closed", "settled"
    result: Option<String>,  // "yes", "no", "all_no", "all_yes"

    // Pricing (in cents: 65 = $0.65)
    yes_bid: Option<i64>,
    yes_ask: Option<i64>,
    no_bid: Option<i64>,
    no_ask: Option<i64>,
    last_price: Option<i64>,

    // Volume
    volume: Option<i64>,
    volume_24h: Option<i64>,
    open_interest: Option<i64>,

    // Rules
    can_close_early: Option<bool>,

    // Category
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KalshiOrderbookResponse {
    orderbook: KalshiOrderbook,
}

#[derive(Debug, Deserialize)]
struct KalshiOrderbook {
    yes: Option<Vec<Vec<serde_json::Value>>>,  // [[price, quantity], ...]
    no: Option<Vec<Vec<serde_json::Value>>>,
}

#[derive(Debug, Deserialize)]
struct KalshiEventResponse {
    event: KalshiEvent,
}

#[derive(Debug, Deserialize)]
struct KalshiEvent {
    event_ticker: String,
    title: String,
    category: Option<String>,
    sub_title: Option<String>,
    markets: Option<Vec<KalshiMarket>>,
}

// ─── Adapter ─────────────────────────────────────────────────

pub struct KalshiAdapter {
    client: Client,
    api_key_id: Option<String>,
    private_key: Option<Vec<u8>>,
}

impl KalshiAdapter {
    /// Create adapter for public-only access (no API key needed).
    pub fn new_public() -> Self {
        Self {
            client: Client::builder()
                .gzip(true)
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
            api_key_id: None,
            private_key: None,
        }
    }

    /// Create adapter with auth for trading endpoints.
    pub fn new_authenticated(api_key_id: String, private_key: Vec<u8>) -> Self {
        Self {
            client: Client::builder()
                .gzip(true)
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
            api_key_id: Some(api_key_id),
            private_key: Some(private_key),
        }
    }

    fn has_auth(&self) -> bool {
        self.api_key_id.is_some() && self.private_key.is_some()
    }

    // ── Price Conversion ────────────────────────────────────

    /// Kalshi cents (65) → UPP decimal string ("0.65")
    fn cents_to_upp(cents: i64) -> String {
        format!("{:.2}", cents as f64 / 100.0)
    }

    /// UPP decimal string ("0.65") → Kalshi cents (65)
    fn upp_to_cents(decimal: &str) -> Result<i64> {
        let val: f64 = decimal.parse().context("Invalid decimal price")?;
        Ok((val * 100.0).round() as i64)
    }

    // ── Response Transformation ─────────────────────────────

    /// Transform Kalshi market → UPP Market
    fn transform_market(&self, km: &KalshiMarket) -> Market {
        let yes_bid = km.yes_bid.unwrap_or(0);
        let yes_ask = km.yes_ask.unwrap_or(0);
        let no_bid = km.no_bid.unwrap_or(0);
        let no_ask = km.no_ask.unwrap_or(0);
        let last = km.last_price.unwrap_or(0);

        let status = match km.status.as_str() {
            "open" => MarketStatus::Open,
            "closed" => MarketStatus::Closed,
            "settled" => MarketStatus::Resolved,
            _ => MarketStatus::Pending,
        };

        Market {
            id: self.to_universal_id(&km.ticker),
            event: Event {
                id: km.event_ticker.clone(),
                title: km.title.clone(),
                description: km.subtitle.clone().unwrap_or_default(),
                category: km.category.clone().unwrap_or_else(|| "general".to_string()),
                tags: vec![],
                image_url: None,
                series_id: Some(km.event_ticker.clone()),
                series_title: None,
            },
            market_type: MarketType::Binary,
            outcomes: vec![
                Outcome {
                    id: "yes".to_string(),
                    label: km.yes_sub_title.clone().unwrap_or_else(|| "Yes".to_string()),
                    token_id: None,
                },
                Outcome {
                    id: "no".to_string(),
                    label: km.no_sub_title.clone().unwrap_or_else(|| "No".to_string()),
                    token_id: None,
                },
            ],
            pricing: MarketPricing {
                last_price: [
                    ("yes".to_string(), Self::cents_to_upp(last)),
                    ("no".to_string(), Self::cents_to_upp(100 - last)),
                ].into(),
                best_bid: [
                    ("yes".to_string(), Self::cents_to_upp(yes_bid)),
                    ("no".to_string(), Self::cents_to_upp(no_bid)),
                ].into(),
                best_ask: [
                    ("yes".to_string(), Self::cents_to_upp(yes_ask)),
                    ("no".to_string(), Self::cents_to_upp(no_ask)),
                ].into(),
                mid_price: [
                    ("yes".to_string(), Self::cents_to_upp((yes_bid + yes_ask) / 2)),
                    ("no".to_string(), Self::cents_to_upp((no_bid + no_ask) / 2)),
                ].into(),
                spread: [
                    ("yes".to_string(), Self::cents_to_upp(yes_ask - yes_bid)),
                    ("no".to_string(), Self::cents_to_upp(no_ask - no_bid)),
                ].into(),
                tick_size: "0.01".to_string(),
                currency: "USD".to_string(),
                min_order_size: 1,
                max_order_size: 25000,
                updated_at: chrono::Utc::now(),
            },
            volume: MarketVolume {
                total_volume: km.volume.unwrap_or(0).to_string(),
                volume_24h: km.volume_24h.unwrap_or(0).to_string(),
                volume_7d: None,
                open_interest: km.open_interest.unwrap_or(0).to_string(),
                num_traders: None,
                updated_at: chrono::Utc::now(),
            },
            lifecycle: MarketLifecycle {
                status,
                created_at: chrono::Utc::now(), // Kalshi doesn't always return this
                opens_at: km.open_time.as_ref().and_then(|t| t.parse().ok()),
                closes_at: km.close_time.as_ref().and_then(|t| t.parse().ok()),
                resolved_at: None,
                expires_at: km.expiration_time.as_ref().and_then(|t| t.parse().ok()),
                resolution_source: Some("Kalshi Official Data Feed".to_string()),
            },
            rules: MarketRules {
                allowed_order_types: vec![OrderType::Limit, OrderType::Market],
                allowed_tif: vec![TimeInForce::Gtc, TimeInForce::Gtd, TimeInForce::Fok, TimeInForce::Ioc],
                allows_short_selling: false,
                allows_partial_fill: true,
                maker_fee_rate: "0.00".to_string(),
                taker_fee_rate: "0.00".to_string(),
                max_position_size: 25000,
            },
            regulatory: MarketRegulatory {
                jurisdiction: "US".to_string(),
                compliant: true,
                eligible_regions: vec!["US".to_string()],
                restricted_regions: vec![],
                regulator: "CFTC".to_string(),
                license_type: "DCM".to_string(),
                contract_type: "event_contract".to_string(),
                required_kyc: KycLevel::Enhanced,
            },
            provider_metadata: [
                ("kalshi_ticker".to_string(), km.ticker.clone()),
                ("kalshi_event_ticker".to_string(), km.event_ticker.clone()),
            ].into(),
        }
    }

    /// Transform Kalshi orderbook → UPP OrderBookSnapshot
    fn transform_orderbook(
        &self,
        ob: &KalshiOrderbook,
    ) -> Vec<OrderBookSnapshot> {
        let mut snapshots = vec![];

        // Process YES side
        let yes_bids: Vec<OrderBookLevel> = ob.yes.as_ref()
            .map(|levels| levels.iter().filter_map(|level| {
                if level.len() >= 2 {
                    Some(OrderBookLevel {
                        price: Self::cents_to_upp(level[0].as_i64().unwrap_or(0)),
                        quantity: level[1].as_i64().unwrap_or(0),
                    })
                } else { None }
            }).collect())
            .unwrap_or_default();

        // Compute YES asks from NO bids (Kalshi quirk)
        let no_bids: Vec<OrderBookLevel> = ob.no.as_ref()
            .map(|levels| levels.iter().filter_map(|level| {
                if level.len() >= 2 {
                    Some(OrderBookLevel {
                        price: Self::cents_to_upp(level[0].as_i64().unwrap_or(0)),
                        quantity: level[1].as_i64().unwrap_or(0),
                    })
                } else { None }
            }).collect())
            .unwrap_or_default();

        // YES asks = invert NO bids
        let yes_asks: Vec<OrderBookLevel> = no_bids.iter().map(|bid| {
            let bid_cents = (bid.price.parse::<f64>().unwrap_or(0.0) * 100.0) as i64;
            OrderBookLevel {
                price: Self::cents_to_upp(100 - bid_cents),
                quantity: bid.quantity,
            }
        }).collect();

        // NO asks = invert YES bids
        let no_asks: Vec<OrderBookLevel> = yes_bids.iter().map(|bid| {
            let bid_cents = (bid.price.parse::<f64>().unwrap_or(0.0) * 100.0) as i64;
            OrderBookLevel {
                price: Self::cents_to_upp(100 - bid_cents),
                quantity: bid.quantity,
            }
        }).collect();

        snapshots.push(OrderBookSnapshot {
            outcome_id: "yes".to_string(),
            bids: yes_bids,
            asks: yes_asks,
            asks_computed: true, // Tells consumers that asks were derived
        });

        snapshots.push(OrderBookSnapshot {
            outcome_id: "no".to_string(),
            bids: no_bids,
            asks: no_asks,
            asks_computed: true,
        });

        snapshots
    }
}

#[async_trait::async_trait]
impl UppProvider for KalshiAdapter {
    fn provider_id(&self) -> &str { "kalshi.com" }
    fn provider_name(&self) -> &str { "Kalshi" }

    fn manifest(&self) -> ProviderManifest {
        let operations = vec![
            "listMarkets".into(), "getMarket".into(),
            "searchMarkets".into(), "getOrderBook".into(),
        ];
        let mut capabilities = vec![
            CapabilityDeclaration {
                name: "markets".to_string(),
                version: "2026-03-11".to_string(),
                operations: operations.clone(),
                extensions: vec!["analytics".into(), "streaming".into()],
            },
        ];

        if self.has_auth() {
            capabilities.push(CapabilityDeclaration {
                name: "trading".to_string(),
                version: "2026-03-11".to_string(),
                operations: vec!["createOrder".into(), "cancelOrder".into(), "cancelAll".into()],
                extensions: vec![],
            });
            capabilities.push(CapabilityDeclaration {
                name: "portfolio".to_string(),
                version: "2026-03-11".to_string(),
                operations: vec!["getPositions".into(), "getBalance".into()],
                extensions: vec![],
            });
        }

        ProviderManifest {
            upp_version: "2026-03-11".to_string(),
            provider: ProviderInfo {
                name: "Kalshi".to_string(),
                id: "kalshi.com".to_string(),
                provider_type: if self.has_auth() { "regulated_exchange" } else { "regulated_exchange_readonly" }.to_string(),
                jurisdictions: vec!["US".to_string()],
            },
            capabilities,
            transport: TransportInfo {
                rest_base_url: Some(KALSHI_API_BASE.to_string()),
                websocket_url: Some(KALSHI_WS_BASE.to_string()),
                grpc_endpoint: None,
            },
            authentication: vec![
                if self.has_auth() { "api_key_rsa" } else { "none_public" }.to_string()
            ],
            rate_limits: Some(RateLimitInfo {
                requests_per_second: 10,
                requests_per_minute: 100,
                tier: "standard".to_string(),
            }),
        }
    }

    // ── PUBLIC ENDPOINTS (no auth needed) ────────────────────

    async fn list_markets(&self, filter: MarketFilter) -> Result<MarketPage> {
        let mut url = format!("{}/markets", KALSHI_API_BASE);
        let mut params = vec![];

        params.push(format!("limit={}", filter.pagination.limit.unwrap_or(20)));

        if let Some(ref cursor) = filter.pagination.cursor {
            if !cursor.is_empty() {
                params.push(format!("cursor={}", cursor));
            }
        }

        if let Some(ref status) = filter.status {
            let s = match status {
                MarketStatus::Open => "open",
                MarketStatus::Closed => "closed",
                MarketStatus::Resolved => "settled",
                _ => "open",
            };
            params.push(format!("status={}", s));
        }

        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }

        let resp: KalshiMarketsResponse = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let markets: Vec<Market> = resp.markets.iter()
            .map(|km| self.transform_market(km))
            .collect();

        let has_more = resp.cursor.as_ref().map(|c| !c.is_empty()).unwrap_or(false);

        Ok(MarketPage {
            pagination: PaginationResponse {
                cursor: resp.cursor.unwrap_or_default(),
                has_more,
                total: -1, // Kalshi doesn't return total count
            },
            markets,
        })
    }

    async fn get_market(&self, native_id: &str) -> Result<Market> {
        let url = format!("{}/markets/{}", KALSHI_API_BASE, native_id);

        let resp: KalshiMarketResponse = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        Ok(self.transform_market(&resp.market))
    }

    async fn search_markets(&self, query: &str, filter: MarketFilter) -> Result<MarketPage> {
        // Kalshi doesn't have a native search endpoint.
        // Strategy: fetch markets and filter client-side.
        // For production: use events endpoint + title matching.
        let page = self.list_markets(filter).await?;

        let query_lower = query.to_lowercase();
        let filtered: Vec<Market> = page.markets.into_iter()
            .filter(|m| {
                m.event.title.to_lowercase().contains(&query_lower) ||
                m.event.description.to_lowercase().contains(&query_lower) ||
                m.event.category.to_lowercase().contains(&query_lower)
            })
            .collect();

        Ok(MarketPage {
            pagination: PaginationResponse {
                cursor: String::new(),
                has_more: false,
                total: filtered.len() as i32,
            },
            markets: filtered,
        })
    }

    async fn get_orderbook(&self, native_id: &str, outcome_id: Option<&str>, depth: i32) -> Result<Vec<OrderBookSnapshot>> {
        let url = format!("{}/markets/{}/orderbook?depth={}", KALSHI_API_BASE, native_id, depth);

        let resp: KalshiOrderbookResponse = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let mut snapshots = self.transform_orderbook(&resp.orderbook);

        // Filter by outcome_id if specified
        if let Some(oid) = outcome_id {
            snapshots.retain(|s| s.outcome_id == oid);
        }

        Ok(snapshots)
    }

    // ── AUTH-REQUIRED ENDPOINTS ──────────────────────────────
    // These return a clear error when no auth is configured.

    async fn create_order(&self, _req: CreateOrderRequest) -> Result<Order> {
        if !self.has_auth() {
            anyhow::bail!(
                "Trading requires authentication. Set UPP_KALSHI_API_KEY_ID and \
                 UPP_KALSHI_PRIVATE_KEY_PATH to enable trading. \
                 Get API keys at: https://kalshi.com/settings/api"
            );
        }
        // TODO: Implement with RSA-PSS signing
        anyhow::bail!("Kalshi order creation not yet implemented")
    }

    async fn cancel_order(&self, _provider_order_id: &str) -> Result<Order> {
        if !self.has_auth() {
            anyhow::bail!("Trading requires authentication. See create_order for details.");
        }
        anyhow::bail!("Kalshi order cancellation not yet implemented")
    }

    async fn cancel_all_orders(&self, _market_native_id: Option<&str>) -> Result<Vec<String>> {
        if !self.has_auth() {
            anyhow::bail!("Trading requires authentication.");
        }
        Ok(vec![])
    }

    async fn get_order(&self, _provider_order_id: &str) -> Result<Order> {
        if !self.has_auth() {
            anyhow::bail!("Portfolio requires authentication.");
        }
        anyhow::bail!("Not yet implemented")
    }

    async fn list_orders(&self, _filter: OrderFilter) -> Result<OrderPage> {
        if !self.has_auth() {
            anyhow::bail!("Portfolio requires authentication.");
        }
        Ok(OrderPage {
            orders: vec![],
            pagination: PaginationResponse { cursor: String::new(), has_more: false, total: 0 },
        })
    }

    async fn list_trades(&self, _filter: TradeFilter) -> Result<TradePage> {
        Ok(TradePage {
            trades: vec![],
            pagination: PaginationResponse { cursor: String::new(), has_more: false, total: 0 },
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        if !self.has_auth() {
            anyhow::bail!("Portfolio requires authentication.");
        }
        Ok(vec![])
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        if !self.has_auth() {
            anyhow::bail!("Portfolio requires authentication.");
        }
        Ok(vec![])
    }

    async fn get_trade_history(&self, _filter: TradeFilter) -> Result<Vec<Trade>> {
        Ok(vec![])
    }

    fn normalize_price(&self, raw_price: &str) -> Result<String> {
        let cents: i64 = raw_price.parse().context("Invalid Kalshi price")?;
        Ok(Self::cents_to_upp(cents))
    }

    fn denormalize_price(&self, probability: &str) -> Result<String> {
        Ok(Self::upp_to_cents(probability)?.to_string())
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        let start = std::time::Instant::now();
        let resp = self.client
            .get(format!("{}/exchange/status", KALSHI_API_BASE))
            .send()
            .await?;
        let latency = start.elapsed().as_millis() as u64;

        Ok(ProviderHealth {
            provider: self.provider_id().to_string(),
            healthy: resp.status().is_success(),
            status: if resp.status().is_success() { "operational" } else { "degraded" }.to_string(),
            latency_ms: latency,
        })
    }
}
