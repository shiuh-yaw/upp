// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Polymarket adapter — translates between UPP and Polymarket's hybrid CLOB.
//
// Polymarket architecture:
//   - Gamma API (gamma-api.polymarket.com) — market metadata, events, search
//   - CLOB API (clob.polymarket.com) — orderbook, pricing, trading
//   - WebSocket (ws-subscriptions-clob.polymarket.com) — real-time updates
//
// Key quirks:
//   - Gamma returns `outcomePrices` as a JSON string: "[\"0.65\",\"0.35\"]"
//   - `clobTokenIds` is also a JSON string: "[\"12345\",\"67890\"]"
//   - `outcomes` is a JSON string: "[\"Yes\",\"No\"]"
//   - CLOB orderbook returns real bids/asks (unlike Kalshi's bid-only model)
//   - Auth uses Ethereum wallet signatures (EIP-712) — only for trading
//   - Market data (Gamma + CLOB GET) is fully public, no auth needed
//   - Prices are in [0, 1] range (probability), same as UPP target format
//   - Settlement on Polygon via Conditional Token Framework (CTF)

#![allow(dead_code)] // API response structs; many fields only used for Deserialize

use super::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn, instrument};

const POLYMARKET_GAMMA_BASE: &str = "https://gamma-api.polymarket.com";
const POLYMARKET_CLOB_BASE: &str = "https://clob.polymarket.com";
const POLYMARKET_WS_BASE: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

// ─── Polymarket Native Response Types ───────────────────────

/// Gamma API: GET /markets  → returns array of GammaMarket
/// Gamma API: GET /markets/{condition_id}  → returns single GammaMarket
#[derive(Debug, Deserialize)]
struct GammaMarket {
    // ── Identity ──
    #[serde(default)]
    condition_id: String,
    #[serde(default)]
    question_id: Option<String>,
    #[serde(default)]
    question: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    description: Option<String>,

    // ── Outcomes & Pricing ──
    // These come as JSON-encoded strings: "[\"Yes\",\"No\"]"
    #[serde(default)]
    outcomes: Option<String>,
    #[serde(default)]
    outcome_prices: Option<String>,    // renamed from outcomePrices
    #[serde(default, rename = "outcomePrices")]
    outcome_prices_alt: Option<String>, // fallback for camelCase

    // CLOB token IDs — JSON string: "[\"71321045...\",\"71321045...\"]"
    #[serde(default)]
    clob_token_ids: Option<String>,
    #[serde(default, rename = "clobTokenIds")]
    clob_token_ids_alt: Option<String>,

    // ── Volume & Liquidity ──
    #[serde(default)]
    volume: Option<f64>,
    #[serde(default, rename = "volume24hr")]
    volume_24hr: Option<f64>,
    #[serde(default)]
    liquidity: Option<f64>,
    #[serde(default)]
    open_interest: Option<f64>,
    #[serde(default, rename = "bestBid")]
    best_bid: Option<f64>,
    #[serde(default, rename = "bestAsk")]
    best_ask: Option<f64>,
    #[serde(default, rename = "lastTradePrice")]
    last_trade_price: Option<f64>,
    #[serde(default)]
    spread: Option<f64>,

    // ── Lifecycle ──
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    closed: Option<bool>,
    #[serde(default)]
    archived: Option<bool>,
    #[serde(default, rename = "acceptingOrders")]
    accepting_orders: Option<bool>,
    #[serde(default, rename = "endDateIso")]
    end_date_iso: Option<String>,
    #[serde(default, rename = "startDateIso")]
    start_date_iso: Option<String>,
    #[serde(default, rename = "createdAt")]
    created_at: Option<String>,

    // ── Market Config ──
    #[serde(default, rename = "enableOrderBook")]
    enable_order_book: Option<bool>,
    #[serde(default, rename = "negRisk")]
    neg_risk: Option<bool>,
    #[serde(default, rename = "tickSize")]
    tick_size: Option<String>,
    #[serde(default, rename = "minOrderSize")]
    min_order_size: Option<f64>,

    // ── Display ──
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    icon: Option<String>,

