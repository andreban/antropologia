// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Anthropic Messages API data transfer objects.

pub mod content;
pub mod request;
pub mod response;
pub mod stream;
pub mod tools;

pub use content::{CacheControl, ContentBlock, Message, Role, System, ToolResultContent};
pub use request::{
    DEFAULT_MAX_TOKENS, Effort, MessagesRequest, MessagesRequestBuilder, Metadata, OutputConfig,
    OutputFormat, ThinkingConfig, ThinkingDisplay,
};
pub use response::{MessagesResponse, StopReason, Usage};
pub use stream::{
    ContentDelta, DeltaUsage, MessageAccumulator, MessageDeltaBody, StreamError, StreamEvent,
};
pub use tools::{Tool, ToolChoice};
