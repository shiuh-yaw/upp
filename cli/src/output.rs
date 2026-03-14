use colored::Colorize;
use serde_json::Value;
use tabled::{builder::Builder, settings::Style};

pub fn print_json(value: &Value) {
    match serde_json::to_string_pretty(value) {
        Ok(json_str) => println!("{}", json_str),
        Err(e) => eprintln!("Error formatting JSON: {}", e),
    }
}

pub fn print_success(msg: &str) {
    println!("{}", msg.green().bold());
}

#[allow(dead_code)]
pub fn print_error(msg: &str) {
    eprintln!("{}", msg.red().bold());
}

#[allow(dead_code)]
pub fn print_info(msg: &str) {
    println!("{}", msg.cyan());
}

pub fn print_warning(msg: &str) {
    println!("{}", msg.yellow().bold());
}

pub fn print_header(title: &str) {
    println!("\n{}", title.blue().bold());
    println!("{}", "─".repeat(title.len()).blue());
}

pub fn print_kv_table(data: &[(String, String)]) {
    let mut builder = Builder::default();
    for (key, value) in data {
        builder.push_record([key.clone(), value.clone()]);
    }

    let table = builder
        .build()
        .with(Style::psql())
        .to_string();

    println!("{}", table);
}

pub fn print_table(headers: Vec<&str>, rows: Vec<Vec<String>>) {
    let mut builder = Builder::default();
    builder.push_record(headers);

    for row in rows {
        builder.push_record(row);
    }

    let table = builder
        .build()
        .with(Style::psql())
        .to_string();

    println!("{}", table);
}

pub fn format_status(status: &str) -> String {
    match status {
        "open" => status.green().to_string(),
        "closed" => status.red().to_string(),
        "pending" => status.yellow().to_string(),
        "filled" => status.green().to_string(),
        "cancelled" => status.red().to_string(),
        _ => status.to_string(),
    }
}

pub fn format_side(side: &str) -> String {
    match side {
        "buy" => side.green().to_string(),
        "sell" => side.red().to_string(),
        _ => side.to_string(),
    }
}

pub fn format_number(num: f64, decimals: usize) -> String {
    format!("{:.prec$}", num, prec = decimals)
}

pub fn format_currency(amount: f64) -> String {
    format!("${:.2}", amount)
}

pub fn format_percentage(pct: f64) -> String {
    if pct >= 0.0 {
        format!("{:.2}%", pct).green().to_string()
    } else {
        format!("{:.2}%", pct).red().to_string()
    }
}

pub struct HealthStatus {
    pub status: String,
    pub uptime: Option<String>,
    pub version: Option<String>,
}

pub fn print_health(health: &HealthStatus) {
    let mut data = vec![
        ("Status".to_string(), format_status(&health.status)),
    ];

    if let Some(uptime) = &health.uptime {
        data.push(("Uptime".to_string(), uptime.clone()));
    }

    if let Some(version) = &health.version {
        data.push(("Version".to_string(), version.clone()));
    }

    print_header("Gateway Health");
    print_kv_table(&data);
}

pub struct MarketSummary {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub status: String,
    pub price: f64,
    pub outcome: Option<String>,
}

pub fn print_markets(markets: &[MarketSummary]) {
    if markets.is_empty() {
        print_warning("No markets found");
        return;
    }

    print_header("Markets");

    let headers = vec!["ID", "Name", "Provider", "Status", "Price", "Outcome"];
    let rows: Vec<Vec<String>> = markets
        .iter()
        .map(|m| {
            vec![
                m.id.clone(),
                m.name.clone(),
                m.provider.clone(),
                format_status(&m.status),
                format_currency(m.price),
                m.outcome.clone().unwrap_or_default(),
            ]
        })
        .collect();

    print_table(headers, rows);
}

pub struct OrderSummary {
    pub id: String,
    pub market_id: String,
    pub side: String,
    pub status: String,
    pub price: f64,
    pub quantity: f64,
    pub filled: f64,
}

pub fn print_orders(orders: &[OrderSummary]) {
    if orders.is_empty() {
        print_warning("No orders found");
        return;
    }

    print_header("Orders");

    let headers = vec!["ID", "Market", "Side", "Status", "Price", "Qty", "Filled"];
    let rows: Vec<Vec<String>> = orders
        .iter()
        .map(|o| {
            vec![
                o.id.clone(),
                o.market_id.clone(),
                format_side(&o.side),
                format_status(&o.status),
                format_currency(o.price),
                format_number(o.quantity, 2),
                format_number(o.filled, 2),
            ]
        })
        .collect();

    print_table(headers, rows);
}

