// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Structured output: the reply is JSON matching a schema, deserialized into a Rust struct.
//!
//! Run with:
//! ```bash
//! cargo run --example structured_output
//! ```

use antropologia::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Book {
    title: String,
    author: String,
    year: u32,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let client = AntropologiaClient::new(std::env::var("ANTHROPIC_API_KEY")?);

    // `json_schema` adds `additionalProperties: false`, which the API requires.
    let request = MessagesRequest::builder("claude-opus-5-5")
        .messages(vec![Message::user(
            "Name one classic book of cultural anthropology.",
        )])
        .json_schema(json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "author": { "type": "string" },
                "year": { "type": "integer" }
            },
            "required": ["title", "author", "year"]
        }))
        .build();

    let response = client.messages(&request).await?;
    let book: Book = serde_json::from_str(&response.text())?;
    println!("{} by {} ({})", book.title, book.author, book.year);
    Ok(())
}
