# gRPC API Reference

High-performance service interface using Protocol Buffers. Ideal for service-to-service communication and latency-critical applications.

## Connection

**Address:** `localhost:50051` (default)

**Protocol:** gRPC over HTTP/2

### Rust Example

```rust
use tonic::transport::Channel;
use upp_proto::markets::Markets;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::from_static("http://localhost:50051")
        .connect()
        .await?;

    let mut client = MarketsClient::new(channel);

    // Create request
    let request = tonic::Request::new(GetMarketsRequest {
        provider: "polymarket".to_string(),
        limit: 10,
        ..Default::default()
    });

    // Call RPC
    let response = client.get_markets(request).await?;

    println!("{:#?}", response.get_ref());
    Ok(())
}
```

### Go Example

```go
package main

import (
    "context"
    "log"
    pb "github.com/universal-prediction-protocol/upp/proto"
    "google.golang.org/grpc"
)

func main() {
    conn, err := grpc.Dial("localhost:50051", grpc.WithInsecure())
    if err != nil {
        log.Fatalf("did not connect: %v", err)
    }
    defer conn.Close()

    client := pb.NewMarketsClient(conn)
    resp, err := client.GetMarkets(context.Background(), &pb.GetMarketsRequest{
        Provider: "polymarket",
        Limit:    10,
    })
    if err != nil {
        log.Fatalf("could not get markets: %v", err)
    }

    log.Printf("Markets: %v", resp.Markets)
}
```

## Authentication

Send credentials in gRPC metadata:

```rust
use tonic::metadata::MetadataValue;

let mut request = tonic::Request::new(GetMarketsRequest::default());
let api_key = MetadataValue::from_str("Bearer api_key_here")?;
request.metadata_mut().insert("authorization", api_key);

let response = client.get_markets(request).await?;
```

## Services

### Markets Service

Market data and queries.

#### GetMarkets

Get markets from a provider.

**Request:**

```protobuf
message GetMarketsRequest {
  string provider = 1;      // "polymarket", "kalshi", "opinion_trade"
  string category = 2;      // Optional filter
  uint32 limit = 3;         // Max 100
  uint32 offset = 4;        // For pagination
  string status = 5;        // "active", "resolved", "cancelled"
}
```

**Response:**

```protobuf
message GetMarketsResponse {
  repeated Market markets = 1;
  uint32 total = 2;
  string cursor = 3;
}

message Market {
  string id = 1;
  string provider = 2;
  string title = 3;
  string description = 4;
  string category = 5;
  repeated Outcome outcomes = 6;
  double liquidity = 7;
  double volume_24h = 8;
  google.protobuf.Timestamp created_at = 9;
  google.protobuf.Timestamp expires_at = 10;
  string status = 11;
}

message Outcome {
  string id = 1;
  string name = 2;
  double price = 3;
  double probability = 4;
}
```

**Rust Example:**

```rust
let request = GetMarketsRequest {
    provider: "polymarket".to_string(),
    limit: 10,
    ..Default::default()
};

let response = client.get_markets(request).await?;

for market in response.into_inner().markets {
    println!(
        "{}: {} ({})",
        market.id, market.title, market.provider
    );
    for outcome in market.outcomes {
        println!("  {} @ {}", outcome.name, outcome.price);
    }
}
```

#### GetMarket

Get a specific market by ID.

**Request:**

```protobuf
message GetMarketRequest {
  string market_id = 1;
  string provider = 2;  // Optional, inferred from ID if possible
}
```

**Response:**

```protobuf
message GetMarketResponse {
  Market market = 1;
}
```

#### SearchMarkets

Full-text search across markets.

**Request:**

```protobuf
message SearchMarketsRequest {
  string query = 1;
  string provider = 2;  // Optional
  uint32 limit = 3;
}
```

**Response:**

```protobuf
message SearchMarketsResponse {
  repeated SearchResult results = 1;
  uint32 total = 2;
}

message SearchResult {
  string market_id = 1;
  string provider = 2;
  string title = 3;
  string description = 4;
  float relevance_score = 5;
  repeated Outcome outcomes = 6;
}
```

### Orders Service

Order management.

#### ListOrders

Get user's orders.

**Request:**

```protobuf
message ListOrdersRequest {
  string provider = 1;     // Optional filter
  string status = 2;       // "open", "filled", "cancelled"
  string market_id = 3;    // Optional filter
  uint32 limit = 4;
}
```