    // ── Event grouping ──
    #[serde(default, rename = "groupItemTitle")]
    group_item_title: Option<String>,

    // ── Tags / category ──
    #[serde(default)]
    tags: Option<Vec<GammaTag>>,

    // ── Resolution ──
    #[serde(default, rename = "resolutionSource")]
    resolution_source: Option<String>,
    #[serde(default, rename = "umaBond")]
    uma_bond: Option<String>,
    #[serde(default, rename = "umaReward")]
    uma_reward: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GammaTag {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    slug: Option<String>,
}

/// Gamma API: GET /events → returns array of GammaEvent
#[derive(Debug, Deserialize)]
struct GammaEvent {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    markets: Option<Vec<GammaMarket>>,
    #[serde(default, rename = "startDate")]
    start_date: Option<String>,
    #[serde(default, rename = "endDate")]
    end_date: Option<String>,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

/// CLOB API: GET /book?token_id={id} → OrderBookSummary
#[derive(Debug, Deserialize)]
struct ClobOrderBook {
    #[serde(default)]
    market: Option<String>,
    #[serde(default)]
    asset_id: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    bids: Vec<ClobOrderLevel>,
    #[serde(default)]
    asks: Vec<ClobOrderLevel>,
    #[serde(default)]
    min_order_size: Option<String>,
    #[serde(default)]
    tick_size: Option<String>,
    #[serde(default)]
    neg_risk: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ClobOrderLevel {
    #[serde(default)]
    price: String,
    #[serde(default)]
    size: String,
}

/// CLOB API: GET /price?token_id={id} → midpoint price
#[derive(Debug, Deserialize)]
struct ClobPrice {
    #[serde(default)]
    mid: Option<String>,
    #[serde(default)]
    bid: Option<String>,
    #[serde(default)]
    ask: Option<String>,
}

// ─── Adapter ────────────────────────────────────────────────

pub struct PolymarketAdapter {
    client: reqwest::Client,
    // Optional wallet key for authenticated trading
    wallet_key: Option<String>,
}

impl PolymarketAdapter {
    /// Create a public-only adapter (no auth, read-only market data).
    pub fn new_public() -> Self {
        Self {
            client: reqwest::Client::builder()
                .gzip(true)
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("UPP-Gateway/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
            wallet_key: None,
        }
    }

    /// Create an authenticated adapter (for trading operations).
    pub fn new_authenticated(wallet_key: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .gzip(true)
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("UPP-Gateway/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
            wallet_key: Some(wallet_key),
        }
    }

    fn is_authenticated(&self) -> bool {
        self.wallet_key.is_some()
    }

    // ── Gamma JSON string helpers ────────────────────────────

    /// Parse Polymarket's JSON-encoded string arrays: "[\"Yes\",\"No\"]" → vec!["Yes", "No"]
    fn parse_json_string_array(s: &str) -> Vec<String> {
        serde_json::from_str::<Vec<String>>(s).unwrap_or_default()
    }

    /// Get outcome prices, checking both snake_case and camelCase fields
    fn get_outcome_prices(market: &GammaMarket) -> Vec<String> {
        let raw = market.outcome_prices.as_deref()
            .or(market.outcome_prices_alt.as_deref())
            .unwrap_or("[]");
        Self::parse_json_string_array(raw)
    }

    /// Get CLOB token IDs
    fn get_clob_token_ids(market: &GammaMarket) -> Vec<String> {
        let raw = market.clob_token_ids.as_deref()
            .or(market.clob_token_ids_alt.as_deref())
            .unwrap_or("[]");
        Self::parse_json_string_array(raw)
    }

    /// Get outcome labels
    fn get_outcomes(market: &GammaMarket) -> Vec<String> {
        let raw = market.outcomes.as_deref().unwrap_or("[\"Yes\",\"No\"]");
        Self::parse_json_string_array(raw)
    }

    // ── Transform Gamma market → UPP Market ──────────────────

    fn transform_market(market: &GammaMarket) -> Market {
        let outcomes_labels = Self::get_outcomes(market);
        let outcome_prices = Self::get_outcome_prices(market);
        let token_ids = Self::get_clob_token_ids(market);

        // Build outcomes with token IDs
        let outcomes: Vec<Outcome> = outcomes_labels.iter().enumerate().map(|(i, label)| {
            let id = label.to_lowercase();
            Outcome {
                id: id.clone(),
                label: label.clone(),
                token_id: token_ids.get(i).cloned(),
            }
        }).collect();

        // Determine market type (binary if exactly 2 outcomes)
        let market_type = if outcomes.len() == 2 {
            MarketType::Binary
        } else {
            MarketType::Categorical
        };

        // Build pricing maps
        let mut last_price = HashMap::new();
        let mut best_bid = HashMap::new();
        let mut best_ask = HashMap::new();
        let mut mid_price = HashMap::new();
        let mut spread_map = HashMap::new();

        for (i, outcome) in outcomes.iter().enumerate() {
            // Last price from outcome_prices
            if let Some(price_str) = outcome_prices.get(i) {
                last_price.insert(outcome.id.clone(), price_str.clone());
                mid_price.insert(outcome.id.clone(), price_str.clone());
            }

            // Best bid/ask from top-level market fields (only for first outcome typically)
            if i == 0 {
                if let Some(bid) = market.best_bid {
                    best_bid.insert(outcome.id.clone(), format!("{:.4}", bid));
                }
                if let Some(ask) = market.best_ask {
                    best_ask.insert(outcome.id.clone(), format!("{:.4}", ask));
                }
                if let Some(s) = market.spread {
                    spread_map.insert(outcome.id.clone(), format!("{:.4}", s));
                }
            }

            // For binary: compute complementary prices for second outcome
            if i == 1 && market_type == MarketType::Binary {
                if let Some(bid) = market.best_bid {
                    // NO best_ask ≈ 1 - YES best_bid
                    best_ask.insert(outcome.id.clone(), format!("{:.4}", 1.0 - bid));
                }
                if let Some(ask) = market.best_ask {
                    // NO best_bid ≈ 1 - YES best_ask
                    best_bid.insert(outcome.id.clone(), format!("{:.4}", 1.0 - ask));
                }
                if let Some(s) = market.spread {
                    spread_map.insert(outcome.id.clone(), format!("{:.4}", s));
                }
            }
        }

        // Parse timestamps
        let now = chrono::Utc::now();
        let created_at = market.created_at.as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let closes_at = market.end_date_iso.as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        // Determine status
        let status = if market.closed.unwrap_or(false) {
            MarketStatus::Closed
        } else if !market.active.unwrap_or(true) {
            MarketStatus::Halted
        } else if market.accepting_orders.unwrap_or(true) {
            MarketStatus::Open
        } else {
            MarketStatus::Pending
        };

        // Tags
        let tags: Vec<String> = market.tags.as_ref()
            .map(|t| t.iter().filter_map(|tag| tag.label.clone()).collect())
            .unwrap_or_default();

        let tick_size = market.tick_size.clone().unwrap_or_else(|| "0.01".to_string());
        let min_order_size = market.min_order_size.map(|v| v as i32).unwrap_or(1);

        Market {
            id: UniversalMarketId::new("polymarket.com", &market.condition_id),
            event: Event {
                id: market.question_id.clone().unwrap_or_else(|| market.condition_id.clone()),
                title: market.question.clone(),
                description: market.description.clone().unwrap_or_default(),
                category: tags.first().cloned().unwrap_or_else(|| "uncategorized".to_string()),
                tags,
                image_url: market.image.clone().or(market.icon.clone()),
                series_id: None,
                series_title: market.group_item_title.clone(),
            },
            market_type,
            outcomes,
            pricing: MarketPricing {
                last_price,
                best_bid,
                best_ask,
                mid_price,
                spread: spread_map,
                tick_size,
                currency: "USDC".to_string(),
                min_order_size,
                max_order_size: 0, // Polymarket has no fixed max
                updated_at: now,
            },
            volume: MarketVolume {
                total_volume: market.volume.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "0".to_string()),
                volume_24h: market.volume_24hr.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "0".to_string()),
                volume_7d: None,
                open_interest: market.open_interest.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "0".to_string()),
                num_traders: None,
                updated_at: now,
            },
            lifecycle: MarketLifecycle {
                status,
                created_at,
                opens_at: None,
                closes_at,
                resolved_at: None,
                expires_at: closes_at,
                resolution_source: market.resolution_source.clone()
                    .or(Some("UMA Optimistic Oracle".to_string())),
            },
            rules: MarketRules {
                allowed_order_types: vec![OrderType::Limit],
                allowed_tif: vec![TimeInForce::Gtc, TimeInForce::Fok],
                allows_short_selling: true,
                allows_partial_fill: true,
                maker_fee_rate: "0.00".to_string(),
                taker_fee_rate: "0.00".to_string(),
                max_position_size: 0,
            },
            regulatory: MarketRegulatory {
                jurisdiction: "GLOBAL".to_string(),
                compliant: false,
                eligible_regions: vec!["GLOBAL".to_string()],
                restricted_regions: vec!["US".to_string()],
                regulator: "none".to_string(),
                license_type: "none".to_string(),
                contract_type: "binary_option".to_string(),
                required_kyc: KycLevel::None,
            },
            provider_metadata: {
                let mut meta = HashMap::new();
                meta.insert("provider_native_type".to_string(), "polymarket_ctf".to_string());
                if let Some(slug) = &market.slug {
                    meta.insert("slug".to_string(), slug.clone());
                    meta.insert("web_url".to_string(), format!("https://polymarket.com/event/{}", slug));
                }
                if let Some(neg_risk) = market.neg_risk {
                    meta.insert("neg_risk".to_string(), neg_risk.to_string());
                }
                meta
            },
        }
    }

