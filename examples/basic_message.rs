// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! A single Messages request, printing the reply and token usage.
//!
//! Run with:
//! ```bash
//! cargo run --example basic_message
//! ```

use antropologia::prelude::*;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let client = AntropologiaClient::new(std::env::var("ANTHROPIC_API_KEY")?);

    let request = MessagesRequest::builder("claude-opus-5-5")
        .system("You are a concise assistant.")
        .messages(vec![Message::user("Explain what anthropology studies.")])
        .effort(Effort::Low)
        .build();

    let response = client.messages(&request).await?;
    println!("{}", response.text());
    println!(
        "\n[stop: {:?}, input tokens: {}, output tokens: {}]",
        response.stop_reason,
        response.usage.total_input_tokens(),
        response.usage.output_tokens
    );
    Ok(())
}
