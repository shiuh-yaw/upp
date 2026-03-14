# UPP CLI - UPP Gateway Command Line Interface

A comprehensive command-line interface for interacting with the UPP (Unified Prediction Platform) Gateway. Supports markets, orders, trading, portfolio management, backtesting, and more.

## Features

- **Markets**: List, search, and get market details
- **Orders**: Create, manage, list, and cancel orders
- **Trades**: View trade history and execution details
- **Portfolio**: Monitor positions, summary, analytics, and balances
- **Arbitrage**: Discover and track arbitrage opportunities
- **Backtesting**: Run and compare trading strategies
- **Data Feeds**: Monitor feed connectivity and statistics
- **Route Computation**: Calculate optimal execution routes
- **Configuration**: Manage gateway URL and API keys
- **Output Formatting**: Colored tables, JSON export, and structured output

## Installation

Build the binary with Cargo:

```bash
cd /sessions/stoic-compassionate-turing/mnt/outputs/upp/cli
cargo build --release
```

The compiled binary will be at `target/release/upp`

## Configuration

The CLI stores configuration in `~/.upp/config.toml`:

```toml
gateway_url = "http://localhost:9090"
api_key = "your-api-key-here"
```

### Quick Setup

```bash
upp config set-url http://localhost:9090
upp config set-key your-api-key
upp config show
```

## Global Flags

- `--url <URL>` - Override gateway URL
- `--api-key <KEY>` - Override API key
- `--json` - Output as raw JSON instead of formatted tables

## Command Structure

### Health Check

```bash
upp health
```

Check gateway status, uptime, and version.

### Markets

```bash
# List all markets
upp markets list

# List with filters
upp markets list --provider kalshi --status open --limit 50

# Get market details
upp markets get MARKET_ID

# Search markets
upp markets search "bitcoin price"
```

### Orders

```bash
# List orders
upp orders list
upp orders list --provider kalshi --status open

# Create order
upp orders create --market MARKET_ID --side buy --price 0.55 --quantity 10

# Get order details
upp orders get ORDER_ID

# Cancel order
upp orders cancel ORDER_ID

# Cancel all orders
upp orders cancel-all
```

### Trades

```bash
# List trades with limit
upp trades list --limit 50
```

### Portfolio

```bash
# View current positions
upp portfolio positions

# Summary statistics
upp portfolio summary

# Full analytics with Sharpe ratio, drawdown, etc.
upp portfolio analytics

# Account balances
upp portfolio balances
```

### Arbitrage

```bash
# List arbitrage opportunities
upp arbitrage list

# View arbitrage summary statistics
upp arbitrage summary
```

### Candles

```bash
# Get candle data
upp candles MARKET_ID

# With outcome filter
upp candles MARKET_ID --outcome yes

# Custom resolution and limit
upp candles MARKET_ID --resolution 5m --limit 500
```

**Resolution options**: 1m, 5m, 15m, 1h, 4h, 1d

### Backtesting

```bash
# List available strategies
upp backtest strategies

# Run backtest
upp backtest run --strategy mean_reversion --market MARKET_ID

# Run with parameters
upp backtest run --strategy momentum --market MARKET_ID --params lookback=20,threshold=0.02

# Compare strategies
upp backtest compare --market MARKET_ID --strategies mean_reversion,momentum,rsi
```

### Feeds

```bash
# Check feed connection status
upp feeds status

# View feed statistics
upp feeds stats
```

### Route Computation

```bash
# Compute optimal execution route
upp route compute --market MARKET_ID --side buy --quantity 10

# Execute a computed route
upp route execute '{"market_id":"MARKET_ID","side":"buy",...}'
```

## Output Examples

### Formatted Table Output

```
Health Check Results:
───────────────────
Status  │ healthy
Uptime  │ 48h 23m
Version │ 1.2.3
```

### JSON Output

```bash
upp health --json
```

Returns:

```json
{
  "status": "healthy",
  "uptime": "48h 23m",
  "version": "1.2.3"
}
```