    /// Transform CLOB orderbook → UPP OrderBookSnapshot
    fn transform_orderbook(book: &ClobOrderBook, outcome_id: &str) -> OrderBookSnapshot {
        let bids: Vec<OrderBookLevel> = book.bids.iter().map(|level| {
            OrderBookLevel {
                price: level.price.clone(),
                quantity: level.size.parse::<f64>().unwrap_or(0.0) as i64,
            }
        }).collect();

        let asks: Vec<OrderBookLevel> = book.asks.iter().map(|level| {
            OrderBookLevel {
                price: level.price.clone(),
                quantity: level.size.parse::<f64>().unwrap_or(0.0) as i64,
            }
        }).collect();

        OrderBookSnapshot {
            outcome_id: outcome_id.to_string(),
            bids,
            asks,
            asks_computed: false, // Polymarket provides real asks
        }
    }
}

// ─── UPP Provider Implementation ────────────────────────────

#[async_trait::async_trait]
impl UppProvider for PolymarketAdapter {
    fn provider_id(&self) -> &str { "polymarket.com" }
    fn provider_name(&self) -> &str { "Polymarket" }

    fn manifest(&self) -> ProviderManifest {
        ProviderManifest {
            upp_version: "2026-03-11".to_string(),
            provider: ProviderInfo {
                name: "Polymarket".to_string(),
                id: "polymarket.com".to_string(),
                provider_type: "hybrid".to_string(),
                jurisdictions: vec!["GLOBAL".to_string()],
            },
            capabilities: vec![
                CapabilityDeclaration {
                    name: "markets".to_string(),
                    version: "2026-03-11".to_string(),
                    operations: vec![
                        "listMarkets".into(),
                        "getMarket".into(),
                        "searchMarkets".into(),
                        "getOrderBook".into(),
                    ],
                    extensions: vec![
                        "analytics".into(),
                        "streaming".into(),
                        "social".into(),
                    ],
                },
                CapabilityDeclaration {
                    name: "trading".to_string(),
                    version: "2026-03-11".to_string(),
                    operations: if self.is_authenticated() {
                        vec!["createOrder".into(), "cancelOrder".into(), "cancelAllOrders".into()]
                    } else {
                        vec![] // No trading without wallet
                    },
                    extensions: vec![],
                },
                CapabilityDeclaration {
                    name: "portfolio".to_string(),
                    version: "2026-03-11".to_string(),
                    operations: if self.is_authenticated() {
                        vec!["getPositions".into(), "getTradeHistory".into()]
                    } else {
                        vec![]
                    },
                    extensions: vec![],
                },
            ],
            transport: TransportInfo {
                rest_base_url: Some(POLYMARKET_CLOB_BASE.to_string()),
                websocket_url: Some(POLYMARKET_WS_BASE.to_string()),
                grpc_endpoint: None,
            },
            authentication: if self.is_authenticated() {
                vec!["wallet_signature".to_string()]
            } else {
                vec!["none".to_string()]
            },
            rate_limits: None,
        }
    }

