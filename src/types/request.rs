// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Request body for the Messages API (`POST /v1/messages`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::{Message, System};
use super::tools::{Tool, ToolChoice};

/// Extended thinking settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    /// The model decides when and how much to think; depth follows
    /// [`OutputConfig::effort`]. The mode for current models.
    Adaptive {
        /// Whether thinking text is returned. Recent models default to omitted (empty text).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<ThinkingDisplay>,
    },
    /// A fixed thinking budget. Only for older models; current models reject it.
    Enabled {
        /// Maximum thinking tokens; must be below `max_tokens`.
        budget_tokens: u32,
    },
    /// No thinking. Not accepted by every model.
    Disabled,
}

impl ThinkingConfig {
    /// Adaptive thinking with summarized thinking text returned.
    pub fn adaptive_summarized() -> Self {
        ThinkingConfig::Adaptive {
            display: Some(ThinkingDisplay::Summarized),
        }
    }
}

/// Whether thinking text is returned in thinking blocks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingDisplay {
    /// A readable summary of the reasoning.
    Summarized,
    /// Thinking blocks are returned with empty text.
    Omitted,
}

/// How much effort the model spends, trading quality for tokens and latency.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    /// Fastest and cheapest.
    Low,
    /// Balanced.
    Medium,
    /// Thorough.
    High,
    /// More thorough than high, on models that support it.
    Xhigh,
    /// Maximum effort.
    Max,
}

/// Output settings: effort and structured output format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OutputConfig {
    /// Effort level; the model's default applies when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    /// Constrains the response to a format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
}

/// A structured output format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputFormat {
    /// The response text is JSON matching `schema`.
    JsonSchema {
        /// The JSON Schema.
        schema: Value,
    },
}

impl OutputFormat {
    /// A JSON Schema format, adjusted to what the API accepts: the `$schema` keyword is
    /// removed, and every object schema without `additionalProperties` gets
    /// `additionalProperties: false`, which the API requires.
    pub fn json_schema(mut schema: Value) -> Self {
        if let Some(object) = schema.as_object_mut() {
            object.remove("$schema");
        }
        close_objects(&mut schema);
        OutputFormat::JsonSchema { schema }
    }
}

fn close_objects(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("object") {
                map.entry("additionalProperties")
                    .or_insert(Value::Bool(false));
            }
            map.values_mut().for_each(close_objects);
        }
        Value::Array(items) => items.iter_mut().for_each(close_objects),
        _ => {}
    }
}

/// Request metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Metadata {
    /// An opaque identifier for the end user, for abuse detection. Never send personal data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Default `max_tokens` used by [`MessagesRequest::builder`].
pub const DEFAULT_MAX_TOKENS: u32 = 16_000;

/// Request body for `POST /v1/messages`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MessagesRequest {
    /// Model ID, e.g. `"claude-opus-5-5"`.
    pub model: String,
    /// Maximum tokens to generate, thinking included.
    pub max_tokens: u32,
    /// The conversation; user and assistant turns alternate.
    pub messages: Vec<Message>,
    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<System>,
    /// Client tools the model may call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Tool-calling control.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Extended thinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    /// Effort and structured output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
    /// Sampling temperature. Recent models reject non-default sampling parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Nucleus sampling. Recent models reject non-default sampling parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k sampling. Recent models reject non-default sampling parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Custom stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    /// Set by the client: `true` for [`crate::AntropologiaClient::messages_stream`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl MessagesRequest {
    /// Creates a builder for `model`, with `max_tokens` set to [`DEFAULT_MAX_TOKENS`].
    pub fn builder(model: impl Into<String>) -> MessagesRequestBuilder {
        MessagesRequestBuilder::new(model)
    }
}

/// Builder for [`MessagesRequest`].
pub struct MessagesRequestBuilder {
    request: MessagesRequest,
}

impl MessagesRequestBuilder {
    /// Creates a builder for `model`, with `max_tokens` set to [`DEFAULT_MAX_TOKENS`].
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            request: MessagesRequest {
                model: model.into(),
                max_tokens: DEFAULT_MAX_TOKENS,
                ..Default::default()
            },
        }
    }

    /// Sets the conversation.
    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.request.messages = messages;
        self
    }

    /// Appends one message.
    pub fn add_message(mut self, message: Message) -> Self {
        self.request.messages.push(message);
        self
    }

    /// Sets the maximum tokens to generate.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.request.max_tokens = max_tokens;
        self
    }

    /// Sets the system prompt.
    pub fn system(mut self, system: impl Into<System>) -> Self {
        self.request.system = Some(system.into());
        self
    }

    /// Sets the client tools.
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.request.tools = Some(tools);
        self
    }

    /// Sets tool choice.
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.request.tool_choice = Some(choice);
        self
    }

    /// Sets extended thinking.
    pub fn thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.request.thinking = Some(thinking);
        self
    }

    /// Sets the effort level.
    pub fn effort(mut self, effort: Effort) -> Self {
        self.request
            .output_config
            .get_or_insert_with(Default::default)
            .effort = Some(effort);
        self
    }

    /// Constrains the response to JSON matching `schema`; see [`OutputFormat::json_schema`].
    pub fn json_schema(mut self, schema: Value) -> Self {
        self.request
            .output_config
            .get_or_insert_with(Default::default)
            .format = Some(OutputFormat::json_schema(schema));
        self
    }

    /// Sets sampling temperature.
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.request.temperature = Some(temperature);
        self
    }

    /// Sets nucleus sampling.
    pub fn top_p(mut self, top_p: f32) -> Self {
        self.request.top_p = Some(top_p);
        self
    }

    /// Sets top-k sampling.
    pub fn top_k(mut self, top_k: u32) -> Self {
        self.request.top_k = Some(top_k);
        self
    }

    /// Sets custom stop sequences.
    pub fn stop_sequences(mut self, stop: Vec<String>) -> Self {
        self.request.stop_sequences = Some(stop);
        self
    }

    /// Sets request metadata.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.request.metadata = Some(metadata);
        self
    }

    /// Builds the [`MessagesRequest`].
    pub fn build(self) -> MessagesRequest {
        self.request
    }
}
