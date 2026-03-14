// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Authentication middleware for the UPP Gateway.
//
// Supports multiple auth schemes:
//   - API Key (X-API-Key header) — simple key lookup
//   - JWT Bearer token — RS256/HS256 validation
//   - None (public) — for unauthenticated market data endpoints
//
// In dev mode, all requests pass through (auth is optional).
// In production, trading/portfolio endpoints require auth.

#![allow(dead_code)]

use axum::http::{header, HeaderMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use base64::Engine;

// ─── Auth Config ────────────────────────────────────────────

/// Authentication configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Whether auth is required (false = dev mode, all pass through).
    pub required: bool,
    /// Valid API keys mapped to their client info.
    pub api_keys: HashMap<String, ClientInfo>,
    /// JWT secret for HS256 (if using symmetric JWT).
    pub jwt_secret: Option<String>,
    /// Public endpoints that never require auth (glob patterns).
    pub public_paths: Vec<String>,
    /// IP allowlist (optional). If set, only these IPs can authenticate.
    pub ip_allowlist: Option<Vec<String>>,
    /// IP blocklist (optional). IPs in this list are always rejected.
    pub ip_blocklist: Option<Vec<String>>,
    /// JWT public key for RS256 validation (base64-encoded, optional).
    pub jwt_public_key: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            required: false, // Dev mode by default
            api_keys: HashMap::new(),
            jwt_secret: None,
            public_paths: vec![
                "/health".to_string(),
                "/ready".to_string(),
                "/metrics".to_string(),
                "/.well-known/upp".to_string(),
                "/upp/v1/discovery/*".to_string(),
                "/upp/v1/markets*".to_string(),
            ],
            ip_allowlist: None,
            ip_blocklist: None,
            jwt_public_key: None,
        }
    }
}

/// Information about an authenticated client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_id: String,
    pub name: String,
    pub tier: ClientTier,
    pub providers: Vec<String>, // Which providers this client can access
}

/// Client access tier — affects rate limits and capabilities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientTier {
    Free,
    Standard,
    Pro,
    Enterprise,
}

impl ClientTier {
    /// Get rate limit multiplier for this tier.
    /// Used to override base rate limits per tier.
    pub fn rate_limit_multiplier(&self) -> f64 {
        match self {
            ClientTier::Free => 0.5,      // Free gets 50% of base
            ClientTier::Standard => 1.0,  // Standard gets 100% of base
            ClientTier::Pro => 2.0,       // Pro gets 200% of base
            ClientTier::Enterprise => 10.0, // Enterprise gets 1000% of base
        }
    }
}

// ─── Auth Result ────────────────────────────────────────────

/// The result of authenticating a request.
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Authenticated with client info.
    Authenticated(ClientInfo),
    /// Public access (no auth provided, endpoint allows it).
    Public,
    /// Auth failed — should return 401.
    Unauthorized(String),
    /// Auth succeeded but insufficient permissions — 403.
    Forbidden(String),
}

// ─── Auth State ─────────────────────────────────────────────

/// Shared auth state, injected into handlers via AppState or extension.
#[derive(Clone)]
pub struct AuthState {
    config: Arc<AuthConfig>,
}

impl AuthState {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Create a dev-mode auth state that allows everything.
    pub fn dev_mode() -> Self {
        Self::new(AuthConfig::default())
    }

    /// Create a production-ready auth state with auth required and strict config.
    pub fn production(config: AuthConfig) -> Self {
        let mut prod_config = config;
        prod_config.required = true;
        Self::new(prod_config)
    }

    /// Authenticate a request based on its headers and path.
    pub fn authenticate(&self, headers: &HeaderMap, path: &str) -> AuthResult {
        // Dev mode: everything passes
        if !self.config.required {
            return AuthResult::Public;
        }

        // Check if path is public
        if self.is_public_path(path) {
            return AuthResult::Public;
        }

        // Try API key auth
        if let Some(key) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
            return self.auth_api_key(key);
        }

