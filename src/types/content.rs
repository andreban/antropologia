// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Messages and content blocks.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Human user turn. Tool results are sent in user turns.
    #[default]
    User,
    /// Model turn.
    Assistant,
}

/// Marks a cache breakpoint: the prompt up to and including this block is cached.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheControl {
    /// Always `"ephemeral"`.
    #[serde(rename = "type")]
    pub cache_type: String,
    /// Cache lifetime, `"5m"` (the default) or `"1h"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

impl CacheControl {
    /// An ephemeral breakpoint with the default 5-minute lifetime.
    pub fn ephemeral() -> Self {
        Self {
            cache_type: "ephemeral".into(),
            ttl: None,
        }
    }

    /// An ephemeral breakpoint with a 1-hour lifetime.
    pub fn one_hour() -> Self {
        Self {
            cache_type: "ephemeral".into(),
            ttl: Some("1h".into()),
        }
    }
}

/// One block of message content.
///
/// Block types this crate does not model (for example server-tool blocks) deserialize into
/// [`ContentBlock::Other`] and serialize back unchanged, so an assistant turn can always be
/// replayed exactly as it was received.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text.
    Text {
        /// The text.
        text: String,
        /// Optional cache breakpoint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// The model's reasoning. Must be sent back unchanged, signature included, when continuing
    /// a conversation on the same model.
    Thinking {
        /// Reasoning text; empty when the request set `display: "omitted"` (the default on
        /// recent models).
        thinking: String,
        /// Opaque signature that verifies the block.
        signature: String,
    },
    /// Reasoning the API returns encrypted. Must be sent back unchanged.
    RedactedThinking {
        /// Encrypted reasoning.
        data: String,
    },
    /// A call to a client tool.
    ToolUse {
        /// Call ID, echoed in the matching [`ContentBlock::ToolResult`].
        id: String,
        /// Tool name.
        name: String,
        /// Arguments, a JSON object matching the tool's input schema.
        input: Value,
    },
    /// The result of a client tool call, sent in a user turn.
    ToolResult {
        /// The ID of the [`ContentBlock::ToolUse`] this answers.
        tool_use_id: String,
        /// The result.
        content: ToolResultContent,
        /// Set when the tool failed, so the model can recover.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        /// Optional cache breakpoint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Any block type this crate does not model, kept as raw JSON.
    #[serde(untagged)]
    Other(Value),
}

impl ContentBlock {
    /// A text block.
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    /// A tool-use block.
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// A successful tool-result block.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: ToolResultContent::Text(content.into()),
            is_error: None,
            cache_control: None,
        }
    }

    /// A tool-result block reporting that the tool failed.
    pub fn tool_error(tool_use_id: impl Into<String>, message: impl Into<String>) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: ToolResultContent::Text(message.into()),
            is_error: Some(true),
            cache_control: None,
        }
    }

    /// Sets a cache breakpoint on a text or tool-result block; other blocks are returned
    /// unchanged.
    pub fn with_cache_control(mut self, control: CacheControl) -> Self {
        match &mut self {
            ContentBlock::Text { cache_control, .. }
            | ContentBlock::ToolResult { cache_control, .. } => *cache_control = Some(control),
            _ => {}
        }
        self
    }
}

/// Content of a tool result: a string, or a list of blocks (e.g. text and images).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Plain text.
    Text(String),
    /// Content blocks.
    Blocks(Vec<ContentBlock>),
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Message {
    /// The author.
    pub role: Role,
    /// Content blocks.
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// A user message with one text block.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::text(text)],
        }
    }

    /// An assistant message with one text block.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::text(text)],
        }
    }

    /// A message with the given role and blocks.
    pub fn with_blocks(role: Role, content: Vec<ContentBlock>) -> Self {
        Self { role, content }
    }
}

/// The system prompt: a plain string, or text blocks (needed for cache breakpoints).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum System {
    /// Plain text.
    Text(String),
    /// Text blocks.
    Blocks(Vec<ContentBlock>),
}

impl From<String> for System {
    fn from(text: String) -> Self {
        System::Text(text)
    }
}

impl From<&str> for System {
    fn from(text: &str) -> Self {
        System::Text(text.to_string())
    }
}
