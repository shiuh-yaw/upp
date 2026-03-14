mod config;
mod output;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use config::Config;
use output::*;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "upp")]
#[command(about = "UPP Gateway CLI - Interact with the UPP prediction market gateway", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(global = true, long, help = "Gateway URL override")]
    url: Option<String>,

    #[arg(global = true, long, help = "API key override")]
    #[arg(value_name = "KEY")]
    api_key: Option<String>,

    #[arg(global = true, long, help = "Output as JSON")]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Check gateway health
    Health,

    /// Manage markets
    #[command(subcommand)]
    Markets(MarketCommands),

    /// Manage orders
    #[command(subcommand)]
    Orders(OrderCommands),

    /// View trades
    #[command(subcommand)]
    Trades(TradeCommands),

    /// Manage portfolio
    #[command(subcommand)]
    Portfolio(PortfolioCommands),

    /// Arbitrage operations
    #[command(subcommand)]
    Arbitrage(ArbitrageCommands),

    /// Candle data
    Candles {
        /// Market ID
        market_id: String,

        /// Outcome to filter by
        #[arg(long)]
        outcome: Option<String>,

        /// Resolution (1m, 5m, 1h, 1d)
        #[arg(long, default_value = "1h")]
        resolution: String,

        /// Limit number of candles
        #[arg(long, default_value = "100")]
        limit: u32,
    },

    /// Backtest strategies
    #[command(subcommand)]
    Backtest(BacktestCommands),

    /// Feed management
    #[command(subcommand)]
    Feeds(FeedCommands),

    /// Route computation and execution
    #[command(subcommand)]
    Route(RouteCommands),

    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
enum MarketCommands {
    /// List all markets
    List {
        /// Filter by provider
        #[arg(long)]
        provider: Option<String>,

        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Limit results
        #[arg(long, default_value = "20")]
        limit: u32,
    },

    /// Get market details
    Get {
        /// Market ID
        market_id: String,
    },

    /// Search markets
    Search {
        /// Search query
        query: String,

        /// Limit results
        #[arg(long, default_value = "20")]
        limit: u32,
    },
}

#[derive(Subcommand)]
enum OrderCommands {
    /// List orders
    List {
        /// Filter by provider
        #[arg(long)]
        provider: Option<String>,

        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Limit results
        #[arg(long, default_value = "20")]
        limit: u32,
    },

    /// Create order
    Create {
        /// Market ID
        #[arg(long)]
        market: String,

        /// Side (buy or sell)
        #[arg(long)]
        side: String,

        /// Price
        #[arg(long)]
        price: f64,

        /// Quantity
        #[arg(long)]
        quantity: f64,
    },

    /// Get order details
    Get {
        /// Order ID
        order_id: String,
    },

    /// Cancel order
    Cancel {
        /// Order ID
        order_id: String,
    },

    /// Cancel all orders
    CancelAll,
}

#[derive(Subcommand)]
enum TradeCommands {
    /// List trades
    List {
        /// Limit results
        #[arg(long, default_value = "20")]
        limit: u32,
    },
}

#[derive(Subcommand)]
enum PortfolioCommands {
    /// List positions
    Positions,

    /// Portfolio summary
    Summary,

    /// Full portfolio analytics
    Analytics,

    /// Account balances
    Balances,
}

#[derive(Subcommand)]
enum ArbitrageCommands {
    /// List arbitrage opportunities
    List,

    /// Arbitrage summary
    Summary,
}

#[derive(Subcommand)]
enum BacktestCommands {
    /// Run backtest
    Run {
        /// Strategy name
        #[arg(long)]
        strategy: String,

        /// Market ID
        #[arg(long)]
        market: String,

        /// Strategy parameters (key=val,key=val)
        #[arg(long)]
        params: Option<String>,
    },

    /// List available strategies
    Strategies,

    /// Compare strategies
    Compare {
        /// Market ID
        #[arg(long)]
        market: String,

        /// Comma-separated strategy names
        #[arg(long)]
        strategies: String,
    },
}

#[derive(Subcommand)]
enum FeedCommands {
    /// Feed connection status
    Status,