    // ── Markets (Public, No Auth) ────────────────────────────

    #[instrument(skip(self), fields(provider = "polymarket"))]
    async fn list_markets(&self, filter: MarketFilter) -> Result<MarketPage> {
        // Gamma API: GET /markets?limit=N&offset=M&active=true
        let limit = filter.pagination.limit.unwrap_or(20).min(100);
        let offset = filter.pagination.cursor.as_deref()
            .and_then(|c| c.parse::<i32>().ok())
            .unwrap_or(0);

        let mut url = format!("{}/markets?limit={}&offset={}", POLYMARKET_GAMMA_BASE, limit, offset);

        // Apply filters
        if let Some(ref status) = filter.status {
            match status {
                MarketStatus::Open => {
                    url.push_str("&active=true&closed=false");
                }
                MarketStatus::Closed | MarketStatus::Resolved => {
                    url.push_str("&closed=true");
                }
                _ => {}
            }
        } else {
            // Default: only active/open markets
            url.push_str("&active=true&closed=false");
        }

        // Sort by volume by default
        if let Some(ref sort) = filter.sort_by {
            match sort.as_str() {
                "volume" | "volume_24h" => url.push_str("&order=volume24hr&ascending=false"),
                "liquidity" => url.push_str("&order=liquidity&ascending=false"),
                "created_at" | "newest" => url.push_str("&order=startDate&ascending=false"),
                "end_date" | "closing_soon" => url.push_str("&order=endDate&ascending=true"),
                _ => url.push_str("&order=volume24hr&ascending=false"),
            }
        } else {
            url.push_str("&order=volume24hr&ascending=false");
        }

        // Category filter via tag
        if let Some(ref _category) = filter.category {
            // Gamma supports tag_slug filter
            // url.push_str(&format!("&tag_slug={}", category));
        }

        debug!("Polymarket list_markets: {}", url);

        let resp = self.client.get(&url)
            .send()
            .await
            .context("Failed to reach Polymarket Gamma API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Polymarket Gamma API returned {}: {}", status, body);
        }

