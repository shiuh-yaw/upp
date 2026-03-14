// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Opinion.trade adapter — translates between UPP and Opinion's BNB Chain API.
//
// Opinion architecture:
//   - Open API (openapi.opinion.trade) — market data, orderbook, pricing
//   - CLOB SDK — trading operations (EIP-712 signed orders on BNB Chain)
//
// Key quirks:
//   - All endpoints require an API key (in `apikey` header)
//   - Response envelope: { "code": 0, "msg": "success", "result": {...} }
//   - Market status is numeric: 0=Pending, 1=InProgress, 2=Activated, 3=Resolved, etc.
//   - Built on BNB Chain (smart contract settlement)
//   - AI-powered multi-agent resolution oracle
//   - Supports both binary and categorical markets
//   - Prices are in 0-1 range (same as UPP target format)
//   - Token IDs are hex strings from smart contracts

#![allow(dead_code)] // API response structs; many fields only used for Deserialize

use super::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn, instrument};

const OPINION_API_BASE: &str = "https://openapi.opinion.trade/openapi";

// ─── Opinion Native Response Types ──────────────────────────

/// Envelope wrapper for all Opinion API responses
#[derive(Debug, Deserialize)]
struct OpinionResponse<T> {
    code: i32,
    msg: String,
    result: Option<T>,
}

/// Paginated result wrapper
#[derive(Debug, Deserialize)]
struct OpinionPagedResult<T> {
    #[serde(default)]
    records: Vec<T>,
    #[serde(default)]
    total: Option<i64>,
    #[serde(default)]
    page: Option<i32>,
    #[serde(default)]
    limit: Option<i32>,
}

/// Opinion market from GET /market
#[derive(Debug, Default, Deserialize)]
struct OpinionMarket {
    #[serde(default, rename = "marketId")]
    market_id: i64,
    #[serde(default, rename = "marketTitle")]
    market_title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "marketType")]
    market_type: Option<i32>, // 0=Binary, 1=Categorical
    #[serde(default)]
    status: Option<i32>,
    #[serde(default, rename = "statusEnum")]
    status_enum: Option<String>,

    // ── Outcomes ──
    #[serde(default, rename = "yesTokenId")]
    yes_token_id: Option<String>,
    #[serde(default, rename = "noTokenId")]
    no_token_id: Option<String>,
    #[serde(default, rename = "yesLabel")]
    yes_label: Option<String>,
    #[serde(default, rename = "noLabel")]
    no_label: Option<String>,
    #[serde(default, rename = "childMarkets")]
    child_markets: Option<Vec<OpinionChildMarket>>,

    // ── Pricing ──
    #[serde(default, rename = "yesPrice")]
    yes_price: Option<f64>,
    #[serde(default, rename = "noPrice")]
    no_price: Option<f64>,

    // ── Volume ──
    #[serde(default)]
    volume: Option<String>,
    #[serde(default, rename = "volume24h")]
    volume_24h: Option<String>,
    #[serde(default, rename = "volume7d")]
    volume_7d: Option<String>,

    // ── Lifecycle ──
    #[serde(default, rename = "createdAt")]
    created_at: Option<String>,
    #[serde(default, rename = "cutoffAt")]
    cutoff_at: Option<String>,
    #[serde(default, rename = "resolvedAt")]
    resolved_at: Option<String>,

    // ── Chain ──
    #[serde(default, rename = "chainId")]
    chain_id: Option<i64>,
    #[serde(default, rename = "conditionId")]
    condition_id: Option<String>,
    #[serde(default, rename = "questionId")]
    question_id: Option<String>,
    #[serde(default, rename = "quoteToken")]
    quote_token: Option<String>,

    // ── Resolution ──
    #[serde(default, rename = "resultTokenId")]
    result_token_id: Option<String>,

    // ── Rules/Config ──
    #[serde(default)]
    rules: Option<String>,
    #[serde(default, rename = "incentiveFactor")]
    incentive_factor: Option<f64>,

    // ── Display ──
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