    /// Feed statistics
    Stats,
}

#[derive(Subcommand)]
enum RouteCommands {
    /// Compute route
    Compute {
        /// Market ID
        #[arg(long)]
        market: String,

        /// Side (buy or sell)
        #[arg(long)]
        side: String,

        /// Quantity
        #[arg(long)]
        quantity: f64,
    },

    /// Execute route
    Execute {
        /// Route JSON
        route_json: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Set gateway URL
    SetUrl {
        /// Gateway URL
        url: String,
    },

    /// Set API key
    SetKey {
        /// API key
        key: String,
    },

    /// Show current config
    Show,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut config = Config::load()?;
    config = config
        .with_url(cli.url.clone())
        .with_api_key(cli.api_key.clone());

    match cli.command {
        Commands::Health => cmd_health(&config, cli.json).await?,

        Commands::Markets(cmd) => match cmd {
            MarketCommands::List {
                provider,
                status,
                limit,
            } => cmd_markets_list(&config, provider, status, limit, cli.json).await?,
            MarketCommands::Get { market_id } => {
                cmd_markets_get(&config, &market_id, cli.json).await?
            }
            MarketCommands::Search { query, limit } => {
                cmd_markets_search(&config, &query, limit, cli.json).await?
            }
        },

        Commands::Orders(cmd) => match cmd {
            OrderCommands::List {
                provider,
                status,
                limit,
            } => cmd_orders_list(&config, provider, status, limit, cli.json).await?,
            OrderCommands::Create {
                market,
                side,
                price,
                quantity,
            } => cmd_orders_create(&config, &market, &side, price, quantity, cli.json).await?,
            OrderCommands::Get { order_id } => {
                cmd_orders_get(&config, &order_id, cli.json).await?
            }
            OrderCommands::Cancel { order_id } => {
                cmd_orders_cancel(&config, &order_id, cli.json).await?
            }
            OrderCommands::CancelAll => cmd_orders_cancel_all(&config, cli.json).await?,
        },

        Commands::Trades(cmd) => match cmd {
            TradeCommands::List { limit } => cmd_trades_list(&config, limit, cli.json).await?,
        },

        Commands::Portfolio(cmd) => match cmd {
            PortfolioCommands::Positions => cmd_portfolio_positions(&config, cli.json).await?,
            PortfolioCommands::Summary => cmd_portfolio_summary(&config, cli.json).await?,
            PortfolioCommands::Analytics => cmd_portfolio_analytics(&config, cli.json).await?,
            PortfolioCommands::Balances => cmd_portfolio_balances(&config, cli.json).await?,
        },

        Commands::Arbitrage(cmd) => match cmd {
            ArbitrageCommands::List => cmd_arbitrage_list(&config, cli.json).await?,
            ArbitrageCommands::Summary => cmd_arbitrage_summary(&config, cli.json).await?,
        },

        Commands::Candles {
            market_id,
            outcome,
            resolution,
            limit,
        } => cmd_candles(&config, &market_id, outcome, &resolution, limit, cli.json).await?,

        Commands::Backtest(cmd) => match cmd {
            BacktestCommands::Run {
                strategy,
                market,
                params,
            } => cmd_backtest_run(&config, &strategy, &market, params, cli.json).await?,
            BacktestCommands::Strategies => cmd_backtest_strategies(&config, cli.json).await?,
            BacktestCommands::Compare {
                market,
                strategies,
            } => cmd_backtest_compare(&config, &market, &strategies, cli.json).await?,
        },

        Commands::Feeds(cmd) => match cmd {
            FeedCommands::Status => cmd_feeds_status(&config, cli.json).await?,
            FeedCommands::Stats => cmd_feeds_stats(&config, cli.json).await?,
        },

        Commands::Route(cmd) => match cmd {
            RouteCommands::Compute {
                market,
                side,
                quantity,
            } => cmd_route_compute(&config, &market, &side, quantity, cli.json).await?,
            RouteCommands::Execute { route_json } => {
                cmd_route_execute(&config, &route_json, cli.json).await?
            }
        },

        Commands::Config(cmd) => match cmd {
            ConfigCommands::SetUrl { url } => {
                config.gateway_url = url;
                config.save()?;
                print_success("Gateway URL updated");
                println!("URL: {}", config.gateway_url);
            }
            ConfigCommands::SetKey { key } => {
                config.api_key = Some(key);
                config.save()?;
                print_success("API key updated");
            }
            ConfigCommands::Show => {
                print_header("Current Configuration");
                let data = vec![
                    ("Gateway URL".to_string(), config.gateway_url.clone()),
                    (
                        "API Key".to_string(),
                        config
                            .api_key
                            .as_ref()
                            .map(|k| format!("{}***", &k[..k.len().min(4)]))
                            .unwrap_or_else(|| "Not set".to_string()),
                    ),
                ];
                print_kv_table(&data);
            }
        },
    }

    Ok(())
}

async fn cmd_health(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/health", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to connect to gateway: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let status = body["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let uptime = body["uptime"].as_str().map(|s| s.to_string());
        let version = body["version"].as_str().map(|s| s.to_string());

        print_health(&HealthStatus {
            status,
            uptime,
            version,
        });
    }

