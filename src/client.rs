// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! HTTP client for the Anthropic API.

use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response};
use serde::Deserialize;
use tokio_stream::{Stream, StreamExt};
use tokio_util::codec::LinesCodecError;

use crate::{
    error::{AntropologiaError, Result},
    network::event_source::{EventSource, ServerSentEvent},
    types::{
        request::MessagesRequest,
        response::MessagesResponse,
        stream::{StreamError, StreamEvent},
    },
};

/// Default Anthropic API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Value of the `anthropic-version` header.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API client.
#[derive(Clone, Debug)]
pub struct AntropologiaClient {
    api_key: String,
    base_url: String,
    betas: Vec<String>,
    client: Client,
}

/// Convenience alias for [`AntropologiaClient`].
pub type AnthropicClient = AntropologiaClient;

impl AntropologiaClient {
    /// Creates a client with the given API key and the default base URL
    /// (`https://api.anthropic.com`).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder(api_key).build()
    }

    /// Creates a builder to configure the client.
    pub fn builder(api_key: impl Into<String>) -> AntropologiaClientBuilder {
        AntropologiaClientBuilder::new(api_key)
    }

    /// Returns the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Sends a non-streaming Messages request.
    pub async fn messages(&self, request: &MessagesRequest) -> Result<MessagesResponse> {
        let mut req = request.clone();
        req.stream = None;
        let response = self.post_messages(&req).send().await?;
        let response = Self::handle_error_status(response).await?;
        Ok(response.json::<MessagesResponse>().await?)
    }

    /// Sends a streaming Messages request and returns its events.
    ///
    /// An `error` event in the stream (for example `overloaded_error`) is yielded as
    /// [`AntropologiaError::Api`]. Use [`crate::types::MessageAccumulator`] to rebuild the
    /// full message.
    pub async fn messages_stream(
        &self,
        request: &MessagesRequest,
    ) -> Result<impl Stream<Item = Result<StreamEvent>> + use<>> {
        let mut req = request.clone();
        req.stream = Some(true);
        let response = self.post_messages(&req).send().await?;
        let response = Self::handle_error_status(response).await?;
        Ok(response.event_stream().filter_map(Self::parse_sse_event))
    }

    fn post_messages(&self, request: &MessagesRequest) -> RequestBuilder {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let mut builder = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(request);
        if !self.betas.is_empty() {
            builder = builder.header("anthropic-beta", self.betas.join(","));
        }
        builder
    }

    /// Parses an SSE event into a [`StreamEvent`].
    fn parse_sse_event(
        event_res: std::result::Result<ServerSentEvent, LinesCodecError>,
    ) -> Option<Result<StreamEvent>> {
        let sse = match event_res {
            Ok(ev) => ev,
            Err(e) => return Some(Err(AntropologiaError::Stream(e.to_string()))),
        };
        let data = sse.data?;
        let trimmed = data.trim();
        if trimmed.is_empty() {
            return None;
        }
        match serde_json::from_str::<StreamEvent>(trimmed) {
            Ok(StreamEvent::Error {
                error:
                    StreamError {
                        error_type,
                        message,
                    },
            }) => Some(Err(AntropologiaError::Api {
                status: reqwest::StatusCode::OK,
                error_type,
                message,
                request_id: None,
                retry_after: None,
            })),
            Ok(event) => Some(Ok(event)),
            Err(e) => {
                tracing::warn!(data = trimmed, error = %e, "Failed to parse StreamEvent");
                Some(Err(AntropologiaError::Json(e)))
            }
        }
    }

    /// Inspects the HTTP status and decodes the error body if it is not 2xx.
    async fn handle_error_status(response: Response) -> Result<Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let header_request_id = response
            .headers()
            .get("request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let body = response.text().await.unwrap_or_default();
        Err(match serde_json::from_str::<ErrorEnvelope>(&body) {
            Ok(envelope) => AntropologiaError::Api {
                status,
                error_type: envelope.error.error_type,
                message: envelope.error.message,
                request_id: envelope.request_id.or(header_request_id),
                retry_after,
            },
            Err(_) => AntropologiaError::Api {
                status,
                error_type: "unknown".into(),
                message: if body.is_empty() {
                    format!("HTTP error {status}")
                } else {
                    body
                },
                request_id: header_request_id,
                retry_after,
            },
        })
    }
}

/// Anthropic error response body.
#[derive(Deserialize)]
struct ErrorEnvelope {
    error: StreamError,
    request_id: Option<String>,
}

/// Builder for [`AntropologiaClient`].
pub struct AntropologiaClientBuilder {
    api_key: String,
    base_url: String,
    betas: Vec<String>,
    timeout: Option<Duration>,
    custom_client: Option<Client>,
}

impl AntropologiaClientBuilder {
    /// Creates a new builder with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            betas: Vec::new(),
            timeout: None,
            custom_client: None,
        }
    }

    /// Sets a custom base URL (e.g. for a proxy or a test server).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Adds a beta feature ID, sent in the `anthropic-beta` header on every request.
    pub fn beta(mut self, beta: impl Into<String>) -> Self {
        self.betas.push(beta.into());
        self
    }

    /// Sets the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets a custom [`reqwest::Client`].
    pub fn client(mut self, client: Client) -> Self {
        self.custom_client = Some(client);
        self
    }

    /// Builds the [`AntropologiaClient`].
    pub fn build(self) -> AntropologiaClient {
        let client = self.custom_client.unwrap_or_else(|| {
            let mut builder = Client::builder();
            if let Some(t) = self.timeout {
                builder = builder.timeout(t);
            }
            builder.build().unwrap_or_default()
        });

        AntropologiaClient {
            api_key: self.api_key,
            base_url: self.base_url,
            betas: self.betas,
            client,
        }
    }
}