/// Child market for categorical outcomes
#[derive(Debug, Deserialize)]
struct OpinionChildMarket {
    #[serde(default, rename = "marketId")]
    market_id: Option<i64>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default, rename = "tokenId")]
    token_id: Option<String>,
    #[serde(default)]
    price: Option<f64>,
    #[serde(default)]
    volume: Option<String>,
}

/// Orderbook from GET /token/orderbook
#[derive(Debug, Deserialize)]
struct OpinionOrderBook {
    #[serde(default, rename = "tokenId")]
    token_id: Option<String>,
    #[serde(default)]
    bids: Vec<OpinionOrderLevel>,
    #[serde(default)]
    asks: Vec<OpinionOrderLevel>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpinionOrderLevel {
    #[serde(default)]
    price: String,
    #[serde(default)]
    size: String,
}

/// Latest price from GET /token/latest-price
#[derive(Debug, Deserialize)]
struct OpinionLatestPrice {
    #[serde(default, rename = "tokenId")]
    token_id: Option<String>,
    #[serde(default)]
    price: Option<f64>,
    #[serde(default)]
    timestamp: Option<String>,
}

// ─── Adapter ────────────────────────────────────────────────

pub struct OpinionAdapter {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl OpinionAdapter {
    /// Create adapter with API key (required for all Opinion endpoints).
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .gzip(true)
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("UPP-Gateway/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
            api_key: Some(api_key),
        }
    }