    Ok(())
}

async fn cmd_markets_list(
    config: &Config,
    provider: Option<String>,
    status: Option<String>,
    limit: u32,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let mut url = format!("{}/markets?limit={}", config.gateway_url(), limit);

    if let Some(provider) = provider {
        url.push_str(&format!("&provider={}", provider));
    }
    if let Some(status) = status {
        url.push_str(&format!("&status={}", status));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch markets: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let markets: Vec<MarketSummary> = body["markets"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|m| MarketSummary {
                id: m["id"].as_str().unwrap_or("").to_string(),
                name: m["name"].as_str().unwrap_or("").to_string(),
                provider: m["provider"].as_str().unwrap_or("").to_string(),
                status: m["status"].as_str().unwrap_or("").to_string(),
                price: m["price"].as_f64().unwrap_or(0.0),
                outcome: m["outcome"].as_str().map(|s| s.to_string()),
            })
            .collect();

        print_markets(&markets);
    }

    Ok(())
}

async fn cmd_markets_get(config: &Config, market_id: &str, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/markets/{}", config.gateway_url(), market_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch market: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Market not found or gateway error"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header(&format!("Market: {}", market_id));

        let data = vec![
            ("ID".to_string(), body["id"].as_str().unwrap_or("").to_string()),
            (
                "Name".to_string(),
                body["name"].as_str().unwrap_or("").to_string(),
            ),
            (
                "Provider".to_string(),
                body["provider"].as_str().unwrap_or("").to_string(),
            ),
            (
                "Status".to_string(),
                format_status(body["status"].as_str().unwrap_or("")),
            ),
            (
                "Price".to_string(),
                format_currency(body["price"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Volume".to_string(),
                format_number(body["volume"].as_f64().unwrap_or(0.0), 2),
            ),
            (
                "Created".to_string(),
                body["created_at"].as_str().unwrap_or("").to_string(),
            ),
        ];

        print_kv_table(&data);
    }

    Ok(())
}

async fn cmd_markets_search(
    config: &Config,
    query: &str,
    limit: u32,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let url = format!(
        "{}/markets/search?q={}&limit={}",
        config.gateway_url(),
        urlencoding::encode(query),
        limit
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to search markets: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Search failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let markets: Vec<MarketSummary> = body["markets"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|m| MarketSummary {
                id: m["id"].as_str().unwrap_or("").to_string(),
                name: m["name"].as_str().unwrap_or("").to_string(),
                provider: m["provider"].as_str().unwrap_or("").to_string(),
                status: m["status"].as_str().unwrap_or("").to_string(),
                price: m["price"].as_f64().unwrap_or(0.0),
                outcome: m["outcome"].as_str().map(|s| s.to_string()),
            })
            .collect();

        print_markets(&markets);
    }

    Ok(())
}

async fn cmd_orders_list(
    config: &Config,
    provider: Option<String>,
    status: Option<String>,
    limit: u32,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let mut url = format!("{}/orders?limit={}", config.gateway_url(), limit);

    if let Some(provider) = provider {
        url.push_str(&format!("&provider={}", provider));
    }
    if let Some(status) = status {
        url.push_str(&format!("&status={}", status));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch orders: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let orders: Vec<OrderSummary> = body["orders"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|o| OrderSummary {
                id: o["id"].as_str().unwrap_or("").to_string(),
                market_id: o["market_id"].as_str().unwrap_or("").to_string(),
                side: o["side"].as_str().unwrap_or("").to_string(),
                status: o["status"].as_str().unwrap_or("").to_string(),
                price: o["price"].as_f64().unwrap_or(0.0),
                quantity: o["quantity"].as_f64().unwrap_or(0.0),
                filled: o["filled"].as_f64().unwrap_or(0.0),
            })
            .collect();

        print_orders(&orders);
    }

    Ok(())
}

async fn cmd_orders_create(
    config: &Config,
    market: &str,
    side: &str,
    price: f64,
    quantity: f64,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/orders", config.gateway_url());

    let payload = json!({
        "market_id": market,
        "side": side,
        "price": price,
        "quantity": quantity,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to create order: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Order creation failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_success(&format!("Order created: {}", body["order_id"].as_str().unwrap_or("")));

        let data = vec![
            (
                "Order ID".to_string(),
                body["order_id"].as_str().unwrap_or("").to_string(),
            ),
            (
                "Status".to_string(),
                format_status(body["status"].as_str().unwrap_or("")),
            ),
            ("Market".to_string(), market.to_string()),
            (
                "Side".to_string(),
                format_side(side),
            ),
            ("Price".to_string(), format_currency(price)),
            ("Quantity".to_string(), format_number(quantity, 2)),
        ];

        print_kv_table(&data);
    }

    Ok(())
}

async fn cmd_orders_get(config: &Config, order_id: &str, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/orders/{}", config.gateway_url(), order_id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch order: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Order not found"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header(&format!("Order: {}", order_id));

        let data = vec![
            ("ID".to_string(), body["id"].as_str().unwrap_or("").to_string()),
            (
                "Market".to_string(),
                body["market_id"].as_str().unwrap_or("").to_string(),
            ),
            (
                "Side".to_string(),
                format_side(body["side"].as_str().unwrap_or("")),
            ),
            (
                "Status".to_string(),
                format_status(body["status"].as_str().unwrap_or("")),
            ),
            (
                "Price".to_string(),
                format_currency(body["price"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Quantity".to_string(),
                format_number(body["quantity"].as_f64().unwrap_or(0.0), 2),
            ),
            (
                "Filled".to_string(),
                format_number(body["filled"].as_f64().unwrap_or(0.0), 2),
            ),
            (
                "Created".to_string(),
                body["created_at"].as_str().unwrap_or("").to_string(),
            ),
        ];

        print_kv_table(&data);
    }

    Ok(())
}

async fn cmd_orders_cancel(config: &Config, order_id: &str, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/orders/{}/cancel", config.gateway_url(), order_id);

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to cancel order: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Order cancellation failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_success(&format!("Order cancelled: {}", order_id));
    }

    Ok(())
}

async fn cmd_orders_cancel_all(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/orders/cancel-all", config.gateway_url());

    let response = client
        .post(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to cancel all orders: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Cancel all failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let count = body["cancelled_count"].as_u64().unwrap_or(0);
        print_success(&format!("Cancelled {} orders", count));
    }

    Ok(())
}

async fn cmd_trades_list(config: &Config, limit: u32, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/trades?limit={}", config.gateway_url(), limit);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch trades: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let trades: Vec<TradeSummary> = body["trades"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|t| TradeSummary {
                id: t["id"].as_str().unwrap_or("").to_string(),
                order_id: t["order_id"].as_str().unwrap_or("").to_string(),
                side: t["side"].as_str().unwrap_or("").to_string(),
                price: t["price"].as_f64().unwrap_or(0.0),
                quantity: t["quantity"].as_f64().unwrap_or(0.0),
                timestamp: t["timestamp"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        print_trades(&trades);
    }

    Ok(())
}

async fn cmd_portfolio_positions(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/portfolio/positions", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch positions: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let positions: Vec<PositionSummary> = body["positions"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|p| PositionSummary {
                market_id: p["market_id"].as_str().unwrap_or("").to_string(),
                outcome: p["outcome"].as_str().unwrap_or("").to_string(),
                quantity: p["quantity"].as_f64().unwrap_or(0.0),
                avg_price: p["avg_price"].as_f64().unwrap_or(0.0),
                value: p["value"].as_f64().unwrap_or(0.0),
            })
            .collect();

        print_positions(&positions);
    }

    Ok(())
}

