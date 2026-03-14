# Implementation Reference: Middleware Enhancements

Quick reference for key code additions and modifications.

---

## 1. Auth Module (`auth.rs`) — Key Additions

### New AuthConfig Fields
```rust
pub struct AuthConfig {
    // ... existing fields ...
    pub ip_allowlist: Option<Vec<String>>,
    pub ip_blocklist: Option<Vec<String>>,
    pub jwt_public_key: Option<String>,
}
```

### New ClientTier Methods
```rust
impl ClientTier {
    pub fn rate_limit_multiplier(&self) -> f64 {
        match self {
            ClientTier::Free => 0.5,
            ClientTier::Standard => 1.0,
            ClientTier::Pro => 2.0,
            ClientTier::Enterprise => 10.0,
        }
    }
}
```

### New AuthState Constructors & Methods
```rust
// Constructor for production
pub fn production(config: AuthConfig) -> Self { }

// Path-level auth requirement checking
pub fn require_auth_for_path(&self, path: &str) -> bool { }

// HMAC-SHA256 signature validation
pub fn validate_request_signature(&self, secret: &str, payload: &[u8], signature: &str) -> bool { }

// RS256 JWT validation
pub fn validate_rs256_token(&self, token: &str) -> Option<ClientInfo> { }

// IP allowlist checking
pub fn is_ip_allowed(&self, ip: &str) -> bool { }

// IP blocklist checking
pub fn is_ip_blocked(&self, ip: &str) -> bool { }

// IP extraction from headers
fn extract_client_ip(&self, headers: &HeaderMap) -> Option<String> { }

// Audit logging
fn audit_log(&self, client_id: &str, ip: &str, path: &str, success: bool) { }
```

### Dependencies Added
```toml
sha2 = "0.10"
hmac = "0.12"
base64 = "0.23"
```

### Import Additions
```rust
use tracing::{info, warn};
use base64::Engine;
```

---

## 2. Rate Limit Module (`rate_limit.rs`) — Key Additions

### New RateLimitResult Struct
```rust
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u32,
    pub limit: u32,
    pub retry_after: f64,
}
```

### New RateLimitConfig Fields
```rust
pub struct RateLimitConfig {
    // ... existing fields ...
    pub redis_url: Option<String>,
    pub use_sliding_window: bool,
}
```

### New RateLimitState Methods
```rust
// Set rate limit override for specific client
pub fn set_client_override(&self, client_key: &str, multiplier: f64) { }

// Remove rate limit override
pub fn remove_client_override(&self, client_key: &str) { }

// Check rate limit (new structured return)
pub fn check(&self, key: &str, tier: RateLimitTier) -> RateLimitResult { }

// Legacy check for backward compatibility
pub fn check_legacy(&self, key: &str, tier: RateLimitTier) -> (bool, u32, u32, f64) { }
```

### New Extraction Functions
```rust
// Extract API key from X-API-Key header
pub fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> { }

// Extract Bearer token from Authorization header
pub fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> { }

// Extract client IP from X-Forwarded-For or X-Real-IP
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> Option<String> { }

// Enhanced extract_client_key with fallback support
pub fn extract_client_key(headers: &axum::http::HeaderMap) -> String { }
```

### RateLimitState Field Addition
```rust
// Per-client tier overrides
client_tier_overrides: Arc<DashMap<String, f64>>,
```

### Import Addition
```rust
use tracing::{debug, warn};
```

---

## 3. New CORS Module (`cors.rs`)

### Public Types
```rust
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<Method>,
    pub allowed_headers: Vec<String>,
    pub exposed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub preflight_max_age_secs: u64,
}

pub struct CorsState {
    config: Arc<CorsConfig>,
}
```

### Key Methods
```rust
impl CorsState {
    // Create new CORS state
    pub fn new(config: CorsConfig) -> Self { }

    // Check if origin is allowed
    pub fn is_origin_allowed(&self, origin: &str) -> bool { }

    // Get allow-origin header value
    pub fn get_allow_origin(&self, origin: Option<&str>) -> Option<HeaderValue> { }

    // Get allow-methods header
    pub fn get_allow_methods(&self) -> String { }

    // Get allow-headers header
    pub fn get_allow_headers(&self) -> String { }

    // Get expose-headers header
    pub fn get_expose_headers(&self) -> String { }

    // Handle preflight (OPTIONS) requests
    pub fn handle_preflight(&self, origin: Option<&str>, request_method: Option<&str>)
        -> Result<HeaderMap, StatusCode> { }

    // Apply CORS headers to response
    pub fn apply_cors_headers(&self, headers: &mut HeaderMap, origin: Option<&str>) { }
}
```