    /// Create adapter without API key — all calls will fail with helpful message.
    pub fn new_without_key() -> Self {
        Self {
            client: reqwest::Client::builder()
                .gzip(true)
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("UPP-Gateway/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
            api_key: None,
        }
    }

    fn require_key(&self) -> Result<&str> {
        self.api_key.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "Opinion.trade requires an API key for all endpoints. \
                 Get your API key at https://opinion.trade and set \
                 UPP_OPINION_API_KEY in your .env file."
            )
        })
    }

    /// Map Opinion numeric status → UPP MarketStatus
    fn map_status(status: Option<i32>, status_enum: Option<&str>) -> MarketStatus {
        // Check enum string first (more reliable)
        if let Some(s) = status_enum {
            return match s.to_lowercase().as_str() {
                "activated" | "active" | "open" => MarketStatus::Open,
                "resolved" | "settled" => MarketStatus::Resolved,
                "pending" | "created" => MarketStatus::Pending,
                "halted" | "paused" => MarketStatus::Halted,
                "closed" => MarketStatus::Closed,
                "voided" | "cancelled" => MarketStatus::Voided,
                _ => MarketStatus::Open,
            };
        }
        // Fallback to numeric
        match status.unwrap_or(0) {
            0 => MarketStatus::Pending,
            1 => MarketStatus::Pending, // InProgress (before active)
            2 => MarketStatus::Open,     // Activated
            3 => MarketStatus::Resolved,
            4 => MarketStatus::Voided,
            5 => MarketStatus::Closed,
            _ => MarketStatus::Open,
        }
    }

    /// Transform Opinion market → UPP Market
    fn transform_market(market: &OpinionMarket) -> Market {
        let now = chrono::Utc::now();

        // Determine if binary or categorical
        let is_binary = market.market_type.unwrap_or(0) == 0;

        // Build outcomes
        let outcomes = if is_binary {
            vec![
                Outcome {
                    id: "yes".to_string(),
                    label: market.yes_label.clone().unwrap_or_else(|| "Yes".to_string()),
                    token_id: market.yes_token_id.clone(),
                },
                Outcome {
                    id: "no".to_string(),
                    label: market.no_label.clone().unwrap_or_else(|| "No".to_string()),
                    token_id: market.no_token_id.clone(),
                },
            ]
        } else if let Some(ref children) = market.child_markets {
            children.iter().enumerate().map(|(i, child)| {
                Outcome {
                    id: child.market_id.map(|id| id.to_string())
                        .unwrap_or_else(|| format!("outcome_{}", i)),
                    label: child.label.clone().unwrap_or_else(|| format!("Option {}", i + 1)),
                    token_id: child.token_id.clone(),
                }
            }).collect()
        } else {
            vec![
                Outcome { id: "yes".to_string(), label: "Yes".to_string(), token_id: None },
                Outcome { id: "no".to_string(), label: "No".to_string(), token_id: None },
            ]
        };

        // Build pricing
        let mut last_price = HashMap::new();
        let mut mid_price = HashMap::new();
        if is_binary {
            if let Some(yp) = market.yes_price {
                last_price.insert("yes".to_string(), format!("{:.4}", yp));
                mid_price.insert("yes".to_string(), format!("{:.4}", yp));
            }
            if let Some(np) = market.no_price {
                last_price.insert("no".to_string(), format!("{:.4}", np));
                mid_price.insert("no".to_string(), format!("{:.4}", np));
            }
        } else if let Some(ref children) = market.child_markets {
            for (i, child) in children.iter().enumerate() {
                if let Some(p) = child.price {
                    let oid = child.market_id.map(|id| id.to_string())
                        .unwrap_or_else(|| format!("outcome_{}", i));
                    last_price.insert(oid.clone(), format!("{:.4}", p));
                    mid_price.insert(oid, format!("{:.4}", p));
                }
            }
        }

        let status = Self::map_status(market.status, market.status_enum.as_deref());

        // Parse timestamps
        let created_at = market.created_at.as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let closes_at = market.cutoff_at.as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let resolved_at = market.resolved_at.as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let tags = market.tags.clone().unwrap_or_default();
        let category = market.category.clone().unwrap_or_else(|| "uncategorized".to_string());

        Market {
            id: UniversalMarketId::new("opinion.trade", &market.market_id.to_string()),
            event: Event {
                id: market.question_id.clone().unwrap_or_else(|| market.market_id.to_string()),
                title: market.market_title.clone(),
                description: market.description.clone().unwrap_or_default(),
                category: category.clone(),
                tags,
                image_url: market.image.clone(),
                series_id: None,
                series_title: None,
            },
            market_type: if is_binary { MarketType::Binary } else { MarketType::Categorical },
            outcomes,
            pricing: MarketPricing {
                last_price,
                best_bid: HashMap::new(), // Would need orderbook call
                best_ask: HashMap::new(),
                mid_price,
                spread: HashMap::new(),
                tick_size: "0.01".to_string(),
                currency: market.quote_token.clone().unwrap_or_else(|| "USDC".to_string()),
                min_order_size: 1,
                max_order_size: 0,
                updated_at: now,
            },
            volume: MarketVolume {
                total_volume: market.volume.clone().unwrap_or_else(|| "0".to_string()),
                volume_24h: market.volume_24h.clone().unwrap_or_else(|| "0".to_string()),
                volume_7d: market.volume_7d.clone(),
                open_interest: "0".to_string(), // Not provided by Opinion
                num_traders: None,
                updated_at: now,
            },
            lifecycle: MarketLifecycle {
                status,
                created_at,
                opens_at: None,
                closes_at,
                resolved_at,
                expires_at: closes_at,
                resolution_source: Some("Opinion AI Multi-Agent Oracle".to_string()),
            },
            rules: MarketRules {
                allowed_order_types: vec![OrderType::Limit, OrderType::Market],
                allowed_tif: vec![TimeInForce::Gtc, TimeInForce::Fok],
                allows_short_selling: true,
                allows_partial_fill: true,
                maker_fee_rate: "0.00".to_string(),
                taker_fee_rate: "0.01".to_string(), // Opinion charges taker fee
                max_position_size: 0,
            },
            regulatory: MarketRegulatory {
                jurisdiction: "GLOBAL".to_string(),
                compliant: false,
                eligible_regions: vec!["GLOBAL".to_string()],
                restricted_regions: vec!["US".to_string()],
                regulator: "none".to_string(),
                license_type: "none".to_string(),
                contract_type: if is_binary { "binary_option".to_string() } else { "categorical".to_string() },
                required_kyc: KycLevel::None,
            },
            provider_metadata: {
                let mut meta = HashMap::new();
                meta.insert("provider_native_type".to_string(), "opinion_bnb".to_string());
                meta.insert("chain".to_string(), "BNB Chain".to_string());
                if let Some(cid) = &market.condition_id {
                    meta.insert("condition_id".to_string(), cid.clone());
                }
                if let Some(chain_id) = market.chain_id {
                    meta.insert("chain_id".to_string(), chain_id.to_string());
                }
                meta
            },
        }
    }
}

