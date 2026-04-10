//! Provider-agnostic SSE byte-stream parser.
//!
//! Converts a raw byte stream (from `reqwest::Response::bytes_stream()`) into a
//! stream of [`SseEvent`] values.  The parser is driven inline by the consumer's
//! poll — no `tokio::spawn` — so the upstream byte stream is dropped immediately
//! when the consumer disconnects (per ARC-001 §10.3).

use bytes::Bytes;
use futures_core::Stream;

use crate::providers::ProviderError;
use crate::streaming::events::SseEvent;

/// Parse a raw SSE byte stream into a stream of [`SseEvent`] values.
///
/// The returned stream is lazy: it reads from the upstream byte stream only
/// when the consumer polls.  When the consumer drops, the upstream is dropped
/// too — no zombie tasks.
pub fn parse_sse_stream(
    byte_stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
    provider_name: String,
) -> impl Stream<Item = Result<SseEvent, ProviderError>> + Send {
    SseParserStream::new(byte_stream, provider_name)
}

// ---------------------------------------------------------------------------
// Inline stream implementation
// ---------------------------------------------------------------------------

use std::pin::Pin;
use std::task::{Context, Poll};

/// A `Stream` adapter that buffers incoming bytes, splits on `\n\n`
/// boundaries, and yields parsed [`SseEvent`] values.
struct SseParserStream<S> {
    inner: Pin<Box<S>>,
    provider_name: String,
    buffer: String,
    /// Parsed events ready to yield (buffered because one byte chunk may
    /// contain multiple complete SSE events).
    pending: std::collections::VecDeque<SseEvent>,
    /// Whether we've stripped the UTF-8 BOM from the first chunk.
    bom_stripped: bool,
    /// Whether the upstream stream has ended.
    done: bool,
}

impl<S> SseParserStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send,
{
    fn new(inner: S, provider_name: String) -> Self {
        Self {
            inner: Box::pin(inner),
            provider_name,
            buffer: String::new(),
            pending: std::collections::VecDeque::new(),
            bom_stripped: false,
            done: false,
        }
    }

    /// Extract all complete SSE events from the buffer into `self.pending`.
    fn drain_events(&mut self) {
        while let Some(boundary) = find_event_boundary(&self.buffer) {
            let event_block = self.buffer[..boundary].to_string();
            // Skip past the delimiter (\n\n or \r\n\r\n).
            let rest = &self.buffer[boundary..];
            let rest = rest
                .strip_prefix("\r\n\r\n")
                .or_else(|| rest.strip_prefix("\n\n"))
                .unwrap_or(rest.trim_start_matches('\n'));
            self.buffer = rest.to_string();

            if let Some(event) = parse_event_block(&event_block) {
                self.pending.push_back(event);
            }
        }
    }
}

impl<S> Stream for SseParserStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send,
{
    type Item = Result<SseEvent, ProviderError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Yield any buffered events first.
        if let Some(event) = this.pending.pop_front() {
            return Poll::Ready(Some(Ok(event)));
        }

        if this.done {
            return Poll::Ready(None);
        }

        // Poll the upstream for more bytes.
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                let text = if this.bom_stripped {
                    String::from_utf8_lossy(&bytes).into_owned()
                } else {
                    this.bom_stripped = true;
                    let s = String::from_utf8_lossy(&bytes);
                    s.strip_prefix('\u{FEFF}').unwrap_or(&s).to_string()
                };

                this.buffer.push_str(&text);
                this.drain_events();