**Response:**

```protobuf
message ListOrdersResponse {
  repeated Order orders = 1;
  uint32 total = 2;
}

message Order {
  string id = 1;
  string market_id = 2;
  string provider = 3;
  string side = 4;           // "BUY" or "SELL"
  string outcome = 5;
  double price = 6;
  double quantity = 7;
  double filled = 8;
  double remaining = 9;
  string status = 10;        // "OPEN", "PARTIALLY_FILLED", "FILLED"
  google.protobuf.Timestamp created_at = 11;
  google.protobuf.Timestamp updated_at = 12;
}
```

#### PlaceOrder

Create a new order.

**Request:**

```protobuf
message PlaceOrderRequest {
  string provider = 1;
  string market_id = 2;
  string side = 3;           // "BUY" or "SELL"
  string outcome = 4;
  double price = 5;
  double quantity = 6;
}
```

**Response:**

```protobuf
message PlaceOrderResponse {
  Order order = 1;
}
```

**Rust Example:**

```rust
let request = PlaceOrderRequest {
    provider: "polymarket".to_string(),
    market_id: "0x1234...abcd".to_string(),
    side: "BUY".to_string(),
    outcome: "Yes".to_string(),
    price: 0.72,
    quantity: 100.0,
};

let response = client.place_order(request).await?;
println!("Order placed: {}", response.into_inner().order.id);
```

#### CancelOrder

Cancel an existing order.

**Request:**

```protobuf
message CancelOrderRequest {
  string order_id = 1;
}
```

**Response:**

```protobuf
message CancelOrderResponse {
  bool success = 1;
  string message = 2;
}
```

### Portfolio Service

User positions and balances.

#### GetPortfolio

Get user's portfolio summary.

**Request:**

```protobuf
message GetPortfolioRequest {
  string user_id = 1;  // Optional, defaults to authenticated user
}
```

**Response:**

```protobuf
message GetPortfolioResponse {
  Portfolio portfolio = 1;
}

message Portfolio {
  string user_id = 1;
  double cash_balance = 2;
  double total_value = 3;
  repeated Position positions = 4;
  double total_pnl = 5;
  double total_pnl_percent = 6;
}

message Position {
  string market_id = 1;
  string provider = 2;
  string outcome = 3;
  double quantity = 4;
  double entry_price = 5;
  double current_price = 6;
  double pnl = 7;
  double pnl_percent = 8;
}
```

**Rust Example:**

```rust
let response = client.get_portfolio(GetPortfolioRequest::default()).await?;
let portfolio = response.into_inner().portfolio.unwrap();

println!("Balance: ${}", portfolio.cash_balance);
println!("Total P&L: ${} ({:.1}%)",
         portfolio.total_pnl,
         portfolio.total_pnl_percent * 100.0);

for position in portfolio.positions {
    println!(
        "  {} shares @ {} (P&L: ${}, {:.1}%)",
        position.quantity,
        position.outcome,
        position.pnl,
        position.pnl_percent * 100.0
    );
}
```

#### GetPositions

Get detailed position information with aggregation.

**Request:**

```protobuf
message GetPositionsRequest {
  string provider = 1;        // Optional filter
  string group_by = 2;        // "outcome", "market", "provider"
  uint32 limit = 3;
}
```

**Response:**

```protobuf
message GetPositionsResponse {
  repeated Position positions = 1;
  PositionsSummary summary = 2;
}

message PositionsSummary {
  uint32 total_positions = 1;
  double total_quantity = 2;
  double total_pnl = 3;
  double total_pnl_percent = 4;
}
```

### Arbitrage Service

Cross-exchange price discrepancies.

#### FindOpportunities

Identify arbitrage opportunities.

**Request:**

```protobuf
message FindOpportunitiesRequest {
  double min_spread = 1;  // Default 0.02 (2%)
  uint32 limit = 2;       // Default 20
}
```

**Response:**

```protobuf
message FindOpportunitiesResponse {
  repeated ArbitrageOpportunity opportunities = 1;
}

message ArbitrageOpportunity {
  string market_id = 1;
  string title = 2;
  string outcome = 3;
  ExchangePrice buy_exchange = 4;
  ExchangePrice sell_exchange = 5;
  double spread = 6;
  double spread_percent = 7;
  double max_volume = 8;
  double potential_profit = 9;
}

message ExchangePrice {
  string provider = 1;
  double price = 2;
  double liquidity = 3;
}
```

