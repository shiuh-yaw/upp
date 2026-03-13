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
