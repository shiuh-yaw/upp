// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{State, ws::WebSocketUpgrade, Query}, response::IntoResponse, http::StatusCode};
use axum::extract::ws::{WebSocket, Message};
use axum::http::HeaderMap;
use futures::{StreamExt, SinkExt};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::broadcast;
use tracing::{info, warn, debug};
use serde::Deserialize;

use crate::AppState;
use crate::transport;
use crate::middleware::auth::{AuthResult, ClientTier};

/// Query parameters for WebSocket upgrade (e.g., ?token=...)
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    #[serde(default)]
    pub token: Option<String>,
}

/// Client info passed to handle_ws for subscription enforcement.
#[derive(Debug, Clone)]
pub struct WsClientContext {
    /// The client's subscription tier (affects market subscription limits).
    pub tier: ClientTier,
    /// Maximum market subscriptions allowed for this client.
    pub max_subscriptions: usize,
}

impl WsClientContext {
    /// Create context from a client tier.
    pub fn from_tier(tier: ClientTier) -> Self {
        let max_subscriptions = match tier {
            ClientTier::Free => 5,
            ClientTier::Standard => 50,
            ClientTier::Pro => 500,
            ClientTier::Enterprise => usize::MAX, // Unlimited
        };
        Self {
            tier,
            max_subscriptions,
        }
    }
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Try to authenticate from either X-API-Key header or token query parameter
    let auth_token = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or(query.token);

    // Build the path for auth (use a standard WebSocket path)
    let auth_path = "/upp/v1/ws/upgrade";

    // Authenticate: if auth is enabled and token provided, validate it
    let client_context = if auth_token.is_some() {
        // Token was provided, so validate it
        let mut headers_with_key = headers.clone();
        if let Some(token) = auth_token {
            headers_with_key.insert(
                axum::http::header::HeaderName::from_static("x-api-key"),
                token.parse().map_err(|_| (StatusCode::BAD_REQUEST, "Invalid token format".to_string()))?,
            );
        }

        match state.auth.authenticate(&headers_with_key, auth_path) {
            AuthResult::Authenticated(client_info) => {
                info!("WebSocket client authenticated: {}", client_info.client_id);
                WsClientContext::from_tier(client_info.tier)
            }
            AuthResult::Unauthorized(reason) => {
                warn!("WebSocket authentication failed: {}", reason);
                return Err((StatusCode::UNAUTHORIZED, reason));
            }
            AuthResult::Forbidden(reason) => {
                warn!("WebSocket forbidden: {}", reason);
                return Err((StatusCode::FORBIDDEN, reason));
            }
            AuthResult::Public => {
                // Public access — use Free tier limits
                WsClientContext::from_tier(ClientTier::Free)
            }
        }
    } else {
        // No token provided — try public auth path
        match state.auth.authenticate(&headers, auth_path) {
            AuthResult::Public => {
                // Allowed as public — use Free tier limits
                WsClientContext::from_tier(ClientTier::Free)
            }
            AuthResult::Authenticated(client_info) => {
                info!("WebSocket client authenticated: {}", client_info.client_id);
                WsClientContext::from_tier(client_info.tier)
            }
            AuthResult::Unauthorized(reason) => {
                warn!("WebSocket authentication required but none provided: {}", reason);
                return Err((StatusCode::UNAUTHORIZED, reason));
            }
            AuthResult::Forbidden(reason) => {
                warn!("WebSocket forbidden: {}", reason);
                return Err((StatusCode::FORBIDDEN, reason));
            }
        }
    };

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, client_context)))
}

/// Internal message type for the send queue
#[derive(Debug, Clone)]
enum SendMessage {
    FanOut(transport::websocket::FanOutMessage),
    JsonRpc(serde_json::Value),
    Heartbeat,
}

