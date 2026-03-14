// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// CORS middleware for the UPP Gateway.
//
// Provides configurable CORS headers based on client tier and allowed origins.
// Supports preflight caching, credentials, and method restrictions.

#![allow(dead_code)]

use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use std::sync::Arc;

/// CORS configuration.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origin patterns (e.g., "https://example.com", "https://*.example.com", "*").
    pub allowed_origins: Vec<String>,
    /// Allowed HTTP methods.
    pub allowed_methods: Vec<Method>,
    /// Allowed request headers.
    pub allowed_headers: Vec<String>,
    /// Exposed response headers.
    pub exposed_headers: Vec<String>,
    /// Whether to allow credentials (cookies, auth headers).
    pub allow_credentials: bool,
    /// Preflight cache duration in seconds.
    pub preflight_max_age_secs: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::OPTIONS,
            ],
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

/// CORS state for handling CORS requests.
#[derive(Clone)]
pub struct CorsState {
    config: Arc<CorsConfig>,
}

impl CorsState {
    pub fn new(config: CorsConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Check if an origin is allowed.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        self.config.allowed_origins.iter().any(|allowed| {
            if allowed == "*" {
                return true;
            }
            if let Some(star_pos) = allowed.find('*') {
                let prefix = &allowed[..star_pos];
                let suffix = &allowed[star_pos + 1..];
                return origin.starts_with(prefix) && origin.ends_with(suffix) && origin.len() >= prefix.len() + suffix.len();
            }
            allowed == origin
        })
    }

    /// Get the Access-Control-Allow-Origin header value.
    pub fn get_allow_origin(&self, origin: Option<&str>) -> Option<HeaderValue> {
        let origin = origin?;
        if self.is_origin_allowed(origin) {
            HeaderValue::from_str(origin).ok()
        } else {
            None
        }
    }

    /// Get the Access-Control-Allow-Methods header value.
    pub fn get_allow_methods(&self) -> String {
        self.config
            .allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Get the Access-Control-Allow-Headers header value.
    pub fn get_allow_headers(&self) -> String {
        self.config.allowed_headers.join(", ")
    }

    /// Get the Access-Control-Expose-Headers header value.
    pub fn get_expose_headers(&self) -> String {
        self.config.exposed_headers.join(", ")
    }

    /// Handle a preflight request (OPTIONS).
    pub fn handle_preflight(
        &self,
        origin: Option<&str>,
        request_method: Option<&str>,
    ) -> Result<HeaderMap, StatusCode> {
        let origin = origin.ok_or(StatusCode::FORBIDDEN)?;

        if !self.is_origin_allowed(origin) {
            return Err(StatusCode::FORBIDDEN);
        }

        if let Some(method_str) = request_method {
            let method = Method::from_bytes(method_str.as_bytes())
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            if !self.config.allowed_methods.contains(&method) {
                return Err(StatusCode::METHOD_NOT_ALLOWED);
            }
        }

        let mut headers = HeaderMap::new();

        if let Ok(origin_header) = HeaderValue::from_str(origin) {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin_header);
        }

        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            self.get_allow_methods().parse().unwrap_or_else(|_| HeaderValue::from_static("")),
        );

        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            self.get_allow_headers().parse().unwrap_or_else(|_| HeaderValue::from_static("")),
        );

        if self.config.allow_credentials {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
        }

        headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            self.config.preflight_max_age_secs.to_string().parse().unwrap_or_else(|_| HeaderValue::from_static("")),
        );

        Ok(headers)
    }

    /// Apply CORS headers to a response.
    pub fn apply_cors_headers(&self, headers: &mut HeaderMap, origin: Option<&str>) {
        if let Some(origin) = origin {
            if let Some(allow_origin) = self.get_allow_origin(Some(origin)) {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin);
            }
        }

        if !self.config.exposed_headers.is_empty() {
            if let Ok(expose_headers) = self.get_expose_headers().parse() {
                headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, expose_headers);
            }
        }

        if self.config.allow_credentials {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_origin_allowed_exact() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            ..Default::default()
        };
        let state = CorsState::new(config);
        assert!(state.is_origin_allowed("https://example.com"));
        assert!(!state.is_origin_allowed("https://other.com"));
    }

    #[test]
    fn test_cors_origin_allowed_wildcard() {
        let config = CorsConfig {
            allowed_origins: vec!["https://*.example.com".to_string()],
            ..Default::default()
        };
        let state = CorsState::new(config);
        assert!(state.is_origin_allowed("https://sub.example.com"));
        assert!(state.is_origin_allowed("https://deep.sub.example.com"));
        assert!(!state.is_origin_allowed("https://example.com"));
    }

    #[test]
    fn test_cors_origin_allowed_all() {
        let config = CorsConfig {
            allowed_origins: vec!["*".to_string()],
            ..Default::default()
        };
        let state = CorsState::new(config);
        assert!(state.is_origin_allowed("https://any.example.com"));
        assert!(state.is_origin_allowed("http://localhost:3000"));
    }

    #[test]
    fn test_cors_preflight_success() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            allowed_methods: vec![Method::GET, Method::POST],
            ..Default::default()
        };
        let state = CorsState::new(config);
        let headers = state.handle_preflight(
            Some("https://example.com"),
            Some("POST"),
        );
        assert!(headers.is_ok());
    }

    #[test]
    fn test_cors_preflight_forbidden_origin() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            ..Default::default()
        };
        let state = CorsState::new(config);
        let result = state.handle_preflight(
            Some("https://malicious.com"),
            Some("GET"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_cors_preflight_missing_origin() {
        let config = CorsConfig::default();
        let state = CorsState::new(config);
        let result = state.handle_preflight(None, Some("GET"));
        assert!(result.is_err());
    }

    #[test]
    fn test_cors_get_allow_methods() {
        let config = CorsConfig {
            allowed_methods: vec![Method::GET, Method::POST, Method::DELETE],
            ..Default::default()
        };
        let state = CorsState::new(config);
        let methods = state.get_allow_methods();
        assert!(methods.contains("GET"));
        assert!(methods.contains("POST"));
        assert!(methods.contains("DELETE"));
    }

    #[test]
    fn test_cors_apply_headers() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            allow_credentials: true,
            ..Default::default()
        };
        let state = CorsState::new(config);
        let mut headers = HeaderMap::new();
        state.apply_cors_headers(&mut headers, Some("https://example.com"));

        assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert!(headers.contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS));
    }
}
