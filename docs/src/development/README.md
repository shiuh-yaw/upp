# Development

This section covers contributing to UPP, code style, testing, and development workflows.

## Welcome Contributors!

We welcome contributions of all kinds:

- **Bug fixes** — Report and fix issues
- **Features** — Implement new adapters, endpoints, or capabilities
- **Documentation** — Improve guides and API docs
- **Tests** — Increase coverage and reliability
- **Performance** — Optimize hot paths
- **Examples** — Create example applications

## Getting Started

### Prerequisites

- Rust 1.70+ ([install via rustup](https://rustup.rs/))
- Protobuf compiler (`protoc`)
- Redis (for local testing)
- Git

### Setup

```bash
# Clone repository
git clone https://github.com/universal-prediction-protocol/upp.git
cd upp

# Install protoc
# macOS
brew install protobuf

# Ubuntu/Debian
apt-get install protobuf-compiler

# Verify
protoc --version

# Build everything
cargo build

# Run tests
cargo test

# Start local development stack
docker-compose up -d
```

## Project Structure

```
upp/
├── gateway/           # REST + gRPC server
│   ├── src/
│   │   ├── main.rs
│   │   ├── router.rs
│   │   ├── adapters/
│   │   └── handlers/
│   └── Cargo.toml
├── sdk/              # Rust client library
│   ├── src/
│   └── Cargo.toml
├── cli/              # Command-line tool
│   ├── src/
│   └── Cargo.toml
├── proto/            # Protocol Buffer definitions
│   └── *.proto
├── config/           # Configuration and dashboards
├── tests/            # Integration tests
└── docker-compose.yml
```

## Development Workflow

### 1. Create a Branch

```bash
git checkout -b feature/new-provider
# or
git checkout -b fix/cache-issue
# or
git checkout -b docs/improve-readme
```

### 2. Make Changes

Write code, tests, and documentation.

### 3. Run Tests Locally

```bash
# Unit tests
cargo test --lib

# Integration tests (requires docker-compose up)
cargo test --test '*'

# All tests with output
cargo test -- --nocapture

# Specific test
cargo test markets::tests::test_get_markets
```

### 4. Format Code

```bash
# Auto-format
cargo fmt

# Check without modifying
cargo fmt -- --check
```

### 5. Lint

```bash
# Run clippy
cargo clippy -- -D warnings

# Fix automatically where possible
cargo clippy --fix
```

### 6. Build for Release

```bash
cargo build --release

# Binary at ./target/release/gateway
```

### 7. Submit Pull Request

Push your branch and open a PR on GitHub with:

- Clear title and description
- Reference to related issues
- Explanation of changes
- Test results

## Code Style

### Naming Conventions

```rust
// Constants: UPPER_SNAKE_CASE
const MAX_RETRIES: u32 = 3;

// Variables & functions: snake_case
fn get_market_by_id(id: &str) -> Result<Market> { }

// Types & traits: PascalCase
struct MarketFilter { }
trait ProviderAdapter { }

// Enums: PascalCase variants
enum Status {
    Active,
    Resolved,
    Cancelled,
}
```

### Error Handling

Use `Result<T>` and descriptive error types:

```rust
// Good
pub fn get_markets(&self, filter: MarketFilter) -> Result<Vec<Market>> {
    let response = self.client.get(&url).await?;
    Ok(response.markets)
}

// Bad
pub fn get_markets(&self, filter: MarketFilter) -> Result<Vec<Market>> {
    match self.client.get(&url).await {
        Ok(response) => Ok(response.markets),
        Err(e) => Err(format!("Failed: {}", e)),  // Too generic
    }
}
```

### Documentation

Document public APIs:

```rust
/// Get markets from the specified provider.
///
/// # Arguments
///
/// * `filter` - Market filtering options
///
/// # Returns
///
/// Vector of matching markets
///
/// # Errors
///
/// Returns `ProviderError::RateLimited` if API quota exceeded
///
/// # Example
///
/// ```
/// let markets = adapter.get_markets(filter).await?;
/// ```
pub async fn get_markets(&self, filter: MarketFilter) -> Result<Vec<Market>> {
}
```

### Comments

Use comments for the "why", not the "what":

```rust
// Good
// Polymarket requires EIP-712 signing, not simple HMAC
let signature = sign_ecdsa(&self.private_key, &message)?;

// Bad
// Sign the message
let signature = sign_ecdsa(&self.private_key, &message)?;
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_price_validation() {
        let market = Market {
            id: "test".into(),
            outcomes: vec![
                Outcome { price: 0.5, .. },
                Outcome { price: 0.5, .. },
            ],
            ..
        };

        // Total should equal 1.0
        assert_eq!(market.total_probability(), 1.0);
    }

    #[tokio::test]
    async fn test_async_operation() {
        let result = fetch_market("id").await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_gateway_health_check() {
    let client = Client::new("http://localhost:8080", "test_key");
    let health = client.health().await;
    assert!(health.is_ok());
}
```

See [Testing](testing.md) for complete guide.

## Commit Messages

Use conventional commits:

```
type(scope): subject

body

footer
```

Examples:

```
feat(adapters): add Origin protocol adapter
fix(cache): prevent stale market data
docs(sdk): improve builder example
test(gateway): add market search tests
perf(redis): optimize cache key generation
chore(deps): update tokio to 1.35
refactor(providers): extract common auth logic
```

Types:

- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation
- `test` — Test additions/changes
- `perf` — Performance improvements
- `refactor` — Code refactoring
- `chore` — Maintenance, dependencies

## Pull Request Process

1. **Fork the repository**
2. **Create a feature branch** from `main`
3. **Write tests** for your changes
4. **Keep commits clean** with descriptive messages
5. **Run `cargo fmt` and `cargo clippy`** before pushing
6. **Push to your fork**
7. **Create a Pull Request** with description
8. **Address review feedback**
9. **Squash commits** if requested
10. **Merge** when approved

## Release Process

Releases are cut by maintainers:

```bash
# Create version tag
git tag -a v0.2.0 -m "Release 0.2.0"

# Build release artifacts
cargo build --release

# Create GitHub release
gh release create v0.2.0 ./target/release/gateway --notes "Release notes"
```

## Continuous Integration

GitHub Actions runs on every PR:

- Unit tests (`cargo test`)
- Code format check (`cargo fmt`)
- Linting (`cargo clippy`)
- Documentation build
- Security audits (`cargo audit`)

All must pass before merge.

## Documentation

Documentation lives in `docs/src/` as Markdown files.

Build locally:

```bash
mdbook serve docs

# Open http://localhost:3000
```

## Community

- **GitHub Issues** — Report bugs, request features
- **GitHub Discussions** — Ask questions, discuss design
- **Discord** — Real-time chat (link in README)
- **Twitter** — Follow for updates

## Code of Conduct

Be respectful, inclusive, and constructive. Treat everyone with respect.

## Licensing

Contributions are licensed under Apache 2.0. By submitting a PR, you agree to this license.

## Getting Help

- Read the [Architecture Guide](../architecture/README.md)
- Check existing [Issues](https://github.com/universal-prediction-protocol/upp/issues)
- Ask in [Discussions](https://github.com/universal-prediction-protocol/upp/discussions)
- Join our [Discord](https://discord.gg/...)

## Next Steps

- **Contributing?** See [Contributing Guide](contributing.md)
- **Writing tests?** See [Testing Guide](testing.md)
- **Need architecture details?** See [Architecture](../architecture/README.md)

Thank you for contributing to UPP!
