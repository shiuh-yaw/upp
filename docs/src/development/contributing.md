# Contributing Guide

Detailed guide for contributing code, documentation, and features to UPP.

## Types of Contributions

### Code Contributions

#### New Adapter (New Provider Support)

Add support for a new prediction market exchange:

1. Create adapter module in `gateway/src/adapters/{provider_name}/`
2. Implement `ProviderAdapter` trait
3. Add configuration in `config/default.toml`
4. Add tests in `tests/adapters/{provider_name}_test.rs`
5. Update documentation in `docs/src/`

See [Adding Providers](../architecture/providers.md#adding-a-new-provider) for details.

#### New API Endpoint

Add a new REST or gRPC endpoint:

1. Add handler in `gateway/src/handlers/`
2. Add route in `gateway/src/router.rs`
3. Add protobuf message in `proto/`
4. Add tests in `tests/api_test.rs`
5. Document in `docs/src/api/`

#### Bug Fixes

1. Add failing test that reproduces the issue
2. Fix the bug
3. Verify test passes
4. Add regression test

#### Performance Improvements

1. Benchmark current performance
2. Implement improvement
3. Benchmark improved version
4. Document benchmark results in PR

### Documentation Contributions

- **API Documentation** — Improve examples, clarify concepts
- **User Guides** — Add tutorials, troubleshooting guides
- **Architecture Docs** — Explain design decisions
- **Examples** — Create sample applications

### Bug Reports

Found a bug? Report it:

```
Title: [BUG] Describe the issue

Description:
- What were you trying to do?
- What happened?
- What did you expect?

Steps to reproduce:
1. Run command X
2. Do action Y
3. Observe error Z

Environment:
- OS: macOS / Linux / Windows
- Rust version: 1.xx
- Gateway version: 0.1.0

Logs:
[Paste relevant error messages or logs]
```

### Feature Requests

Suggest improvements:

```
Title: [FEATURE] Describe desired feature

Problem:
What problem would this solve?

Solution:
How should it work?

Alternatives considered:
Any other approaches?

Example use case:
When would you use this?
```

## Development Checklist

When submitting code, ensure:

- [ ] Code follows style guidelines (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] All tests pass (`cargo test`)
- [ ] New code has tests (aim for >80% coverage)
- [ ] Documentation is updated
- [ ] Commit messages are clear and descriptive
- [ ] No merge conflicts with `main` branch
- [ ] PR description explains the change

## Code Review Process

1. **Automated checks** run first:
   - Tests must pass
   - Formatting must be correct
   - No clippy warnings

2. **Maintainers review** for:
   - Correctness
   - Performance
   - Security
   - API compatibility
   - Documentation quality

3. **Discussion & refinement**:
   - Address feedback
   - Request clarification if needed
   - Update based on suggestions

4. **Approval & merge**:
   - Approval from at least 1 maintainer
   - All CI checks pass
   - No unresolved conversations

## Common Patterns

### Adding a New Handler

```rust
// In gateway/src/handlers.rs
use axum::{extract::{State, Query}, Json};

pub async fn handle_new_endpoint(
    State(state): State<AppState>,
    Query(params): Query<NewRequest>,
) -> Result<Json<NewResponse>, ApiError> {
    // Validate input
    if params.limit > 100 {
        return Err(ApiError::InvalidInput(
            "Limit must be <= 100".into()
        ));
    }

    // Call adapter
    let results = state.adapter
        .new_method(&params)
        .await
        .map_err(|e| ApiError::Provider(e))?;

    // Return response
    Ok(Json(NewResponse { results }))
}
```

Register in `router.rs`:

```rust
.route("/api/v1/new-endpoint", get(handle_new_endpoint))
```

### Adding a New Metric

```rust
use prometheus::{Counter, Histogram};

lazy_static::lazy_static! {
    pub static ref CUSTOM_OPERATIONS: Counter = Counter::new(
        "upp_custom_operations_total",
        "Total custom operations"
    ).unwrap();

    pub static ref CUSTOM_DURATION: Histogram = Histogram::new(
        "upp_custom_duration_seconds",
        "Custom operation duration"
    ).unwrap();
}

// Use in code
CUSTOM_OPERATIONS.inc();
let timer = CUSTOM_DURATION.start_timer();
// ... do work ...
timer.observe_duration();
```

### Adding a New Adapter Method

```rust
#[async_trait]
impl ProviderAdapter for PolymarketAdapter {
    async fn new_method(
        &self,
        params: &NewParams,
    ) -> Result<NewResponse, ProviderError> {
        // Add tracing span
        let span = tracing::info_span!("polymarket_new_method");
        let _enter = span.enter();

        // Check cache
        let cache_key = format!("new_method:{:?}", params);
        if let Ok(cached) = self.get_from_cache(&cache_key).await {
            return Ok(cached);
        }

        // Call API
        let url = format!("{}new_endpoint", self.base_url);
        let response = self.client
            .get(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let result = self.parse_response(response)?;

        // Cache result
        self.cache_result(&cache_key, &result, Duration::from_secs(300)).await.ok();

        Ok(result)
    }
}
```

## Testing Standards

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_outcome_probability() {
        let market = create_test_market();
        let total: f64 = market.outcomes.iter().map(|o| o.probability).sum();
        assert!((total - 1.0).abs() < 0.0001, "Probabilities should sum to 1");
    }

    #[tokio::test]
    async fn test_place_order_validation() {
        let adapter = PolymarketAdapter::test_instance();

        let order = OrderRequest {
            price: 1.5,  // Invalid (>1)
            ..Default::default()
        };

        let result = adapter.place_order(order).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ProviderError::InvalidInput(_))));
    }

    #[test]
    fn test_error_mapping() {
        let http_err = http::StatusCode::RATE_LIMIT_EXCEEDED;
        let provider_err = map_http_error(http_err);
        assert!(matches!(provider_err, ProviderError::RateLimited(_)));
    }
}
```

### Mock Example

```rust
#[cfg(test)]
mod tests {
    use mockito::mock;

