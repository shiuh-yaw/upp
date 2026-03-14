//! WebSocket client for real-time UPP Gateway feeds
//!
//! Provides a typed, async WebSocket client for subscribing to real-time market data
//! feeds from the UPP Gateway. Supports automatic reconnection with exponential backoff,
//! ping/pong keepalive, and channel-based message dispatch.
//!
//! # Example
//!
//! ```no_run
//! use upp_sdk::ws::{UppWebSocket, WsMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create and connect a WebSocket client
//!     let mut ws = UppWebSocket::builder()
//!         .url("ws://localhost:9090/upp/v1/ws")
//!         .auto_reconnect(true)
//!         .build()
//!         .await?;
//!
//!     // Subscribe to price updates for specific markets
//!     ws.subscribe(&["prices"], Some(&["market-1", "market-2"])).await?;
//!
//!     // Receive messages
//!     while let Some(msg) = ws.next_message().await? {
//!         match msg {
//!             WsMessage::Price { market_id, yes_price, no_price } => {
//!                 println!("Price update for {}: YES={}, NO={}", market_id, yes_price, no_price);
//!             }
//!             WsMessage::OrderBook { market_id, bids, asks } => {
//!                 println!("Orderbook update for {}", market_id);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use crate::error::{Result, UppSdkError};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::Message as WsInnerMessage;
use tokio_tungstenite::{connect_async, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;

/// Subscription message sent to the WebSocket server
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscribeMessage {
    /// Action type (e.g., "subscribe")
    action: String,
    /// Channels to subscribe to (e.g., ["prices", "orderbook", "trades"])
    channels: Vec<String>,
    /// Optional market IDs to filter subscriptions
    #[serde(skip_serializing_if = "Option::is_none")]
    market_ids: Option<Vec<String>>,
}

/// Unsubscription message sent to the WebSocket server
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UnsubscribeMessage {
    /// Action type (e.g., "unsubscribe")
    action: String,
    /// Channels to unsubscribe from
    channels: Vec<String>,
    /// Optional market IDs to filter unsubscriptions
    #[serde(skip_serializing_if = "Option::is_none")]
    market_ids: Option<Vec<String>>,
}

/// Status message sent from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusMessage {
    /// Market identifier
    market_id: String,
    /// Status string (e.g., "open", "closed")
    status: String,
}

/// Price update message received from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceUpdate {
    /// Market identifier
    pub market_id: String,
    /// Price of YES outcome
    pub yes_price: f64,
    /// Price of NO outcome
    pub no_price: f64,
}

/// Orderbook level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookLevel {
    /// Price at this level
    pub price: f64,
    /// Size at this level
    pub size: f64,
}

/// Orderbook update message received from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderbookUpdate {
    /// Market identifier
    pub market_id: String,
    /// Bid levels
    pub bids: Vec<OrderbookLevel>,
    /// Ask levels
    pub asks: Vec<OrderbookLevel>,
}

/// Trade execution message received from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    /// Trade identifier
    pub id: String,
    /// Execution price
    pub price: f64,
    /// Execution quantity
    pub quantity: f64,
    /// Trade side (BUY or SELL)
    pub side: String,
}

/// Trade update message received from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TradeUpdate {
    /// Market identifier
    pub market_id: String,
    /// Trade data
    pub data: TradeData,
}

/// Raw server message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerMessage {
    /// Message type (e.g., "price", "orderbook", "trade", "status")
    #[serde(rename = "type")]
    msg_type: String,
    /// Message payload as JSON value
    data: serde_json::Value,
}

/// WebSocket message received from the server
#[derive(Clone)]
pub enum WsMessage {
    /// Price update for a market
    Price {
        /// Market identifier
        market_id: String,
        /// Price of YES outcome (0.0 to 1.0)
        yes_price: f64,
        /// Price of NO outcome (0.0 to 1.0)
        no_price: f64,
    },
    /// Orderbook snapshot for a market
    OrderBook {
        /// Market identifier
        market_id: String,
        /// Bid levels sorted by price descending
        bids: Vec<OrderbookLevel>,
        /// Ask levels sorted by price ascending
        asks: Vec<OrderbookLevel>,
    },
    /// Trade execution
    Trade {
        /// Market identifier
        market_id: String,
        /// Trade data
        data: TradeData,
    },
    /// Status update for a market
    Status {
        /// Market identifier
        market_id: String,
        /// New status
        status: String,
    },
    /// Raw message (unparsed or unrecognized)
    Raw(String),
}