async fn handle_ws(socket: WebSocket, state: AppState, client_context: WsClientContext) {
    state.metrics.ws_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let (ws_sender, ws_receiver) = socket.split();
    let (tx_queue, rx_queue) = mpsc::channel::<SendMessage>(256);
    let subscriptions = Arc::new(tokio::sync::Mutex::new(HashMap::<String, HashSet<String>>::new()));

    info!(
        "WebSocket client connected with tier {:?}, max subscriptions: {}",
        client_context.tier, client_context.max_subscriptions
    );

    // Receive task
    let rx_handle = {
        let state_clone = state.clone();
        let subscriptions_clone = Arc::clone(&subscriptions);
        let tx_queue_clone = tx_queue.clone();
        let client_ctx = client_context.clone();

        tokio::spawn(async move {
            let mut receiver = ws_receiver;
            while let Some(Ok(msg)) = receiver.next().await {
                match msg {
                    Message::Text(text) => {
                        handle_incoming_rpc(&text, &state_clone, &subscriptions_clone, &tx_queue_clone, &client_ctx).await;
                    }
                    Message::Ping(_data) => {
                        let _ = tx_queue_clone.send(SendMessage::Heartbeat).await;
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        })
    };

    // Send task
    let send_handle = {
        let ws_sender_arc = Arc::new(tokio::sync::Mutex::new(ws_sender));
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
                    SendMessage::JsonRpc(value) => Message::Text(value.to_string()),
                    SendMessage::Heartbeat => Message::Ping(vec![]),
                };

                let mut sender = ws_sender_arc.lock().await;
                if sender.send(ws_msg).await.is_err() {
                    break;
                }
            }
        })
    };

    // Heartbeat task
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

    tokio::select! {
        _ = rx_handle => { info!("RPC receiver task exited"); }
        _ = send_handle => { info!("Send task exited"); }
        _ = heartbeat_handle => { info!("Heartbeat task exited"); }
    }

    // Cleanup
    let all_subs = {
        let subs = subscriptions.lock().await;
        subs.clone()
    };
    for (channel, market_ids) in all_subs {
        for market_id in market_ids {
            state.ws_manager.unsubscribe(&channel, &market_id).await;
        }
    }

    state.metrics.ws_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    info!("WebSocket client disconnected");
}

async fn handle_incoming_rpc(
    text: &str,
    state: &AppState,
    subscriptions: &Arc<tokio::sync::Mutex<HashMap<String, HashSet<String>>>>,
    tx_queue: &mpsc::Sender<SendMessage>,
    client_context: &WsClientContext,
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
            let market_ids = extract_market_ids(&params);
            // Enforce tier-based subscription limits
            if let Err(error_msg) = check_subscription_limit(subscriptions, &market_ids, client_context).await {
                (serde_json::json!({
                    "error": error_msg,
                    "tier": format!("{:?}", client_context.tier),
                    "max_subscriptions": client_context.max_subscriptions
                }), None)
            } else {
                track_subscriptions(subscriptions, "prices", &market_ids).await;
                let tasks = spawn_subscription_tasks(state, "prices", &market_ids, tx_queue.clone()).await;
                (serde_json::json!({ "subscribed": market_ids, "channel": "prices", "status": "active" }), Some(tasks))
            }
        }
        "subscribe_orderbook" => {
            let market_ids = extract_market_ids(&params);
            // Enforce tier-based subscription limits
            if let Err(error_msg) = check_subscription_limit(subscriptions, &market_ids, client_context).await {
                (serde_json::json!({
                    "error": error_msg,
                    "tier": format!("{:?}", client_context.tier),
                    "max_subscriptions": client_context.max_subscriptions
                }), None)
            } else {
                track_subscriptions(subscriptions, "orderbook", &market_ids).await;
                let tasks = spawn_subscription_tasks(state, "orderbook", &market_ids, tx_queue.clone()).await;
                (serde_json::json!({ "subscribed": market_ids, "channel": "orderbook", "status": "active" }), Some(tasks))
            }
        }
        "subscribe_arbitrage" => {
            let market_ids = extract_market_ids(&params);
            let subscribe_ids = if market_ids.is_empty() {
                vec!["*".to_string()]
            } else {
                market_ids.clone()
            };
            // Enforce tier-based subscription limits
            if let Err(error_msg) = check_subscription_limit(subscriptions, &subscribe_ids, client_context).await {
                (serde_json::json!({
                    "error": error_msg,
                    "tier": format!("{:?}", client_context.tier),
                    "max_subscriptions": client_context.max_subscriptions
                }), None)
            } else {
                track_subscriptions(subscriptions, "arbitrage", &subscribe_ids).await;
                let tasks = spawn_subscription_tasks(state, "arbitrage", &subscribe_ids, tx_queue.clone()).await;
                (serde_json::json!({
                    "subscribed": subscribe_ids,
                    "channel": "arbitrage",
                    "filter": if market_ids.is_empty() { "all" } else { "filtered" },
                    "status": "active"
                }), Some(tasks))
            }
        }
        "unsubscribe" => {
            let channel = params.get("channel").and_then(|v| v.as_str()).unwrap_or("");
            let market_ids = extract_market_ids(&params);
            {
                let mut subs = subscriptions.lock().await;
                if let Some(channel_subs) = subs.get_mut(channel) {
                    for market_id in &market_ids {
                        channel_subs.remove(market_id);
                    }
                }
            }
            for market_id in &market_ids {
                state.ws_manager.unsubscribe(channel, market_id).await;
            }
            (serde_json::json!({ "status": "unsubscribed", "channel": channel, "market_ids": market_ids }), None)
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
            (serde_json::json!({
                "error": format!("Unknown method: {}", method),
                "available_methods": ["subscribe_prices", "subscribe_orderbook", "subscribe_arbitrage", "unsubscribe", "get_market", "ping"]
            }), None)
        }
    };

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    });

    let _ = tx_queue.send(SendMessage::JsonRpc(response)).await;
    let _ = new_task;
}