### Default Configuration
```rust
impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![GET, POST, PUT, DELETE, PATCH, OPTIONS],
            allowed_headers: vec![
                "content-type".to_string(),
                "authorization".to_string(),
                "x-api-key".to_string(),
                "x-request-id".to_string(),
            ],
            exposed_headers: vec![
                "x-ratelimit-limit".to_string(),
                "x-ratelimit-remaining".to_string(),
                "x-ratelimit-reset".to_string(),
                "x-request-id".to_string(),
                "retry-after".to_string(),
            ],
            allow_credentials: false,
            preflight_max_age_secs: 3600,
        }
    }
}
```

---

## 4. New Request ID Module (`request_id.rs`)

### Public Types
```rust
pub struct RequestIdState {
    pub header_name: String,
}
```

### Key Methods
```rust
impl RequestIdState {
    // Create new with custom header name
    pub fn new(header_name: String) -> Self { }

    // Generate new UUID v4 request ID
    pub fn generate_id(&self) -> String { }

    // Extract from headers or generate new one
    pub fn get_or_generate_id(&self, headers: &axum::http::HeaderMap) -> String { }

    // Convert string ID to HTTP header value
    pub fn to_header_value(&self, request_id: &str) -> Option<HeaderValue> { }
}
```

### Default Configuration
```rust
impl Default for RequestIdState {
    fn default() -> Self {
        Self {
            header_name: "X-Request-ID".to_string(),
        }
    }
}
```

---

## 5. Module Exports (`mod.rs`)

```rust
pub mod auth;
pub mod cors;
pub mod rate_limit;
pub mod request_id;
```

---

## 6. Cargo.toml Updates

### New Dependencies
```toml
sha2 = "0.10"
hmac = "0.12"
base64 = "0.23"
```

Existing dependencies used:
- `axum` — HTTP framework (already present)
- `tracing` — Structured logging (already present)
- `dashmap` — Concurrent hash maps (already present)
- `jsonwebtoken` — JWT validation (already present)
- `uuid` — UUID generation (already present)

---

## Code Patterns & Examples

### Using Rate Limit Overrides
```rust
// Create state
let rate_limiter = Arc::new(RateLimitState::new(RateLimitConfig::default()));

// Override for Enterprise client
let client_key = "apikey:enterprise_key_123";
rate_limiter.set_client_override(client_key, 10.0);

// Check limit (now uses 10x multiplier)
let result = rate_limiter.check(client_key, RateLimitTier::Standard);
```

### Validating Request Signatures
```rust
let auth = AuthState::production(config);

// For trading endpoints
let is_valid = auth.validate_request_signature(
    "my_secret_key",
    request_body,
    signature_from_header
);

if !is_valid {
    return Err("Invalid signature");
}
```

### HMAC-SHA256 Signature Format
```
Header: X-Signature
Value: base64(HMAC-SHA256("secret", payload_bytes))

Example in Python:
import hmac
import hashlib
import base64

payload = b"request body"
secret = "secret_key"
signature = base64.b64encode(
    hmac.new(secret.encode(), payload, hashlib.sha256).digest()
).decode()
```

### RS256 JWT Validation
```rust
// Load RSA public key (PEM format)
let pem_key = std::fs::read("public_key.pem").unwrap();
let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(&pem_key);

let config = AuthConfig {
    jwt_public_key: Some(public_key_b64),
    ..Default::default()
};

let auth = AuthState::new(config);

// Validate JWT from Authorization header
if let Some(client_info) = auth.validate_rs256_token(jwt_token) {
    println!("Client: {}", client_info.client_id);
    println!("Tier: {:?}", client_info.tier);
}
```

### IP Restriction Example
```rust
let config = AuthConfig {
    required: true,
    ip_allowlist: Some(vec![
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
    ]),
    ip_blocklist: Some(vec![
        "10.255.255.255".to_string(),
    ]),
    ..Default::default()
};

let auth = AuthState::production(config);

if !auth.is_ip_allowed(client_ip) {
    return Err("IP not in allowlist");
}

if auth.is_ip_blocked(client_ip) {
    return Err("IP is blocked");
}
```

