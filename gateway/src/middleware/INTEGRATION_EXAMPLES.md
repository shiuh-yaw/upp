# Integration Examples: Using Enhanced Middleware

Complete, copy-paste-ready examples for integrating the enhanced middleware.

---

## Example 1: Production Auth with HMAC Signatures

**Use Case**: Secure trading endpoint that requires HMAC-signed requests.

```rust
use axum::extract::{State, Json};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use crate::middleware::auth::AuthState;

pub struct AppState {
    auth: AuthState,
}

/// POST /api/trading/place-order
/// Expected headers:
///   - X-API-Key: <api_key>
///   - X-Signature: <hmac_sha256_signature>
pub async fn place_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Vec<u8>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Extract client IP
    let client_ip = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    // Check IP restrictions
    if !state.auth.is_ip_allowed(client_ip) {
        return Err((StatusCode::FORBIDDEN, "IP not allowed".to_string()));
    }

    // Check if path requires auth
    if state.auth.require_auth_for_path("/api/trading/place-order") {
        // Get API key
        let api_key = headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing X-API-Key".to_string()))?;

        // Get signature
        let signature = headers
            .get("X-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing X-Signature".to_string()))?;

        // Validate signature
        if !state.auth.validate_request_signature(api_key, &body, signature) {
            return Err((StatusCode::UNAUTHORIZED, "Invalid signature".to_string()));
        }
    }

    // Authenticate request
    let auth_result = state.auth.authenticate(&headers, "/api/trading/place-order");
    let client_info = match auth_result {
        crate::middleware::auth::AuthResult::Authenticated(info) => info,
        crate::middleware::auth::AuthResult::Unauthorized(msg) => {
            return Err((StatusCode::UNAUTHORIZED, msg));
        }
        _ => return Err((StatusCode::FORBIDDEN, "Auth failed".to_string())),
    };

    // Process order
    let order = serde_json::json!({
        "order_id": "ord_123",
        "status": "placed",
        "client_id": client_info.client_id,
        "tier": format!("{:?}", client_info.tier),
    });

    Ok(Json(order))
}
```

---

## Example 2: RS256 JWT Validation

**Use Case**: API that validates JWTs signed by external auth provider (Auth0, Cognito, etc).

```rust
use axum::extract::{State, Json};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use crate::middleware::auth::{AuthState, AuthConfig};

pub async fn setup_rs256_auth() -> AuthState {
    // Load RSA public key from file (PEM format)
    let pem_key = std::fs::read("auth_provider_public_key.pem")
        .expect("Failed to read public key file");

    // Encode as base64
    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(&pem_key);

    // Create config with RS256 public key
    let config = AuthConfig {
        required: true,
        jwt_public_key: Some(public_key_b64),
        ..Default::default()
    };

    AuthState::production(config)
}

/// GET /api/profile
/// Expected header: Authorization: Bearer <jwt_token>
pub async fn get_profile(
    State(auth): State<AuthState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Extract Bearer token
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Bearer token".to_string()))?;

    // Validate RS256 JWT
    let client_info = auth
        .validate_rs256_token(token)
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid JWT".to_string()))?;

    // Return user profile
    Ok(Json(serde_json::json!({
        "client_id": client_info.client_id,
        "name": client_info.name,
        "tier": format!("{:?}", client_info.tier),
        "providers": client_info.providers,
    })))
}
```

---

## Example 3: IP-Based Access Control

**Use Case**: Restrict API access to specific IP ranges (e.g., corporate VPN).

```rust
use axum::middleware::{self, Next};
use axum::http::Request;
use axum::extract::State;
use crate::middleware::auth::AuthState;

pub async fn setup_ip_restricted_auth() -> AuthState {
    let config = crate::middleware::auth::AuthConfig {
        required: true,
        // Only allow corporate network and staging
        ip_allowlist: Some(vec![
            "203.0.113.0/24".to_string(),      // Corporate HQ
            "198.51.100.0/24".to_string(),     // Corporate Remote
            "192.0.2.0/24".to_string(),        // Staging
            "10.0.0.0/8".to_string(),          // Internal network
        ]),
        // Explicitly block known hostile IPs
        ip_blocklist: Some(vec![
            "203.0.113.42".to_string(),        // Compromised machine
        ]),
        api_keys: Default::default(),
        ..Default::default()
    };

    AuthState::production(config)
}

pub async fn ip_check_middleware(
    State(auth): State<AuthState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let client_ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    // Check if blocked
    if auth.is_ip_blocked(client_ip) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "IP address is blocked"
            })),
        )
            .into_response();
    }

    // Check if allowed
    if !auth.is_ip_allowed(client_ip) {
        return (
            axum::http::StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "Access denied: IP not in allowlist"
            })),
        )
            .into_response();
    }

    next.run(req).await
}
```

