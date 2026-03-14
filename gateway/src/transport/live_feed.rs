// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Live Feed Manager — real-time WebSocket connections to prediction market
// provider APIs (Kalshi, Polymarket, Opinion.trade).
//
// Architecture:
//   - Each provider gets a dedicated connection task with auto-reconnect
//   - Exponential backoff on disconnect (1s → 2s → 4s → ... → 60s)
//   - Heartbeat/ping every 30s per connection to detect stale links
//   - Incoming messages parsed + normalized → published to WebSocketManager fan-out
//   - Connection state machine: Disconnected → Connecting → Connected → Subscribed
//   - Health metrics exposed: connected_providers, reconnect_count, messages_received

use crate::transport::websocket::WebSocketManager;
use chrono::Utc;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn, debug};

// ─── Connection State ──────────────────────────────────────

/// State of a single provider WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Subscribed,
    Backoff,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Subscribed => write!(f, "subscribed"),
            ConnectionState::Backoff => write!(f, "backoff"),
        }
    }
}

/// Per-provider connection metadata.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderConnection {
    pub provider_id: String,
    pub ws_url: String,
    pub state: ConnectionState,
    pub connected_since: Option<String>,
    pub last_message_at: Option<String>,
    pub messages_received: u64,
    pub reconnect_count: u64,
    pub subscribed_markets: Vec<String>,
}

/// Configuration for a provider's live feed.
#[derive(Debug, Clone)]
pub struct FeedConfig {
    pub provider_id: String,
    pub ws_url: String,
    pub heartbeat_interval_secs: u64,
    pub max_backoff_secs: u64,
    pub initial_backoff_secs: u64,
    /// Markets to auto-subscribe on connect
    pub auto_subscribe: Vec<String>,
}

impl FeedConfig {
    pub fn kalshi() -> Self {
        Self {
            provider_id: "kalshi.com".to_string(),
            ws_url: "wss://api.elections.kalshi.com/trade-api/ws/v2".to_string(),
            heartbeat_interval_secs: 30,
            max_backoff_secs: 60,
            initial_backoff_secs: 1,
            auto_subscribe: vec![],
        }
    }

    pub fn polymarket() -> Self {
        Self {
            provider_id: "polymarket.com".to_string(),
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            heartbeat_interval_secs: 30,
            max_backoff_secs: 60,
            initial_backoff_secs: 1,
            auto_subscribe: vec![],
        }
    }

    pub fn opinion() -> Self {
        Self {
            provider_id: "opinion.trade".to_string(),
            ws_url: "wss://ws.opinion.trade/v1/stream".to_string(),
            heartbeat_interval_secs: 30,
            max_backoff_secs: 60,
            initial_backoff_secs: 1,
            auto_subscribe: vec![],
        }
    }
}

// ─── Provider Message Parsing ────────────────────────────────