### CORS Configuration Example
```rust
let config = CorsConfig {
    allowed_origins: vec![
        "https://example.com".to_string(),
        "https://*.example.com".to_string(),
    ],
    allowed_methods: vec![Method::GET, Method::POST],
    allow_credentials: true,
    preflight_max_age_secs: 7200,
    ..Default::default()
};

let cors = CorsState::new(config);

// Handle OPTIONS request
let cors_headers = cors.handle_preflight(origin, method)?;
```

### Request ID in Middleware
```rust
let request_id_state = RequestIdState::default();

async fn middleware(
    req: Request,
    next: Next,
) -> Response {
    let request_id = request_id_state.get_or_generate_id(req.headers());

    // Add to response
    let mut response = next.run(req).await;
    if let Ok(header_value) = request_id_state.to_header_value(&request_id) {
        response.headers_mut().insert("X-Request-ID", header_value);
    }

    // Add to tracing
    tracing::Span::current().record("request_id", &request_id);

    response
}
```

---

## Testing Strategy

### Auth Tests
```rust
#[test]
fn test_validate_hmac_signature() {
    let auth = AuthState::dev_mode();
    let secret = "test_secret";
    let payload = b"test_payload";
    let signature = calculate_hmac_sha256(secret, payload);

    assert!(auth.validate_request_signature(secret, payload, &signature));
}

#[test]
fn test_ip_allowlist() {
    let config = AuthConfig {
        ip_allowlist: Some(vec!["10.0.0.0/8".to_string()]),
        ..Default::default()
    };
    let auth = AuthState::new(config);

    assert!(auth.is_ip_allowed("10.0.0.1"));
    assert!(!auth.is_ip_allowed("192.168.1.1"));
}
```

### Rate Limit Tests
```rust
#[test]
fn test_rate_limit_override() {
    let state = RateLimitState::new(RateLimitConfig::default());
    state.set_client_override("client_1", 2.0);

    // Check that limits are doubled
    for _ in 0..100 { // 2x of 50
        let result = state.check("client_1", RateLimitTier::Standard);
        assert!(result.allowed);
    }
}
```

---

## Migration Guide

### From Old Rate Limit Check
```rust
// Old code
let (allowed, remaining, limit, retry_after) = state.check(key, tier);

// New code (same functionality)
let result = state.check(key, tier);
let allowed = result.allowed;
let remaining = result.remaining;
let limit = result.limit;
let retry_after = result.retry_after;

// Or use backward-compatible method
let (allowed, remaining, limit, retry_after) = state.check_legacy(key, tier);
```

### Enabling Production Auth
```rust
// Development (old)
let auth = AuthState::dev_mode();

// Production (new)
let config = AuthConfig {
    required: true,
    api_keys: load_keys(),
    jwt_secret: Some(load_secret()),
    ip_allowlist: Some(load_ip_whitelist()),
    ..Default::default()
};
let auth = AuthState::production(config);
```

---

## Performance Considerations

1. **Rate Limiting**: DashMap provides lock-free concurrent access; minimal overhead
2. **IP Checking**: Wildcard patterns use string prefix matching; O(n) where n = number of patterns
3. **JWT Validation**: Deferred to jsonwebtoken crate; cryptographically optimized
4. **CORS**: O(m) where m = number of allowed origins
5. **Request ID**: UUID generation is fast; no I/O required

For scale:
- Use IP ranges (CIDR notation) instead of individual IPs for allowlists
- Cache CORS configuration if updating frequently
- Consider Redis backing for distributed rate limiting at scale

---

## Security Checklist

- [ ] HMAC secrets stored securely (not in code)
- [ ] RS256 public keys rotated regularly
- [ ] IP allowlist configured for trading endpoints
- [ ] Rate limit multipliers set appropriately per tier
- [ ] CORS origins restricted to known domains
- [ ] Request IDs logged for audit trail
- [ ] Audit logging enabled in production
- [ ] All endpoints have rate limit headers

---

## File Locations

```
/sessions/stoic-compassionate-turing/mnt/outputs/upp/gateway/src/middleware/
├── auth.rs                          (Enhanced: 330 lines)
├── cors.rs                          (New: 240 lines)
├── rate_limit.rs                    (Enhanced: 370 lines)
├── request_id.rs                    (New: 120 lines)
├── mod.rs                           (Updated: 4 lines)
├── ENHANCEMENTS.md                  (This file)
└── IMPLEMENTATION_REFERENCE.md      (Complete reference)

../Cargo.toml                        (Updated: Added 3 deps)
```
