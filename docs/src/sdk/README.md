# SDK Overview

The UPP Rust SDK provides a type-safe, async-first client library for building applications with prediction market data.

## Features

- **Type-Safe** — Rust's type system catches errors at compile time
- **Async/Await** — Built on Tokio for concurrent, non-blocking I/O
- **Builder Pattern** — Fluent API for constructing requests
- **Error Handling** — Rich error types with context
- **Streaming** — Support for WebSocket subscriptions and server-sent events
- **Caching** — Optional client-side response caching
- **Retries** — Automatic exponential backoff on failures
- **Tracing** — Integration with OpenTelemetry for observability

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
upp-sdk = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Quick Start

Get markets in 5 lines:

```rust
use upp_sdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("http://localhost:8080", "api_key");
    let markets = client.markets()
        .provider("polymarket")
        .limit(10)
        .fetch()
        .await?;

    for market in markets {
        println!("{}: {}", market.id, market.title);
    }

    Ok(())
}
```

## Core Concepts

### Client

The main entry point. Create once and reuse:

```rust
let client = Client::builder()
    .base_url("http://localhost:8080")
    .api_key("api_key")
    .timeout(Duration::from_secs(30))
    .build()?;
```

### Builders

Fluent API for constructing requests:

```rust
let markets = client
    .markets()
    .provider("polymarket")
    .category("crypto")
    .limit(20)
    .offset(10)
    .fetch()
    .await?;
```

### Error Handling

Rich error types:

```rust
use upp_sdk::error::{ClientError, ProviderError};

match client.markets().fetch().await {
    Ok(markets) => { /* ... */ }
    Err(ClientError::InvalidProvider { provider }) => {
        eprintln!("Unknown provider: {}", provider);
    }
    Err(ClientError::Provider(ProviderError::RateLimited(backoff))) => {
        eprintln!("Rate limited, retry after {:?}", backoff);
    }
    Err(ClientError::Network(e)) => {
        eprintln!("Network error: {}", e);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

### Result Type

All operations return `Result<T, ClientError>`:

```rust
pub type Result<T> = std::result::Result<T, ClientError>;
```

## Documentation

This section includes:

- **[Rust Client Guide](rust.md)** — Complete API documentation with examples

## External Resources

- [GitHub Repository](https://github.com/universal-prediction-protocol/upp)
- [Crate Documentation](https://docs.rs/upp-sdk/)
- [Examples Directory](https://github.com/universal-prediction-protocol/upp/tree/main/examples)

## Integration Patterns

### Web Server Integration

```rust
use axum::{Router, extract::State};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    upp_client: Arc<upp_sdk::Client>,
}

async fn get_markets(
    State(state): State<AppState>,
) -> Result<Json<Vec<Market>>, StatusCode> {
    let markets = state.upp_client
        .markets()
        .provider("polymarket")
        .fetch()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(markets))
}
```

### Background Jobs

```rust
use tokio::spawn;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("http://localhost:8080", "api_key");

    // Spawn background task
    spawn(async move {
        loop {
            // Update portfolio every 30 seconds
            if let Ok(portfolio) = client.portfolio().fetch().await {
                println!("Updated portfolio: {:?}", portfolio);
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    // Main application logic
    Ok(())
}
```

## Next: Full Rust Client Guide

Ready to learn more? See [Rust Client Guide](rust.md) for complete API documentation, streaming examples, and advanced patterns.
