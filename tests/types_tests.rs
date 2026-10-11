// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

use antropologia::prelude::*;
use antropologia::types::stream::{DeltaUsage, MessageDeltaBody};
use serde_json::json;

#[test]
fn request_serializes_to_messages_api_shape() -> anyhow::Result<()> {
    let request = MessagesRequest::builder("claude-opus-5-5")
        .system("Be brief.")
        .messages(vec![
            Message::user("Weather in Paris?"),
            Message::with_blocks(
                Role::Assistant,
                vec![ContentBlock::tool_use(
                    "toolu_1",
                    "get_weather",
                    json!({"city": "Paris"}),
                )],
            ),
            Message::with_blocks(
                Role::User,
                vec![ContentBlock::tool_error("toolu_1", "service down")],
            ),
        ])
        .tools(vec![Tool::new(
            "get_weather",
            "Current weather for a city",
            json!({"type": "object", "properties": {"city": {"type": "string"}}}),
        )])
        .thinking(ThinkingConfig::adaptive_summarized())
        .effort(Effort::High)
        .build();

    let body = serde_json::to_value(&request)?;
    assert_eq!(
        body,
        json!({
            "model": "claude-opus-5-5",
            "max_tokens": 16000,
            "system": "Be brief.",
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "Weather in Paris?"}]},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {"city": "Paris"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": "service down", "is_error": true}
                ]}
            ],
            "tools": [{
                "name": "get_weather",
                "description": "Current weather for a city",
                "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}}
            }],
            "thinking": {"type": "adaptive", "display": "summarized"},
            "output_config": {"effort": "high"}
        })
    );
    Ok(())
}

#[test]
fn json_schema_format_closes_objects_and_drops_schema_keyword() -> anyhow::Result<()> {
    let request = MessagesRequest::builder("claude-opus-5-5")
        .json_schema(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "items": {"type": "array", "items": {"type": "object", "properties": {"id": {"type": "string"}}}},
                "extra": {"type": "object", "additionalProperties": true}
            }
        }))
        .build();

    let format = &serde_json::to_value(&request)?["output_config"]["format"];
    assert_eq!(format["type"], "json_schema");
    assert!(format["schema"].get("$schema").is_none());
    assert_eq!(format["schema"]["additionalProperties"], false);
    assert_eq!(
        format["schema"]["properties"]["items"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        format["schema"]["properties"]["extra"]["additionalProperties"],
        true
    );
    Ok(())
}

#[test]
fn response_helpers_read_text_thinking_and_tool_uses() -> anyhow::Result<()> {
    let response: MessagesResponse = serde_json::from_value(json!({
        "id": "msg_1",
        "type": "message",
        "role": "assistant",
        "model": "claude-opus-5-5",
        "content": [
            {"type": "thinking", "thinking": "Check the weather.", "signature": "sig"},
            {"type": "text", "text": "Let me look."},
            {"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {"city": "Paris"}}
        ],
        "stop_reason": "tool_use",
        "stop_sequence": null,
        "usage": {"input_tokens": 10, "output_tokens": 20, "cache_read_input_tokens": 90}
    }))?;

    assert_eq!(response.text(), "Let me look.");
    assert_eq!(response.thinking().as_deref(), Some("Check the weather."));
    assert_eq!(
        response.tool_uses(),
        vec![("toolu_1", "get_weather", &json!({"city": "Paris"}))]
    );
    assert_eq!(response.stop_reason, Some(StopReason::ToolUse));
    assert_eq!(response.usage.total_input_tokens(), 100);
    Ok(())
}

#[test]
fn unknown_blocks_and_stop_reasons_round_trip() -> anyhow::Result<()> {
    let raw = json!({
        "id": "msg_1",
        "role": "assistant",
        "model": "claude-opus-5-5",
        "content": [
            {"type": "server_tool_use", "id": "srv_1", "name": "web_search", "input": {"query": "x"}},
            {"type": "redacted_thinking", "data": "abc"}
        ],
        "stop_reason": "some_future_reason",
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });
    let response: MessagesResponse = serde_json::from_value(raw.clone())?;

    assert_eq!(
        response.content[0],
        ContentBlock::Other(raw["content"][0].clone())
    );
    assert_eq!(
        response.content[1],
        ContentBlock::RedactedThinking { data: "abc".into() }
    );
    assert_eq!(response.stop_reason, Some(StopReason::Unknown));
    // Replaying the content sends the unknown block back unchanged.
    assert_eq!(serde_json::to_value(&response.content)?, raw["content"]);
    Ok(())
}

fn start_event() -> anyhow::Result<StreamEvent> {
    Ok(serde_json::from_value(json!({
        "type": "message_start",
        "message": {
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-opus-5-5",
            "content": [], "stop_reason": null, "stop_sequence": null,
            "usage": {"input_tokens": 12, "output_tokens": 1}
        }
    }))?)
}

#[test]
fn accumulator_rebuilds_thinking_text_and_tool_use() -> anyhow::Result<()> {
    let events: Vec<StreamEvent> = [
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "", "signature": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "Need "}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "weather."}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig"}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Checking."}}),
        json!({"type": "content_block_stop", "index": 1}),
        json!({"type": "content_block_start", "index": 2, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {}}}),
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": "{\"city\": "}}),
        json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": "\"Paris\"}"}}),
        json!({"type": "content_block_stop", "index": 2}),
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use", "stop_sequence": null}, "usage": {"output_tokens": 42}}),
        json!({"type": "message_stop"}),
    ]
    .into_iter()
    .map(serde_json::from_value)
    .collect::<std::result::Result<_, serde_json::Error>>()?;

    let mut acc = MessageAccumulator::new();
    acc.push(start_event()?)?;
    for event in events {
        acc.push(event)?;
    }
    let message = acc.finish()?;

    assert_eq!(
        message.content,
        vec![
            ContentBlock::Thinking {
                thinking: "Need weather.".into(),
                signature: "sig".into()
            },
            ContentBlock::text("Checking."),
            ContentBlock::tool_use("toolu_1", "get_weather", json!({"city": "Paris"})),
        ]
    );
    assert_eq!(message.stop_reason, Some(StopReason::ToolUse));
    assert_eq!(message.usage.input_tokens, 12);
    assert_eq!(message.usage.output_tokens, 42);
    Ok(())
}

#[test]
fn accumulator_gives_tool_use_without_input_an_empty_object() -> anyhow::Result<()> {
    let mut acc = MessageAccumulator::new();
    acc.push(start_event()?)?;
    acc.push(StreamEvent::ContentBlockStart {
        index: 0,
        content_block: ContentBlock::tool_use("toolu_1", "list", json!({})),
    })?;
    acc.push(StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::InputJsonDelta {
            partial_json: String::new(),
        },
    })?;
    acc.push(StreamEvent::ContentBlockStop { index: 0 })?;
    acc.push(StreamEvent::MessageDelta {
        delta: MessageDeltaBody::default(),
        usage: DeltaUsage::default(),
    })?;

    assert_eq!(acc.finish()?.tool_uses()[0].2, &json!({}));
    Ok(())
}

#[test]
fn accumulator_rejects_events_before_message_start() {
    let mut acc = MessageAccumulator::new();
    let result = acc.push(StreamEvent::ContentBlockStop { index: 0 });
    assert!(
        result.is_ok(),
        "a stop without partial JSON touches nothing"
    );

    let result = acc.push(StreamEvent::ContentBlockStart {
        index: 0,
        content_block: ContentBlock::text(""),
    });
    assert!(matches!(result, Err(AntropologiaError::Stream(_))));
}