/// Normalized incoming message from any provider.
#[derive(Debug, Clone)]
pub struct NormalizedUpdate {
    pub provider_id: String,
    pub market_id: String,     // UPP-format: upp:{provider}:{native_id}
    pub update_type: UpdateType,
    pub data: serde_json::Value,
    pub received_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateType {
    Price,
    OrderBook,
    Trade,
    MarketStatus,
}

/// Parse a Kalshi WebSocket message into normalized updates.
fn parse_kalshi_message(raw: &str) -> Vec<NormalizedUpdate> {
    let mut updates = Vec::new();
    let now = Utc::now();

    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(raw) {
        let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match msg_type {
            "orderbook_snapshot" | "orderbook_delta" => {
                if let Some(market_ticker) = msg.get("msg").and_then(|m| m.get("market_ticker")).and_then(|t| t.as_str()) {
                    let market_id = format!("upp:kalshi.com:{}", market_ticker);

                    // Extract yes/no prices from orderbook
                    if let Some(msg_body) = msg.get("msg") {
                        let yes_price = msg_body.get("yes")
                            .and_then(|y| y.as_array())
                            .and_then(|a| a.first())
                            .and_then(|l| l.as_array())
                            .and_then(|l| l.first())
                            .and_then(|p| p.as_f64())
                            .map(|p| p / 100.0); // Kalshi prices in cents

                        let price_data = serde_json::json!({
                            "market_id": market_id,
                            "prices": {
                                "yes": yes_price.map(|p| format!("{:.2}", p)).unwrap_or_default(),
                                "no": yes_price.map(|p| format!("{:.2}", 1.0 - p)).unwrap_or_default(),
                            },
                            "raw": msg_body,
                        });

                        updates.push(NormalizedUpdate {
                            provider_id: "kalshi.com".to_string(),
                            market_id: market_id.clone(),
                            update_type: UpdateType::OrderBook,
                            data: price_data,
                            received_at: now,
                        });

                        // Also emit a price update
                        if let Some(yp) = yes_price {
                            updates.push(NormalizedUpdate {
                                provider_id: "kalshi.com".to_string(),
                                market_id,
                                update_type: UpdateType::Price,
                                data: serde_json::json!({
                                    "yes": format!("{:.4}", yp),
                                    "no": format!("{:.4}", 1.0 - yp),
                                }),
                                received_at: now,
                            });
                        }
                    }
                }
            }
            "trade" | "fill" => {
                if let Some(market_ticker) = msg.get("msg").and_then(|m| m.get("market_ticker")).and_then(|t| t.as_str()) {
                    updates.push(NormalizedUpdate {
                        provider_id: "kalshi.com".to_string(),
                        market_id: format!("upp:kalshi.com:{}", market_ticker),
                        update_type: UpdateType::Trade,
                        data: msg.get("msg").cloned().unwrap_or_default(),
                        received_at: now,
                    });
                }
            }
            _ => {
                debug!(msg_type = %msg_type, "Unhandled Kalshi message type");
            }
        }
    }

    updates
}

/// Parse a Polymarket WebSocket message into normalized updates.
fn parse_polymarket_message(raw: &str) -> Vec<NormalizedUpdate> {
    let mut updates = Vec::new();
    let now = Utc::now();

    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(raw) {
        // Polymarket sends array of events
        let events = if msg.is_array() {
            msg.as_array().cloned().unwrap_or_default()
        } else {
            vec![msg]
        };

        for event in events {
            let event_type = event.get("event_type").and_then(|t| t.as_str()).unwrap_or("");
            let asset_id = event.get("asset_id").and_then(|t| t.as_str()).unwrap_or("");

            if asset_id.is_empty() { continue; }

            let market_id = format!("upp:polymarket.com:{}", asset_id);

            match event_type {
                "price_change" | "book" => {
                    let price = event.get("price")
                        .and_then(|p| p.as_str())
                        .or_else(|| event.get("price").and_then(|p| p.as_f64()).map(|_| ""))
                        .unwrap_or("");

                    if !price.is_empty() {
                        updates.push(NormalizedUpdate {
                            provider_id: "polymarket.com".to_string(),
                            market_id: market_id.clone(),
                            update_type: UpdateType::Price,
                            data: serde_json::json!({
                                "yes": price,
                                "no": format!("{:.4}", 1.0 - price.parse::<f64>().unwrap_or(0.5)),
                            }),
                            received_at: now,
                        });
                    }

                    // Orderbook data
                    if event.get("bids").is_some() || event.get("asks").is_some() {
                        updates.push(NormalizedUpdate {
                            provider_id: "polymarket.com".to_string(),
                            market_id,
                            update_type: UpdateType::OrderBook,
                            data: event.clone(),
                            received_at: now,
                        });
                    }
                }
                "trade" | "last_trade_price" => {
                    updates.push(NormalizedUpdate {
                        provider_id: "polymarket.com".to_string(),
                        market_id,
                        update_type: UpdateType::Trade,
                        data: event.clone(),
                        received_at: now,
                    });
                }
                _ => {
                    debug!(event_type = %event_type, "Unhandled Polymarket event type");
                }
            }
        }
    }

    updates
}

/// Parse an Opinion.trade WebSocket message into normalized updates.
fn parse_opinion_message(raw: &str) -> Vec<NormalizedUpdate> {
    let mut updates = Vec::new();
    let now = Utc::now();

    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(raw) {
        let channel = msg.get("channel").and_then(|c| c.as_str()).unwrap_or("");
        let market_slug = msg.get("market").and_then(|m| m.as_str()).unwrap_or("");

        if market_slug.is_empty() { return updates; }

        let market_id = format!("upp:opinion.trade:{}", market_slug);

        match channel {
            "prices" | "ticker" => {
                if let Some(data) = msg.get("data") {
                    let yes_price = data.get("yes_price").and_then(|p| p.as_f64())
                        .or_else(|| data.get("price").and_then(|p| p.as_f64()));

                    if let Some(yp) = yes_price {
                        updates.push(NormalizedUpdate {
                            provider_id: "opinion.trade".to_string(),
                            market_id,
                            update_type: UpdateType::Price,
                            data: serde_json::json!({
                                "yes": format!("{:.4}", yp),
                                "no": format!("{:.4}", 1.0 - yp),
                            }),
                            received_at: now,
                        });
                    }
                }
            }
            "orderbook" => {
                updates.push(NormalizedUpdate {
                    provider_id: "opinion.trade".to_string(),
                    market_id,
                    update_type: UpdateType::OrderBook,
                    data: msg.get("data").cloned().unwrap_or_default(),
                    received_at: now,
                });
            }
            "trades" => {
                updates.push(NormalizedUpdate {
                    provider_id: "opinion.trade".to_string(),
                    market_id,
                    update_type: UpdateType::Trade,
                    data: msg.get("data").cloned().unwrap_or_default(),
                    received_at: now,
                });
            }
            _ => {
                debug!(channel = %channel, "Unhandled Opinion.trade channel");
            }
        }
    }

    updates
}

/// Route a raw message to the correct provider parser.
fn parse_provider_message(provider_id: &str, raw: &str) -> Vec<NormalizedUpdate> {
    match provider_id {
        "kalshi.com" => parse_kalshi_message(raw),
        "polymarket.com" => parse_polymarket_message(raw),
        "opinion.trade" => parse_opinion_message(raw),
        _ => {
            warn!(provider = %provider_id, "Unknown provider for message parsing");
            vec![]
        }
    }
}

// ─── Live Feed Manager ───────────────────────────────────────

/// Manages persistent WebSocket connections to all prediction market providers.
/// Handles auto-reconnect with exponential backoff, heartbeats, and message
/// normalization. Publishes normalized updates to the fan-out WebSocketManager.
pub struct LiveFeedManager {
    ws_manager: Arc<WebSocketManager>,
    connections: DashMap<String, Arc<RwLock<ProviderConnectionState>>>,
    configs: DashMap<String, FeedConfig>,
    shutdown_tx: broadcast::Sender<()>,

