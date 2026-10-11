// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Client tool definitions and tool choice.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content::CacheControl;

/// A client tool the model may call. Your code runs it and returns a tool result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    /// Tool name, `^[a-zA-Z0-9_-]{1,64}$`.
    pub name: String,
    /// What the tool does and when to use it. The model relies on this to choose tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema of the tool's input object.
    pub input_schema: Value,
    /// Optional cache breakpoint; on the last tool it caches all tool definitions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl Tool {
    /// Creates a tool with a description and input schema.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
            input_schema,
            cache_control: None,
        }
    }
}

/// Controls whether and which tools the model calls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// The model decides (the default when tools are present).
    Auto {
        /// Allow at most one tool call per turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// The model must call some tool.
    Any {
        /// Allow at most one tool call per turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// The model must call the named tool.
    Tool {
        /// Tool name.
        name: String,
        /// Allow at most one tool call per turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// The model must not call tools.
    None,
}
