// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// WebSocket integration tests for the UPP gateway.
// Tests WebSocket connections, echo functionality, and UPP subscribe protocol.

use upp_gateway::test_harness::start_test_server;
use tokio_tungstenite::connect_async;
use futures::stream::StreamExt;
use futures::SinkExt;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn ws_connect_and_echo() {
    let server = start_test_server().await;
    let ws_url = format!("ws://127.0.0.1:{}/ws", server.addr.port());

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Send a text message
    let test_msg = "Hello, WebSocket!";
    write
        .send(Message::Text(test_msg.to_string()))
        .await
        .expect("Failed to send message");

    // Receive the echo response
    if let Some(Ok(Message::Text(response))) = read.next().await {
        let echo_json: serde_json::Value = serde_json::from_str(&response)
            .expect("Failed to parse echo response");
        assert_eq!(
            echo_json.get("echo").and_then(|v| v.as_str()),
            Some(test_msg),
            "Echo response should contain original message"
        );
    } else {
        panic!("Expected text message response");
    }
}

#[tokio::test]
async fn ws_subscribe_prices() {
    let server = start_test_server().await;
    let ws_url = format!("ws://127.0.0.1:{}/ws", server.addr.port());

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Send a subscribe request
    let subscribe_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "subscribe",
        "params": {
            "channels": ["prices"]
        }
    });

    write
        .send(Message::Text(subscribe_msg.to_string()))
        .await
        .expect("Failed to send subscribe message");

    // Receive subscription confirmation
    if let Some(Ok(Message::Text(response))) = read.next().await {
        let response_json: serde_json::Value = serde_json::from_str(&response)
            .expect("Failed to parse subscription confirmation");
        assert_eq!(
            response_json.get("result").and_then(|v| v.as_str()),
            Some("subscribed"),
            "Should receive subscription confirmation"
        );
    } else {
        panic!("Expected subscription confirmation");
    }

    // Receive the mock price update
    if let Some(Ok(Message::Text(update))) = read.next().await {
        let update_json: serde_json::Value = serde_json::from_str(&update)
            .expect("Failed to parse price update");
        assert_eq!(
            update_json.get("channel").and_then(|v| v.as_str()),
            Some("prices"),
            "Should receive price update with correct channel"
        );
        assert_eq!(
            update_json
                .get("data")
                .and_then(|d| d.get("market_id"))
                .and_then(|v| v.as_str()),
            Some("test-market"),
            "Price update should contain test market ID"
        );
        assert_eq!(
            update_json
                .get("data")
                .and_then(|d| d.get("yes_price")),
            Some(&serde_json::json!(0.65)),
            "Price update should contain yes_price of 0.65"
        );
    } else {
        panic!("Expected price update message");
    }
}

#[tokio::test]
async fn ws_multiple_messages() {
    let server = start_test_server().await;
    let ws_url = format!("ws://127.0.0.1:{}/ws", server.addr.port());

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    let messages = vec!["First", "Second", "Third"];

    // Send multiple messages
    for msg in &messages {
        write
            .send(Message::Text(msg.to_string()))
            .await
            .expect("Failed to send message");
    }

    // Receive all echo responses
    for expected_msg in messages {
        if let Some(Ok(Message::Text(response))) = read.next().await {
            let echo_json: serde_json::Value = serde_json::from_str(&response)
                .expect("Failed to parse echo response");
            assert_eq!(
                echo_json.get("echo").and_then(|v| v.as_str()),
                Some(expected_msg),
                "Echo response should match sent message"
            );
        } else {
            panic!("Expected text message response");
        }
    }
}

#[tokio::test]
async fn ws_close_graceful() {
    let server = start_test_server().await;
    let ws_url = format!("ws://127.0.0.1:{}/ws", server.addr.port());

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Send a message to verify connection is working
    write
        .send(Message::Text("ping".to_string()))
        .await
        .expect("Failed to send message");

    // Receive the response
    if let Some(Ok(Message::Text(_response))) = read.next().await {
        // Successfully received response
    } else {
        panic!("Expected response to ping");
    }

    // Send a close frame
    write
        .send(Message::Close(None))
        .await
        .expect("Failed to send close message");

    // Drain until stream ends or we receive a Close frame. Server may send
    // Pong/Text/other frames before closing; all are acceptable.
    while let Some(msg) = read.next().await {
        if let Ok(Message::Close(_)) = msg {
            break;
        }
        // Ignore any other frames (Pong, final Text, etc.) and keep draining
    }
    // If we get here, the stream ended or we saw Close — connection closed gracefully.
}
