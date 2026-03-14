// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// WebSocket fan-out manager — handles real-time price and orderbook
// subscriptions across multiple providers.
//
// Architecture:
//   - Each connected client gets a Sender channel
//   - Subscriptions are tracked per-market per-channel (prices, orderbook)
//   - Background tasks poll providers and fan out updates to subscribers
//   - Clients send JSON-RPC messages to subscribe/unsubscribe

use crate::core::config::GatewayConfig;
use crate::core::registry::ProviderRegistry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn, debug};

/// Maximum number of buffered messages per broadcast channel.
const BROADCAST_CAPACITY: usize = 256;

/// A fan-out message sent to WebSocket clients.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FanOutMessage {
    pub channel: String,          // "prices" or "orderbook"
    pub market_id: String,        // UPP universal market ID
    pub data: serde_json::Value,  // Channel-specific payload
    pub timestamp: String,
}

/// Tracks which markets a client is subscribed to, per channel.
#[allow(dead_code)]
#[derive(Debug, Default)]
struct ClientSubscriptions {
    /// market_ids subscribed to price updates
    prices: HashSet<String>,
    /// market_ids subscribed to orderbook updates
    orderbook: HashSet<String>,
}

/// The WebSocket fan-out manager.
///
/// Maintains a broadcast channel for each (channel, market_id) pair.
/// Clients subscribe by market ID and receive updates via broadcast receivers.
pub struct WebSocketManager {
    registry: Arc<ProviderRegistry>,
    _config: Arc<GatewayConfig>,

    /// Broadcast senders keyed by "channel:market_id"
    /// e.g. "prices:upp:kalshi.com:TRUMP-WIN" or "orderbook:upp:polymarket.com:0xabc..."
    channels: RwLock<HashMap<String, broadcast::Sender<FanOutMessage>>>,

    /// Count of active subscribers per channel key (for cleanup)
    subscriber_counts: RwLock<HashMap<String, usize>>,
}

