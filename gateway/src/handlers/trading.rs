// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Path, Query, State}, response::IntoResponse, Json};
use axum::http::StatusCode;
use serde::Deserialize;
use tracing::warn;

use crate::{AppState, bad_request, not_found, internal_error};
use crate::core::types::*;
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
    if let Ok(Some(stored)) = state.storage.get_order(&order_id).await {
        return Json(serde_json::to_value(&stored).unwrap()).into_response();
    }

    let provider_id = params.provider.unwrap_or_else(|| "kalshi.com".to_string());
    if let Some(adapter) = state.registry.get(&provider_id) {
        match adapter.get_order(&order_id).await {
            Ok(order) => {
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