---

## Example 4: Tiered Rate Limiting with Client Overrides

**Use Case**: Different rate limits per client tier, with special overrides for key accounts.

```rust
use std::sync::Arc;
use axum::middleware::{self, Next};
use axum::http::{Request, StatusCode};
use axum::extract::State;
use crate::middleware::rate_limit::{
    RateLimitState, RateLimitConfig, RateLimitTier, extract_client_key, classify_endpoint,
};

pub fn setup_rate_limiter() -> Arc<RateLimitState> {
    let config = RateLimitConfig::default();
    let state = Arc::new(RateLimitState::new(config));

    // Grant specific enterprise clients higher limits
    state.set_client_override("apikey:enterprise_customer_1", 20.0);  // 20x
    state.set_client_override("apikey:enterprise_customer_2", 10.0);  // 10x
    state.set_client_override("apikey:vip_partner", 5.0);             // 5x

    state
}

pub async fn rate_limit_middleware(
    State(rate_limiter): State<Arc<RateLimitState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let client_key = extract_client_key(req.headers());
    let endpoint_tier = classify_endpoint(req.uri().path());

    // Check rate limit
    let result = rate_limiter.check(&client_key, endpoint_tier);

    if !result.allowed {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("X-RateLimit-Limit", result.limit.to_string()),
                ("X-RateLimit-Remaining", "0".to_string()),
                ("Retry-After", format!("{}", result.retry_after.ceil() as u64)),
            ],
            axum::Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "retry_after_seconds": result.retry_after.ceil() as u64,
            })),
        )
            .into_response();
    }

    let mut response = next.run(req).await;

    // Add rate limit headers
    response.headers_mut().insert(
        "X-RateLimit-Limit",
        result.limit.to_string().parse().unwrap_or_default(),
    );
    response.headers_mut().insert(
        "X-RateLimit-Remaining",
        result.remaining.to_string().parse().unwrap_or_default(),
    );

    response
}
```

---

## Example 5: CORS Configuration for SPA

**Use Case**: Allow specific domains for a single-page application.

```rust
use crate::middleware::cors::{CorsState, CorsConfig};
use axum::http::Method;

pub fn setup_cors_for_spa() -> CorsState {
    let config = CorsConfig {
        // Allow exact domains and subdomains
        allowed_origins: vec![
            "https://app.example.com".to_string(),
            "https://*.example.com".to_string(),  // Subdomains
            "http://localhost:3000".to_string(),  // Local dev
        ],
        // Allow common SPA methods
        allowed_methods: vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ],
        // Allow headers SPA might need
        allowed_headers: vec![
            "content-type".to_string(),
            "authorization".to_string(),
            "x-api-key".to_string(),
            "x-request-id".to_string(),
            "accept".to_string(),
            "accept-language".to_string(),
        ],
        // Expose headers for client
        exposed_headers: vec![
            "x-ratelimit-limit".to_string(),
            "x-ratelimit-remaining".to_string(),
            "x-request-id".to_string(),
            "retry-after".to_string(),
            "content-type".to_string(),
        ],
        // Allow credentials (cookies)
        allow_credentials: true,
        // Cache preflight for 1 hour
        preflight_max_age_secs: 3600,
    };

    CorsState::new(config)
}

pub async fn cors_middleware(
    State(cors): State<CorsState>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    use axum::http::Method;

    let origin = req
        .headers()
        .get("Origin")
        .and_then(|v| v.to_str().ok());

    // Handle preflight
    if req.method() == Method::OPTIONS {
        let request_method = req
            .headers()
            .get("Access-Control-Request-Method")
            .and_then(|v| v.to_str().ok());

        match cors.handle_preflight(origin, request_method) {
            Ok(headers) => {
                let mut response = axum::response::Response::new(axum::body::Body::empty());
                *response.headers_mut() = headers;
                response.headers_mut().insert("Content-Length", "0".parse().unwrap());
                return response;
            }
            Err(status) => return (status, "").into_response(),
        }
    }

    // Apply CORS to response
    let mut response = next.run(req).await;
    cors.apply_cors_headers(response.headers_mut(), origin);
    response
}
```

---

## Example 6: Request ID Correlation

**Use Case**: Track requests across distributed system with unique IDs.