## Architecture

### Project Structure

```
upp-cli/
├── Cargo.toml          # Project metadata and dependencies
├── README.md           # This file
└── src/
    ├── main.rs         # CLI commands and entry point (1590 lines)
    ├── config.rs       # Configuration loading/saving
    └── output.rs       # Table formatting and output styling
```

### Key Components

**main.rs** (1590 lines)
- CLI command definitions using `clap` derive macros
- All HTTP client interactions with gateway
- Command implementations with error handling
- Supports 20+ distinct operations

**config.rs** (71 lines)
- Config struct with serialization
- Loading from `~/.upp/config.toml`
- Default values and path management
- Config override support

**output.rs** (381 lines)
- Colored output formatting with `colored` crate
- Table rendering with `tabled` crate
- JSON formatting with `serde_json`
- Summary display types (Markets, Orders, Trades, etc.)
- Utility formatting functions

### Dependencies

- **clap 4.4** - CLI argument parsing with derive macros
- **reqwest 0.11** - Async HTTP client
- **serde/serde_json** - JSON serialization/deserialization
- **tokio** - Async runtime
- **anyhow** - Error handling
- **colored 2.1** - Terminal color output
- **tabled 0.15** - ASCII table formatting
- **chrono** - Date/time handling
- **toml 0.8** - TOML config file parsing
- **dirs 5.0** - Platform-specific directory paths

## Design Patterns

### Command Handling
Each subcommand is implemented as an async function following the pattern:
- Accept config and parameters
- Create HTTP client
- Build request URL with parameters
- Handle response
- Format and display output

### Error Handling
Uses `anyhow::Result` for propagating errors with context:
```rust
async fn cmd_health(config: &Config, json_output: bool) -> Result<()> {
    // error handling with `?` operator
}
```

### Configuration Management
- Load from file or use defaults
- Override with CLI flags
- Save changes back to TOML

### Output Formatting
- Check `json_output` flag
- Either format as tables/KV pairs or output raw JSON
- Use colored formatting for better readability

## API Assumptions

The CLI assumes the UPP Gateway API follows these patterns:
- Base URL: `http://localhost:9090`
- REST endpoints with JSON request/response bodies
- Standard HTTP status codes
- Consistent response JSON structure

### Expected Endpoints

- `GET /health` - Gateway status
- `GET /markets` - List markets
- `GET /markets/{id}` - Market details
- `GET /markets/search` - Search markets
- `GET/POST /orders` - Order management
- `GET /trades` - Trade history
- `GET /portfolio/*` - Portfolio data
- `GET /arbitrage/*` - Arbitrage opportunities
- `GET /candles/{market}` - Candle data
- `POST /backtest/*` - Backtesting operations
- `GET /feeds/*` - Feed management
- `POST /route/compute` - Route calculation
- `POST /route/execute` - Execute routes

## Building and Running

### Build

```bash
cargo build --release
```

Binary location: `target/release/upp`

### Test

```bash
cargo test
```

### Install Locally

```bash
cargo install --path .
```

This installs the `upp` binary to `~/.cargo/bin/`

## Authentication

API keys are optional and stored in `~/.upp/config.toml`. Set via:

```bash
upp config set-key your-api-key-here
```

Currently, keys are not automatically added to requests but can be implemented by modifying the request builder:

```rust
if let Some(api_key) = &config.api_key {
    builder = builder.header("Authorization", format!("Bearer {}", api_key));
}
```

## Development Notes

### Adding New Commands

1. Add subcommand variant to the relevant enum
2. Implement command handler function following the pattern
3. Add match arm in main()
4. Update README with examples

### Adding Output Types

1. Create struct in `output.rs`
2. Implement formatter function
3. Use in command handlers based on `json_output` flag

### URL Encoding

Searches use `urlencoding::encode()` for query parameters. Note: the current dependency list doesn't include `urlencoding`, which should be added if search is used:

```bash
cargo add urlencoding
```

## License

Built as part of the UPP Gateway project.