                if let Some(event) = this.pending.pop_front() {
                    Poll::Ready(Some(Ok(event)))
                } else {
                    // No complete event yet — need more bytes.
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(_))) => {
                this.done = true;
                Poll::Ready(Some(Err(ProviderError::ConnectionFailed {
                    provider: std::mem::take(&mut this.provider_name),
                })))
            }
            Poll::Ready(None) => {
                this.done = true;
                // Process any remaining data in the buffer.
                if !this.buffer.trim().is_empty() {
                    if let Some(event) = parse_event_block(&this.buffer) {
                        this.buffer.clear();
                        return Poll::Ready(Some(Ok(event)));
                    }
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// Event parsing helpers
// ---------------------------------------------------------------------------

/// Find the position of the first `\n\n` (or `\r\n\r\n`) event boundary.
fn find_event_boundary(buffer: &str) -> Option<usize> {
    if let Some(pos) = buffer.find("\n\n") {
        return Some(pos);
    }
    if let Some(pos) = buffer.find("\r\n\r\n") {
        return Some(pos);
    }
    None
}

/// Parse a single SSE event block into an [`SseEvent`].
/// Returns `None` for empty or comment-only blocks.
fn parse_event_block(block: &str) -> Option<SseEvent> {
    let mut event_type: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    let mut id: Option<String> = None;

    for line in block.lines() {
        if line.starts_with(':') {
            continue;
        }

        if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.strip_prefix(' ').unwrap_or(value));
        } else if let Some(value) = line.strip_prefix("event:") {
            event_type = Some(value.strip_prefix(' ').unwrap_or(value).to_string());
        } else if let Some(value) = line.strip_prefix("id:") {
            id = Some(value.strip_prefix(' ').unwrap_or(value).to_string());
        }
    }

    if data_lines.is_empty() {
        return None;
    }

    Some(SseEvent {
        event_type,
        data: data_lines.join("\n"),
        id,
    })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // parse_event_block unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_simple_data_event() {
        let event = parse_event_block("data: hello world").expect("should parse");
        assert_eq!(event.data, "hello world");
        assert!(event.event_type.is_none());
        assert!(event.id.is_none());
    }

    #[test]
    fn parse_multiline_data() {
        let event = parse_event_block("data: line1\ndata: line2\ndata: line3").expect("parse");
        assert_eq!(event.data, "line1\nline2\nline3");
    }

    #[test]
    fn parse_event_with_type_and_id() {
        let event = parse_event_block("event: message\nid: 42\ndata: payload").expect("parse");
        assert_eq!(event.event_type.as_deref(), Some("message"));
        assert_eq!(event.id.as_deref(), Some("42"));
        assert_eq!(event.data, "payload");
    }

    #[test]
    fn parse_ignores_comments() {
        let event = parse_event_block(": this is a comment\ndata: actual data").expect("parse");
        assert_eq!(event.data, "actual data");
    }

    #[test]
    fn parse_empty_block_returns_none() {
        assert!(parse_event_block("").is_none());
    }

    #[test]
    fn parse_comment_only_returns_none() {
        assert!(parse_event_block(": just a comment").is_none());
    }

    #[test]
    fn parse_data_without_space_after_colon() {
        let event = parse_event_block("data:no-space").expect("parse");
        assert_eq!(event.data, "no-space");
    }

    #[test]
    fn parse_done_sentinel() {
        let event = parse_event_block("data: [DONE]").expect("parse");
        assert!(event.is_done());
    }

    // -----------------------------------------------------------------------
    // find_event_boundary
    // -----------------------------------------------------------------------

    #[test]
    fn find_boundary_double_newline() {
        assert_eq!(find_event_boundary("data: hello\n\ndata: world"), Some(11));
    }

    #[test]
    fn find_boundary_crlf() {
        assert_eq!(
            find_event_boundary("data: hello\r\n\r\ndata: world"),
            Some(11)
        );
    }

    #[test]
    fn find_boundary_no_boundary() {
        assert_eq!(find_event_boundary("data: incomplete"), None);
    }

    // -----------------------------------------------------------------------
    // Integration: parse_sse_stream
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn parse_stream_single_event() {
        use tokio_stream::StreamExt as _;

        let chunks = vec![Ok(Bytes::from("data: hello\n\n"))];
        let byte_stream = tokio_stream::iter(chunks);

        let mut stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        let event = stream.next().await.expect("one event").expect("ok");
        assert_eq!(event.data, "hello");
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn parse_stream_multiple_events() {
        use tokio_stream::StreamExt as _;

        let chunks = vec![Ok(Bytes::from(
            "data: first\n\ndata: second\n\ndata: [DONE]\n\n",
        ))];
        let byte_stream = tokio_stream::iter(chunks);

        let stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        let events: Vec<_> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("all ok");

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
        assert!(events[2].is_done());
    }

    #[tokio::test]
    async fn parse_stream_chunked_across_boundaries() {
        use tokio_stream::StreamExt as _;

        let chunks = vec![Ok(Bytes::from("data: hel")), Ok(Bytes::from("lo\n\n"))];
        let byte_stream = tokio_stream::iter(chunks);

        let mut stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        let event = stream.next().await.expect("one event").expect("ok");
        assert_eq!(event.data, "hello");
    }

    #[tokio::test]
    async fn parse_stream_with_bom() {
        use tokio_stream::StreamExt as _;

        let chunks = vec![Ok(Bytes::from("\u{FEFF}data: bom\n\n"))];
        let byte_stream = tokio_stream::iter(chunks);

        let mut stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        let event = stream.next().await.expect("one event").expect("ok");
        assert_eq!(event.data, "bom");
    }

    #[tokio::test]
    async fn parse_stream_connection_error() {
        use tokio_stream::StreamExt as _;

        // Install TLS provider required by reqwest.
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });

        // Get a real reqwest::Error by sending to a refused port.
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/unreachable")
            .send()
            .await
            .expect_err("connection should be refused");
        let chunks: Vec<Result<Bytes, reqwest::Error>> = vec![Err(err)];
        let byte_stream = tokio_stream::iter(chunks);

        let mut stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        let result = stream.next().await.expect("one item");
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::ConnectionFailed { .. }
        ));
    }

    #[tokio::test]
    async fn parse_stream_with_event_type() {
        use tokio_stream::StreamExt as _;

        let chunks = vec![Ok(Bytes::from(
            "event: response.created\ndata: {\"id\":\"resp_1\"}\n\n",
        ))];
        let byte_stream = tokio_stream::iter(chunks);

        let mut stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        let event = stream.next().await.expect("one event").expect("ok");
        assert_eq!(event.event_type.as_deref(), Some("response.created"));
        assert!(event.data.contains("resp_1"));
    }

    #[tokio::test]
    async fn consumer_drop_stops_upstream() {
        use tokio_stream::StreamExt as _;

        // A stream that yields many events — dropping the consumer mid-way
        // should NOT leave a zombie task.
        use std::fmt::Write as _;

        let mut body = String::new();
        for i in 0..100 {
            let _ = write!(body, "data: event-{i}\n\n");
        }
        let chunks = vec![Ok(Bytes::from(body))];
        let byte_stream = tokio_stream::iter(chunks);

        let mut stream = std::pin::pin!(parse_sse_stream(byte_stream, "test".into()));
        // Read only the first event — the rest are never consumed.
        let first = stream.next().await.expect("first").expect("ok");
        assert_eq!(first.data, "event-0");
        // No zombie task since there's no tokio::spawn — stream is dropped here.
    }
}