        // Try Bearer token auth
        if let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
            if let Some(token) = auth.strip_prefix("Bearer ") {
                return self.auth_bearer(token);
            }
        }

        // No credentials provided
        AuthResult::Unauthorized(
            "Authentication required. Provide X-API-Key header or Bearer token.".to_string()
        )
    }

    /// Check if a path matches any public path pattern.
    fn is_public_path(&self, path: &str) -> bool {
        for pattern in &self.config.public_paths {
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if path.starts_with(prefix) {
                    return true;
                }
            } else if path == pattern {
                return true;
            }
        }
        false
    }

    /// Authenticate via API key.
    fn auth_api_key(&self, key: &str) -> AuthResult {
        match self.config.api_keys.get(key) {
            Some(client) => AuthResult::Authenticated(client.clone()),
            None => AuthResult::Unauthorized("Invalid API key".to_string()),
        }
    }

    /// Authenticate via Bearer token (JWT).
    fn auth_bearer(&self, token: &str) -> AuthResult {
        // Basic JWT validation using jsonwebtoken crate
        let secret = match &self.config.jwt_secret {
            Some(s) => s,
            None => return AuthResult::Unauthorized("JWT auth not configured".to_string()),
        };

        let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());

        match jsonwebtoken::decode::<JwtClaims>(token, &key, &validation) {
            Ok(data) => {
                let claims = data.claims;
                AuthResult::Authenticated(ClientInfo {
                    client_id: claims.sub,
                    name: claims.name.unwrap_or_else(|| "JWT User".to_string()),
                    tier: claims.tier.unwrap_or(ClientTier::Standard),
                    providers: claims.providers.unwrap_or_default(),
                })
            }
            Err(e) => AuthResult::Unauthorized(format!("Invalid JWT: {}", e)),
        }
    }

    /// Check if an authenticated client can access a specific provider.
    pub fn can_access_provider(&self, client: &ClientInfo, provider_id: &str) -> bool {
        // Empty providers list means access to all
        client.providers.is_empty() || client.providers.contains(&provider_id.to_string())
    }

    /// Check if a path requires authentication.
    pub fn require_auth_for_path(&self, path: &str) -> bool {
        !self.is_public_path(path)
    }

    /// Validate HMAC-SHA256 request signature for trading endpoints.
    /// Expects X-Signature header with format: base64(HMAC-SHA256(secret, payload))
    pub fn validate_request_signature(&self, secret: &str, payload: &[u8], signature: &str) -> bool {
        use sha2::Sha256;
        use hmac::{Hmac, Mac};

        type HmacSha256 = Hmac<Sha256>;

        let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };

        mac.update(payload);
        let expected = mac.finalize();
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(expected.into_bytes());

        expected_b64 == signature
    }

    /// Validate RS256 JWT token with a public key.
    pub fn validate_rs256_token(&self, token: &str) -> Option<ClientInfo> {
        let public_key = match &self.config.jwt_public_key {
            Some(key_b64) => {
                let key_bytes = match base64::engine::general_purpose::STANDARD.decode(key_b64) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        warn!("Failed to decode JWT public key");
                        return None;
                    }
                };
                match jsonwebtoken::DecodingKey::from_rsa_pem(&key_bytes) {
                    Ok(key) => key,
                    Err(e) => {
                        warn!("Failed to load RSA public key: {}", e);
                        return None;
                    }
                }
            }
            None => {
                warn!("RS256 validation requested but no public key configured");
                return None;
            }
        };

        let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);

        match jsonwebtoken::decode::<JwtClaims>(token, &public_key, &validation) {
            Ok(data) => {
                let claims = data.claims;
                Some(ClientInfo {
                    client_id: claims.sub,
                    name: claims.name.unwrap_or_else(|| "JWT User".to_string()),
                    tier: claims.tier.unwrap_or(ClientTier::Standard),
                    providers: claims.providers.unwrap_or_default(),
                })
            }
            Err(e) => {
                warn!("RS256 token validation failed: {}", e);
                None
            }
        }
    }

    /// Check if an IP is allowed to authenticate.
    /// Returns true if:
    /// - No allowlist is configured, OR
    /// - The IP is in the allowlist.
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        if let Some(ref allowlist) = self.config.ip_allowlist {
            return allowlist.iter().any(|allowed| {
                allowed == ip || (allowed.ends_with('*') && ip.starts_with(&allowed[..allowed.len() - 1]))
            });
        }
        true
    }

    /// Check if an IP is blocked.
    pub fn is_ip_blocked(&self, ip: &str) -> bool {
        if let Some(ref blocklist) = self.config.ip_blocklist {
            return blocklist.iter().any(|blocked| {
                blocked == ip || (blocked.ends_with('*') && ip.starts_with(&blocked[..blocked.len() - 1]))
            });
        }
        false
    }

    /// Extract client IP from headers (X-Forwarded-For > X-Real-IP fallback).
    fn extract_client_ip(&self, headers: &HeaderMap) -> Option<String> {
        // Try X-Forwarded-For first (take first IP)
        if let Some(xff) = headers.get("X-Forwarded-For").and_then(|v| v.to_str().ok()) {
            if let Some(first_ip) = xff.split(',').next() {
                return Some(first_ip.trim().to_string());
            }
        }

        // Fallback to X-Real-IP
        if let Some(real_ip) = headers.get("X-Real-IP").and_then(|v| v.to_str().ok()) {
            return Some(real_ip.to_string());
        }

        None
    }

    /// Log an authentication attempt (audit log).
    fn audit_log(&self, client_id: &str, ip: &str, path: &str, success: bool) {
        if success {
            info!(client_id = %client_id, ip = %ip, path = %path, "Auth attempt succeeded");
        } else {
            warn!(client_id = %client_id, ip = %ip, path = %path, "Auth attempt failed");
        }
    }
}