```rust
use crate::middleware::request_id::RequestIdState;
use axum::middleware::{self, Next};
use axum::http::Request;

pub fn setup_request_id() -> RequestIdState {
    RequestIdState::new("X-Request-ID".to_string())
}

pub async fn request_id_middleware(
    State(request_id_state): State<RequestIdState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    // Get or generate request ID
    let request_id = request_id_state.get_or_generate_id(req.headers());

    // Add to tracing span
    tracing::Span::current().record("request_id", &request_id);

    // Process request
    let mut response = next.run(req).await;

    // Add to response header
    if let Ok(header_value) = request_id_state.to_header_value(&request_id) {
        response.headers_mut().insert("X-Request-ID", header_value);
    }

    response
}

// In your logging setup:
pub fn setup_tracing_with_request_id() {
    use tracing_subscriber::fmt;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        // Include request_id in all logs
        .with_span_list(false)
        .json()
        .init();
}

// Usage in handlers:
pub async fn my_handler() {
    // request_id will be automatically included in logs from this span
    tracing::info!("Processing request");
    tracing::debug!("Debug info");
    tracing::warn!("Warning");
}
```

---

## Example 7: Complete Middleware Stack Setup

**Use Case**: Wire all middleware together in application startup.

```rust
use axum::{
    middleware,
    Router,
    extract::DefaultBodyLimit,
};
use std::sync::Arc;
use crate::middleware::{
    auth::{AuthState, AuthConfig},
    rate_limit::{RateLimitState, RateLimitConfig},
    cors::{CorsState, CorsConfig},
    request_id::RequestIdState,
};

pub struct AppState {
    auth: AuthState,
    rate_limiter: Arc<RateLimitState>,
    cors: CorsState,
    request_id: RequestIdState,
    // ... other app state
}

pub async fn setup_app() -> Router {
    // Setup auth
    let auth_config = AuthConfig {
        required: true,
        api_keys: load_api_keys(),
        jwt_secret: Some(std::env::var("JWT_SECRET").unwrap_or_default()),
        jwt_public_key: load_rs256_public_key(),
        ip_allowlist: load_ip_allowlist(),
        ip_blocklist: load_ip_blocklist(),
        public_paths: vec![
            "/health".to_string(),
            "/metrics".to_string(),
        ],
    };
    let auth = AuthState::production(auth_config);

    // Setup rate limiter
    let rate_limiter_config = RateLimitConfig::default();
    let rate_limiter = Arc::new(RateLimitState::new(rate_limiter_config));

    // Grant enterprise clients overrides
    rate_limiter.set_client_override("apikey:enterprise_1", 10.0);

    // Setup CORS
    let cors_config = CorsConfig::default();
    let cors = CorsState::new(cors_config);

    // Setup request ID
    let request_id = RequestIdState::default();

    let app_state = AppState {
        auth,
        rate_limiter,
        cors,
        request_id,
    };

    Router::new()
        // Public routes
        .route("/health", axum::routing::get(health_check))
        .route("/metrics", axum::routing::get(metrics))
        // Protected routes
        .route("/api/trading/place-order", axum::routing::post(place_order))
        .route("/api/profile", axum::routing::get(get_profile))
        // Middleware stack (applied in reverse order)
        .layer(middleware::from_fn_with_state(
            app_state.request_id.clone(),
            request_id_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            app_state.rate_limiter.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            app_state.cors.clone(),
            cors_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            app_state.auth.clone(),
            auth_middleware,
        ))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10MB max
        .with_state(app_state)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn metrics() -> &'static str {
    "# HELP...\n"
}
```

---

## Example 8: Client-Side Request Signing (Python)

**Use Case**: Example of how clients should sign requests for HMAC validation.

```python
import hmac
import hashlib
import base64
import requests

def sign_request(secret_key: str, payload: bytes) -> str:
    """
    Create HMAC-SHA256 signature for request payload.

    Args:
        secret_key: API secret key
        payload: Request body bytes

    Returns:
        Base64-encoded HMAC-SHA256 signature
    """
    signature = hmac.new(
        secret_key.encode(),
        payload,
        hashlib.sha256
    ).digest()

    return base64.b64encode(signature).decode()

def place_order(api_key: str, secret_key: str, order_data: dict):
    """
    Place order with HMAC signature.
    """
    import json

    # Prepare payload
    payload = json.dumps(order_data).encode()

    # Generate signature
    signature = sign_request(secret_key, payload)

    # Make request
    response = requests.post(
        "https://api.example.com/api/trading/place-order",
        json=order_data,
        headers={
            "X-API-Key": api_key,
            "X-Signature": signature,
            "Content-Type": "application/json",
        }
    )

    return response.json()

# Usage
if __name__ == "__main__":
    order = {
        "symbol": "ETH-USD",
        "quantity": 10,
        "price": 2500,
    }

    result = place_order(
        api_key="upp_k_abc123...",
        secret_key="upp_s_xyz789...",
        order_data=order
    )

    print(result)
```