async fn cmd_portfolio_summary(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/portfolio/summary", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch portfolio: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_portfolio_summary(&PortfolioSummary {
            total_value: body["total_value"].as_f64().unwrap_or(0.0),
            cash: body["cash"].as_f64().unwrap_or(0.0),
            invested: body["invested"].as_f64().unwrap_or(0.0),
            pnl: body["pnl"].as_f64().unwrap_or(0.0),
            pnl_percentage: body["pnl_percentage"].as_f64().unwrap_or(0.0),
        });
    }

    Ok(())
}

async fn cmd_portfolio_analytics(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/portfolio/analytics", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch analytics: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Portfolio Analytics");

        let data = vec![
            (
                "Total Value".to_string(),
                format_currency(body["total_value"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Cash".to_string(),
                format_currency(body["cash"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Invested".to_string(),
                format_currency(body["invested"].as_f64().unwrap_or(0.0)),
            ),
            (
                "P&L".to_string(),
                format_currency(body["pnl"].as_f64().unwrap_or(0.0)),
            ),
            (
                "P&L %".to_string(),
                format_percentage(body["pnl_percentage"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Volatility".to_string(),
                format_percentage(body["volatility"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Sharpe Ratio".to_string(),
                format_number(body["sharpe_ratio"].as_f64().unwrap_or(0.0), 2),
            ),
            (
                "Max Drawdown".to_string(),
                format_percentage(body["max_drawdown"].as_f64().unwrap_or(0.0)),
            ),
        ];

        print_kv_table(&data);
    }

    Ok(())
}

async fn cmd_portfolio_balances(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/portfolio/balances", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch balances: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let balances: Vec<BalanceSummary> = body["balances"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|b| BalanceSummary {
                symbol: b["symbol"].as_str().unwrap_or("").to_string(),
                available: b["available"].as_f64().unwrap_or(0.0),
                reserved: b["reserved"].as_f64().unwrap_or(0.0),
                total: b["total"].as_f64().unwrap_or(0.0),
            })
            .collect();

        print_balances(&balances);
    }

    Ok(())
}

async fn cmd_arbitrage_list(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/arbitrage/list", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch arbitrage opportunities: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let opportunities: Vec<ArbitrageSummary> = body["opportunities"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|a| ArbitrageSummary {
                id: a["id"].as_str().unwrap_or("").to_string(),
                market_id: a["market_id"].as_str().unwrap_or("").to_string(),
                potential_profit: a["potential_profit"].as_f64().unwrap_or(0.0),
                profit_percentage: a["profit_percentage"].as_f64().unwrap_or(0.0),
                status: a["status"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        print_arbitrage(&opportunities);
    }

    Ok(())
}

async fn cmd_arbitrage_summary(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/arbitrage/summary", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch arbitrage summary: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Arbitrage Summary");

        let data = vec![
            (
                "Total Opportunities".to_string(),
                body["total_count"]
                    .as_u64()
                    .unwrap_or(0)
                    .to_string(),
            ),
            (
                "Average Profit %".to_string(),
                format_percentage(body["avg_profit_percentage"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Max Profit %".to_string(),
                format_percentage(body["max_profit_percentage"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Active Trades".to_string(),
                body["active_trades"]
                    .as_u64()
                    .unwrap_or(0)
                    .to_string(),
            ),
        ];

        print_kv_table(&data);
    }

    Ok(())
}

async fn cmd_candles(
    config: &Config,
    market_id: &str,
    outcome: Option<String>,
    resolution: &str,
    limit: u32,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let mut url = format!(
        "{}/candles/{}?resolution={}&limit={}",
        config.gateway_url(),
        market_id,
        resolution,
        limit
    );

    if let Some(outcome) = outcome {
        url.push_str(&format!("&outcome={}", outcome));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch candles: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to fetch candles"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        let candles: Vec<CandleSummary> = body["candles"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|c| CandleSummary {
                open: c["open"].as_f64().unwrap_or(0.0),
                high: c["high"].as_f64().unwrap_or(0.0),
                low: c["low"].as_f64().unwrap_or(0.0),
                close: c["close"].as_f64().unwrap_or(0.0),
                volume: c["volume"].as_f64().unwrap_or(0.0),
                timestamp: c["timestamp"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        print_candles(&candles);
    }

    Ok(())
}

async fn cmd_backtest_run(
    config: &Config,
    strategy: &str,
    market: &str,
    params: Option<String>,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/backtest/run", config.gateway_url());

    let mut payload = json!({
        "strategy": strategy,
        "market_id": market,
    });

    if let Some(params) = params {
        let mut param_map: HashMap<String, String> = HashMap::new();
        for pair in params.split(',') {
            if let Some((k, v)) = pair.split_once('=') {
                param_map.insert(k.to_string(), v.to_string());
            }
        }
        payload["params"] = serde_json::to_value(param_map)?;
    }

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to run backtest: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Backtest failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Backtest Results");

        let data = vec![
            ("Strategy".to_string(), strategy.to_string()),
            ("Market".to_string(), market.to_string()),
            (
                "Return".to_string(),
                format_percentage(body["return"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Sharpe Ratio".to_string(),
                format_number(body["sharpe_ratio"].as_f64().unwrap_or(0.0), 2),
            ),
            (
                "Max Drawdown".to_string(),
                format_percentage(body["max_drawdown"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Win Rate".to_string(),
                format_percentage(body["win_rate"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Trades".to_string(),
                body["trades"].as_u64().unwrap_or(0).to_string(),
            ),
        ];

        print_kv_table(&data);
    }

    Ok(())
}

async fn cmd_backtest_strategies(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/backtest/strategies", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch strategies: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Available Strategies");

        let headers = vec!["Name", "Description", "Parameters"];
        let rows: Vec<Vec<String>> = body["strategies"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|s| {
                vec![
                    s["name"].as_str().unwrap_or("").to_string(),
                    s["description"].as_str().unwrap_or("").to_string(),
                    s["parameters"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|p| p.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default(),
                ]
            })
            .collect();

        print_table(headers, rows);
    }

    Ok(())
}

async fn cmd_backtest_compare(
    config: &Config,
    market: &str,
    strategies: &str,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/backtest/compare", config.gateway_url());

    let payload = json!({
        "market_id": market,
        "strategies": strategies.split(',').collect::<Vec<_>>(),
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to compare strategies: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Comparison failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Strategy Comparison");

        let headers = vec!["Strategy", "Return", "Sharpe", "Max DD", "Win Rate", "Trades"];
        let rows: Vec<Vec<String>> = body["results"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|r| {
                vec![
                    r["strategy"].as_str().unwrap_or("").to_string(),
                    format_percentage(r["return"].as_f64().unwrap_or(0.0)),
                    format_number(r["sharpe_ratio"].as_f64().unwrap_or(0.0), 2),
                    format_percentage(r["max_drawdown"].as_f64().unwrap_or(0.0)),
                    format_percentage(r["win_rate"].as_f64().unwrap_or(0.0)),
                    r["trades"].as_u64().unwrap_or(0).to_string(),
                ]
            })
            .collect();

        print_table(headers, rows);
    }

    Ok(())
}

async fn cmd_feeds_status(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/feeds/status", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch feed status: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Feed Status");

        let headers = vec!["Feed", "Status", "Connected", "Last Update"];
        let rows: Vec<Vec<String>> = body["feeds"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|f| {
                vec![
                    f["name"].as_str().unwrap_or("").to_string(),
                    format_status(f["status"].as_str().unwrap_or("")),
                    if f["connected"].as_bool().unwrap_or(false) {
                        "Yes".green().to_string()
                    } else {
                        "No".red().to_string()
                    },
                    f["last_update"].as_str().unwrap_or("").to_string(),
                ]
            })
            .collect();

        print_table(headers, rows);
    }

    Ok(())
}

async fn cmd_feeds_stats(config: &Config, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/feeds/stats", config.gateway_url());

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch feed stats: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Gateway returned status: {}", response.status()));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Feed Statistics");

        let headers = vec!["Feed", "Messages/sec", "Errors", "Latency (ms)", "Uptime"];
        let rows: Vec<Vec<String>> = body["stats"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|s| {
                vec![
                    s["name"].as_str().unwrap_or("").to_string(),
                    format_number(s["messages_per_sec"].as_f64().unwrap_or(0.0), 2),
                    s["errors"].as_u64().unwrap_or(0).to_string(),
                    format_number(s["latency_ms"].as_f64().unwrap_or(0.0), 2),
                    s["uptime"].as_str().unwrap_or("").to_string(),
                ]
            })
            .collect();

        print_table(headers, rows);
    }

    Ok(())
}

async fn cmd_route_compute(
    config: &Config,
    market: &str,
    side: &str,
    quantity: f64,
    json_output: bool,
) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/route/compute", config.gateway_url());

    let payload = json!({
        "market_id": market,
        "side": side,
        "quantity": quantity,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to compute route: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Route computation failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_header("Route Computation");

        let data = vec![
            ("Market".to_string(), market.to_string()),
            (
                "Side".to_string(),
                format_side(side),
            ),
            ("Quantity".to_string(), format_number(quantity, 2)),
            (
                "Best Price".to_string(),
                format_currency(body["best_price"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Total Cost".to_string(),
                format_currency(body["total_cost"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Legs".to_string(),
                body["legs"].as_array().map(|a| a.len()).unwrap_or(0).to_string(),
            ),
        ];

        print_kv_table(&data);

        if let Some(legs) = body["legs"].as_array() {
            if !legs.is_empty() {
                print_header("Route Legs");
                let headers = vec!["Provider", "Quantity", "Price", "Cost"];
                let rows: Vec<Vec<String>> = legs
                    .iter()
                    .map(|leg| {
                        vec![
                            leg["provider"].as_str().unwrap_or("").to_string(),
                            format_number(leg["quantity"].as_f64().unwrap_or(0.0), 2),
                            format_currency(leg["price"].as_f64().unwrap_or(0.0)),
                            format_currency(leg["cost"].as_f64().unwrap_or(0.0)),
                        ]
                    })
                    .collect();

                print_table(headers, rows);
            }
        }
    }

    Ok(())
}

async fn cmd_route_execute(config: &Config, route_json: &str, json_output: bool) -> Result<()> {
    let client = Client::new();
    let url = format!("{}/route/execute", config.gateway_url());

    let route: Value = serde_json::from_str(route_json)
        .map_err(|_| anyhow!("Invalid route JSON"))?;

    let response = client
        .post(&url)
        .json(&route)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to execute route: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Route execution failed"));
    }

    let body = response.json::<Value>().await?;

    if json_output {
        print_json(&body);
    } else {
        print_success("Route executed successfully");

        let data = vec![
            (
                "Execution ID".to_string(),
                body["execution_id"].as_str().unwrap_or("").to_string(),
            ),
            (
                "Status".to_string(),
                format_status(body["status"].as_str().unwrap_or("")),
            ),
            (
                "Total Cost".to_string(),
                format_currency(body["total_cost"].as_f64().unwrap_or(0.0)),
            ),
            (
                "Timestamp".to_string(),
                body["timestamp"].as_str().unwrap_or("").to_string(),
            ),
        ];

        print_kv_table(&data);
    }

    Ok(())
}
