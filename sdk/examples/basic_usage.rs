//! Basic usage example for the UPP SDK
//!
//! Run with: cargo run --example basic_usage

use upp_sdk::{UppClient, OrderSide, OrderType, CreateOrderRequest};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("UPP SDK Basic Usage Example\n");

    // Create a client
    let client = UppClient::builder()
        .base_url("http://localhost:9090")
        .timeout(Duration::from_secs(30))
        .build()?;

    // Check health
    println!("1. Checking gateway health...");
    match client.health().await {
        Ok(health) => println!("   Health: {:?}\n", health),
        Err(e) => println!("   Error: {}\n", e),
    }

    // Check readiness
    println!("2. Checking gateway readiness...");
    match client.ready().await {
        Ok(ready) => println!("   Ready: {:?}\n", ready),
        Err(e) => println!("   Error: {}\n", e),
    }

    // List markets
    println!("3. Listing markets...");
    match client.list_markets(None, None, None, Some(10), None).await {
        Ok(response) => {
            println!("   Found {} markets", response.markets.len());
            for market in response.markets.iter().take(3) {
                println!("     - {} ({})", market.title, market.id);
            }
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // Search markets
    println!("4. Searching markets for 'Bitcoin'...");
    match client.search_markets(Some("Bitcoin"), None, None, Some(5)).await {
        Ok(response) => {
            println!("   Found {} results", response.total);
            for result in response.results.iter().take(3) {
                println!("     - {} ({})", result.title, result.id);
            }
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // List arbitrage opportunities
    println!("5. Listing arbitrage opportunities...");
    match client.list_arbitrage().await {
        Ok(response) => {
            println!("   Found {} opportunities", response.opportunities.len());
            for opp in response.opportunities.iter().take(3) {
                println!("     - Market {} ({}% profit)", opp.market_id, opp.profit_percentage);
            }
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // Get arbitrage summary
    println!("6. Getting arbitrage summary...");
    match client.arbitrage_summary().await {
        Ok(response) => {
            println!("   Total opportunities: {}", response.total_opportunities);
            println!("   24h profit: ${}", response.total_profit_24h);
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // Get price index stats
    println!("7. Getting price index statistics...");
    match client.price_index_stats().await {
        Ok(response) => {
            println!("   Price: ${}", response.price);
            println!("   24h change: {:.2}%", response.change_percent_24h);
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // Get feed status
    println!("8. Getting feed status...");
    match client.feed_status().await {
        Ok(response) => {
            println!("   Found {} feeds", response.feeds.len());
            for feed in response.feeds.iter().take(3) {
                println!("     - {} ({})", feed.name, feed.status);
            }
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // Get feed stats
    println!("9. Getting feed statistics...");
    match client.feed_stats().await {
        Ok(response) => {
            println!("   Total feeds: {}", response.total_feeds);
            println!("   Active feeds: {}", response.active_feeds);
            println!("   Uptime: {:.2}%", response.uptime_percent);
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // List available backtest strategies
    println!("10. Listing backtest strategies...");
    match client.list_strategies().await {
        Ok(response) => {
            println!("   Found {} strategies", response.strategies.len());
            for strategy in response.strategies.iter().take(3) {
                println!("     - {} ({})", strategy.name, strategy.id);
            }
            println!();
        }
        Err(e) => println!("   Error: {}\n", e),
    }

    // Example: Create an order (requires authentication)
    println!("11. Example order creation (requires API key)...");
    let client_with_key = UppClient::builder()
        .base_url("http://localhost:9090")
        .api_key("your-api-key-here")
        .timeout(Duration::from_secs(30))
        .build()?;

    let order_request = CreateOrderRequest {
        market_id: "market-example-1".to_string(),
        outcome_id: "yes".to_string(),
        side: OrderSide::Buy,
        quantity: 10.0,
        price: 0.5,
        order_type: OrderType::Limit,
    };

    println!("   Would send order: {:?}", order_request);
    match client_with_key.create_order(order_request).await {
        Ok(order) => println!("   Order created: {}", order.order.id),
        Err(e) => println!("   Error (expected if gateway unavailable): {}", e),
    }

    println!("\nExample completed!");
    Ok(())
}
