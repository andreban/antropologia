// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! A manual tool-use loop: the model calls `get_weather`, the example runs it locally and
//! sends the result back until the model answers. Assistant turns are replayed with all their
//! content blocks, thinking included, as the API requires.
//!
//! Run with:
//! ```bash
//! cargo run --example tool_use
//! ```

use antropologia::prelude::*;
use serde_json::{Value, json};

fn get_weather(input: &Value) -> String {
    let city = input["city"].as_str().unwrap_or("somewhere");
    json!({ "city": city, "temperature": "20°C", "condition": "Partly cloudy" }).to_string()
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let client = AntropologiaClient::new(std::env::var("ANTHROPIC_API_KEY")?);

    let tools = vec![Tool::new(
        "get_weather",
        "Current weather for a city.",
        json!({
            "type": "object",
            "properties": { "city": { "type": "string", "description": "City name" } },
            "required": ["city"]
        }),
    )];
    let mut messages = vec![Message::user("What's the weather in Lisbon and in Paris?")];

    loop {
        let request = MessagesRequest::builder("claude-opus-5-5")
            .tools(tools.clone())
            .messages(messages.clone())
            .build();
        let response = client.messages(&request).await?;

        if response.stop_reason != Some(StopReason::ToolUse) {
            println!("{}", response.text());
            return Ok(());
        }

        let results: Vec<ContentBlock> = response
            .tool_uses()
            .into_iter()
            .map(|(id, name, input)| {
                println!("-> {name}({input})");
                ContentBlock::tool_result(id, get_weather(input))
            })
            .collect();
        messages.push(Message::with_blocks(Role::Assistant, response.content));
        messages.push(Message::with_blocks(Role::User, results));
    }
}
