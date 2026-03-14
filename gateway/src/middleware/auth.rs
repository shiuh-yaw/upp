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
}