        let gamma_markets: Vec<GammaMarket> = resp.json()
            .await
            .context("Failed to parse Polymarket markets response")?;

        let markets: Vec<Market> = gamma_markets.iter()
            .filter(|m| !m.condition_id.is_empty())
            .map(Self::transform_market)
            .collect();

        let has_more = markets.len() as i32 >= limit;
        let next_cursor = if has_more {
            (offset + limit).to_string()
        } else {
            String::new()
        };

        Ok(MarketPage {
            markets,
            pagination: PaginationResponse {
                cursor: next_cursor,
                has_more,
                total: 0, // Gamma doesn't return total count
            },
        })
    }

    #[instrument(skip(self), fields(provider = "polymarket"))]
    async fn get_market(&self, native_id: &str) -> Result<Market> {
        // Gamma API: GET /markets/{condition_id}
        // Try condition_id first, then slug
        let url = format!("{}/markets/{}", POLYMARKET_GAMMA_BASE, native_id);
        debug!("Polymarket get_market: {}", url);

        let resp = self.client.get(&url)
            .send()
            .await
            .context("Failed to reach Polymarket Gamma API")?;

        if !resp.status().is_success() {
            // Might be a slug — try searching
            let search_url = format!("{}/markets?slug={}", POLYMARKET_GAMMA_BASE, native_id);
            let search_resp = self.client.get(&search_url)
                .send()
                .await
                .context("Failed to search Polymarket by slug")?;

            if search_resp.status().is_success() {
                let results: Vec<GammaMarket> = search_resp.json().await?;
                if let Some(market) = results.first() {
                    return Ok(Self::transform_market(market));
                }
            }
            anyhow::bail!("Polymarket market not found: {}", native_id);
        }

        let gamma_market: GammaMarket = resp.json()
            .await
            .context("Failed to parse Polymarket market response")?;

        Ok(Self::transform_market(&gamma_market))
    }

