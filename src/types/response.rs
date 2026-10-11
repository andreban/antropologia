// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Response types for the Messages API.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::{ContentBlock, Role};

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The model finished its turn.
    EndTurn,
    /// The response hit `max_tokens`.
    MaxTokens,
    /// A custom stop sequence was generated.
    StopSequence,
    /// The model called one or more client tools.
    ToolUse,
    /// A long-running server-tool turn paused; send the response back to continue.
    PauseTurn,
    /// The model or a safety classifier declined. `stop_details` may say why.
    Refusal,
    /// The context window filled up.
    ModelContextWindowExceeded,
    /// A stop reason this crate does not know yet.
    #[serde(other)]
    Unknown,
}

/// Token usage for one request.
///
/// `input_tokens` excludes cached tokens: the total prompt size is
/// [`Usage::total_input_tokens`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Usage {
    /// Input tokens after the last cache breakpoint (not read from or written to the cache).
    #[serde(default)]
    pub input_tokens: u32,
    /// Output tokens, thinking included.
    #[serde(default)]
    pub output_tokens: u32,
    /// Input tokens written to the cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    /// Input tokens read from the cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

impl Usage {
    /// All input tokens: uncached, cache writes and cache reads.
    pub fn total_input_tokens(&self) -> u32 {
        self.input_tokens
            + self.cache_creation_input_tokens.unwrap_or(0)
            + self.cache_read_input_tokens.unwrap_or(0)
    }
}

/// Response from `POST /v1/messages`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessagesResponse {
    /// Message ID.
    pub id: String,
    /// The model that produced the message.
    pub model: String,
    /// Always [`Role::Assistant`].
    pub role: Role,
    /// Content blocks, in order.
    pub content: Vec<ContentBlock>,
    /// Why generation stopped. `None` only in a stream's `message_start` event.
    pub stop_reason: Option<StopReason>,
    /// The stop sequence that ended generation, if any.
    #[serde(default)]
    pub stop_sequence: Option<String>,
    /// Structured details for some stop reasons, such as a refusal's category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_details: Option<Value>,
    /// Token usage.
    pub usage: Usage,
}

impl MessagesResponse {
    /// All text blocks, joined with newlines. Empty when there is no text.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// All thinking text, joined with newlines. `None` when no thinking block has text.
    pub fn thinking(&self) -> Option<String> {
        let parts: Vec<&str> = self
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Thinking { thinking, .. } if !thinking.is_empty() => {
                    Some(thinking.as_str())
                }
                _ => None,
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join("\n"))
    }

    /// The tool calls, as `(id, name, input)`.
    pub fn tool_uses(&self) -> Vec<(&str, &str, &Value)> {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.as_str(), name.as_str(), input))
                }
                _ => None,
            })
            .collect()
    }
}