pub struct TradeSummary {
    pub id: String,
    pub order_id: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub timestamp: String,
}

pub fn print_trades(trades: &[TradeSummary]) {
    if trades.is_empty() {
        print_warning("No trades found");
        return;
    }

    print_header("Trades");

    let headers = vec!["ID", "Order ID", "Side", "Price", "Quantity", "Time"];
    let rows: Vec<Vec<String>> = trades
        .iter()
        .map(|t| {
            vec![
                t.id.clone(),
                t.order_id.clone(),
                format_side(&t.side),
                format_currency(t.price),
                format_number(t.quantity, 2),
                t.timestamp.clone(),
            ]
        })
        .collect();

    print_table(headers, rows);
}

pub struct PositionSummary {
    pub market_id: String,
    pub outcome: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub value: f64,
}

pub fn print_positions(positions: &[PositionSummary]) {
    if positions.is_empty() {
        print_warning("No positions found");
        return;
    }

    print_header("Positions");

    let headers = vec!["Market", "Outcome", "Quantity", "Avg Price", "Value"];
    let rows: Vec<Vec<String>> = positions
        .iter()
        .map(|p| {
            vec![
                p.market_id.clone(),
                p.outcome.clone(),
                format_number(p.quantity, 2),
                format_currency(p.avg_price),
                format_currency(p.value),
            ]
        })
        .collect();

    print_table(headers, rows);
}

pub struct PortfolioSummary {
    pub total_value: f64,
    pub cash: f64,
    pub invested: f64,
    pub pnl: f64,
    pub pnl_percentage: f64,
}

pub fn print_portfolio_summary(portfolio: &PortfolioSummary) {
    print_header("Portfolio Summary");

    let data = vec![
        ("Total Value".to_string(), format_currency(portfolio.total_value)),
        ("Cash".to_string(), format_currency(portfolio.cash)),
        ("Invested".to_string(), format_currency(portfolio.invested)),
        ("P&L".to_string(), format_currency(portfolio.pnl)),
        ("P&L %".to_string(), format_percentage(portfolio.pnl_percentage)),
    ];

    print_kv_table(&data);
}

pub struct BalanceSummary {
    pub symbol: String,
    pub available: f64,
    pub reserved: f64,
    pub total: f64,
}

pub fn print_balances(balances: &[BalanceSummary]) {
    if balances.is_empty() {
        print_warning("No balances found");
        return;
    }

    print_header("Account Balances");

    let headers = vec!["Symbol", "Available", "Reserved", "Total"];
    let rows: Vec<Vec<String>> = balances
        .iter()
        .map(|b| {
            vec![
                b.symbol.clone(),
                format_currency(b.available),
                format_currency(b.reserved),
                format_currency(b.total),
            ]
        })
        .collect();

    print_table(headers, rows);
}

pub struct ArbitrageSummary {
    pub id: String,
    pub market_id: String,
    pub potential_profit: f64,
    pub profit_percentage: f64,
    pub status: String,
}

pub fn print_arbitrage(opportunities: &[ArbitrageSummary]) {
    if opportunities.is_empty() {
        print_warning("No arbitrage opportunities found");
        return;
    }

    print_header("Arbitrage Opportunities");

    let headers = vec!["ID", "Market", "Profit", "Profit %", "Status"];
    let rows: Vec<Vec<String>> = opportunities
        .iter()
        .map(|a| {
            vec![
                a.id.clone(),
                a.market_id.clone(),
                format_currency(a.potential_profit),
                format_percentage(a.profit_percentage),
                a.status.clone(),
            ]
        })
        .collect();

    print_table(headers, rows);
}

pub struct CandleSummary {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: String,
}

pub fn print_candles(candles: &[CandleSummary]) {
    if candles.is_empty() {
        print_warning("No candles found");
        return;
    }

    print_header("Candles");

    let headers = vec!["Time", "Open", "High", "Low", "Close", "Volume"];
    let rows: Vec<Vec<String>> = candles
        .iter()
        .map(|c| {
            vec![
                c.timestamp.clone(),
                format_currency(c.open),
                format_currency(c.high),
                format_currency(c.low),
                format_currency(c.close),
                format_number(c.volume, 0),
            ]
        })
        .collect();

    print_table(headers, rows);
}