    // Global metrics
    messages_received_total: AtomicU64,
    reconnects_total: AtomicU64,
    parse_errors_total: AtomicU64,
}

/// Internal mutable state for a single provider connection.
struct ProviderConnectionState {
    state: ConnectionState,
    connected_since: Option<chrono::DateTime<Utc>>,
    last_message_at: Option<chrono::DateTime<Utc>>,
    messages_received: u64,
    reconnect_count: u64,
    subscribed_markets: Vec<String>,
    current_backoff_secs: u64,
}

impl LiveFeedManager {
    pub fn new(ws_manager: Arc<WebSocketManager>) -> Arc<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);
        Arc::new(Self {
            ws_manager,
            connections: DashMap::new(),
            configs: DashMap::new(),
            shutdown_tx,
            messages_received_total: AtomicU64::new(0),
            reconnects_total: AtomicU64::new(0),
            parse_errors_total: AtomicU64::new(0),
        })
    }

    /// Register a provider feed configuration.
    pub fn register_feed(&self, config: FeedConfig) {
        let provider_id = config.provider_id.clone();
        let state = ProviderConnectionState {
            state: ConnectionState::Disconnected,
            connected_since: None,
            last_message_at: None,
            messages_received: 0,
            reconnect_count: 0,
            subscribed_markets: config.auto_subscribe.clone(),
            current_backoff_secs: config.initial_backoff_secs,
        };
        self.connections.insert(provider_id.clone(), Arc::new(RwLock::new(state)));
        self.configs.insert(provider_id, config);
    }

    /// Start all registered provider feeds. Each gets its own reconnecting task.
    pub fn start_all(self: &Arc<Self>) {
        let provider_ids: Vec<String> = self.configs.iter().map(|e| e.key().clone()).collect();
        for pid in provider_ids {
            self.start_feed(&pid);
        }
        info!(providers = self.configs.len(), "Live feed manager started");
    }

    /// Start (or restart) a single provider feed.
    pub fn start_feed(self: &Arc<Self>, provider_id: &str) {
        let config = match self.configs.get(provider_id) {
            Some(c) => c.clone(),
            None => {
                warn!(provider = %provider_id, "No config registered for provider");
                return;
            }
        };

        let manager = Arc::clone(self);
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!(provider = %config.provider_id, url = %config.ws_url, "Starting live feed connection task");

            loop {
                // Check for shutdown
                if shutdown_rx.try_recv().is_ok() {
                    info!(provider = %config.provider_id, "Live feed shutting down");
                    break;
                }

                // Update state to Connecting
                manager.set_state(&config.provider_id, ConnectionState::Connecting).await;

                match connect_async(&config.ws_url).await {
                    Ok((ws_stream, _response)) => {
                        info!(provider = %config.provider_id, "WebSocket connected");
                        manager.set_state(&config.provider_id, ConnectionState::Connected).await;
                        manager.reset_backoff(&config.provider_id).await;

                        // Record connection time
                        if let Some(conn) = manager.connections.get(&config.provider_id) {
                            let mut state = conn.write().await;
                            state.connected_since = Some(Utc::now());
                        }

                        let (mut write, mut read) = ws_stream.split();

                        // Send subscription messages for auto-subscribe markets
                        let sub_msg = manager.build_subscribe_message(&config).await;
                        if let Some(msg) = sub_msg {
                            if let Err(e) = write.send(Message::Text(msg.into())).await {
                                warn!(provider = %config.provider_id, error = %e, "Failed to send subscribe");
                            } else {
                                manager.set_state(&config.provider_id, ConnectionState::Subscribed).await;
                            }
                        }

                        // Setup heartbeat timer
                        let heartbeat_interval = Duration::from_secs(config.heartbeat_interval_secs);
                        let mut heartbeat = tokio::time::interval(heartbeat_interval);
                        heartbeat.tick().await; // skip first immediate tick

                        // Message read loop
                        loop {
                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(Message::Text(text))) => {
                                            manager.handle_message(&config.provider_id, &text).await;
                                        }
                                        Some(Ok(Message::Binary(bin))) => {
                                            if let Ok(text) = String::from_utf8(bin.to_vec()) {
                                                manager.handle_message(&config.provider_id, &text).await;
                                            }
                                        }
                                        Some(Ok(Message::Ping(data))) => {
                                            let _ = write.send(Message::Pong(data)).await;
                                        }
                                        Some(Ok(Message::Pong(_))) => {
                                            debug!(provider = %config.provider_id, "Pong received");
                                        }
                                        Some(Ok(Message::Close(frame))) => {
                                            warn!(provider = %config.provider_id, ?frame, "WebSocket closed by server");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            warn!(provider = %config.provider_id, error = %e, "WebSocket error");
                                            break;
                                        }
                                        None => {
                                            warn!(provider = %config.provider_id, "WebSocket stream ended");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                _ = heartbeat.tick() => {
                                    if let Err(e) = write.send(Message::Ping(vec![].into())).await {
                                        warn!(provider = %config.provider_id, error = %e, "Heartbeat ping failed");
                                        break;
                                    }
                                    debug!(provider = %config.provider_id, "Heartbeat ping sent");
                                }
                                _ = shutdown_rx.recv() => {
                                    info!(provider = %config.provider_id, "Shutdown received during read loop");
                                    let _ = write.send(Message::Close(None)).await;
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(provider = %config.provider_id, error = %e, "WebSocket connection failed");
                    }
                }

                // Connection lost — enter backoff
                manager.set_state(&config.provider_id, ConnectionState::Backoff).await;
                let backoff = manager.get_backoff(&config.provider_id).await;
                manager.increment_reconnect(&config.provider_id).await;

                info!(provider = %config.provider_id, backoff_secs = backoff, "Reconnecting after backoff");

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                    _ = shutdown_rx.recv() => {
                        info!(provider = %config.provider_id, "Shutdown during backoff");
                        return;
                    }
                }

                // Increase backoff for next failure (exponential)
                manager.increase_backoff(&config.provider_id, config.max_backoff_secs).await;
            }

            manager.set_state(&config.provider_id, ConnectionState::Disconnected).await;
        });
    }

    /// Handle an incoming message — parse, normalize, and fan out.
    async fn handle_message(&self, provider_id: &str, raw: &str) {
        self.messages_received_total.fetch_add(1, Ordering::Relaxed);

        // Update per-provider stats
        if let Some(conn) = self.connections.get(provider_id) {
            let mut state = conn.write().await;
            state.messages_received += 1;
            state.last_message_at = Some(Utc::now());
        }

        // Parse and normalize
        let updates = parse_provider_message(provider_id, raw);

        if updates.is_empty() && !raw.is_empty() {
            // Not necessarily an error — could be a heartbeat/ack
            debug!(provider = %provider_id, len = raw.len(), "No updates parsed from message");
        }

        // Fan out normalized updates through the WebSocket manager
        for update in updates {
            let channel = match update.update_type {
                UpdateType::Price => "prices",
                UpdateType::OrderBook => "orderbook",
                UpdateType::Trade => "trades",
                UpdateType::MarketStatus => "status",
            };

            self.ws_manager.publish(channel, &update.market_id, update.data).await;
        }
    }

    /// Build a provider-specific subscription message.
    async fn build_subscribe_message(&self, config: &FeedConfig) -> Option<String> {
        let markets = if let Some(conn) = self.connections.get(&config.provider_id) {
            conn.read().await.subscribed_markets.clone()
        } else {
            config.auto_subscribe.clone()
        };

        if markets.is_empty() { return None; }

        let msg = match config.provider_id.as_str() {
            "kalshi.com" => {
                // Kalshi subscribe format
                serde_json::json!({
                    "id": 1,
                    "cmd": "subscribe",
                    "params": {
                        "channels": ["orderbook_delta", "trade"],
                        "market_tickers": markets,
                    }
                })
            }
            "polymarket.com" => {
                // Polymarket subscribe format
                serde_json::json!({
                    "type": "subscribe",
                    "assets_ids": markets,
                })
            }
            "opinion.trade" => {
                // Opinion.trade subscribe format
                serde_json::json!({
                    "action": "subscribe",
                    "channels": ["prices", "orderbook", "trades"],
                    "markets": markets,
                })
            }
            _ => return None,
        };

        Some(msg.to_string())
    }

    /// Subscribe to additional markets on an active connection.
    pub async fn subscribe_markets(&self, provider_id: &str, market_ids: Vec<String>) {
        if let Some(conn) = self.connections.get(provider_id) {
            let mut state = conn.write().await;
            for mid in &market_ids {
                if !state.subscribed_markets.contains(mid) {
                    state.subscribed_markets.push(mid.clone());
                }
            }
        }
        // Note: To actually send the subscribe message on the live connection,
        // the connection task would need a channel to receive commands.
        // For now, new subscriptions take effect on the next reconnect.
        debug!(provider = %provider_id, markets = ?market_ids, "Markets queued for subscription");
    }

    /// Get the status of all provider connections.
    pub async fn status(&self) -> Vec<ProviderConnection> {
        let mut result = Vec::new();
        for entry in self.connections.iter() {
            let provider_id = entry.key().clone();
            let state = entry.value().read().await;
            let config = self.configs.get(&provider_id);

            result.push(ProviderConnection {
                provider_id: provider_id.clone(),
                ws_url: config.map(|c| c.ws_url.clone()).unwrap_or_default(),
                state: state.state,
                connected_since: state.connected_since.map(|t| t.to_rfc3339()),
                last_message_at: state.last_message_at.map(|t| t.to_rfc3339()),
                messages_received: state.messages_received,
                reconnect_count: state.reconnect_count,
                subscribed_markets: state.subscribed_markets.clone(),
            });
        }
        result
    }

    /// Get global feed statistics.
    pub fn stats(&self) -> LiveFeedStats {
        LiveFeedStats {
            providers_registered: self.configs.len(),
            messages_received_total: self.messages_received_total.load(Ordering::Relaxed),
            reconnects_total: self.reconnects_total.load(Ordering::Relaxed),
            parse_errors_total: self.parse_errors_total.load(Ordering::Relaxed),
        }
    }

    /// Signal all connection tasks to shut down gracefully.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
        info!("Live feed manager shutdown signaled");
    }

    // ── Internal helpers ────────────────────────────────────

    async fn set_state(&self, provider_id: &str, new_state: ConnectionState) {
        if let Some(conn) = self.connections.get(provider_id) {
            conn.write().await.state = new_state;
        }
    }

    async fn get_backoff(&self, provider_id: &str) -> u64 {
        match self.connections.get(provider_id) {
            Some(conn) => conn.value().read().await.current_backoff_secs,
            None => 1,
        }
    }

    async fn increase_backoff(&self, provider_id: &str, max: u64) {
        if let Some(conn) = self.connections.get(provider_id) {
            let mut state = conn.write().await;
            state.current_backoff_secs = (state.current_backoff_secs * 2).min(max);
        }
    }

    async fn reset_backoff(&self, provider_id: &str) {
        if let Some(conn) = self.connections.get(provider_id) {
            let config_initial = self.configs.get(provider_id)
                .map(|c| c.initial_backoff_secs)
                .unwrap_or(1);
            conn.write().await.current_backoff_secs = config_initial;
        }
    }

    async fn increment_reconnect(&self, provider_id: &str) {
        self.reconnects_total.fetch_add(1, Ordering::Relaxed);
        if let Some(conn) = self.connections.get(provider_id) {
            conn.write().await.reconnect_count += 1;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveFeedStats {
    pub providers_registered: usize,
    pub messages_received_total: u64,
    pub reconnects_total: u64,
    pub parse_errors_total: u64,
}

// ─── Convenience ─────────────────────────────────────────────

/// Create and start a LiveFeedManager with default provider configs.
pub fn start_live_feeds(ws_manager: Arc<WebSocketManager>) -> Arc<LiveFeedManager> {
    let manager = LiveFeedManager::new(ws_manager);
    manager.register_feed(FeedConfig::kalshi());
    manager.register_feed(FeedConfig::polymarket());
    manager.register_feed(FeedConfig::opinion());
    manager.start_all();
    manager
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kalshi_orderbook() {
        let msg = r#"{"type":"orderbook_snapshot","msg":{"market_ticker":"TRUMP-WIN","yes":[[65,100],[63,200]],"no":[[35,100]]}}"#;
        let updates = parse_kalshi_message(msg);
        assert!(updates.len() >= 1);
        assert_eq!(updates[0].provider_id, "kalshi.com");
        assert_eq!(updates[0].market_id, "upp:kalshi.com:TRUMP-WIN");
        assert_eq!(updates[0].update_type, UpdateType::OrderBook);
    }

    #[test]
    fn test_parse_kalshi_trade() {
        let msg = r#"{"type":"trade","msg":{"market_ticker":"FED-CUT-50","price":42,"count":5}}"#;
        let updates = parse_kalshi_message(msg);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].update_type, UpdateType::Trade);
        assert!(updates[0].market_id.contains("FED-CUT-50"));
    }

    #[test]
    fn test_parse_polymarket_price() {
        let msg = r#"{"event_type":"price_change","asset_id":"0xabc123","price":"0.72"}"#;
        let updates = parse_polymarket_message(msg);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].provider_id, "polymarket.com");
        assert_eq!(updates[0].update_type, UpdateType::Price);
        assert_eq!(updates[0].market_id, "upp:polymarket.com:0xabc123");
    }

    #[test]
    fn test_parse_polymarket_array() {
        let msg = r#"[{"event_type":"price_change","asset_id":"0x1","price":"0.50"},{"event_type":"trade","asset_id":"0x2"}]"#;
        let updates = parse_polymarket_message(msg);
        assert_eq!(updates.len(), 2);
    }

    #[test]
    fn test_parse_opinion_price() {
        let msg = r#"{"channel":"prices","market":"rain-nyc","data":{"yes_price":0.35}}"#;
        let updates = parse_opinion_message(msg);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].provider_id, "opinion.trade");
        assert_eq!(updates[0].market_id, "upp:opinion.trade:rain-nyc");
        assert_eq!(updates[0].update_type, UpdateType::Price);
    }

    #[test]
    fn test_parse_opinion_orderbook() {
        let msg = r#"{"channel":"orderbook","market":"btc-100k","data":{"bids":[],"asks":[]}}"#;
        let updates = parse_opinion_message(msg);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].update_type, UpdateType::OrderBook);
    }

    #[test]
    fn test_parse_unknown_provider() {
        let updates = parse_provider_message("unknown.com", "{}");
        assert!(updates.is_empty());
    }

    #[test]
    fn test_parse_invalid_json() {
        let updates = parse_kalshi_message("not json at all");
        assert!(updates.is_empty());
    }

    #[test]
    fn test_parse_empty_market() {
        let msg = r#"{"channel":"prices","market":"","data":{}}"#;
        let updates = parse_opinion_message(msg);
        assert!(updates.is_empty());
    }

    #[test]
    fn test_connection_state_display() {
        assert_eq!(ConnectionState::Connected.to_string(), "connected");
        assert_eq!(ConnectionState::Backoff.to_string(), "backoff");
        assert_eq!(ConnectionState::Subscribed.to_string(), "subscribed");
    }

    #[test]
    fn test_feed_config_defaults() {
        let kalshi = FeedConfig::kalshi();
        assert_eq!(kalshi.provider_id, "kalshi.com");
        assert!(kalshi.ws_url.contains("kalshi"));
        assert_eq!(kalshi.heartbeat_interval_secs, 30);
        assert_eq!(kalshi.max_backoff_secs, 60);

        let poly = FeedConfig::polymarket();
        assert_eq!(poly.provider_id, "polymarket.com");

        let opinion = FeedConfig::opinion();
        assert_eq!(opinion.provider_id, "opinion.trade");
    }

    #[test]
    fn test_parse_kalshi_unknown_type() {
        let msg = r#"{"type":"heartbeat","msg":{}}"#;
        let updates = parse_kalshi_message(msg);
        assert!(updates.is_empty());
    }
}
