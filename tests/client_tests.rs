// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use antropologia::prelude::*;
use serde_json::json;
use tokio_stream::StreamExt;
use wiremock::matchers::{body_partial_json, header, headers, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> AntropologiaClient {
    AntropologiaClient::builder("test-key")
        .base_url(server.uri())
        .beta("beta-a")
        .beta("beta-b")
        .build()
}

fn request() -> MessagesRequest {
    MessagesRequest::builder("claude-opus-5-5")
        .messages(vec![Message::user("Hi")])
        .build()
}

#[tokio::test]
async fn messages_sends_auth_version_and_beta_headers() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(headers("anthropic-beta", vec!["beta-a", "beta-b"]))
        .and(body_partial_json(
            json!({"model": "claude-opus-5-5", "max_tokens": 16000}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
            "content": [{"type": "text", "text": "Hello."}],
            "stop_reason": "end_turn", "stop_sequence": null,
            "usage": {"input_tokens": 5, "output_tokens": 2}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let response = client(&server).messages(&request()).await?;

    assert_eq!(response.text(), "Hello.");
    assert_eq!(response.stop_reason, Some(StopReason::EndTurn));
    Ok(())
}

#[tokio::test]
async fn api_errors_carry_type_message_and_request_id() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(529).set_body_json(json!({
            "type": "error",
            "error": {"type": "overloaded_error", "message": "Overloaded"},
            "request_id": "req_123"
        })))
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("a 529 is an error");

    let AntropologiaError::Api {
        status,
        ref error_type,
        ref message,
        ref request_id,
        retry_after,
    } = err
    else {
        anyhow::bail!("expected an API error, got {err:?}");
    };
    assert_eq!(status.as_u16(), 529);
    assert_eq!(error_type, "overloaded_error");
    assert_eq!(message, "Overloaded");
    assert_eq!(request_id.as_deref(), Some("req_123"));
    assert_eq!(retry_after, None);
    assert!(err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn api_errors_carry_retry_after_on_429() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .set_body_json(json!({
                    "type": "error",
                    "error": {"type": "rate_limit_error", "message": "Rate limited"}
                })),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("a 429 is an error");

    let AntropologiaError::Api {
        status,
        ref error_type,
        retry_after,
        ..
    } = err
    else {
        anyhow::bail!("expected an API error, got {err:?}");
    };
    assert_eq!(status.as_u16(), 429);
    assert_eq!(error_type, "rate_limit_error");
    assert_eq!(retry_after, Some(Duration::from_secs(30)));
    assert!(err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn api_errors_carry_retry_after_on_529() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(529)
                .insert_header("retry-after", "5")
                .set_body_json(json!({
                    "type": "error",
                    "error": {"type": "overloaded_error", "message": "Overloaded"}
                })),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("a 529 is an error");

    let AntropologiaError::Api {
        status,
        ref error_type,
        retry_after,
        ..
    } = err
    else {
        anyhow::bail!("expected an API error, got {err:?}");
    };
    assert_eq!(status.as_u16(), 529);
    assert_eq!(error_type, "overloaded_error");
    assert_eq!(retry_after, Some(Duration::from_secs(5)));
    assert!(err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn api_errors_handle_unparsable_retry_after() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "not-a-number")
                .set_body_json(json!({
                    "type": "error",
                    "error": {"type": "rate_limit_error", "message": "Rate limited"}
                })),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("a 429 is an error");

    let AntropologiaError::Api { retry_after, .. } = err else {
        anyhow::bail!("expected an API error, got {err:?}");
    };
    assert_eq!(retry_after, None);
    Ok(())
}

#[tokio::test]
async fn api_errors_carry_retry_after_on_non_json_body() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("retry-after", "30")
                .set_body_string("Service Unavailable"),
        )
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("a 503 is an error");

    let AntropologiaError::Api {
        status,
        ref error_type,
        ref message,
        retry_after,
        ..
    } = err
    else {
        anyhow::bail!("expected an API error, got {err:?}");
    };
    assert_eq!(status.as_u16(), 503);
    assert_eq!(error_type, "unknown");
    assert_eq!(message, "Service Unavailable");
    assert_eq!(retry_after, Some(Duration::from_secs(30)));
    assert!(err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn invalid_request_errors_are_not_retryable() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "error",
            "error": {"type": "invalid_request_error", "message": "messages: empty"}
        })))
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("a 400 is an error");

    assert!(!err.is_retryable());
    Ok(())
}

fn sse(events: &[serde_json::Value]) -> String {
    events
        .iter()
        .map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap_or("")))
        .collect()
}

#[tokio::test]
async fn messages_stream_yields_events_that_accumulate() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let body = sse(&[
        json!({"type": "message_start", "message": {
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
            "content": [], "stop_reason": null, "stop_sequence": null,
            "usage": {"input_tokens": 5, "output_tokens": 1}}}),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "ping"}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hel"}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "lo."}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": null}, "usage": {"output_tokens": 3}}),
        json!({"type": "message_stop"}),
    ]);
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"stream": true})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let mut stream = Box::pin(client(&server).messages_stream(&request()).await?);
    let mut acc = MessageAccumulator::new();
    while let Some(event) = stream.next().await {
        acc.push(event?)?;
    }
    let message = acc.finish()?;

    assert_eq!(message.text(), "Hello.");
    assert_eq!(message.usage.output_tokens, 3);
    Ok(())
}

#[tokio::test]
async fn messages_stream_turns_error_events_into_errors() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let body = sse(&[json!({
        "type": "error",
        "error": {"type": "overloaded_error", "message": "Overloaded"}
    })]);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let mut stream = Box::pin(client(&server).messages_stream(&request()).await?);
    let first = stream.next().await.expect("one item");

    assert!(matches!(
        first,
        Err(AntropologiaError::Api {
            ref error_type,
            retry_after: None,
            ..
        }) if error_type == "overloaded_error"
    ));
    Ok(())
}

#[tokio::test]
async fn connection_failures_are_retryable() -> anyhow::Result<()> {
    // Bind and drop a listener so nothing is listening on the port.
    let address = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?;
    let client = AntropologiaClient::builder("test-key")
        .base_url(format!("http://{address}"))
        .build();

    let err = client
        .messages(&request())
        .await
        .expect_err("nothing is listening");

    assert!(matches!(err, AntropologiaError::Http(_)), "{err:?}");
    assert!(err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn invalid_urls_are_not_retryable() -> anyhow::Result<()> {
    let client = AntropologiaClient::builder("test-key")
        .base_url("http://[::1")
        .build();

    let err = client
        .messages(&request())
        .await
        .expect_err("the URL is invalid");

    assert!(matches!(err, AntropologiaError::Http(_)), "{err:?}");
    assert!(!err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn timeouts_are_retryable() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(1)))
        .mount(&server)
        .await;
    let client = AntropologiaClient::builder("test-key")
        .base_url(server.uri())
        .timeout(Duration::from_millis(50))
        .build();

    let err = client
        .messages(&request())
        .await
        .expect_err("the server answers after the timeout");

    assert!(matches!(err, AntropologiaError::Http(_)), "{err:?}");
    assert!(err.is_retryable());
    Ok(())
}

#[tokio::test]
async fn undecodable_responses_are_not_retryable() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let err = client(&server)
        .messages(&request())
        .await
        .expect_err("the body isn't a message");

    assert!(matches!(err, AntropologiaError::Http(_)), "{err:?}");
    assert!(!err.is_retryable());
    Ok(())
}