    #[instrument(skip(self), fields(provider = "polymarket"))]
    async fn search_markets(&self, query: &str, filter: MarketFilter) -> Result<MarketPage> {
        // Gamma API supports _q parameter for text search
        let limit = filter.pagination.limit.unwrap_or(20).min(100);
        let offset = filter.pagination.cursor.as_deref()
            .and_then(|c| c.parse::<i32>().ok())
            .unwrap_or(0);

        let url = format!(
            "{}/markets?_q={}&limit={}&offset={}&active=true&closed=false&order=volume24hr&ascending=false",
            POLYMARKET_GAMMA_BASE,
            urlencoding::encode(query),
            limit,
            offset
        );

        debug!("Polymarket search: {}", url);

        let resp = self.client.get(&url)
            .send()
            .await
            .context("Failed to search Polymarket markets")?;

        if !resp.status().is_success() {
            // Fallback: fetch all and filter client-side
            warn!("Polymarket search API failed, falling back to client-side filter");
            let all = self.list_markets(filter).await?;
            let query_lower = query.to_lowercase();
            let filtered: Vec<Market> = all.markets.into_iter()
                .filter(|m| {
                    m.event.title.to_lowercase().contains(&query_lower)
                        || m.event.description.to_lowercase().contains(&query_lower)
                })
                .collect();
            return Ok(MarketPage {
                pagination: PaginationResponse {
                    cursor: String::new(),
                    has_more: false,
                    total: filtered.len() as i32,
                },
                markets: filtered,
            });
        }

        let gamma_markets: Vec<GammaMarket> = resp.json()
            .await
            .context("Failed to parse Polymarket search response")?;

        let markets: Vec<Market> = gamma_markets.iter()
            .filter(|m| !m.condition_id.is_empty())
            .map(Self::transform_market)
            .collect();

        let has_more = markets.len() as i32 >= limit;

        Ok(MarketPage {
            markets,
            pagination: PaginationResponse {
                cursor: if has_more { (offset + limit).to_string() } else { String::new() },
                has_more,
                total: 0,
            },
        })
    }

