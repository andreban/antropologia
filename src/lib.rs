// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! # antropologia
//!
//! An ergonomic, async Rust client for the Anthropic Claude API.
//!
//! Named after *anthropology*, for Anthropic, following the naming tradition of
//! [`geologia`](https://github.com/andreban/geologia) (Google AI) and
//! [`cetologia`](https://github.com/andreban/cetologia) (DeepSeek).
//!
//! ## Features
//!
//! - **Messages API**: typed requests and responses, with builders.
//! - **Streaming**: SSE events, plus [`MessageAccumulator`] to rebuild the full message.
//! - **Tool use**: client tools, tool choice, tool results with `is_error`.
//! - **Thinking**: adaptive thinking, thinking display, effort, and lossless replay of
//!   thinking blocks (unknown block types round-trip unchanged).
//! - **Structured output**: JSON Schema output, adjusted to what the API accepts.
//! - **Prompt caching**: cache breakpoints and cache usage counts.
//!
//! ## Example
//!
//! ```no_run
//! use antropologia::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//!     let client = AntropologiaClient::new("ANTHROPIC_API_KEY");
//!
//!     let request = MessagesRequest::builder("claude-opus-5-5")
//!         .system("You are a helpful assistant.")
//!         .messages(vec![Message::user("Hello!")])
//!         .build();
//!
//!     let response = client.messages(&request).await?;
//!     println!("Response: {}", response.text());
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod network;
pub mod types;

pub use client::{
    ANTHROPIC_VERSION, AnthropicClient, AntropologiaClient, AntropologiaClientBuilder,
    DEFAULT_BASE_URL,
};
pub use error::{AntropologiaError, Result};
pub use types::*;

/// Convenient re-exports for common usage.
pub mod prelude {
    pub use crate::client::{AnthropicClient, AntropologiaClient, AntropologiaClientBuilder};
    pub use crate::error::{AntropologiaError, Result};
    pub use crate::types::content::{
        CacheControl, ContentBlock, Message, Role, System, ToolResultContent,
    };
    pub use crate::types::request::{
        Effort, MessagesRequest, MessagesRequestBuilder, Metadata, OutputConfig, OutputFormat,
        ThinkingConfig, ThinkingDisplay,
    };
    pub use crate::types::response::{MessagesResponse, StopReason, Usage};
    pub use crate::types::stream::{ContentDelta, MessageAccumulator, StreamEvent};
    pub use crate::types::tools::{Tool, ToolChoice};
}