    #[tokio::test]
    async fn test_with_mock_api() {
        let _m = mock("GET", mockito::Matcher::Regex(".*markets.*".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":"1","name":"Market 1"}]"#)
            .create();

        let adapter = PolymarketAdapter::new(test_config());
        let markets = adapter.get_markets(MarketFilter::default()).await.unwrap();

        assert_eq!(markets.len(), 1);
    }
}
```

## Commit Message Guidelines

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Type:**
- feat: New feature
- fix: Bug fix
- docs: Documentation
- style: Code style
- refactor: Refactoring
- perf: Performance
- test: Test additions
- chore: Build/dep updates

**Scope:** Component affected (adapters, handlers, cache, etc.)

**Subject:**
- Imperative mood ("add" not "added")
- Don't capitalize first letter
- No period at end
- Max 50 characters

**Body:**
- Wrap at 72 characters
- Explain what and why, not how
- Reference issues: "Fixes #123"

**Example:**

```
feat(adapters): add Origin protocol support

Add support for Origin prediction market protocol. Includes:
- New OriginAdapter implementation
- Configuration for Origin API credentials
- Comprehensive test suite
- Integration with existing gateway routes

Closes #456
```

## Documentation Standards

### API Documentation Example

```rust
/// Get markets from a provider.
///
/// Returns a paginated list of active markets.
///
/// # Arguments
///
/// * `filter` - Filtering and pagination options
///
/// # Returns
///
/// `Ok(Vec<Market>)` with matching markets
/// `Err(ProviderError)` if the API call fails
///
/// # Errors
///
/// Returns `ProviderError::RateLimited` if too many requests
/// Returns `ProviderError::NetworkError` on connection issues
///
/// # Example
///
/// ```
/// let filter = MarketFilter {
///     limit: 10,
///     ..Default::default()
/// };
/// let markets = adapter.get_markets(filter).await?;
/// for market in markets {
///     println!("{}", market.title);
/// }
/// ```
pub async fn get_markets(
    &self,
    filter: MarketFilter,
) -> Result<Vec<Market>, ProviderError>
```

### Markdown Example

```markdown
## Feature Name

Brief description.

### How It Works

Explanation of mechanism.

### Example

```bash
$ command --flag value
output
```

### Configuration

What settings affect this feature?

### Performance

Expected latency, throughput, or resource usage.

### See Also

- [Related feature](related.md)
- [API docs](api.md)
```

## Security Considerations

When contributing:

1. **Never commit secrets** — No API keys, private keys, or credentials
2. **Input validation** — Validate all external input
3. **Error handling** — Don't leak sensitive info in error messages
4. **Dependencies** — Keep dependencies up-to-date
5. **Review security** — Point out any security concerns in PR

Example:

```rust
// Bad - exposes API key in error
return Err(ProviderError::AuthError(format!(
    "Failed to authenticate with key: {}",
    self.api_key  // NEVER DO THIS
)));

// Good - hides sensitive data
return Err(ProviderError::InvalidCredentials);
```

## Performance Considerations

When optimizing:

1. **Benchmark before & after** — Use `criterion.rs`
2. **Document trade-offs** — Speed vs. memory, complexity vs. maintainability
3. **Profile** — Use flame graphs, perf, or similar tools
4. **Real-world data** — Test with realistic input sizes
5. **Measure impact** — Quantify improvements

Example:

```rust
// Benchmark
#[cfg(test)]
mod benches {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn bench_market_search(c: &mut Criterion) {
        c.bench_function("search 10000 markets", |b| {
            b.iter(|| search_markets(black_box("ethereum"), 10000))
        });
    }

    criterion_group!(benches, bench_market_search);
    criterion_main!(benches);
}
```

## Troubleshooting

### Build Issues

```bash
# Clean and rebuild
cargo clean
cargo build

# Check dependencies
cargo update
cargo tree

# Run with verbose output
cargo build -vv
```

### Test Issues

```bash
# Run single test
cargo test test_name -- --nocapture

# Run tests sequentially (no parallelization)
cargo test -- --test-threads=1

# Show full output
cargo test -- --nocapture --test-threads=1
```

### Git Issues

```bash
# Undo last commit (keep changes)
git reset --soft HEAD~1

# Undo changes to a file
git checkout -- path/to/file

# View recent commits
git log --oneline -10

# Rebase on main
git fetch origin
git rebase origin/main
```

## Asking for Help

Stuck? Ask in:

- **GitHub Discussions** — Design questions, architecture help
- **GitHub Issues** — Clarify requirements, get feedback
- **Pull Request Comments** — Ask reviewers for guidance
- **Discord** — Real-time help from community

Be specific:

```
I'm working on [feature X] and stuck on [specific problem].

Here's what I've tried:
1. ...
2. ...

Here's the error:
[paste error message]

Any suggestions?
```

## Thank You!

Thank you for contributing to UPP. Your efforts make prediction markets more accessible to everyone!
