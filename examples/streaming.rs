// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Streams a reply with summarized adaptive thinking, printing thinking and text as they
//! arrive, then rebuilds the full message with `MessageAccumulator`.
//!
//! Run with:
//! ```bash
//! cargo run --example streaming
//! ```

use std::io::Write;

use antropologia::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let client = AntropologiaClient::new(std::env::var("ANTHROPIC_API_KEY")?);

    let request = MessagesRequest::builder("claude-opus-5-5")
        .max_tokens(32_000)
        .thinking(ThinkingConfig::adaptive_summarized())
        .messages(vec![Message::user("Which is larger: 9.9 or 9.11?")])
        .build();

    let mut stream = client.messages_stream(&request).await?;
    let mut accumulator = MessageAccumulator::new();
    while let Some(event) = stream.next().await {
        let event = event?;
        if let StreamEvent::ContentBlockDelta { delta, .. } = &event {
            match delta {
                ContentDelta::ThinkingDelta { thinking } => print!("[THINK] {thinking}"),
                ContentDelta::TextDelta { text } => print!("{text}"),
                _ => {}
            }
            std::io::stdout().flush()?;
        }
        accumulator.push(event)?;
    }

    let message = accumulator.finish()?;
    println!(
        "\n\n[{} content blocks, stop: {:?}]",
        message.content.len(),
        message.stop_reason
    );
    Ok(())
}
