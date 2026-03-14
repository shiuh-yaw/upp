// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Request ID middleware for the UPP Gateway.
//
// Generates UUID v4 request IDs and injects them into response headers
// and tracing spans for request correlation and debugging.

#![allow(dead_code)]

use axum::http::HeaderValue;

/// Request ID state for generating and managing request identifiers.
#[derive(Clone)]
pub struct RequestIdState {
    /// Header name for request IDs (default: X-Request-ID).
    pub header_name: String,
}

impl Default for RequestIdState {
    fn default() -> Self {
        Self {
            header_name: "X-Request-ID".to_string(),
        }
    }
}

impl RequestIdState {
    pub fn new(header_name: String) -> Self {
        Self { header_name }
    }

    /// Generate a new UUID v4 request ID.
    pub fn generate_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Extract request ID from headers or generate a new one.
    pub fn get_or_generate_id(&self, headers: &axum::http::HeaderMap) -> String {
        headers
            .get(&self.header_name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.generate_id())
    }

    /// Create a header value from a request ID string.
    pub fn to_header_value(&self, request_id: &str) -> Option<HeaderValue> {
        HeaderValue::from_str(request_id).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_format() {
        let state = RequestIdState::default();
        let id = state.generate_id();

        // Should be a valid UUID v4 format (36 chars with hyphens)
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);

        // Should be valid UUID
        let parsed = uuid::Uuid::parse_str(&id);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let state = RequestIdState::default();
        let id1 = state.generate_id();
        let id2 = state.generate_id();

        // Each generated ID should be unique
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_get_or_generate_from_headers() {
        let state = RequestIdState::default();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-Request-ID", "my-request-id-123".parse().unwrap());

        let id = state.get_or_generate_id(&headers);
        assert_eq!(id, "my-request-id-123");
    }

    #[test]
    fn test_get_or_generate_when_missing() {
        let state = RequestIdState::default();
        let headers = axum::http::HeaderMap::new();

        let id = state.get_or_generate_id(&headers);

        // Should generate a new ID if not present
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);
    }

    #[test]
    fn test_to_header_value() {
        let state = RequestIdState::default();
        let id = "550e8400-e29b-41d4-a716-446655440000";

        let header_value = state.to_header_value(id);
        assert!(header_value.is_some());

        let header_value = header_value.unwrap();
        assert_eq!(header_value.to_str().unwrap(), id);
    }

    #[test]
    fn test_custom_header_name() {
        let state = RequestIdState::new("X-Custom-ID".to_string());
        assert_eq!(state.header_name, "X-Custom-ID");

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-Custom-ID", "custom-123".parse().unwrap());

        let id = state.get_or_generate_id(&headers);
        assert_eq!(id, "custom-123");
    }

    #[test]
    fn test_to_header_value_preserves_format() {
        let state = RequestIdState::default();
        let id = "f47ac10b-58cc-4372-a567-0e02b2c3d479";

        let header_value = state.to_header_value(id).unwrap();
        assert_eq!(header_value.as_bytes(), id.as_bytes());
    }
}
