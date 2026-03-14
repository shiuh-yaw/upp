# UPP Gateway Middleware

Production-ready authentication, rate limiting, CORS, and request ID middleware for the Universal Prediction Protocol Gateway.

## Quick Start

### Files
- **auth.rs** — Enhanced authentication with HMAC, RS256, IP restrictions, audit logging
- **rate_limit.rs** — Token bucket rate limiting with tier overrides and distributed support
- **cors.rs** — CORS handling with preflight caching and origin validation
- **request_id.rs** — UUID v4 request ID generation and injection
- **mod.rs** — Module exports

### Documentation
- **ENHANCEMENTS.md** — Complete feature overview
- **IMPLEMENTATION_REFERENCE.md** — API reference and code patterns
- **INTEGRATION_EXAMPLES.md** — Copy-paste-ready examples
- **README.md** — This file

## Key Features

### Authentication (`auth.rs`)
- ✅ API Key validation (X-API-Key header)
- ✅ JWT validation (HS256 and RS256)
- ✅ HMAC-SHA256 request signing
- ✅ IP allowlist/blocklist support
- ✅ Path-level auth requirements
- ✅ Audit logging
- ✅ Production mode constructor
- ✅ Client tier rate limit multipliers

### Rate Limiting (`rate_limit.rs`)
- ✅ Token bucket algorithm
- ✅ Multi-tier classification (Light/Standard/Heavy/WebSocket)
- ✅ Per-client overrides for Enterprise tiers
- ✅ Rate limit response headers
- ✅ IP and API-key based bucketing
- ✅ Sliding window option (framework)
- ✅ Redis distributed support (framework)
- ✅ Lock-free concurrent access (DashMap)

### CORS (`cors.rs`)
- ✅ Configurable allowed origins
- ✅ Wildcard subdomain support
- ✅ Preflight caching
- ✅ Credentials support
- ✅ Method restrictions
- ✅ Custom header exposure

### Request ID (`request_id.rs`)
- ✅ UUID v4 generation
- ✅ Extract or generate
- ✅ Configurable header name
- ✅ Tracing integration
- ✅ Response header injection

## Usage

### Basic Setup
```rust
use upp_gateway::middleware::auth::AuthState;
use upp_gateway::middleware::rate_limit::{RateLimitState, RateLimitConfig};

let auth = AuthState::production(config);
let rate_limiter = RateLimitState::new(RateLimitConfig::default());
```

### With Middleware
```rust
let app = Router::new()
    .route("/api/orders", post(place_order))
    .layer(middleware::from_fn_with_state(
        rate_limiter.clone(),
        rate_limit_middleware,
    ))
    .layer(middleware::from_fn_with_state(
        auth.clone(),
        auth_middleware,
    ));
```

See **INTEGRATION_EXAMPLES.md** for complete examples.

## Testing

All modules include comprehensive unit tests:
```bash
cargo test -p upp-gateway --lib middleware
```

## Dependencies

```toml
# Existing (already in Cargo.toml)
axum = "0.7"
jsonwebtoken = "9"
uuid = "1"
dashmap = "6"
tracing = "0.1"

# New
sha2 = "0.10"      # HMAC-SHA256
hmac = "0.12"      # HMAC implementation
base64 = "0.23"    # Base64 encoding
```

## Performance

- **Auth**: O(1) API key lookup, O(1) JWT validation
- **Rate Limit**: O(1) bucket check, DashMap lock-free access
- **CORS**: O(n) origin matching where n = allowed origins
- **Request ID**: UUID generation is fast, no I/O

## Production Checklist

- [ ] Use `AuthState::production()` in production
- [ ] Configure IP allowlist for sensitive endpoints
- [ ] Set rate limit multipliers per tier
- [ ] Enable RS256 JWT validation
- [ ] Configure CORS for your domains
- [ ] Enable audit logging to monitoring
- [ ] Test rate limits under load
- [ ] Verify request IDs in logs

## API Reference

### AuthState
```rust
pub fn production(config: AuthConfig) -> Self
pub fn require_auth_for_path(&self, path: &str) -> bool
pub fn validate_request_signature(&self, secret: &str, payload: &[u8], sig: &str) -> bool
pub fn validate_rs256_token(&self, token: &str) -> Option<ClientInfo>
pub fn is_ip_allowed(&self, ip: &str) -> bool
pub fn is_ip_blocked(&self, ip: &str) -> bool
```

### RateLimitState
```rust
pub fn set_client_override(&self, key: &str, multiplier: f64)
pub fn remove_client_override(&self, key: &str)
pub fn check(&self, key: &str, tier: RateLimitTier) -> RateLimitResult
pub fn check_legacy(&self, key: &str, tier: RateLimitTier) -> (bool, u32, u32, f64)
```

### CorsState
```rust
pub fn is_origin_allowed(&self, origin: &str) -> bool
pub fn handle_preflight(&self, origin: Option<&str>, method: Option<&str>) -> Result<HeaderMap, StatusCode>
pub fn apply_cors_headers(&self, headers: &mut HeaderMap, origin: Option<&str>)
```

### RequestIdState
```rust
pub fn generate_id(&self) -> String
pub fn get_or_generate_id(&self, headers: &HeaderMap) -> String
pub fn to_header_value(&self, request_id: &str) -> Option<HeaderValue>
```

## Examples

### HMAC Signature Validation
```rust
let auth = AuthState::production(config);
let is_valid = auth.validate_request_signature(
    api_secret,
    request_body,
    signature_header
);
```

### Rate Limit Override
```rust
let limiter = RateLimitState::new(config);
limiter.set_client_override("apikey:enterprise123", 10.0);  // 10x limits
```

### CORS Preflight
```rust
let cors = CorsState::new(config);
let headers = cors.handle_preflight(origin, method)?;
```

### Request ID Injection
```rust
let request_id_state = RequestIdState::default();
let id = request_id_state.get_or_generate_id(headers);
```

## See Also

- **ENHANCEMENTS.md** — Detailed feature documentation
- **IMPLEMENTATION_REFERENCE.md** — Type signatures and patterns
- **INTEGRATION_EXAMPLES.md** — Ready-to-use examples

## Support

For issues or questions, refer to the documentation files or unit tests for reference implementations.

---

**Last Updated**: March 14, 2026  
**Version**: 1.0.0  
**License**: Apache-2.0
