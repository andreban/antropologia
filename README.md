# Antropologia

An ergonomic, async Rust client for the [Anthropic Claude](https://platform.claude.com/docs) Messages API.

Named after *anthropology*, for Anthropic, following the naming tradition of [`geologia`](https://github.com/andreban/geologia) (Google AI) and [`cetologia`](https://github.com/andreban/cetologia) (DeepSeek).

## Features

- **Messages API**: typed requests and responses with builders, and helpers for text, thinking and tool calls.
- **Streaming**: Server-Sent Events as typed `StreamEvent`s, and a `MessageAccumulator` that rebuilds the full message.
- **Tool use**: client tools, `tool_choice`, and tool results with `is_error`.
- **Thinking and effort**: adaptive thinking, summarized or omitted thinking display, and `output_config.effort`.
- **Lossless replay**: thinking blocks keep their signatures, and block types the crate doesn't model round-trip unchanged as raw JSON, so assistant turns can always be sent back as received.
- **Structured output**: JSON Schema output; `json_schema` adds the `additionalProperties: false` the API requires.
- **Prompt caching**: `cache_control` breakpoints, and cache read/write counts in `usage`.
- **Errors**: API errors carry their type, message and request ID, and `is_retryable()` flags 429, 5xx and overload errors, and failures to connect or timeouts.
- **Small**: built on `reqwest`, `serde` and `tokio`.

## Quick Start

```rust
use antropologia::prelude::*;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let client = AntropologiaClient::new(std::env::var("ANTHROPIC_API_KEY")?);

    let request = MessagesRequest::builder("claude-opus-5-5")
        .system("You are a helpful assistant.")
        .messages(vec![Message::user("Explain what anthropology studies.")])
        .build();

    let response = client.messages(&request).await?;
    println!("{}", response.text());
    Ok(())
}
```

`max_tokens` defaults to 16,000; set it with `.max_tokens(...)`.

## Thinking and Effort

Current models think adaptively. Control depth with effort, and ask for summarized thinking to read it:

```rust
let request = MessagesRequest::builder("claude-opus-5-5")
    .thinking(ThinkingConfig::adaptive_summarized())
    .effort(Effort::High)
    .messages(vec![Message::user("Which is larger: 9.9 or 9.11?")])
    .build();
```

When continuing a conversation, send the assistant's content blocks back unchanged, thinking blocks included: `Message::with_blocks(Role::Assistant, response.content)`.

## Streaming

```rust
use antropologia::prelude::*;
use tokio_stream::StreamExt;

let mut stream = client.messages_stream(&request).await?;
let mut accumulator = MessageAccumulator::new();
while let Some(event) = stream.next().await {
    let event = event?;
    if let StreamEvent::ContentBlockDelta { delta: ContentDelta::TextDelta { text }, .. } = &event {
        print!("{text}");
    }
    accumulator.push(event)?;
}
let message = accumulator.finish()?;
```

An `error` event mid-stream (for example `overloaded_error`) arrives as an `Err`.

## Examples

The examples load your API key from a `.env` file using [`dotenvy`](https://crates.io/crates/dotenvy), or from the `ANTHROPIC_API_KEY` environment variable.

1. Copy [`.env.example`](.env.example) to `.env` and add your API key:
   ```bash
   cp .env.example .env
   ```

2. Run any example with `cargo run --example <name>`:

| Example | Description | Command |
|---|---|---|
| **Basic Message** | One request, with token usage | `cargo run --example basic_message` |
| **Streaming** | Thinking and text as they arrive, then the full message from the accumulator | `cargo run --example streaming` |
| **Tool Use** | A manual tool loop that replays assistant turns and returns tool results | `cargo run --example tool_use` |
| **Structured Output** | JSON Schema output deserialized into a Rust struct | `cargo run --example structured_output` |

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE)).