    #[instrument(skip(self), fields(provider = "polymarket"))]
    async fn get_orderbook(
        &self,
        native_id: &str,
        outcome_id: Option<&str>,
        depth: i32,
    ) -> Result<Vec<OrderBookSnapshot>> {
        // First, get the market to find token IDs
        let market = self.get_market(native_id).await?;

        let mut snapshots = Vec::new();

        for outcome in &market.outcomes {
            // Skip if specific outcome requested and this isn't it
            if let Some(target) = outcome_id {
                if outcome.id != target {
                    continue;
                }
            }

            if let Some(ref token_id) = outcome.token_id {
                // CLOB API: GET /book?token_id={token_id}
                let url = format!("{}/book?token_id={}", POLYMARKET_CLOB_BASE, token_id);
                debug!("Polymarket orderbook: {}", url);

                match self.client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<ClobOrderBook>().await {
                            Ok(book) => {
                                let mut snapshot = Self::transform_orderbook(&book, &outcome.id);

                                // Apply depth limit
                                let depth = if depth <= 0 { 10 } else { depth as usize };
                                snapshot.bids.truncate(depth);
                                snapshot.asks.truncate(depth);

                                snapshots.push(snapshot);
                            }
                            Err(e) => {
                                warn!("Failed to parse orderbook for {}: {}", token_id, e);
                            }
                        }
                    }
                    Ok(resp) => {
                        warn!("CLOB orderbook returned {}", resp.status());
                    }
                    Err(e) => {
                        warn!("Failed to fetch orderbook for {}: {}", token_id, e);
                    }
                }
            } else {
                debug!("No token_id for outcome {} — skipping orderbook", outcome.id);
            }
        }

        Ok(snapshots)
    }

    // ── Trading (Requires Wallet Auth) ──────────────────────

    async fn create_order(&self, _req: CreateOrderRequest) -> Result<Order> {
        if !self.is_authenticated() {
            anyhow::bail!(
                "Polymarket trading requires an Ethereum wallet. \
                 Set UPP_POLYMARKET_WALLET_KEY in your .env file with your \
                 wallet private key. Orders are signed with EIP-712 and \
                 submitted to the CLOB at {}",
                POLYMARKET_CLOB_BASE
            );
        }
        // TODO: Implement EIP-712 order signing and submission
        // 1. Build order struct per CLOB spec
        // 2. Sign with ethers-rs EIP-712 typed data
        // 3. POST /order to CLOB API
        anyhow::bail!("Polymarket EIP-712 order signing not yet implemented. \
                       Use the Polymarket Python SDK or JS SDK for trading.")
    }

    async fn cancel_order(&self, _id: &str) -> Result<Order> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket trading requires wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        anyhow::bail!("Polymarket order cancellation not yet implemented")
    }

    async fn cancel_all_orders(&self, _market: Option<&str>) -> Result<Vec<String>> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket trading requires wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        Ok(vec![])
    }

    async fn get_order(&self, _id: &str) -> Result<Order> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket order lookup requires wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        anyhow::bail!("Polymarket order lookup not yet implemented")
    }

    async fn list_orders(&self, _f: OrderFilter) -> Result<OrderPage> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket order listing requires wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        Ok(OrderPage {
            orders: vec![],
            pagination: PaginationResponse { cursor: String::new(), has_more: false, total: 0 },
        })
    }

    async fn list_trades(&self, _f: TradeFilter) -> Result<TradePage> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket trade history requires wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        Ok(TradePage {
            trades: vec![],
            pagination: PaginationResponse { cursor: String::new(), has_more: false, total: 0 },
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket positions require wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        Ok(vec![])
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket balances require wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        Ok(vec![])
    }

    async fn get_trade_history(&self, _f: TradeFilter) -> Result<Vec<Trade>> {
        if !self.is_authenticated() {
            anyhow::bail!("Polymarket trade history requires wallet authentication. Set UPP_POLYMARKET_WALLET_KEY.");
        }
        Ok(vec![])
    }

    // ── Normalization ────────────────────────────────────────

    fn normalize_price(&self, raw_price: &str) -> Result<String> {
        // Polymarket prices are already in 0-1 probability range
        let p: f64 = raw_price.parse()
            .context("Invalid Polymarket price")?;
        if !(0.0..=1.0).contains(&p) {
            anyhow::bail!("Polymarket price {} out of [0,1] range", p);
        }
        Ok(format!("{:.4}", p))
    }

    fn denormalize_price(&self, probability: &str) -> Result<String> {
        // UPP probability → Polymarket price (same format)
        let p: f64 = probability.parse()
            .context("Invalid UPP probability")?;
        Ok(format!("{:.2}", p))
    }

    // ── Health ───────────────────────────────────────────────

    async fn health_check(&self) -> Result<ProviderHealth> {
        let start = std::time::Instant::now();

        // Check both Gamma and CLOB endpoints
        let gamma_ok = self.client
            .get(format!("{}/markets?limit=1", POLYMARKET_GAMMA_BASE))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        let clob_ok = self.client
            .get(format!("{}/time", POLYMARKET_CLOB_BASE))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        let latency = start.elapsed().as_millis() as u64;

        let (healthy, status) = match (gamma_ok, clob_ok) {
            (true, true) => (true, "operational"),
            (true, false) => (true, "degraded_clob"),
            (false, true) => (true, "degraded_gamma"),
            (false, false) => (false, "down"),
        };

        Ok(ProviderHealth {
            provider: self.provider_id().to_string(),
            healthy,
            status: status.to_string(),
            latency_ms: latency,
        })
    }
}
