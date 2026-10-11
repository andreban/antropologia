// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Streaming events and an accumulator that rebuilds the full message from them.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::ContentBlock;
use super::response::{MessagesResponse, StopReason};
use crate::error::{AntropologiaError, Result};

/// One server-sent event from a streaming Messages request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// The message shell: ID, model, and input usage. `content` is empty.
    MessageStart {
        /// The message so far.
        message: MessagesResponse,
    },
    /// A new content block begins at `index`.
    ContentBlockStart {
        /// Block position in `content`.
        index: usize,
        /// The block's initial state (empty text, empty input, ...).
        content_block: ContentBlock,
    },
    /// An incremental update to the block at `index`.
    ContentBlockDelta {
        /// Block position in `content`.
        index: usize,
        /// The update.
        delta: ContentDelta,
    },
    /// The block at `index` is complete.
    ContentBlockStop {
        /// Block position in `content`.
        index: usize,
    },
    /// Top-level changes: stop reason and cumulative usage.
    MessageDelta {
        /// Stop reason and related fields.
        delta: MessageDeltaBody,
        /// Cumulative usage; fields absent here are unchanged.
        #[serde(default)]
        usage: DeltaUsage,
    },
    /// The stream is complete.
    MessageStop,
    /// Keep-alive.
    Ping,
    /// An error mid-stream, such as `overloaded_error`.
    Error {
        /// The error.
        error: StreamError,
    },
    /// An event type this crate does not know yet.
    #[serde(untagged)]
    Other(Value),
}

/// An incremental content update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    /// More text for a text block.
    TextDelta {
        /// The new text.
        text: String,
    },
    /// More of a tool-use block's input, as a partial JSON string.
    InputJsonDelta {
        /// The next fragment of the input JSON.
        partial_json: String,
    },
    /// More thinking text.
    ThinkingDelta {
        /// The new thinking text.
        thinking: String,
    },
    /// The signature of a thinking block.
    SignatureDelta {
        /// The signature.
        signature: String,
    },
    /// A delta type this crate does not know yet.
    #[serde(untagged)]
    Other(Value),
}

/// Body of a `message_delta` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MessageDeltaBody {
    /// Why generation stopped.
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    /// The stop sequence that ended generation, if any.
    #[serde(default)]
    pub stop_sequence: Option<String>,
    /// Structured details for some stop reasons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_details: Option<Value>,
}

/// Cumulative usage carried by `message_delta`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DeltaUsage {
    /// Output tokens so far.
    #[serde(default)]
    pub output_tokens: Option<u32>,
    /// Uncached input tokens, when reported.
    #[serde(default)]
    pub input_tokens: Option<u32>,
    /// Cache-write input tokens, when reported.
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
    /// Cache-read input tokens, when reported.
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
}

/// Error payload of a stream `error` event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamError {
    /// Error type, such as `overloaded_error`.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Human-readable message.
    pub message: String,
}

/// Rebuilds a [`MessagesResponse`] from [`StreamEvent`]s.
///
/// Feed every event to [`MessageAccumulator::push`], then call
/// [`MessageAccumulator::finish`]. The result has the same content blocks a non-streaming
/// request would return, so it can be replayed in the next request.
///
/// ```
/// use antropologia::prelude::*;
/// # fn run(events: Vec<StreamEvent>) -> antropologia::Result<()> {
/// let mut acc = MessageAccumulator::new();
/// for event in events {
///     acc.push(event)?;
/// }
/// let message = acc.finish()?;
/// println!("{}", message.text());
/// # Ok(()) }
/// ```
#[derive(Debug, Default)]
pub struct MessageAccumulator {
    message: Option<MessagesResponse>,
    partial_json: HashMap<usize, String>,
}

impl MessageAccumulator {
    /// Creates an empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event. Fails on an `error` event or on events out of order.
    pub fn push(&mut self, event: StreamEvent) -> Result<()> {
        match event {
            StreamEvent::MessageStart { message } => self.message = Some(message),
            StreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let content = &mut self.message_mut()?.content;
                if index != content.len() {
                    return Err(AntropologiaError::Stream(format!(
                        "content_block_start for index {index}, expected {}",
                        content.len()
                    )));
                }
                content.push(content_block);
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                if let ContentDelta::InputJsonDelta { partial_json } = &delta {
                    self.partial_json
                        .entry(index)
                        .or_default()
                        .push_str(partial_json);
                    return Ok(());
                }
                let block = self.block_mut(index)?;
                match (block, delta) {
                    (ContentBlock::Text { text, .. }, ContentDelta::TextDelta { text: more }) => {
                        text.push_str(&more)
                    }
                    (
                        ContentBlock::Thinking { thinking, .. },
                        ContentDelta::ThinkingDelta { thinking: more },
                    ) => thinking.push_str(&more),
                    (
                        ContentBlock::Thinking { signature, .. },
                        ContentDelta::SignatureDelta { signature: sig },
                    ) => *signature = sig,
                    // Deltas for block types this crate does not model are ignored.
                    _ => {}
                }
            }
            StreamEvent::ContentBlockStop { index } => {
                if let Some(json) = self.partial_json.remove(&index) {
                    let block = self.block_mut(index)?;
                    if let ContentBlock::ToolUse { input, .. } = block {
                        *input = if json.trim().is_empty() {
                            Value::Object(Default::default())
                        } else {
                            serde_json::from_str(&json)?
                        };
                    }
                }
            }
            StreamEvent::MessageDelta { delta, usage } => {
                let message = self.message_mut()?;
                message.stop_reason = delta.stop_reason.or(message.stop_reason);
                message.stop_sequence = delta.stop_sequence.or(message.stop_sequence.take());
                message.stop_details = delta.stop_details.or(message.stop_details.take());
                let total = &mut message.usage;
                if let Some(n) = usage.output_tokens {
                    total.output_tokens = n;
                }
                if let Some(n) = usage.input_tokens {
                    total.input_tokens = n;
                }
                if let Some(n) = usage.cache_creation_input_tokens {
                    total.cache_creation_input_tokens = Some(n);
                }
                if let Some(n) = usage.cache_read_input_tokens {
                    total.cache_read_input_tokens = Some(n);
                }
            }
            StreamEvent::Error { error } => {
                return Err(AntropologiaError::Api {
                    status: reqwest::StatusCode::OK,
                    error_type: error.error_type,
                    message: error.message,
                    request_id: None,
                });
            }
            StreamEvent::MessageStop | StreamEvent::Ping | StreamEvent::Other(_) => {}
        }
        Ok(())
    }

    /// Returns the message built so far. Fails if no `message_start` was received.
    pub fn finish(self) -> Result<MessagesResponse> {
        self.message
            .ok_or_else(|| AntropologiaError::Stream("stream ended before message_start".into()))
    }

    fn message_mut(&mut self) -> Result<&mut MessagesResponse> {
        self.message
            .as_mut()
            .ok_or_else(|| AntropologiaError::Stream("event before message_start".into()))
    }

    fn block_mut(&mut self, index: usize) -> Result<&mut ContentBlock> {
        self.message_mut()?
            .content
            .get_mut(index)
            .ok_or_else(|| AntropologiaError::Stream(format!("no content block at index {index}")))
    }
}