---

## Example 9: Environment Configuration

**Use Case**: Load middleware configuration from environment variables.

```rust
use std::env;
use crate::middleware::auth::AuthConfig;
use crate::middleware::rate_limit::RateLimitConfig;

pub fn load_auth_config_from_env() -> AuthConfig {
    let ip_allowlist = env::var("AUTH_IP_ALLOWLIST")
        .ok()
        .map(|s| s.split(',').map(|ip| ip.trim().to_string()).collect());

    let ip_blocklist = env::var("AUTH_IP_BLOCKLIST")
        .ok()
        .map(|s| s.split(',').map(|ip| ip.trim().to_string()).collect());

    let jwt_public_key = env::var("JWT_PUBLIC_KEY").ok();

    AuthConfig {
        required: env::var("AUTH_REQUIRED")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(true),
        api_keys: load_api_keys_from_db(),
        jwt_secret: env::var("JWT_SECRET").ok(),
        jwt_public_key,
        ip_allowlist,
        ip_blocklist,
        public_paths: vec![
            "/health".to_string(),
            "/metrics".to_string(),
        ],
    }
}

pub fn load_rate_limit_config_from_env() -> RateLimitConfig {
    let mut config = RateLimitConfig::default();

    if let Ok(redis_url) = env::var("REDIS_URL") {
        config.redis_url = Some(redis_url);
    }

    if let Ok(sliding_window) = env::var("USE_SLIDING_WINDOW") {
        config.use_sliding_window = sliding_window.parse().unwrap_or(false);
    }

    config
}

// .env example:
/*
AUTH_REQUIRED=true
AUTH_IP_ALLOWLIST=10.0.0.0/8,172.16.0.0/12
AUTH_IP_BLOCKLIST=192.168.1.100
JWT_SECRET=your_jwt_secret_here
JWT_PUBLIC_KEY=base64_encoded_rsa_public_key
REDIS_URL=redis://localhost:6379
USE_SLIDING_WINDOW=false
*/
```

---

## Example 10: Integration Tests

**Use Case**: Test middleware with actual HTTP requests.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_rate_limit_header_on_success() {
        let app = setup_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("X-RateLimit-Limit"));
        assert!(response.headers().contains_key("X-RateLimit-Remaining"));
    }

    #[tokio::test]
    async fn test_rate_limit_exhaustion() {
        let app = setup_app().await;

        // Make max_burst requests
        for i in 0..200 {
            let request = Request::builder()
                .method("GET")
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();

            if i < 200 {
                assert_eq!(response.status(), StatusCode::OK);
            }
        }

        // Next request should be rate limited
        let request = Request::builder()
            .method("GET")
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn test_cors_preflight() {
        let app = setup_app().await;

        let request = Request::builder()
            .method("OPTIONS")
            .uri("/api/data")
            .header("Origin", "https://example.com")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("Access-Control-Allow-Origin"));
    }

    #[tokio::test]
    async fn test_request_id_injection() {
        let app = setup_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.headers().contains_key("X-Request-ID"));
        let request_id = response
            .headers()
            .get("X-Request-ID")
            .and_then(|v| v.to_str().ok())
            .unwrap();

        // Should be valid UUID
        assert_eq!(request_id.len(), 36);
        assert_eq!(request_id.matches('-').count(), 4);
    }
}
```

---

## Environment Variables Reference

```bash
# Authentication
AUTH_REQUIRED=true
AUTH_IP_ALLOWLIST=10.0.0.0/8,172.16.0.0/12
AUTH_IP_BLOCKLIST=192.168.1.100
JWT_SECRET=your_jwt_secret
JWT_PUBLIC_KEY=base64_encoded_public_key

# Rate Limiting
REDIS_URL=redis://localhost:6379
USE_SLIDING_WINDOW=false

# CORS
CORS_ALLOWED_ORIGINS=https://example.com,https://*.example.com
CORS_ALLOW_CREDENTIALS=true
CORS_MAX_AGE_SECS=3600

# Request ID
REQUEST_ID_HEADER=X-Request-ID
```

---

## Deployment Checklist

- [ ] Load all configs from environment variables
- [ ] Set `AUTH_REQUIRED=true` in production
- [ ] Configure IP allowlist for internal APIs
- [ ] Set RS256 public key for JWT validation
- [ ] Configure Redis URL for distributed rate limiting
- [ ] Set appropriate rate limit multipliers per client tier
- [ ] Configure CORS for your frontend domains
- [ ] Enable audit logging to monitoring system
- [ ] Test rate limit behavior under load
- [ ] Verify CORS headers on OPTIONS requests
- [ ] Validate request IDs appear in all logs