impl fmt::Debug for WsMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Price { market_id, yes_price, no_price } => f
                .debug_struct("Price")
                .field("market_id", market_id)
                .field("yes_price", yes_price)
                .field("no_price", no_price)
                .finish(),
            Self::OrderBook { market_id, bids, asks } => f
                .debug_struct("OrderBook")
                .field("market_id", market_id)
                .field("bids_count", &bids.len())
                .field("asks_count", &asks.len())
                .finish(),
            Self::Trade { market_id, data } => f
                .debug_struct("Trade")
                .field("market_id", market_id)
                .field("data", data)
                .finish(),
            Self::Status { market_id, status } => f
                .debug_struct("Status")
                .field("market_id", market_id)
                .field("status", status)
                .finish(),
            Self::Raw(s) => f.debug_tuple("Raw").field(s).finish(),
        }
    }
}

/// WebSocket client configuration
#[derive(Debug, Clone)]
pub struct WsConfig {
    /// WebSocket endpoint URL
    pub url: String,
    /// Enable automatic reconnection on disconnect
    pub auto_reconnect: bool,
    /// Initial backoff duration for reconnection attempts
    pub initial_backoff_ms: u64,
    /// Maximum backoff duration for reconnection attempts
    pub max_backoff_ms: u64,
    /// Ping interval to keep connection alive
    pub ping_interval_ms: u64,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:9090/upp/v1/ws".to_string(),
            auto_reconnect: true,
            initial_backoff_ms: 100,
            max_backoff_ms: 30000,
            ping_interval_ms: 30000,
        }
    }
}

/// Builder for constructing a UppWebSocket with custom configuration
#[derive(Debug)]
pub struct UppWebSocketBuilder {
    config: WsConfig,
}

impl Default for UppWebSocketBuilder {
    fn default() -> Self {
        Self {
            config: WsConfig::default(),
        }
    }
}

impl UppWebSocketBuilder {
    /// Create a new builder with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the WebSocket endpoint URL
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.config.url = url.into();
        self
    }

    /// Enable or disable automatic reconnection
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.config.auto_reconnect = enabled;
        self
    }

    /// Set the initial backoff duration in milliseconds
    pub fn initial_backoff_ms(mut self, ms: u64) -> Self {
        self.config.initial_backoff_ms = ms;
        self
    }

    /// Set the maximum backoff duration in milliseconds
    pub fn max_backoff_ms(mut self, ms: u64) -> Self {
        self.config.max_backoff_ms = ms;
        self
    }

    /// Set the ping interval in milliseconds
    pub fn ping_interval_ms(mut self, ms: u64) -> Self {
        self.config.ping_interval_ms = ms;
        self
    }

    /// Build the WebSocket client and establish connection
    pub async fn build(self) -> Result<UppWebSocket> {
        UppWebSocket::new(self.config).await
    }
}

type WsSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// WebSocket client for receiving real-time UPP Gateway feeds
#[derive(Debug)]
pub struct UppWebSocket {
    /// Client configuration
    config: WsConfig,
    /// Active WebSocket connection
    socket: Option<WsSocket>,
    /// Broadcast channel for distributing messages
    tx: broadcast::Sender<WsMessage>,
    /// Current reconnection backoff duration
    current_backoff_ms: u64,
}

impl UppWebSocket {
    /// Create a new builder for constructing a WebSocket client
    pub fn builder() -> UppWebSocketBuilder {
        UppWebSocketBuilder::new()
    }

    /// Create and connect a new WebSocket client with the given configuration
    async fn new(config: WsConfig) -> Result<Self> {
        let (tx, _rx) = broadcast::channel(256);

        let initial_backoff = config.initial_backoff_ms;
        let mut ws = Self {
            config,
            socket: None,
            tx,
            current_backoff_ms: initial_backoff,
        };

        ws.connect_internal().await?;
        Ok(ws)
    }

    /// Establish connection to the WebSocket endpoint
    async fn connect_internal(&mut self) -> Result<()> {
        loop {
            match connect_async(&self.config.url).await {
                Ok((socket, _response)) => {
                    self.socket = Some(socket);
                    self.current_backoff_ms = self.config.initial_backoff_ms;
                    return Ok(());
                }
                Err(e) => {
                    if !self.config.auto_reconnect {
                        return Err(UppSdkError::ConfigError(format!(
                            "Failed to connect to WebSocket: {}",
                            e
                        )));
                    }

                    eprintln!(
                        "WebSocket connection failed: {}. Retrying in {}ms...",
                        e, self.current_backoff_ms
                    );
                    sleep(Duration::from_millis(self.current_backoff_ms)).await;

                    self.current_backoff_ms = (self.current_backoff_ms * 2)
                        .min(self.config.max_backoff_ms);
                }
            }
        }
    }

