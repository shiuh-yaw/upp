// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;

use crate::{AppState, bad_request, internal_error};
use crate::core::types::*;

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

/// GET /upp/v1/orders/route/stats
pub async fn get_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let stats = state.smart_router.stats();
    Json(serde_json::to_value(&stats).unwrap_or_default())
}
