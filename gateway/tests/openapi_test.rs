// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Tests validating OpenAPI spec consistency and coverage

use upp_gateway::test_harness::TestServer;

/// Helper to start a test server
async fn setup() -> TestServer {
    upp_gateway::test_harness::start_test_server().await
}

/// Test 1: Validate that the static OpenAPI JSON is valid
#[tokio::test]
async fn openapi_spec_is_valid_json() {
    let server = setup().await;

    // Fetch the OpenAPI spec from the test server
    let response = reqwest::Client::new()
        .get(format!("{}/docs/openapi.json", server.base_url))
        .send()
        .await
        .expect("Failed to fetch openapi.json");

    assert_eq!(
        response.status(),
        200,
        "OpenAPI endpoint should return 200 OK"
    );

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("application/json"),
        "Content-Type should be application/json"
    );

    // Parse as JSON and ensure it's valid
    let spec_text = response
        .text()
        .await
        .expect("Failed to read response body");
    let spec: serde_json::Value =
        serde_json::from_str(&spec_text).expect("OpenAPI spec should be valid JSON");

    // Verify required top-level keys
    assert!(
        spec.get("openapi").is_some(),
        "OpenAPI spec must have 'openapi' field"
    );
    assert!(
        spec.get("info").is_some(),
        "OpenAPI spec must have 'info' field"
    );
    assert!(
        spec.get("paths").is_some(),
        "OpenAPI spec must have 'paths' field"
    );
}

/// Test 2: Validate that the spec contains paths for key endpoints
#[tokio::test]
async fn openapi_spec_has_required_paths() {
    let server = setup().await;

    let response = reqwest::Client::new()
        .get(format!("{}/docs/openapi.json", server.base_url))
        .send()
        .await
        .expect("Failed to fetch openapi.json");

    let spec: serde_json::Value = serde_json::from_str(
        &response
            .text()
            .await
            .expect("Failed to read response body"),
    )
    .expect("OpenAPI spec should be valid JSON");

    let paths = spec
        .get("paths")
        .and_then(|p| p.as_object())
        .expect("paths should be an object");

    // Check for required paths
    let required_paths = vec!["/health", "/upp/v1/markets", "/upp/v1/arbitrage"];

    for path in required_paths {
        assert!(
            paths.contains_key(path),
            "OpenAPI spec must include path: {}",
            path
        );
    }
}

/// Test 3: Validate that the OpenAPI spec version matches the Cargo.toml version
#[tokio::test]
async fn openapi_spec_version_matches() {
    let server = setup().await;

    let response = reqwest::Client::new()
        .get(format!("{}/docs/openapi.json", server.base_url))
        .send()
        .await
        .expect("Failed to fetch openapi.json");

    let spec: serde_json::Value = serde_json::from_str(
        &response
            .text()
            .await
            .expect("Failed to read response body"),
    )
    .expect("OpenAPI spec should be valid JSON");

    let spec_version = spec
        .get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .expect("OpenAPI info.version should be a string");

    // Expected version from Cargo.toml is "0.1.0"
    let expected_version = "0.1.0";
    assert_eq!(
        spec_version, expected_version,
        "OpenAPI spec version should match Cargo.toml version"
    );
}

/// Test 4: Validate that the static file matches what the server returns
#[tokio::test]
async fn openapi_static_file_matches_served() {
    let server = setup().await;

    // Get the served spec from the server
    let response = reqwest::Client::new()
        .get(format!("{}/docs/openapi.json", server.base_url))
        .send()
        .await
        .expect("Failed to fetch openapi.json");

    let served_text = response
        .text()
        .await
        .expect("Failed to read response body");

    // Load the static file directly via include_str
    let static_text = include_str!("../static/openapi.json");

    // Parse both as JSON to compare semantically (ignoring whitespace differences)
    let served_json: serde_json::Value =
        serde_json::from_str(&served_text).expect("Served spec should be valid JSON");
    let static_json: serde_json::Value =
        serde_json::from_str(static_text).expect("Static spec should be valid JSON");

    assert_eq!(
        served_json, static_json,
        "Served OpenAPI spec should match the static file"
    );
}