    /// Subscribe to one or more channels
    ///
    /// # Arguments
    ///
    /// * `channels` - Channel names to subscribe to (e.g., ["prices", "orderbook", "trades"])
    /// * `market_ids` - Optional list of market IDs to filter by. If None, subscribes to all markets.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use upp_sdk::ws::UppWebSocket;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ws = UppWebSocket::builder().build().await?;
    /// ws.subscribe(&["prices"], Some(&["market-1", "market-2"])).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn subscribe(&mut self, channels: &[&str], market_ids: Option<&[&str]>) -> Result<()> {
        let msg = SubscribeMessage {
            action: "subscribe".to_string(),
            channels: channels.iter().map(|s| s.to_string()).collect(),
            market_ids: market_ids.map(|ids| ids.iter().map(|s| s.to_string()).collect()),
        };

        let json = serde_json::to_string(&msg)?;
        self.send_raw(json).await?;
        Ok(())
    }

    /// Unsubscribe from one or more channels
    ///
    /// # Arguments
    ///
    /// * `channels` - Channel names to unsubscribe from
    /// * `market_ids` - Optional list of market IDs to filter by. If None, unsubscribes from all markets.
    pub async fn unsubscribe(&mut self, channels: &[&str], market_ids: Option<&[&str]>) -> Result<()> {
        let msg = UnsubscribeMessage {
            action: "unsubscribe".to_string(),
            channels: channels.iter().map(|s| s.to_string()).collect(),
            market_ids: market_ids.map(|ids| ids.iter().map(|s| s.to_string()).collect()),
        };

        let json = serde_json::to_string(&msg)?;
        self.send_raw(json).await?;
        Ok(())
    }

    /// Receive the next message from the WebSocket
    ///
    /// Returns `Ok(None)` if the connection is closed cleanly.
    /// Returns `Ok(Some(msg))` if a message was received and successfully parsed.
    /// Returns `Err(...)` if a connection or parsing error occurred.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use upp_sdk::ws::UppWebSocket;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut ws = UppWebSocket::builder().build().await?;
    /// loop {
    ///     match ws.next_message().await {
    ///         Ok(Some(msg)) => println!("Received: {:?}", msg),
    ///         Ok(None) => break,
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next_message(&mut self) -> Result<Option<WsMessage>> {
        loop {
            if self.socket.is_none() && self.config.auto_reconnect {
                self.connect_internal().await?;
            }

            match self.socket.as_mut() {
                Some(socket) => {
                    match socket.next().await {
                        Some(Ok(WsInnerMessage::Text(text))) => {
                            match self.parse_message(&text) {
                                Ok(msg) => {
                                    // Broadcast the message (ignore if no receivers)
                                    let _ = self.tx.send(msg.clone());
                                    return Ok(Some(msg));
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse message: {}. Raw: {}", e, text);
                                    return Ok(Some(WsMessage::Raw(text)));
                                }
                            }
                        }
                        Some(Ok(WsInnerMessage::Binary(_))) => {
                            continue;
                        }
                        Some(Ok(WsInnerMessage::Ping(ping))) => {
                            if let Some(socket) = self.socket.as_mut() {
                                let _ = socket.send(WsInnerMessage::Pong(ping)).await;
                            }
                            continue;
                        }
                        Some(Ok(WsInnerMessage::Pong(_))) => {
                            continue;
                        }
                        Some(Ok(WsInnerMessage::Close(_))) => {
                            self.socket = None;
                            if !self.config.auto_reconnect {
                                return Ok(None);
                            }
                            continue;
                        }
                        Some(Ok(_)) => {
                            continue;
                        }
                        Some(Err(e)) => {
                            self.socket = None;
                            if !self.config.auto_reconnect {
                                return Err(UppSdkError::ConfigError(format!(
                                    "WebSocket error: {}",
                                    e
                                )));
                            }
                            sleep(Duration::from_millis(self.current_backoff_ms)).await;
                            self.current_backoff_ms = (self.current_backoff_ms * 2)
                                .min(self.config.max_backoff_ms);
                            continue;
                        }
                        None => {
                            self.socket = None;
                            if !self.config.auto_reconnect {
                                return Ok(None);
                            }
                            continue;
                        }
                    }
                }
                None => {
                    return Err(UppSdkError::ConfigError(
                        "WebSocket not connected".to_string(),
                    ))
                }
            }
        }
    }

    /// Close the WebSocket connection
    pub async fn close(&mut self) -> Result<()> {
        if let Some(socket) = self.socket.take() {
            let mut socket = socket;
            socket
                .close(None)
                .await
                .map_err(|e| UppSdkError::ConfigError(format!("Close failed: {}", e)))?;
        }
        Ok(())
    }

    /// Subscribe to a broadcast channel for receiving parsed messages
    ///
    /// This allows multiple concurrent consumers to receive the same messages.
    pub fn subscribe_to_broadcast(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }

    // Internal methods

    async fn send_raw(&mut self, json: String) -> Result<()> {
        if let Some(socket) = self.socket.as_mut() {
            socket
                .send(WsInnerMessage::Text(json))
                .await
                .map_err(|e| UppSdkError::ConfigError(format!("Send failed: {}", e)))?;
            Ok(())
        } else {
            Err(UppSdkError::ConfigError(
                "WebSocket not connected".to_string(),
            ))
        }
    }

    fn parse_message(&self, text: &str) -> Result<WsMessage> {
        let server_msg: ServerMessage = serde_json::from_str(text)?;

        match server_msg.msg_type.as_str() {
            "price" => {
                let update: PriceUpdate = serde_json::from_value(server_msg.data)?;
                Ok(WsMessage::Price {
                    market_id: update.market_id,
                    yes_price: update.yes_price,
                    no_price: update.no_price,
                })
            }
            "orderbook" => {
                let update: OrderbookUpdate = serde_json::from_value(server_msg.data)?;
                Ok(WsMessage::OrderBook {
                    market_id: update.market_id,
                    bids: update.bids,
                    asks: update.asks,
                })
            }
            "trade" => {
                let update: TradeUpdate = serde_json::from_value(server_msg.data)?;
                Ok(WsMessage::Trade {
                    market_id: update.market_id,
                    data: update.data,
                })
            }
            "status" => {
                let status: StatusMessage = serde_json::from_value(server_msg.data)?;
                Ok(WsMessage::Status {
                    market_id: status.market_id,
                    status: status.status,
                })
            }
            _ => Err(UppSdkError::UnexpectedResponse(format!(
                "Unknown message type: {}",
                server_msg.msg_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_config_defaults() {
        let config = WsConfig::default();
        assert_eq!(config.url, "ws://localhost:9090/upp/v1/ws");
        assert!(config.auto_reconnect);
        assert_eq!(config.initial_backoff_ms, 100);
        assert_eq!(config.max_backoff_ms, 30000);
    }

    #[test]
    fn test_ws_builder_defaults() {
        let builder = UppWebSocketBuilder::new();
        assert_eq!(builder.config.url, "ws://localhost:9090/upp/v1/ws");
        assert!(builder.config.auto_reconnect);
    }

    #[test]
    fn test_ws_builder_custom() {
        let builder = UppWebSocketBuilder::new()
            .url("ws://example.com/ws")
            .auto_reconnect(false)
            .initial_backoff_ms(200)
            .max_backoff_ms(10000);

        assert_eq!(builder.config.url, "ws://example.com/ws");
        assert!(!builder.config.auto_reconnect);
        assert_eq!(builder.config.initial_backoff_ms, 200);
        assert_eq!(builder.config.max_backoff_ms, 10000);
    }

    #[test]
    fn test_subscribe_message_serialization() {
        let msg = SubscribeMessage {
            action: "subscribe".to_string(),
            channels: vec!["prices".to_string()],
            market_ids: Some(vec!["market-1".to_string()]),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("prices"));
        assert!(json.contains("market-1"));
    }

    #[test]
    fn test_unsubscribe_message_serialization() {
        let msg = UnsubscribeMessage {
            action: "unsubscribe".to_string(),
            channels: vec!["prices".to_string()],
            market_ids: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("unsubscribe"));
        assert!(json.contains("prices"));
    }

    #[test]
    fn test_ws_message_debug_price() {
        let msg = WsMessage::Price {
            market_id: "market-1".to_string(),
            yes_price: 0.6,
            no_price: 0.4,
        };

        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("Price"));
        assert!(debug_str.contains("market-1"));
    }

    #[test]
    fn test_ws_message_debug_orderbook() {
        let msg = WsMessage::OrderBook {
            market_id: "market-1".to_string(),
            bids: vec![OrderbookLevel {
                price: 0.5,
                size: 100.0,
            }],
            asks: vec![OrderbookLevel {
                price: 0.51,
                size: 200.0,
            }],
        };

        let debug_str = format!("{:?}", msg);
        assert!(debug_str.contains("OrderBook"));
        assert!(debug_str.contains("market-1"));
    }
}
