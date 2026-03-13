// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// gRPC transport — Tonic server implementing UPP proto service traits.
//
// Runs alongside the REST gateway on a separate port (default 50051).
// Shares the same ProviderRegistry, MarketCache, and AppState.
//
// Proto types (prost-generated) are bridged to core types via conversion
// functions in this module.

use crate::core::cache::MarketCache;
use crate::core::config::GatewayConfig;
use crate::core::registry::ProviderRegistry;
use crate::core::types as core;
use crate::gen::upp::v1 as pb;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::info;

// ─── Shared State ─────────────────────────────────────────────

/// State shared between all gRPC service implementations.
#[derive(Clone)]
pub struct GrpcState {
    pub registry: Arc<ProviderRegistry>,
    pub cache: Arc<MarketCache>,
    #[allow(dead_code)]
    pub config: Arc<GatewayConfig>,
}

// ─── Type Conversions: Core → Proto ──────────────────────────

fn core_market_id_to_pb(id: &core::UniversalMarketId) -> pb::UniversalMarketId {
    pb::UniversalMarketId {
        provider: id.provider.clone(),
        native_id: id.native_id.clone(),
        full_id: id.to_full_id(),
    }
}

fn core_market_to_pb(m: &core::Market) -> pb::Market {
    pb::Market {
        id: Some(core_market_id_to_pb(&m.id)),
        event: Some(pb::Event {
            id: m.event.id.clone(),
            title: m.event.title.clone(),
            description: m.event.description.clone(),
            category: m.event.category.clone(),
            tags: m.event.tags.clone(),
            image_url: m.event.image_url.clone().unwrap_or_default(),
            series_id: m.event.series_id.clone().unwrap_or_default(),
            series_title: m.event.series_title.clone().unwrap_or_default(),
        }),
        market_type: match m.market_type {
            core::MarketType::Binary => pb::MarketType::Binary as i32,
            core::MarketType::Categorical => pb::MarketType::Categorical as i32,
            core::MarketType::Scalar => pb::MarketType::Scalar as i32,
        },
        outcomes: m.outcomes.iter().map(|o| pb::Outcome {
            id: o.id.clone(),
            label: o.label.clone(),
            token_id: o.token_id.clone().unwrap_or_default(),
        }).collect(),
        pricing: Some(pb::MarketPricing {
            last_price: m.pricing.last_price.clone(),
            best_bid: m.pricing.best_bid.clone(),
            best_ask: m.pricing.best_ask.clone(),
            mid_price: m.pricing.mid_price.clone(),
            spread: m.pricing.spread.clone(),
            tick_size: m.pricing.tick_size.clone(),
            currency: m.pricing.currency.clone(),
            min_order_size: m.pricing.min_order_size,
            max_order_size: m.pricing.max_order_size,
            updated_at: Some(datetime_to_pb(&m.pricing.updated_at)),
        }),
        volume: Some(pb::MarketVolume {
            total_volume: m.volume.total_volume.clone(),
            volume_24h: m.volume.volume_24h.clone(),
            volume_7d: m.volume.volume_7d.clone().unwrap_or_default(),
            open_interest: m.volume.open_interest.clone(),
            num_traders: m.volume.num_traders.unwrap_or(0),
            updated_at: Some(datetime_to_pb(&m.volume.updated_at)),
        }),
        lifecycle: Some(pb::MarketLifecycle {
            status: match m.lifecycle.status {
                core::MarketStatus::Open => pb::MarketStatus::Open as i32,
                core::MarketStatus::Closed => pb::MarketStatus::Closed as i32,
                core::MarketStatus::Resolved => pb::MarketStatus::Resolved as i32,
                core::MarketStatus::Halted => pb::MarketStatus::Halted as i32,
                core::MarketStatus::Pending => pb::MarketStatus::Pending as i32,
                core::MarketStatus::Voided => pb::MarketStatus::Voided as i32,
                core::MarketStatus::Disputed => pb::MarketStatus::Halted as i32, // closest match
            },
            created_at: Some(datetime_to_pb(&m.lifecycle.created_at)),
            opens_at: m.lifecycle.opens_at.as_ref().map(datetime_to_pb),
            closes_at: m.lifecycle.closes_at.as_ref().map(datetime_to_pb),
            resolved_at: m.lifecycle.resolved_at.as_ref().map(datetime_to_pb),
            expires_at: m.lifecycle.expires_at.as_ref().map(datetime_to_pb),
            resolution_source: m.lifecycle.resolution_source.clone().unwrap_or_default(),
        }),
        rules: None,
        regulatory: None,
        provider_metadata: m.provider_metadata.clone(),
    }
}