**Rust Example:**

```rust
let request = FindOpportunitiesRequest {
    min_spread: 0.05,  // 5% minimum spread
    limit: 10,
    ..Default::default()
};

let response = client.find_opportunities(request).await?;

for opp in response.into_inner().opportunities {
    println!(
        "Arbitrage: {} @ {} buy on {} / {} sell on {} ({:.1}% spread)",
        opp.outcome,
        opp.buy_exchange.price,
        opp.buy_exchange.provider,
        opp.sell_exchange.price,
        opp.sell_exchange.provider,
        opp.spread_percent * 100.0
    );
}
```

### Backtest Service

Historical strategy simulation.

#### RunBacktest

Run a backtest simulation.

**Request:**

```protobuf
message RunBacktestRequest {
  string market_id = 1;
  string provider = 2;
  string start_date = 3;
  string end_date = 4;
  double initial_balance = 5;
  repeated BacktestTrade trades = 6;
}

message BacktestTrade {
  string date = 1;
  string side = 2;     // "BUY" or "SELL"
  string outcome = 3;
  double quantity = 4;
  double price = 5;
}
```

**Response:**

```protobuf
message RunBacktestResponse {
  BacktestResult result = 1;
}

message BacktestResult {
  string market_id = 1;
  double initial_balance = 2;
  double final_balance = 3;
  double total_pnl = 4;
  double total_pnl_percent = 5;
  double max_drawdown = 6;
  double max_drawdown_percent = 7;
  double sharpe_ratio = 8;
  repeated BacktestStep steps = 9;
}

message BacktestStep {
  string date = 1;
  string action = 2;
  double price = 3;
  double quantity = 4;
  double balance = 5;
  double pnl = 6;
}
```

## Server Streaming

### Stream Markets

Subscribe to real-time market updates via server streaming.

**Request:**

```protobuf
message StreamMarketsRequest {
  string provider = 1;
  uint32 interval_ms = 2;  // Update interval
}
```

**Response Stream:**

```protobuf
message MarketUpdate {
  Market market = 1;
  google.protobuf.Timestamp timestamp = 2;
  string change_type = 3;  // "price", "volume", "status"
}
```

**Rust Example:**

```rust
let mut stream = client
    .stream_markets(StreamMarketsRequest {
        provider: "polymarket".to_string(),
        interval_ms: 5000,  // Every 5 seconds
    })
    .await?
    .into_inner();

while let Some(update) = stream.message().await? {
    println!("Update: {}", update.market.title);
    for outcome in update.market.outcomes {
        println!("  {} @ {}", outcome.name, outcome.price);
    }
}
```

## Error Handling

gRPC errors use standard status codes:

| Code | HTTP | Meaning |
|------|------|---------|
| `OK` | 200 | Success |
| `INVALID_ARGUMENT` | 400 | Invalid request |
| `UNAUTHENTICATED` | 401 | Missing/invalid credentials |
| `PERMISSION_DENIED` | 403 | Insufficient permissions |
| `NOT_FOUND` | 404 | Resource not found |
| `RESOURCE_EXHAUSTED` | 429 | Rate limited |
| `INTERNAL` | 500 | Server error |
| `UNAVAILABLE` | 503 | Service unavailable |

**Rust Example:**

```rust
match client.get_markets(request).await {
    Ok(response) => {
        println!("Success: {:?}", response.get_ref());
    }
    Err(status) => {
        match status.code() {
            tonic::Code::InvalidArgument => {
                println!("Bad request: {}", status.message());
            }
            tonic::Code::Unauthenticated => {
                println!("Auth failed: {}", status.message());
            }
            tonic::Code::ResourceExhausted => {
                println!("Rate limited: {}", status.message());
            }
            _ => {
                println!("Error: {}", status.message());
            }
        }
    }
}
```

## Performance

Expected latencies with local Redis cache:

| Operation | Latency |
|-----------|---------|
| GetMarkets (cached) | 5-15ms |
| GetMarkets (uncached) | 40-80ms |
| PlaceOrder | 50-150ms |
| SearchMarkets | 30-70ms |
| StreamMarkets | 5-10ms per update |

## Reflection

Enable gRPC reflection for tool discovery:

```bash
grpcurl -plaintext localhost:50051 list
grpcurl -plaintext localhost:50051 describe upp.Markets
grpcurl -plaintext localhost:50051 describe upp.Markets.GetMarkets
```