/// JWT claims structure.
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,                       // Client ID
    exp: usize,                        // Expiration
    name: Option<String>,              // Client name
    tier: Option<ClientTier>,          // Access tier
    providers: Option<Vec<String>>,    // Allowed providers
}

// ─── API Key Management ───────────────────────────────────

/// Manages API key lifecycle: create, list, revoke, rotate.
/// Thread-safe via DashMap for concurrent access.
#[derive(Clone)]
pub struct ApiKeyManager {
    keys: Arc<dashmap::DashMap<String, ApiKeyRecord>>,
}

/// A stored API key record with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    /// The API key itself (stored hashed in production, plaintext in dev).
    pub key_prefix: String,
    /// The full key hash (for lookup). In dev mode this is the plaintext key.
    pub key_hash: String,
    /// Associated client info.
    pub client: ClientInfo,
    /// When the key was created.
    pub created_at: String,
    /// When the key expires (None = never).
    pub expires_at: Option<String>,
    /// Whether this key is active.
    pub active: bool,
    /// Human-readable label.
    pub label: Option<String>,
}

/// Request to create a new API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub client_name: String,
    pub tier: Option<ClientTier>,
    pub providers: Option<Vec<String>>,
    pub label: Option<String>,
    pub expires_in_days: Option<u32>,
}

/// Response after creating an API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    pub key: String,           // Only returned once at creation
    pub key_prefix: String,    // e.g. "upp_k_abc1..."
    pub client_id: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

/// Summary of an API key (without the full key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeySummary {
    pub key_prefix: String,
    pub client_id: String,
    pub client_name: String,
    pub tier: ClientTier,
    pub label: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub expires_at: Option<String>,
}