fn datetime_to_pb(dt: &chrono::DateTime<chrono::Utc>) -> pb::Timestamp {
    pb::Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn capability_to_pb(c: crate::adapters::CapabilityDeclaration) -> pb::CapabilityDeclaration {
    pb::CapabilityDeclaration {
        name: c.name,
        version: c.version,
        schema_url: String::new(),
        operations: c.operations,
        extensions: c.extensions.into_iter().map(|ext| pb::ExtensionDeclaration {
            name: ext,
            version: String::new(),
            schema_url: String::new(),
            operations: Vec::new(),
        }).collect(),
        config: None,
    }
}

fn core_orderbook_to_pb(snap: &crate::adapters::OrderBookSnapshot) -> pb::OrderBook {
    pb::OrderBook {
        market_id: None,
        outcome_id: snap.outcome_id.clone(),
        bids: snap.bids.iter().map(|l| pb::OrderBookLevel {
            price: l.price.clone(),
            quantity: l.quantity,
        }).collect(),
        asks: snap.asks.iter().map(|l| pb::OrderBookLevel {
            price: l.price.clone(),
            quantity: l.quantity,
        }).collect(),
        timestamp: None,
        asks_computed: snap.asks_computed,
    }
}

// ─── MarketService ───────────────────────────────────────────

pub struct UppMarketService {
    pub state: GrpcState,
}

#[tonic::async_trait]
impl pb::market_service_server::MarketService for UppMarketService {
    async fn list_markets(
        &self,
        request: Request<pb::ListMarketsRequest>,
    ) -> Result<Response<pb::ListMarketsResponse>, Status> {
        let req = request.into_inner();
        let filter = crate::adapters::MarketFilter {
            provider: if req.provider.is_empty() { None } else { Some(req.provider.clone()) },
            category: if req.category.is_empty() { None } else { Some(req.category) },
            status: None,
            market_type: None,
            sort_by: None,
            pagination: core::PaginationRequest {
                limit: if req.pagination.as_ref().map_or(true, |p| p.limit == 0) {
                    Some(20)
                } else {
                    Some(req.pagination.as_ref().unwrap().limit)
                },
                cursor: req.pagination.as_ref().and_then(|p| {
                    if p.cursor.is_empty() { None } else { Some(p.cursor.clone()) }
                }),
            },
            ..Default::default()
        };

        let provider_ids: Vec<String> = if req.provider.is_empty() {
            self.state.registry.provider_ids()
        } else {
            vec![req.provider]
        };

        let mut all_markets = Vec::new();
        for pid in &provider_ids {
            if let Some(adapter) = self.state.registry.get(pid) {
                match adapter.list_markets(filter.clone()).await {
                    Ok(page) => {
                        for m in &page.markets {
                            self.state.cache.put_market(m.id.to_full_id(), m.clone()).await;
                        }
                        all_markets.extend(page.markets.iter().map(core_market_to_pb));
                    }
                    Err(e) => {
                        tracing::warn!(provider = %pid, error = %e, "gRPC list_markets failed");
                    }
                }
            }
        }

        Ok(Response::new(pb::ListMarketsResponse {
            markets: all_markets,
            pagination: Some(pb::PaginationResponse {
                cursor: String::new(),
                has_more: false,
                total: -1,
            }),
        }))
    }

    async fn get_market(
        &self,
        request: Request<pb::GetMarketRequest>,
    ) -> Result<Response<pb::Market>, Status> {
        let req = request.into_inner();

        // Determine market lookup key
        let (provider_id, native_id) = if !req.universal_id.is_empty() {
            parse_market_id(&req.universal_id)
        } else if !req.provider.is_empty() && !req.native_id.is_empty() {
            (req.provider.clone(), req.native_id.clone())
        } else {
            return Err(Status::invalid_argument("Provide universal_id or provider+native_id"));
        };

        let cache_key = format!("upp:{}:{}", provider_id, native_id);

        // Try cache first
        if let Some(cached) = self.state.cache.get_market(&cache_key).await {
            return Ok(Response::new(core_market_to_pb(&cached)));
        }

        let adapter = self.state.registry.get(&provider_id)
            .ok_or_else(|| Status::not_found(format!("Unknown provider: {}", provider_id)))?;

        match adapter.get_market(&native_id).await {
            Ok(market) => {
                self.state.cache.put_market(cache_key, market.clone()).await;
                Ok(Response::new(core_market_to_pb(&market)))
            }
            Err(e) => Err(Status::not_found(format!("Market not found: {}", e))),
        }
    }

    async fn search_markets(
        &self,
        request: Request<pb::SearchMarketsRequest>,
    ) -> Result<Response<pb::SearchMarketsResponse>, Status> {
        let req = request.into_inner();
        let filter = crate::adapters::MarketFilter {
            pagination: core::PaginationRequest {
                limit: Some(req.pagination.as_ref().map_or(20, |p| if p.limit == 0 { 20 } else { p.limit })),
                cursor: None,
            },
            ..Default::default()
        };

        let mut results = Vec::new();
        for pid in self.state.registry.provider_ids() {
            if let Some(adapter) = self.state.registry.get(&pid) {
                match adapter.search_markets(&req.query, filter.clone()).await {
                    Ok(page) => results.extend(page.markets.iter().map(core_market_to_pb)),
                    Err(_) => {}
                }
            }
        }

        Ok(Response::new(pb::SearchMarketsResponse {
            markets: results,
            pagination: Some(pb::PaginationResponse {
                cursor: String::new(),
                has_more: false,
                total: -1,
            }),
        }))
    }

    async fn get_order_book(
        &self,
        request: Request<pb::GetOrderBookRequest>,
    ) -> Result<Response<pb::OrderBook>, Status> {
        let req = request.into_inner();
        let (provider_id, native_id) = parse_market_id(&req.universal_id);

        let adapter = self.state.registry.get(&provider_id)
            .ok_or_else(|| Status::not_found(format!("Unknown provider: {}", provider_id)))?;

        let outcome = if req.outcome_id.is_empty() { None } else { Some(req.outcome_id.as_str()) };

        match adapter.get_orderbook(&native_id, outcome, req.depth).await {
            Ok(snapshots) => {
                if let Some(snap) = snapshots.first() {
                    Ok(Response::new(core_orderbook_to_pb(snap)))
                } else {
                    Ok(Response::new(pb::OrderBook::default()))
                }
            }
            Err(e) => Err(Status::internal(format!("Orderbook error: {}", e))),
        }
    }

    async fn list_categories(
        &self,
        _request: Request<pb::ListCategoriesRequest>,
    ) -> Result<Response<pb::ListCategoriesResponse>, Status> {
        let categories = vec![
            "politics", "crypto", "sports", "science",
            "economics", "entertainment", "weather", "technology",
        ];
        Ok(Response::new(pb::ListCategoriesResponse {
            categories: categories.into_iter().map(|c| pb::Category {
                id: c.to_string(),
                name: c.to_string(),
                market_count: 0,
            }).collect(),
        }))
    }

    async fn get_related_markets(
        &self,
        _request: Request<pb::GetRelatedMarketsRequest>,
    ) -> Result<Response<pb::GetRelatedMarketsResponse>, Status> {
        Ok(Response::new(pb::GetRelatedMarketsResponse {
            markets: Vec::new(),
        }))
    }
}

// ─── DiscoveryService ────────────────────────────────────────

pub struct UppDiscoveryService {
    pub state: GrpcState,
}

#[tonic::async_trait]
impl pb::discovery_service_server::DiscoveryService for UppDiscoveryService {
    async fn get_manifest(
        &self,
        request: Request<pb::GetManifestRequest>,
    ) -> Result<Response<pb::ProviderManifest>, Status> {
        let req = request.into_inner();
        match self.state.registry.get_manifest(&req.provider).await {
            Ok(manifest) => {
                Ok(Response::new(pb::ProviderManifest {
                    upp_version: manifest.upp_version,
                    provider: Some(pb::ProviderInfo {
                        name: manifest.provider.name.clone(),
                        id: manifest.provider.id.clone(),
                        r#type: 0, // Unspecified, let adapter set this
                        description: String::new(),
                        website_url: String::new(),
                        docs_url: String::new(),
                        support_email: String::new(),
                        logo_url: String::new(),
                        regulatory: if manifest.provider.jurisdictions.is_empty() {
                            None
                        } else {
                            Some(pb::RegulatoryInfo {
                                jurisdictions: manifest.provider.jurisdictions,
                                regulator: String::new(),
                                license_type: String::new(),
                                compliant: true,
                                compliance_url: String::new(),
                            })
                        },
                    }),
                    capabilities: manifest.capabilities.into_iter().map(|c| capability_to_pb(c)).collect(),
                    transport: None,
                    authentication: None,
                    rate_limits: None,
                    updated_at: None,
                }))
            }
            Err(e) => Err(Status::not_found(e.to_string())),
        }
    }

    async fn list_providers(
        &self,
        _request: Request<pb::ListProvidersRequest>,
    ) -> Result<Response<pb::ListProvidersResponse>, Status> {
        let manifests = self.state.registry.list_providers().await;
        let pb_manifests: Vec<pb::ProviderManifest> = manifests.into_iter().map(|m| {
            pb::ProviderManifest {
                upp_version: m.upp_version,
                provider: Some(pb::ProviderInfo {
                    name: m.provider.name.clone(),
                    id: m.provider.id.clone(),
                    r#type: 0,
                    description: String::new(),
                    website_url: String::new(),
                    docs_url: String::new(),
                    support_email: String::new(),
                    logo_url: String::new(),
                    regulatory: None,
                }),
                capabilities: m.capabilities.into_iter().map(|c| capability_to_pb(c)).collect(),
                transport: None,
                authentication: None,
                rate_limits: None,
                updated_at: None,
            }
        }).collect();

        Ok(Response::new(pb::ListProvidersResponse {
            providers: pb_manifests,
        }))
    }

    async fn negotiate(
        &self,
        request: Request<pb::NegotiateRequest>,
    ) -> Result<Response<pb::NegotiationResult>, Status> {
        let req = request.into_inner();
        match self.state.registry.get_manifest(&req.provider).await {
            Ok(manifest) => {
                Ok(Response::new(pb::NegotiationResult {
                    active_capabilities: manifest.capabilities.into_iter().map(|c| capability_to_pb(c)).collect(),
                    active_extensions: Vec::new(),
                    selected_transport: "rest".to_string(),
                    selected_auth: 0,
                    restrictions: Vec::new(),
                }))
            }
            Err(e) => Err(Status::not_found(e.to_string())),
        }
    }

    async fn health_check(
        &self,
        request: Request<pb::HealthCheckRequest>,
    ) -> Result<Response<pb::HealthCheckResponse>, Status> {
        let req = request.into_inner();
        match self.state.registry.health_check(&req.provider).await {
            Ok(health) => Ok(Response::new(pb::HealthCheckResponse {
                provider: req.provider,
                healthy: health.healthy,
                status: if health.healthy { "operational".to_string() } else { "degraded".to_string() },
                latency_ms: health.latency_ms as i64,
                checked_at: None,
            })),
            Err(e) => Err(Status::unavailable(e.to_string())),
        }
    }
}

// ─── TradingService ──────────────────────────────────────────

pub struct UppTradingService {
    pub state: GrpcState,
}

#[tonic::async_trait]
impl pb::trading_service_server::TradingService for UppTradingService {
    async fn create_order(
        &self,
        request: Request<pb::CreateOrderRequest>,
    ) -> Result<Response<pb::CreateOrderResponse>, Status> {
        let req = request.into_inner();
        let provider_id = req.provider.clone();

        let adapter = self.state.registry.get(&provider_id)
            .ok_or_else(|| Status::invalid_argument(format!("Unknown provider: {}", provider_id)))?;

        let side = match req.side {
            1 => core::Side::Buy,
            2 => core::Side::Sell,
            _ => return Err(Status::invalid_argument("Invalid side")),
        };

        let order_type = match req.order_type {
            1 => core::OrderType::Limit,
            2 => core::OrderType::Market,
            _ => core::OrderType::Limit,
        };

        let tif = match req.tif {
            1 => core::TimeInForce::Gtc,
            2 => core::TimeInForce::Fok,
            3 => core::TimeInForce::Ioc,
            4 => core::TimeInForce::Gtd,
            _ => core::TimeInForce::Gtc,
        };

        let create_req = crate::adapters::CreateOrderRequest {
            market_native_id: req.market_native_id,
            outcome_id: req.outcome_id,
            side,
            order_type,
            tif,
            price: if req.price.is_empty() { None } else { Some(req.price) },
            quantity: req.quantity,
            client_order_id: if req.client_order_id.is_empty() { None } else { Some(req.client_order_id) },
        };

        match adapter.create_order(create_req).await {
            Ok(order) => Ok(Response::new(pb::CreateOrderResponse {
                order: Some(core_order_to_pb(&order)),
                estimate: None,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn cancel_order(
        &self,
        request: Request<pb::CancelOrderRequest>,
    ) -> Result<Response<pb::CancelOrderResponse>, Status> {
        let req = request.into_inner();
        let adapter = self.state.registry.get(&req.provider)
            .ok_or_else(|| Status::not_found("Unknown provider"))?;

        match adapter.cancel_order(&req.order_id).await {
            Ok(order) => Ok(Response::new(pb::CancelOrderResponse {
                order: Some(core_order_to_pb(&order)),
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn cancel_all_orders(
        &self,
        request: Request<pb::CancelAllOrdersRequest>,
    ) -> Result<Response<pb::CancelAllOrdersResponse>, Status> {
        let req = request.into_inner();
        let adapter = self.state.registry.get(&req.provider)
            .ok_or_else(|| Status::not_found("Unknown provider"))?;

        let market_id = if req.market_id.is_empty() { None } else { Some(req.market_id.as_str()) };
        match adapter.cancel_all_orders(market_id).await {
            Ok(ids) => {
                let count = ids.len() as i32;
                Ok(Response::new(pb::CancelAllOrdersResponse {
                    cancelled_count: count,
                    cancelled_order_ids: ids,
                }))
            }
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_order(
        &self,
        request: Request<pb::GetOrderRequest>,
    ) -> Result<Response<pb::Order>, Status> {
        let req = request.into_inner();
        let adapter = self.state.registry.get(&req.provider)
            .ok_or_else(|| Status::not_found("Unknown provider"))?;

        match adapter.get_order(&req.order_id).await {
            Ok(order) => Ok(Response::new(core_order_to_pb(&order))),
            Err(e) => Err(Status::not_found(e.to_string())),
        }
    }

    async fn list_orders(
        &self,
        request: Request<pb::ListOrdersRequest>,
    ) -> Result<Response<pb::ListOrdersResponse>, Status> {
        let req = request.into_inner();
        let provider_ids: Vec<String> = if req.provider.is_empty() {
            self.state.registry.provider_ids()
        } else {
            vec![req.provider]
        };

        let mut all_orders = Vec::new();
        for pid in &provider_ids {
            if let Some(adapter) = self.state.registry.get(pid) {
                let filter = crate::adapters::OrderFilter {
                    market_id: if req.market_id.is_empty() { None } else { Some(req.market_id.clone()) },
                    status: None,
                    side: None,
                    pagination: core::PaginationRequest {
                        limit: Some(50),
                        cursor: None,
                    },
                };
                if let Ok(page) = adapter.list_orders(filter).await {
                    all_orders.extend(page.orders.into_iter().map(|o| core_order_to_pb(&o)));
                }
            }
        }

        Ok(Response::new(pb::ListOrdersResponse {
            orders: all_orders,
            pagination: Some(pb::PaginationResponse {
                cursor: String::new(),
                has_more: false,
                total: -1,
            }),
        }))
    }

    async fn list_trades(
        &self,
        request: Request<pb::ListTradesRequest>,
    ) -> Result<Response<pb::ListTradesResponse>, Status> {
        let req = request.into_inner();
        let provider_ids: Vec<String> = if req.provider.is_empty() {
            self.state.registry.provider_ids()
        } else {
            vec![req.provider]
        };

        let mut all_trades = Vec::new();
        for pid in &provider_ids {
            if let Some(adapter) = self.state.registry.get(pid) {
                let filter = crate::adapters::TradeFilter {
                    market_id: if req.market_id.is_empty() { None } else { Some(req.market_id.clone()) },
                    order_id: if req.order_id.is_empty() { None } else { Some(req.order_id.clone()) },
                    pagination: core::PaginationRequest {
                        limit: Some(50),
                        cursor: None,
                    },
                };
                if let Ok(page) = adapter.list_trades(filter).await {
                    all_trades.extend(page.trades.into_iter().map(|t| core_trade_to_pb(&t)));
                }
            }
        }

        Ok(Response::new(pb::ListTradesResponse {
            trades: all_trades,
            pagination: Some(pb::PaginationResponse {
                cursor: String::new(),
                has_more: false,
                total: -1,
            }),
        }))
    }

    async fn estimate_order(
        &self,
        request: Request<pb::EstimateOrderRequest>,
    ) -> Result<Response<pb::EstimateOrderResponse>, Status> {
        let req = request.into_inner();
        let price: f64 = req.price.parse().unwrap_or(0.5);
        let cost = price * req.quantity as f64;

        Ok(Response::new(pb::EstimateOrderResponse {
            estimate: Some(pb::OrderEstimate {
                max_cost: format!("{:.2}", cost),
                estimated_fees: "0.00".to_string(),
                total_with_fees: format!("{:.2}", cost),
                funding_instrument: String::new(),
                available_balance: String::new(),
                sufficient_funds: true,
            }),
        }))
    }
}

// ─── Order/Trade Conversions ─────────────────────────────────

fn core_order_to_pb(o: &core::Order) -> pb::Order {
    pb::Order {
        id: o.id.clone(),
        provider_order_id: o.provider_order_id.clone(),
        client_order_id: o.client_order_id.clone().unwrap_or_default(),
        market_id: Some(core_market_id_to_pb(&o.market_id)),
        outcome_id: o.outcome_id.clone(),
        side: match o.side {
            core::Side::Buy => 1,
            core::Side::Sell => 2,
        },
        order_type: match o.order_type {
            core::OrderType::Limit => 1,
            core::OrderType::Market => 2,
        },
        tif: match o.tif {
            core::TimeInForce::Gtc => 1,
            core::TimeInForce::Fok => 2,
            core::TimeInForce::Ioc => 3,
            core::TimeInForce::Gtd => 4,
        },
        price: o.price.clone().unwrap_or_default(),
        quantity: o.quantity,
        filled_quantity: o.filled_quantity,
        average_fill_price: o.average_fill_price.clone().unwrap_or_default(),
        remaining_quantity: o.remaining_quantity,
        status: match o.status {
            core::OrderStatus::Pending => 0,
            core::OrderStatus::Open => 1,
            core::OrderStatus::PartiallyFilled => 2,
            core::OrderStatus::Filled => 3,
            core::OrderStatus::Cancelled => 4,
            core::OrderStatus::Expired => 5,
            core::OrderStatus::Rejected => 6,
        },
        fees: Some(pb::OrderFees {
            maker_fee: o.fees.maker_fee.clone(),
            taker_fee: o.fees.taker_fee.clone(),
            total_fee: o.fees.total_fee.clone(),
        }),
        cost_basis: o.cost_basis.clone().unwrap_or_default(),
        created_at: Some(datetime_to_pb(&o.created_at)),
        updated_at: Some(datetime_to_pb(&o.updated_at)),
        expires_at: o.expires_at.as_ref().map(datetime_to_pb),
        filled_at: o.filled_at.as_ref().map(datetime_to_pb),
        cancelled_at: o.cancelled_at.as_ref().map(datetime_to_pb),
        cancel_reason: o.cancel_reason.clone().unwrap_or_default(),
        provider_metadata: HashMap::new(),
    }
}

fn core_trade_to_pb(t: &core::Trade) -> pb::Trade {
    pb::Trade {
        id: t.id.clone(),
        order_id: t.order_id.clone(),
        market_id: Some(core_market_id_to_pb(&t.market_id)),
        outcome_id: t.outcome_id.clone(),
        side: match t.side {
            core::Side::Buy => 1,
            core::Side::Sell => 2,
        },
        price: t.price.clone(),
        quantity: t.quantity,
        notional: t.notional.clone(),
        role: match t.role {
            core::TradeRole::Maker => 1,
            core::TradeRole::Taker => 2,
        },
        fees: Some(pb::OrderFees {
            maker_fee: t.fees.maker_fee.clone(),
            taker_fee: t.fees.taker_fee.clone(),
            total_fee: t.fees.total_fee.clone(),
        }),
        executed_at: Some(datetime_to_pb(&t.executed_at)),
        provider_metadata: HashMap::new(),
    }
}

// ─── Helpers ─────────────────────────────────────────────────

fn parse_market_id(id: &str) -> (String, String) {
    let id = id.strip_prefix("upp:").unwrap_or(id);
    if let Some(colon_pos) = id.find(':') {
        (id[..colon_pos].to_string(), id[colon_pos + 1..].to_string())
    } else {
        ("kalshi.com".to_string(), id.to_string())
    }
}

// ─── Server Startup ──────────────────────────────────────────

/// Start the gRPC server on the given port.
pub async fn start_grpc_server(
    state: GrpcState,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let market_svc = UppMarketService { state: state.clone() };
    let discovery_svc = UppDiscoveryService { state: state.clone() };
    let trading_svc = UppTradingService { state: state.clone() };

    info!(address = %addr, "gRPC server listening");

    tonic::transport::Server::builder()
        .add_service(pb::market_service_server::MarketServiceServer::new(market_svc))
        .add_service(pb::discovery_service_server::DiscoveryServiceServer::new(discovery_svc))
        .add_service(pb::trading_service_server::TradingServiceServer::new(trading_svc))
        .serve(addr)
        .await?;

    Ok(())
}