// ─── UPP Provider Implementation ────────────────────────────

#[async_trait::async_trait]
impl UppProvider for OpinionAdapter {
    fn provider_id(&self) -> &str { "opinion.trade" }
    fn provider_name(&self) -> &str { "Opinion" }

    fn manifest(&self) -> ProviderManifest {
        ProviderManifest {
            upp_version: "2026-03-11".to_string(),
            provider: ProviderInfo {
                name: "Opinion".to_string(),
                id: "opinion.trade".to_string(),
                provider_type: "decentralized".to_string(),
                jurisdictions: vec!["GLOBAL".to_string()],
            },
            capabilities: vec![
                CapabilityDeclaration {
                    name: "markets".to_string(),
                    version: "2026-03-11".to_string(),
                    operations: if self.api_key.is_some() {
                        vec!["listMarkets".into(), "getMarket".into(), "searchMarkets".into(), "getOrderBook".into()]
                    } else {
                        vec![] // Can't do anything without API key
                    },
                    extensions: vec!["ai_signals".into()],
                },
                CapabilityDeclaration {
                    name: "trading".to_string(),
                    version: "2026-03-11".to_string(),
                    operations: vec![], // Requires CLOB SDK, not REST
                    extensions: vec![],
                },
                CapabilityDeclaration {
                    name: "resolution".to_string(),
                    version: "2026-03-11".to_string(),
                    operations: vec!["getResolution".into(), "getResolutionSources".into()],
                    extensions: vec!["ai_oracle".into()],
                },
            ],
            transport: TransportInfo {
                rest_base_url: Some(OPINION_API_BASE.to_string()),
                websocket_url: None,
                grpc_endpoint: None,
            },
            authentication: vec!["api_key".to_string()],
            rate_limits: None,
        }
    }

    // ── Markets ──────────────────────────────────────────────

    #[instrument(skip(self), fields(provider = "opinion"))]
    async fn list_markets(&self, filter: MarketFilter) -> Result<MarketPage> {
        let api_key = self.require_key()?;

        let page = filter.pagination.cursor.as_deref()
            .and_then(|c| c.parse::<i32>().ok())
            .unwrap_or(1);
        let limit = filter.pagination.limit.unwrap_or(20).min(100);

        let mut url = format!("{}/market?page={}&limit={}", OPINION_API_BASE, page, limit);

        // Status filter
        if let Some(ref status) = filter.status {
            match status {
                MarketStatus::Open => url.push_str("&status=2"),      // Activated
                MarketStatus::Resolved => url.push_str("&status=3"),
                MarketStatus::Pending => url.push_str("&status=0"),
                _ => {}
            }
        }

        // Market type filter
        if let Some(ref mt) = filter.market_type {
            match mt {
                MarketType::Binary => url.push_str("&marketType=0"),
                MarketType::Categorical => url.push_str("&marketType=1"),
                _ => url.push_str("&marketType=2"), // All
            }
        }

        // Sort
        if let Some(ref sort) = filter.sort_by {
            match sort.as_str() {
                "volume" => url.push_str("&sortBy=volume&sortOrder=desc"),
                "volume_24h" => url.push_str("&sortBy=volume24h&sortOrder=desc"),
                "newest" | "created_at" => url.push_str("&sortBy=createdAt&sortOrder=desc"),
                _ => url.push_str("&sortBy=volume24h&sortOrder=desc"),
            }
        }

        debug!("Opinion list_markets: {}", url);

        let resp = self.client.get(&url)
            .header("apikey", api_key)
            .send()
            .await
            .context("Failed to reach Opinion API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Opinion API returned {}: {}", status, body);
        }

        let envelope: OpinionResponse<OpinionPagedResult<OpinionMarket>> = resp.json()
            .await
            .context("Failed to parse Opinion markets response")?;

        if envelope.code != 0 {
            anyhow::bail!("Opinion API error: {} (code {})", envelope.msg, envelope.code);
        }