impl Default for ApiKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiKeyManager {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Create a new API key with a unique ID and random secret.
    pub fn create_key(&self, req: CreateApiKeyRequest) -> CreateApiKeyResponse {
        let client_id = format!("client_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0000"));
        let key_secret = uuid::Uuid::new_v4().to_string().replace('-', "");
        let full_key = format!("upp_k_{}", key_secret);
        let key_prefix = format!("upp_k_{}...", &key_secret[..8]);

        let now = chrono::Utc::now();
        let created_at = now.to_rfc3339();
        #[allow(deprecated)]
        let expires_at = req.expires_in_days.map(|days| {
            (now + chrono::Duration::days(days as i64)).to_rfc3339()
        });

        let record = ApiKeyRecord {
            key_prefix: key_prefix.clone(),
            key_hash: full_key.clone(),
            client: ClientInfo {
                client_id: client_id.clone(),
                name: req.client_name,
                tier: req.tier.unwrap_or(ClientTier::Standard),
                providers: req.providers.unwrap_or_default(),
            },
            created_at: created_at.clone(),
            expires_at: expires_at.clone(),
            active: true,
            label: req.label,
        };

        self.keys.insert(full_key.clone(), record);

        CreateApiKeyResponse {
            key: full_key,
            key_prefix,
            client_id,
            created_at,
            expires_at,
        }
    }

    /// List all API keys (without exposing the full key).
    pub fn list_keys(&self) -> Vec<ApiKeySummary> {
        self.keys
            .iter()
            .map(|entry| {
                let r = entry.value();
                ApiKeySummary {
                    key_prefix: r.key_prefix.clone(),
                    client_id: r.client.client_id.clone(),
                    client_name: r.client.name.clone(),
                    tier: r.client.tier,
                    label: r.label.clone(),
                    active: r.active,
                    created_at: r.created_at.clone(),
                    expires_at: r.expires_at.clone(),
                }
            })
            .collect()
    }

    /// Revoke an API key by its prefix.
    pub fn revoke_by_prefix(&self, prefix: &str) -> bool {
        let mut revoked = false;
        for mut entry in self.keys.iter_mut() {
            if entry.value().key_prefix == prefix {
                entry.value_mut().active = false;
                revoked = true;
            }
        }
        revoked
    }

    /// Look up a client by their full API key.
    pub fn authenticate_key(&self, key: &str) -> Option<ClientInfo> {
        self.keys.get(key).and_then(|record| {
            if record.active {
                // Check expiration
                if let Some(ref exp) = record.expires_at {
                    if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(exp) {
                        let exp_utc = exp_time.with_timezone(&chrono::Utc);
                        if chrono::Utc::now() > exp_utc {
                            return None;
                        }
                    }
                }
                Some(record.client.clone())
            } else {
                None
            }
        })
    }

    /// Get total key count.
    pub fn count(&self) -> usize {
        self.keys.len()
    }

    /// Get active key count.
    pub fn active_count(&self) -> usize {
        self.keys.iter().filter(|e| e.value().active).count()
    }

    /// Rotate an API key — creates new key, marks old key to expire in 24 hours (grace period).
    /// Returns (new_key_response, old_key_prefix) if successful, or None if old key not found.
    pub fn rotate_key(
        &self,
        old_key_prefix: &str,
        req: CreateApiKeyRequest,
    ) -> Option<(CreateApiKeyResponse, String)> {
        // Find and validate the old key
        let mut old_key_hash = None;
        for entry in self.keys.iter() {
            if entry.value().key_prefix == old_key_prefix {
                old_key_hash = Some(entry.key().clone());
                break;
            }
        }

        let old_key_hash = old_key_hash?;

        // Get the old key record for client info
        let old_record = self.keys.get(&old_key_hash)?.clone();

        // Create new key with the same client info (but allow overriding tier/providers/label)
        let client_id = old_record.client.client_id.clone();
        let key_secret = uuid::Uuid::new_v4().to_string().replace('-', "");
        let full_key = format!("upp_k_{}", key_secret);
        let key_prefix = format!("upp_k_{}...", &key_secret[..8]);

        let now = chrono::Utc::now();
        let created_at = now.to_rfc3339();
        #[allow(deprecated)]
        let new_expires_at = req.expires_in_days.map(|days| {
            (now + chrono::Duration::days(days as i64)).to_rfc3339()
        });

        let new_record = ApiKeyRecord {
            key_prefix: key_prefix.clone(),
            key_hash: full_key.clone(),
            client: ClientInfo {
                client_id: client_id.clone(),
                name: req.client_name,
                tier: req.tier.unwrap_or(old_record.client.tier),
                providers: req.providers.unwrap_or(old_record.client.providers.clone()),
            },
            created_at: created_at.clone(),
            expires_at: new_expires_at.clone(),
            active: true,
            label: req.label,
        };

        // Insert new key
        self.keys.insert(full_key.clone(), new_record);

        // Set old key to expire in 24 hours (grace period)
        if let Some(mut old_entry) = self.keys.get_mut(&old_key_hash) {
            #[allow(deprecated)]
            let grace_period_expiry =
                (now + chrono::Duration::days(1)).to_rfc3339();
            old_entry.expires_at = Some(grace_period_expiry);
        }

        let response = CreateApiKeyResponse {
            key: full_key,
            key_prefix,
            client_id,
            created_at,
            expires_at: new_expires_at,
        };

        Some((response, old_key_prefix.to_string()))
    }

    /// Remove all expired keys and return count removed.
    pub fn cleanup_expired(&self) -> usize {
        let now = chrono::Utc::now();
        let mut removed_count = 0;

        let mut to_remove = Vec::new();

        for entry in self.keys.iter() {
            if let Some(ref exp) = entry.value().expires_at {
                if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(exp) {
                    let exp_utc = exp_time.with_timezone(&chrono::Utc);
                    if now > exp_utc {
                        to_remove.push(entry.key().clone());
                    }
                }
            }
        }

        for key in to_remove {
            self.keys.remove(&key);
            removed_count += 1;
        }

        removed_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dev_mode_auth_passes_all() {
        let state = AuthState::dev_mode();
        let headers = HeaderMap::new();
        match state.authenticate(&headers, "/upp/v1/orders") {
            AuthResult::Public => {} // expected
            other => panic!("Expected Public, got {:?}", other),
        }
    }

    #[test]
    fn test_production_auth_rejects_missing_creds() {
        let config = AuthConfig {
            required: true,
            ..Default::default()
        };
        let state = AuthState::new(config);
        let headers = HeaderMap::new();
        match state.authenticate(&headers, "/upp/v1/orders") {
            AuthResult::Unauthorized(_) => {} // expected
            other => panic!("Expected Unauthorized, got {:?}", other),
        }
    }

    #[test]
    fn test_production_auth_allows_public_paths() {
        let config = AuthConfig {
            required: true,
            ..Default::default()
        };
        let state = AuthState::new(config);
        let headers = HeaderMap::new();
        match state.authenticate(&headers, "/health") {
            AuthResult::Public => {} // expected
            other => panic!("Expected Public for /health, got {:?}", other),
        }
    }

    #[test]
    fn test_production_auth_api_key() {
        let mut keys = HashMap::new();
        keys.insert("test-key-123".to_string(), ClientInfo {
            client_id: "test-client".to_string(),
            name: "Test".to_string(),
            tier: ClientTier::Pro,
            providers: vec![],
        });
        let config = AuthConfig {
            required: true,
            api_keys: keys,
            ..Default::default()
        };
        let state = AuthState::new(config);
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", "test-key-123".parse().unwrap());
        match state.authenticate(&headers, "/upp/v1/orders") {
            AuthResult::Authenticated(client) => {
                assert_eq!(client.client_id, "test-client");
                assert_eq!(client.tier, ClientTier::Pro);
            }
            other => panic!("Expected Authenticated, got {:?}", other),
        }
    }

    #[test]
    fn test_production_auth_rejects_invalid_key() {
        let config = AuthConfig {
            required: true,
            api_keys: HashMap::new(),
            ..Default::default()
        };
        let state = AuthState::new(config);
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", "bad-key".parse().unwrap());
        match state.authenticate(&headers, "/upp/v1/orders") {
            AuthResult::Unauthorized(_) => {} // expected
            other => panic!("Expected Unauthorized, got {:?}", other),
        }
    }

    #[test]
    fn test_api_key_manager_create() {
        let mgr = ApiKeyManager::new();
        let resp = mgr.create_key(CreateApiKeyRequest {
            client_name: "Test Client".to_string(),
            tier: Some(ClientTier::Pro),
            providers: None,
            label: Some("dev key".to_string()),
            expires_in_days: None,
        });
        assert!(resp.key.starts_with("upp_k_"));
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn test_api_key_manager_authenticate() {
        let mgr = ApiKeyManager::new();
        let resp = mgr.create_key(CreateApiKeyRequest {
            client_name: "Auth Test".to_string(),
            tier: None,
            providers: None,
            label: None,
            expires_in_days: None,
        });
        let client = mgr.authenticate_key(&resp.key).expect("Should authenticate");
        assert_eq!(client.name, "Auth Test");
        assert_eq!(client.tier, ClientTier::Standard);
    }

    #[test]
    fn test_api_key_manager_revoke() {
        let mgr = ApiKeyManager::new();
        let resp = mgr.create_key(CreateApiKeyRequest {
            client_name: "Revoke Test".to_string(),
            tier: None,
            providers: None,
            label: None,
            expires_in_days: None,
        });
        assert!(mgr.revoke_by_prefix(&resp.key_prefix));
        assert_eq!(mgr.active_count(), 0);
        assert!(mgr.authenticate_key(&resp.key).is_none());
    }

    #[test]
    fn test_api_key_manager_list() {
        let mgr = ApiKeyManager::new();
        mgr.create_key(CreateApiKeyRequest {
            client_name: "Client A".to_string(),
            tier: Some(ClientTier::Free),
            providers: None,
            label: Some("key-a".to_string()),
            expires_in_days: None,
        });
        mgr.create_key(CreateApiKeyRequest {
            client_name: "Client B".to_string(),
            tier: Some(ClientTier::Enterprise),
            providers: Some(vec!["kalshi".to_string()]),
            label: None,
            expires_in_days: Some(30),
        });
        let keys = mgr.list_keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().any(|k| k.client_name == "Client A"));
        assert!(keys.iter().any(|k| k.client_name == "Client B"));
    }

    #[test]
    fn test_api_key_manager_provider_access() {
        let state = AuthState::dev_mode();
        let client_all = ClientInfo {
            client_id: "all".to_string(),
            name: "All".to_string(),
            tier: ClientTier::Pro,
            providers: vec![],
        };
        assert!(state.can_access_provider(&client_all, "kalshi"));
        assert!(state.can_access_provider(&client_all, "polymarket"));

        let client_limited = ClientInfo {
            client_id: "limited".to_string(),
            name: "Limited".to_string(),
            tier: ClientTier::Standard,
            providers: vec!["kalshi".to_string()],
        };
        assert!(state.can_access_provider(&client_limited, "kalshi"));
        assert!(!state.can_access_provider(&client_limited, "polymarket"));
    }

    #[test]
    fn test_api_key_rotation() {
        let mgr = ApiKeyManager::new();

        // Create initial key
        let resp1 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Rotation Test".to_string(),
            tier: Some(ClientTier::Pro),
            providers: Some(vec!["kalshi".to_string(), "polymarket".to_string()]),
            label: Some("original-key".to_string()),
            expires_in_days: None,
        });

        let old_prefix = resp1.key_prefix.clone();
        let old_key = resp1.key.clone();

        // Verify old key works
        assert!(mgr.authenticate_key(&old_key).is_some());
        assert_eq!(mgr.count(), 1);

        // Rotate the key
        let rotate_result = mgr.rotate_key(
            &old_prefix,
            CreateApiKeyRequest {
                client_name: "Rotation Test".to_string(),
                tier: Some(ClientTier::Enterprise),
                providers: Some(vec!["opinion".to_string()]),
                label: Some("rotated-key".to_string()),
                expires_in_days: Some(90),
            },
        );

        assert!(rotate_result.is_some());
        let (new_resp, returned_old_prefix) = rotate_result.unwrap();
        assert_eq!(returned_old_prefix, old_prefix);
        assert_ne!(new_resp.key, old_key);

        // Verify new key works and has updated properties
        let client = mgr.authenticate_key(&new_resp.key).expect("New key should work");
        assert_eq!(client.tier, ClientTier::Enterprise);
        assert_eq!(client.providers, vec!["opinion".to_string()]);

        // Verify we have both keys (2 total)
        assert_eq!(mgr.count(), 2);

        // Old key should still work (during grace period)
        assert!(mgr.authenticate_key(&old_key).is_some());
    }

    #[test]
    fn test_api_key_rotation_grace_period() {
        // Grace period uses 24-hour expiry; not time-dependent in this test

        let mgr = ApiKeyManager::new();

        // Create initial key
        let resp1 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Grace Period Test".to_string(),
            tier: Some(ClientTier::Standard),
            providers: None,
            label: None,
            expires_in_days: None,
        });

        let old_key = resp1.key.clone();
        let old_prefix = resp1.key_prefix.clone();

        // Rotate the key (24-hour grace period)
        let rotate_result = mgr.rotate_key(
            &old_prefix,
            CreateApiKeyRequest {
                client_name: "Grace Period Test".to_string(),
                tier: None,
                providers: None,
                label: None,
                expires_in_days: Some(30),
            },
        );

        assert!(rotate_result.is_some());
        let (new_resp, _) = rotate_result.unwrap();

        // Old key should still authenticate (grace period)
        assert!(mgr.authenticate_key(&old_key).is_some());

        // New key should authenticate
        assert!(mgr.authenticate_key(&new_resp.key).is_some());

        // Both keys should exist
        assert_eq!(mgr.count(), 2);
    }

    #[test]
    fn test_cleanup_expired() {
        let mgr = ApiKeyManager::new();

        // Create a key that expires immediately (in the past)
        let resp1 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Expiring Key".to_string(),
            tier: Some(ClientTier::Free),
            providers: None,
            label: None,
            expires_in_days: Some(0), // Expires now
        });

        // Create a key that expires in the future
        let resp2 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Active Key".to_string(),
            tier: Some(ClientTier::Standard),
            providers: None,
            label: None,
            expires_in_days: Some(30),
        });

        assert_eq!(mgr.count(), 2);

        // Wait a moment to ensure the expires_in_days=0 key is in the past
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Cleanup expired keys
        let removed = mgr.cleanup_expired();

        // Should have removed at least 1 key (the one with 0 expiry)
        assert_eq!(removed, 1);
        assert_eq!(mgr.count(), 1);

        // The remaining key should be the non-expiring one
        assert!(mgr.authenticate_key(&resp2.key).is_some());
        assert!(mgr.authenticate_key(&resp1.key).is_none());
    }

    #[test]
    fn test_rotate_key_not_found() {
        let mgr = ApiKeyManager::new();

        // Try to rotate a key that doesn't exist
        let result = mgr.rotate_key(
            "upp_k_nonexistent...",
            CreateApiKeyRequest {
                client_name: "Test".to_string(),
                tier: None,
                providers: None,
                label: None,
                expires_in_days: None,
            },
        );

        assert!(result.is_none());
        assert_eq!(mgr.count(), 0);
    }
}
