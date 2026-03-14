// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Path, Query, State}, response::IntoResponse, Json};
use serde::Deserialize;
use tracing::warn;

use crate::{AppState, not_found, internal_error};
use crate::core::types::*;
use crate::adapters::MarketFilter;

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

    let agg = crate::core::aggregation::parallel_list_markets(
        &state.registry, filter, provider_ids,
    ).await;

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
            state.cache.put_market(cache_key, market.clone()).await;
            return Json(serde_json::to_value(&market).unwrap()).into_response();
        }
    }

    let (provider_id, native_id) = parse_market_id(&market_id);

    if let Some(adapter) = state.registry.get(&provider_id) {
        match adapter.get_market(&native_id).await {
            Ok(market) => {
                state.cache.put_market(cache_key.clone(), market.clone()).await;
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

pub fn parse_market_id(id: &str) -> (String, String) {
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
