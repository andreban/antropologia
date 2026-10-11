// Copyright 2026 Andre Cipriani Bandarra
// SPDX-License-Identifier: Apache-2.0

//! Server-Sent Events (SSE) decoder for streaming HTTP responses.

use reqwest::Response;
use std::mem;
use tokio_stream::{Stream, StreamExt};
use tokio_util::{
    bytes::BytesMut,
    codec::{Decoder, FramedRead, LinesCodec, LinesCodecError},
    io::StreamReader,
};
use tracing::warn;

const EVENT: &str = "event:";
const DATA: &str = "data:";
const ID: &str = "id:";
const RETRY: &str = "retry:";

/// Extension trait for converting an HTTP response into a stream of [`ServerSentEvent`]s.
pub trait EventSource {
    /// Consumes the response and returns a stream of parsed SSE events.
    fn event_stream(
        self,
    ) -> impl Stream<Item = std::result::Result<ServerSentEvent, LinesCodecError>>;
}

impl EventSource for Response {
    fn event_stream(
        self,
    ) -> impl Stream<Item = std::result::Result<ServerSentEvent, LinesCodecError>> {
        stream_response(self)
    }
}

/// A parsed Server-Sent Event.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ServerSentEvent {
    /// The event type (from the `event:` field).
    pub event: Option<String>,
    /// The event payload (from one or more `data:` fields, joined by `\n`).
    pub data: Option<String>,
    /// The event ID (from the `id:` field).
    pub id: Option<String>,
    /// The reconnection time in milliseconds (from the `retry:` field).
    pub retry: Option<usize>,
}

impl ServerSentEvent {
    fn is_empty(&self) -> bool {
        self.data.is_none() && self.event.is_none()
    }
}

/// A [`Decoder`] that parses a byte stream of SSE-formatted data into [`ServerSentEvent`]s.
pub struct ServerSentEventsCodec {
    lines_codec: LinesCodec,
    next: ServerSentEvent,
}

impl Default for ServerSentEventsCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerSentEventsCodec {
    /// Creates a new SSE codec.
    pub fn new() -> Self {
        Self {
            lines_codec: LinesCodec::new(),
            next: Default::default(),
        }
    }

    /// Applies one line to the event being built. Returns the finished event when the line is
    /// the blank line that dispatches it.
    fn process_line(&mut self, line: String) -> Option<ServerSentEvent> {
        if line.is_empty() {
            return (!self.next.is_empty()).then(|| mem::take(&mut self.next));
        }
        // Comment lines (e.g. keep-alive pings) start with ':'.
        if line.starts_with(':') {
            return None;
        }

        let field_value = |prefix: &str| {
            line.strip_prefix(prefix)
                .map(|v| v.strip_prefix(' ').unwrap_or(v).to_string())
        };
        if let Some(event) = field_value(EVENT) {
            self.next.event = Some(event);
        } else if let Some(data) = field_value(DATA) {
            match &mut self.next.data {
                Some(existing) => {
                    existing.push('\n');
                    existing.push_str(&data);
                }
                None => self.next.data = Some(data),
            }
        } else if let Some(id) = field_value(ID) {
            self.next.id = Some(id);
        } else if let Some(retry) = field_value(RETRY) {
            match retry.parse() {
                Ok(retry) => self.next.retry = Some(retry),
                Err(_) => warn!(retry, "Received invalid retry value"),
            }
        }
        None
    }
}

impl Decoder for ServerSentEventsCodec {
    type Item = ServerSentEvent;
    type Error = LinesCodecError;

    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        while let Some(line) = self.lines_codec.decode(src)? {
            if let Some(event) = self.process_line(line) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    fn decode_eof(
        &mut self,
        src: &mut BytesMut,
    ) -> std::result::Result<Option<Self::Item>, Self::Error> {
        while let Some(line) = self.lines_codec.decode_eof(src)? {
            if let Some(event) = self.process_line(line) {
                return Ok(Some(event));
            }
        }
        // Emit an event left without its terminating blank line.
        if !self.next.is_empty() {
            return Ok(Some(mem::take(&mut self.next)));
        }
        src.clear();
        Ok(None)
    }
}

/// Converts a [`Response`] into a stream of [`ServerSentEvent`]s.
pub fn stream_response(
    response: Response,
) -> impl Stream<Item = std::result::Result<ServerSentEvent, LinesCodecError>> {
    let bytes_stream = response.bytes_stream();
    let body_reader = StreamReader::new(bytes_stream.map(|res| res.map_err(std::io::Error::other)));
    FramedRead::new(body_reader, ServerSentEventsCodec::new())
}
