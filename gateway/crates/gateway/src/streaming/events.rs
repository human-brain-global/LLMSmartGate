//! Server-Sent Events (SSE) types for provider streaming responses.

/// A parsed SSE event from an upstream provider byte stream.
///
/// Follows the [SSE specification](https://html.spec.whatwg.org/multipage/server-sent-events.html):
/// - `event:` sets the event type
/// - `data:` carries the payload (multiple lines concatenated with `\n`)
/// - `id:` sets the last event ID
/// - Lines starting with `:` are comments (ignored by the parser)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The event type from the `event:` field, if present.
    pub event_type: Option<String>,
    /// The concatenated data from all `data:` lines, joined with `\n`.
    pub data: String,
    /// The last event ID from the `id:` field, if present.
    pub id: Option<String>,
}

impl SseEvent {
    /// Returns `true` if this is the OpenAI `[DONE]` sentinel event.
    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_done_detects_sentinel() {
        let event = SseEvent {
            event_type: None,
            data: "[DONE]".into(),
            id: None,
        };
        assert!(event.is_done());
    }

    #[test]
    fn is_done_trims_whitespace() {
        let event = SseEvent {
            event_type: None,
            data: " [DONE] ".into(),
            id: None,
        };
        assert!(event.is_done());
    }

    #[test]
    fn is_done_false_for_json() {
        let event = SseEvent {
            event_type: None,
            data: r#"{"id":"chatcmpl-123"}"#.into(),
            id: None,
        };
        assert!(!event.is_done());
    }
}