fn extract_market_ids(params: &serde_json::Value) -> Vec<String> {
    params.get("market_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Check if adding new market subscriptions would exceed the tier limit.
/// Returns Ok(()) if allowed, Err(message) if limit exceeded.
async fn check_subscription_limit(
    subscriptions: &Arc<tokio::sync::Mutex<HashMap<String, HashSet<String>>>>,
    new_market_ids: &[String],
    client_context: &WsClientContext,
) -> Result<(), String> {
    if client_context.max_subscriptions == usize::MAX {
        // Enterprise tier: unlimited
        return Ok(());
    }

    let subs = subscriptions.lock().await;

    // Count total unique markets across all channels
    let mut all_markets = HashSet::new();
    for (_channel, market_set) in subs.iter() {
        for market in market_set {
            all_markets.insert(market.clone());
        }
    }

    // Check if adding new markets would exceed limit
    let new_count = new_market_ids.len();
    let total_would_be = all_markets.len() + new_count;

    if total_would_be > client_context.max_subscriptions {
        return Err(format!(
            "Subscription limit exceeded. Current: {}, requested: {}, max allowed: {}",
            all_markets.len(),
            new_count,
            client_context.max_subscriptions
        ));
    }

    Ok(())
}

async fn track_subscriptions(
    subscriptions: &Arc<tokio::sync::Mutex<HashMap<String, HashSet<String>>>>,
    channel: &str,
    market_ids: &[String],
) {
    let mut subs = subscriptions.lock().await;
    let channel_subs = subs.entry(channel.to_string()).or_insert_with(HashSet::new);
    for market_id in market_ids {
        channel_subs.insert(market_id.clone());
    }
}

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
            let mut rx = state_clone.ws_manager.subscribe(
                &channel_clone, &market_id_clone,
            ).await;

            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        if tx_queue_clone.send(SendMessage::FanOut(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        debug!("Subscriber lagged on {}: {}", channel_clone, market_id_clone);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            state_clone.ws_manager.unsubscribe(&channel_clone, &market_id_clone).await;
            debug!("Unsubscribed from {}: {}", channel_clone, market_id_clone);
        });

        handles.push(handle);
    }

    handles
}
