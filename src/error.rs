// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Error types for the Antropologia client.

use std::time::Duration;

use thiserror::Error;

/// Errors returned by the Antropologia client.
#[derive(Debug, Error)]
pub enum AntropologiaError {
    /// Network or HTTP transport error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization or deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error returned by the Anthropic API, either as an HTTP error status or as an `error`
    /// event in a stream.
    #[error("API error (status {status}, {error_type}): {message}")]
    Api {
        /// HTTP status. For an `error` event in a stream, the status of the stream response (200).
        status: reqwest::StatusCode,
        /// Error type, such as `invalid_request_error` or `overloaded_error`.
        error_type: String,
        /// Human-readable error message.
        message: String,
        /// The `request-id` header or `request_id` field, to quote when reporting issues.
        request_id: Option<String>,
        /// The `retry-after` header, specifying how long to wait before retrying.
        retry_after: Option<Duration>,
    },

    /// Streaming / SSE decoding error.
    #[error("Stream error: {0}")]
    Stream(String),
}

impl AntropologiaError {
    /// Whether retrying the same request may succeed: rate limits (429), server errors (5xx),
    /// overload (529), and failures to connect or timeouts. Other transport errors, such as an
    /// invalid URL or header, fail the same way every time.
    pub fn is_retryable(&self) -> bool {
        match self {
            AntropologiaError::Http(error) => error.is_connect() || error.is_timeout(),
            AntropologiaError::Api {
                status, error_type, ..
            } => {
                status.as_u16() == 429
                    || status.is_server_error()
                    || matches!(
                        error_type.as_str(),
                        "rate_limit_error" | "overloaded_error" | "api_error"
                    )
            }
            AntropologiaError::Json(_) | AntropologiaError::Stream(_) => false,
        }
    }
}

/// Convenience type alias for [`Result<T, AntropologiaError>`].
pub type Result<T> = std::result::Result<T, AntropologiaError>;