impl WebSocketManager {
    pub fn new(registry: Arc<ProviderRegistry>, config: Arc<GatewayConfig>) -> Self {
        Self {
            registry,
            _config: config,
            channels: RwLock::new(HashMap::new()),
            subscriber_counts: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a broadcast channel for a given channel+market pair.
    /// Returns a Receiver that the client can listen on.
    pub async fn subscribe(
        &self,
        channel: &str,
        market_id: &str,
    ) -> broadcast::Receiver<FanOutMessage> {
        let key = format!("{}:{}", channel, market_id);

        // Check if channel already exists
        {
            let channels = self.channels.read().await;
            if let Some(sender) = channels.get(&key) {
                let rx = sender.subscribe();
                // Increment subscriber count
                let mut counts = self.subscriber_counts.write().await;
                *counts.entry(key.clone()).or_insert(0) += 1;
                debug!(channel = %channel, market_id = %market_id, "Client subscribed (existing channel)");
                return rx;
            }
        }

        // Create new channel
        let (tx, rx) = broadcast::channel(BROADCAST_CAPACITY);

        {
            let mut channels = self.channels.write().await;
            channels.insert(key.clone(), tx);
        }
        {
            let mut counts = self.subscriber_counts.write().await;
            counts.insert(key.clone(), 1);
        }

        info!(channel = %channel, market_id = %market_id, "Created new broadcast channel");
        rx
    }

    /// Remove a subscriber from a channel. Cleans up the channel if no subscribers remain.
    pub async fn unsubscribe(&self, channel: &str, market_id: &str) {
        let key = format!("{}:{}", channel, market_id);

        let should_remove = {
            let mut counts = self.subscriber_counts.write().await;
            if let Some(count) = counts.get_mut(&key) {
                *count = count.saturating_sub(1);
                *count == 0
            } else {
                false
            }
        };

        if should_remove {
            let mut channels = self.channels.write().await;
            channels.remove(&key);
            let mut counts = self.subscriber_counts.write().await;
            counts.remove(&key);
            debug!(channel = %channel, market_id = %market_id, "Removed empty broadcast channel");
        }
    }

    /// Publish a message to all subscribers of a channel+market pair.
    pub async fn publish(&self, channel: &str, market_id: &str, data: serde_json::Value) {
        let key = format!("{}:{}", channel, market_id);

        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(&key) {
            let msg = FanOutMessage {
                channel: channel.to_string(),
                market_id: market_id.to_string(),
                data,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            match sender.send(msg) {
                Ok(n) => {
                    debug!(channel = %channel, market_id = %market_id, receivers = n, "Published update");
                }
                Err(_) => {
                    // No active receivers — channel will be cleaned up on next unsubscribe
                    debug!(channel = %channel, market_id = %market_id, "No active receivers");
                }
            }
        }
    }

    /// Get the number of active broadcast channels.
    pub async fn active_channels(&self) -> usize {
        self.channels.read().await.len()
    }

    /// Get the total number of subscribers across all channels.
    pub async fn total_subscribers(&self) -> usize {
        self.subscriber_counts.read().await.values().sum()
    }

    /// Get a snapshot of the latest published prices across all channels.
    /// Returns map of market_id -> map of outcome_id -> price_string.
    /// Used by the price indexer to ingest current prices.
    pub async fn get_price_snapshot(&self) -> HashMap<String, HashMap<String, String>> {
        let keys: Vec<String> = {
            let channels = self.channels.read().await;
            channels.keys()
                .filter(|k| k.starts_with("prices:"))
                .cloned()
                .collect()
        };

        let mut snapshot = HashMap::new();
        for key in keys {
            let market_id = key.strip_prefix("prices:").unwrap_or(&key).to_string();
            let (provider_id, native_id) = parse_market_id(&market_id);

            if let Some(adapter) = self.registry.get(&provider_id) {
                if let Ok(market) = adapter.get_market(&native_id).await {
                    snapshot.insert(market_id, market.pricing.last_price.clone());
                }
            }
        }

        snapshot
    }

    /// Start a background price polling task for subscribed markets.
    /// This polls providers at a fixed interval and fans out price updates.
    pub fn start_price_poller(self: &Arc<Self>, interval_ms: u64) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(self);
        let registry = Arc::clone(&manager.registry);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(interval_ms),
            );

            loop {
                interval.tick().await;

                // Get all active price subscription keys
                let keys: Vec<String> = {
                    let channels = manager.channels.read().await;
                    channels.keys()
                        .filter(|k| k.starts_with("prices:"))
                        .cloned()
                        .collect()
                };

                if keys.is_empty() {
                    continue;
                }

                // For each subscribed market, fetch latest price
                for key in &keys {
                    let market_id = key.strip_prefix("prices:").unwrap_or(key);

                    // Parse provider and native_id from UPP market ID
                    let (provider_id, native_id) = parse_market_id(market_id);

                    if let Some(adapter) = registry.get(&provider_id) {
                        match adapter.get_market(&native_id).await {
                            Ok(market) => {
                                let price_data = serde_json::json!({
                                    "market_id": market_id,
                                    "prices": market.pricing.last_price,
                                    "bid": market.pricing.best_bid,
                                    "ask": market.pricing.best_ask,
                                    "spread": market.pricing.spread,
                                });
                                manager.publish("prices", market_id, price_data).await;
                            }
                            Err(e) => {
                                warn!(market_id = %market_id, error = %e, "Price poll failed");
                            }
                        }
                    }
                }
            }
        })
    }

    /// Start a background orderbook polling task for subscribed markets.
    /// This polls providers at a fixed interval and fans out orderbook snapshots.
    pub fn start_orderbook_poller(self: &Arc<Self>, interval_ms: u64) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(self);
        let registry = Arc::clone(&manager.registry);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(interval_ms),
            );

            loop {
                interval.tick().await;

                // Get all active orderbook subscription keys
                let keys: Vec<String> = {
                    let channels = manager.channels.read().await;
                    channels.keys()
                        .filter(|k| k.starts_with("orderbook:"))
                        .cloned()
                        .collect()
                };

                if keys.is_empty() {
                    continue;
                }

                // For each subscribed market, fetch latest orderbook
                for key in &keys {
                    let market_id = key.strip_prefix("orderbook:").unwrap_or(key);

                    // Parse provider and native_id from UPP market ID
                    let (provider_id, native_id) = parse_market_id(market_id);

                    if let Some(adapter) = registry.get(&provider_id) {
                        match adapter.get_orderbook(&native_id, None, 10).await {
                            Ok(snapshots) => {
                                let orderbook_data = serde_json::json!({
                                    "market_id": market_id,
                                    "snapshots": snapshots,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                });
                                manager.publish("orderbook", market_id, orderbook_data).await;
                            }
                            Err(e) => {
                                warn!(market_id = %market_id, error = %e, "Orderbook poll failed");
                            }
                        }
                    }
                }
            }
        })
    }
}

/// Parse a UPP market ID into (provider_id, native_id).
fn parse_market_id(id: &str) -> (String, String) {
    let id = id.strip_prefix("upp:").unwrap_or(id);
    if let Some(colon_pos) = id.find(':') {
        (id[..colon_pos].to_string(), id[colon_pos + 1..].to_string())
    } else {
        ("kalshi.com".to_string(), id.to_string())
    }
}
