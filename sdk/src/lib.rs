//! # UPP SDK - Rust Client for UPP Gateway
//!
//! A production-grade, typed Rust client library for the UPP (Unified Price Platform) Gateway REST API.
//!
//! ## Features
//!
//! - **Fully Typed**: All API responses and requests are strongly typed
//! - **Async/Await**: Built on tokio for high-performance async operations
//! - **Builder Pattern**: Flexible client configuration via builder pattern
//! - **Comprehensive**: Covers all UPP Gateway endpoints
//! - **Error Handling**: Rich error types with thiserror
//! - **Well Documented**: Extensive doc comments and examples
//!
//! ## Quick Start
//!
//! ```no_run
//! use upp_sdk::UppClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client
//!     let client = UppClient::new("http://localhost:9090")?;
//!
//!     // Check health
//!     let health = client.health().await?;
//!     println!("Health: {:?}", health);
//!
//!     // List markets
//!     let markets = client.list_markets(None, None, None, None, None).await?;
//!     println!("Markets: {:?}", markets.markets);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Authenticated Operations
//!
//! For authenticated operations (orders, portfolio, etc.), set an API key:
//!
//! ```no_run
//! use upp_sdk::UppClient;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = UppClient::builder()
//!         .base_url("http://localhost:9090")
//!         .api_key("your-api-key")
//!         .timeout(Duration::from_secs(30))
//!         .build()?;
//!
//!     // Create an order
//!     let order_req = upp_sdk::CreateOrderRequest {
//!         market_id: "market-1".to_string(),
//!         outcome_id: "outcome-1".to_string(),
//!         side: upp_sdk::OrderSide::Buy,
//!         quantity: 10.0,
//!         price: 0.5,
//!         order_type: upp_sdk::OrderType::Limit,
//!     };
//!
//!     let order = client.create_order(order_req).await?;
//!     println!("Order created: {:?}", order);
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(unused_results)]

pub mod client;
pub mod error;
pub mod types;

// Re-export commonly used types and client
pub use client::{UppClient, UppClientBuilder};
pub use error::{Result, UppSdkError};
pub use types::*;

#[cfg(test)]
mod tests {
    use crate::types::*;

    #[test]
    fn test_market_serialize() {
        let market = Market {
            id: "market-1".to_string(),
            title: "Bitcoin Price".to_string(),
            description: Some("BTC/USD price market".to_string()),
            provider: "polymarket".to_string(),
            status: "active".to_string(),
            category: Some("crypto".to_string()),
            outcomes: vec![
                MarketOutcome {
                    id: "yes".to_string(),
                    title: "Yes".to_string(),
                    price: Some(0.6),
                },
                MarketOutcome {
                    id: "no".to_string(),
                    title: "No".to_string(),
                    price: Some(0.4),
                },
            ],
            volume: Some(1000000.0),
            volume_24h: Some(100000.0),
            created_at: Some("2026-03-14T00:00:00Z".to_string()),
            closes_at: Some("2026-04-14T00:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&market).expect("Should serialize");
        assert!(json.contains("market-1"));
        assert!(json.contains("Bitcoin Price"));
    }

    #[test]
    fn test_market_deserialize() {
        let json = r#"{
            "id": "market-1",
            "title": "Bitcoin Price",
            "description": "BTC/USD price market",
            "provider": "polymarket",
            "status": "active",
            "category": "crypto",
            "outcomes": [
                {"id": "yes", "title": "Yes", "price": 0.6},
                {"id": "no", "title": "No", "price": 0.4}
            ],
            "volume": 1000000.0,
            "volume_24h": 100000.0,
            "created_at": "2026-03-14T00:00:00Z",
            "closes_at": "2026-04-14T00:00:00Z"
        }"#;

        let market: Market = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(market.id, "market-1");
        assert_eq!(market.title, "Bitcoin Price");
        assert_eq!(market.outcomes.len(), 2);
    }

    #[test]
    fn test_order_side_serialize() {
        let buy = OrderSide::Buy;
        let json = serde_json::to_string(&buy).expect("Should serialize");
        assert_eq!(json, "\"BUY\"");

        let sell = OrderSide::Sell;
        let json = serde_json::to_string(&sell).expect("Should serialize");
        assert_eq!(json, "\"SELL\"");
    }

    #[test]
    fn test_order_side_deserialize() {
        let buy: OrderSide = serde_json::from_str("\"BUY\"").expect("Should deserialize");
        assert_eq!(buy, OrderSide::Buy);

        let sell: OrderSide = serde_json::from_str("\"SELL\"").expect("Should deserialize");
        assert_eq!(sell, OrderSide::Sell);
    }

    #[test]
    fn test_order_type_serialize() {
        let limit = OrderType::Limit;
        let json = serde_json::to_string(&limit).expect("Should serialize");
        assert_eq!(json, "\"LIMIT\"");

        let market = OrderType::Market;
        let json = serde_json::to_string(&market).expect("Should serialize");
        assert_eq!(json, "\"MARKET\"");
    }

    #[test]
    fn test_order_type_deserialize() {
        let limit: OrderType = serde_json::from_str("\"LIMIT\"").expect("Should deserialize");
        assert_eq!(limit, OrderType::Limit);

        let market: OrderType = serde_json::from_str("\"MARKET\"").expect("Should deserialize");
        assert_eq!(market, OrderType::Market);
    }

    #[test]
    fn test_candle_serialize() {
        let candle = Candle {
            timestamp: "2026-03-14T10:00:00Z".to_string(),
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: 1000.0,
        };

        let json = serde_json::to_string(&candle).expect("Should serialize");
        assert!(json.contains("100.0"));
        assert!(json.contains("105.0"));
    }

    #[test]
    fn test_portfolio_summary_serialize() {
        let summary = PortfolioSummaryResponse {
            total_balance: 10000.0,
            available_balance: 5000.0,
            total_positions_value: 4000.0,
            total_unrealized_pnl: 200.0,
            total_realized_pnl: 500.0,
        };

        let json = serde_json::to_string(&summary).expect("Should serialize");
        assert!(json.contains("10000"));
        assert!(json.contains("5000"));
    }

    #[test]
    fn test_position_serialize() {
        let position = Position {
            market_id: "market-1".to_string(),
            outcome_id: "yes".to_string(),
            quantity: 100.0,
            average_entry_price: 0.5,
            current_price: 0.6,
            unrealized_pnl: 10.0,
        };

        let json = serde_json::to_string(&position).expect("Should serialize");
        assert!(json.contains("market-1"));
        assert!(json.contains("100"));
    }
}