        let paged = envelope.result.unwrap_or(OpinionPagedResult {
            records: vec![],
            total: Some(0),
            page: Some(1),
            limit: Some(20),
        });

        let markets: Vec<Market> = paged.records.iter()
            .map(Self::transform_market)
            .collect();

        let total = paged.total.unwrap_or(0) as i32;
        let has_more = (page * limit) < total;

        Ok(MarketPage {
            markets,
            pagination: PaginationResponse {
                cursor: if has_more { (page + 1).to_string() } else { String::new() },
                has_more,
                total,
            },
        })
    }

    #[instrument(skip(self), fields(provider = "opinion"))]
    async fn get_market(&self, native_id: &str) -> Result<Market> {
        let api_key = self.require_key()?;

        let url = format!("{}/market/{}", OPINION_API_BASE, native_id);
        debug!("Opinion get_market: {}", url);

        let resp = self.client.get(&url)
            .header("apikey", api_key)
            .send()
            .await
            .context("Failed to reach Opinion API")?;

        if !resp.status().is_success() {
            anyhow::bail!("Opinion market not found: {}", native_id);
        }

        let envelope: OpinionResponse<OpinionMarket> = resp.json()
            .await
            .context("Failed to parse Opinion market response")?;

        if envelope.code != 0 {
            anyhow::bail!("Opinion API error: {}", envelope.msg);
        }

        let market = envelope.result
            .ok_or_else(|| anyhow::anyhow!("No market data in Opinion response"))?;

        Ok(Self::transform_market(&market))
    }

    #[instrument(skip(self), fields(provider = "opinion"))]
    async fn search_markets(&self, query: &str, filter: MarketFilter) -> Result<MarketPage> {
        // Opinion doesn't have a dedicated search endpoint
        // Fetch markets and filter client-side
        let all = self.list_markets(MarketFilter {
            pagination: PaginationRequest { limit: Some(100), cursor: None },
            ..filter
        }).await?;

        let query_lower = query.to_lowercase();
        let filtered: Vec<Market> = all.markets.into_iter()
            .filter(|m| {
                m.event.title.to_lowercase().contains(&query_lower)
                    || m.event.description.to_lowercase().contains(&query_lower)
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

    #[instrument(skip(self), fields(provider = "opinion"))]
    async fn get_orderbook(
        &self,
        native_id: &str,
        outcome_id: Option<&str>,
        depth: i32,
    ) -> Result<Vec<OrderBookSnapshot>> {
        let api_key = self.require_key()?;

        // First get market to find token IDs
        let market = self.get_market(native_id).await?;
        let mut snapshots = Vec::new();

        for outcome in &market.outcomes {
            if let Some(target) = outcome_id {
                if outcome.id != target {
                    continue;
                }
            }

            if let Some(ref token_id) = outcome.token_id {
                let url = format!("{}/token/orderbook?tokenId={}", OPINION_API_BASE, token_id);
                debug!("Opinion orderbook: {}", url);

                match self.client.get(&url)
                    .header("apikey", api_key)
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<OpinionResponse<OpinionOrderBook>>().await {
                            Ok(envelope) if envelope.code == 0 => {
                                if let Some(book) = envelope.result {
                                    let depth = if depth <= 0 { 10 } else { depth as usize };

                                    let bids: Vec<OrderBookLevel> = book.bids.iter()
                                        .take(depth)
                                        .map(|l| OrderBookLevel {
                                            price: l.price.clone(),
                                            quantity: l.size.parse::<f64>().unwrap_or(0.0) as i64,
                                        })
                                        .collect();

                                    let asks: Vec<OrderBookLevel> = book.asks.iter()
                                        .take(depth)
                                        .map(|l| OrderBookLevel {
                                            price: l.price.clone(),
                                            quantity: l.size.parse::<f64>().unwrap_or(0.0) as i64,
                                        })
                                        .collect();

                                    snapshots.push(OrderBookSnapshot {
                                        outcome_id: outcome.id.clone(),
                                        bids,
                                        asks,
                                        asks_computed: false,
                                    });
                                }
                            }
                            Ok(envelope) => {
                                warn!("Opinion orderbook API error: {}", envelope.msg);
                            }
                            Err(e) => {
                                warn!("Failed to parse Opinion orderbook: {}", e);
                            }
                        }
                    }
                    Ok(resp) => {
                        warn!("Opinion orderbook returned {}", resp.status());
                    }
                    Err(e) => {
                        warn!("Failed to fetch Opinion orderbook: {}", e);
                    }
                }
            }
        }

        Ok(snapshots)
    }

    // ── Trading (Requires CLOB SDK) ─────────────────────────

    async fn create_order(&self, _req: CreateOrderRequest) -> Result<Order> {
        anyhow::bail!(
            "Opinion.trade trading requires the CLOB SDK (BNB Chain). \
             Orders must be signed with EIP-712 and submitted via the \
             Opinion CLOB SDK. See: https://docs.opinion.trade/developer-guide/opinion-clob-sdk"
        )
    }

    async fn cancel_order(&self, _id: &str) -> Result<Order> {
        anyhow::bail!("Opinion.trade order cancellation requires the CLOB SDK")
    }

    async fn cancel_all_orders(&self, _m: Option<&str>) -> Result<Vec<String>> {
        anyhow::bail!("Opinion.trade order cancellation requires the CLOB SDK")
    }

    async fn get_order(&self, _id: &str) -> Result<Order> {
        anyhow::bail!("Opinion.trade order lookup requires the CLOB SDK")
    }

    async fn list_orders(&self, _f: OrderFilter) -> Result<OrderPage> {
        anyhow::bail!("Opinion.trade order listing requires the CLOB SDK")
    }

    async fn list_trades(&self, _f: TradeFilter) -> Result<TradePage> {
        // Opinion Open API has a /trade endpoint
        let _api_key = self.require_key()?;
        // TODO: Implement GET /trade with apikey header
        Ok(TradePage {
            trades: vec![],
            pagination: PaginationResponse { cursor: String::new(), has_more: false, total: 0 },
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>> {
        // Opinion has a /position endpoint
        let _api_key = self.require_key()?;
        // TODO: Implement GET /position with apikey header
        Ok(vec![])
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        anyhow::bail!("Opinion.trade balances are on-chain (BNB). Query the wallet directly.")
    }

    async fn get_trade_history(&self, _f: TradeFilter) -> Result<Vec<Trade>> {
        Ok(vec![])
    }

    // ── Normalization ────────────────────────────────────────

    fn normalize_price(&self, raw_price: &str) -> Result<String> {
        // Opinion prices are already in 0-1 range
        let p: f64 = raw_price.parse()
            .context("Invalid Opinion price")?;
        if !(0.0..=1.0).contains(&p) {
            anyhow::bail!("Opinion price {} out of [0,1] range", p);
        }
        Ok(format!("{:.4}", p))
    }

    fn denormalize_price(&self, probability: &str) -> Result<String> {
        Ok(probability.to_string())
    }

    // ── Health ───────────────────────────────────────────────

    async fn health_check(&self) -> Result<ProviderHealth> {
        if self.api_key.is_none() {
            return Ok(ProviderHealth {
                provider: self.provider_id().to_string(),
                healthy: false,
                status: "no_api_key".to_string(),
                latency_ms: 0,
            });
        }

        let api_key = self.api_key.as_deref().unwrap();
        let start = std::time::Instant::now();

        let resp = self.client
            .get(format!("{}/market?page=1&limit=1", OPINION_API_BASE))
            .header("apikey", api_key)
            .send()
            .await;

        let latency = start.elapsed().as_millis() as u64;

        let (healthy, status) = match resp {
            Ok(r) if r.status().is_success() => (true, "operational"),
            Ok(r) if r.status().as_u16() == 401 => (false, "invalid_api_key"),
            Ok(r) if r.status().as_u16() == 429 => (true, "rate_limited"),
            Ok(_) => (false, "error"),
            Err(_) => (false, "unreachable"),
        };

        Ok(ProviderHealth {
            provider: self.provider_id().to_string(),
            healthy,
            status: status.to_string(),
            latency_ms: latency,
        })
    }
}
