//! Streaming relay -- wraps a [`ChunkStream`] to emit SSE bytes while
//! accumulating token usage from the final chunk.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures_core::Stream;
use tokio_stream::StreamExt as _;

use crate::providers::traits::{ChunkStream, ProviderError, UsageData};

/// Wrap a [`ChunkStream`] into:
///
/// 1. A byte stream suitable for an Axum SSE response (`data: {json}\n\n`)
/// 2. A shared handle to read the accumulated usage after the stream completes
///
/// Usage data is extracted from any chunk that carries a `usage` field
/// (OpenAI sends it in the final chunk when `stream_options.include_usage: true`).
pub fn relay_with_usage(
    stream: ChunkStream,
) -> (
    impl Stream<Item = Result<Bytes, ProviderError>> + Send,
    Arc<Mutex<Option<UsageData>>>,
) {
    // std::sync::Mutex (not tokio::sync::Mutex) is correct here: the critical
    // section is a single pointer-swap with no .await, so holding it never
    // blocks the Tokio runtime.  tokio::sync::Mutex would add unnecessary overhead.
    let usage_handle: Arc<Mutex<Option<UsageData>>> = Arc::new(Mutex::new(None));
    let usage_writer = Arc::clone(&usage_handle);

    let byte_stream = stream.map(move |result| match result {
        Ok(chunk) => {
            // If this chunk carries usage, store it.
            if let Some(ref usage) = chunk.usage {
                if let Ok(mut guard) = usage_writer.lock() {
                    *guard = Some(UsageData::from(usage.clone()));
                }
            }

            // Serialize the chunk back to SSE format.
            match serde_json::to_string(&chunk) {
                Ok(json) => Ok(Bytes::from(format!("data: {json}\n\n"))),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to serialize chunk for relay");
                    // Skip this chunk rather than killing the stream.
                    Ok(Bytes::from(""))
                }
            }
        }
        Err(e) => Err(e),
    });

    (byte_stream, usage_handle)
}

/// Extract [`UsageData`] from a non-streaming chat completions response.
///
/// Returns `None` if the response did not include usage data, logging a warning.
pub fn extract_non_streaming_usage(
    usage: Option<&crate::providers::traits::Usage>,
) -> Option<UsageData> {
    if let Some(u) = usage {
        Some(UsageData::from(u.clone()))
    } else {
        tracing::warn!("non-streaming response missing usage data");
        None
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::traits::{ChatCompletionChunk, ChunkChoice, Delta, Usage};

    fn make_chunk(content: &str, usage: Option<Usage>) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "chatcmpl-123".into(),
            object: "chat.completion.chunk".into(),
            created: 1_700_000_000,
            model: "gpt-4".into(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: None,
                    content: Some(content.into()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage,
            extra: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn relay_emits_sse_bytes() {
        let chunks = vec![
            Ok(make_chunk("Hello", None)),
            Ok(make_chunk(" world", None)),
        ];
        let chunk_stream: ChunkStream = Box::pin(tokio_stream::iter(chunks));

        let (byte_stream, usage_handle) = relay_with_usage(chunk_stream);
        let bytes: Vec<_> = tokio_stream::StreamExt::collect::<Vec<_>>(byte_stream)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("all ok");

        assert_eq!(bytes.len(), 2);
        let first = String::from_utf8_lossy(&bytes[0]);
        assert!(first.starts_with("data: "));
        assert!(first.ends_with("\n\n"));
        assert!(first.contains("Hello"));

        // No usage in these chunks.
        assert!(usage_handle.lock().expect("lock").is_none());
    }

    #[tokio::test]
    async fn relay_accumulates_usage_from_final_chunk() {
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let chunks = vec![
            Ok(make_chunk("Hello", None)),
            Ok(make_chunk("", Some(usage))),
        ];
        let chunk_stream: ChunkStream = Box::pin(tokio_stream::iter(chunks));

        let (byte_stream, usage_handle) = relay_with_usage(chunk_stream);

        // Consume the stream.
        let _: Vec<_> = tokio_stream::StreamExt::collect::<Vec<_>>(byte_stream).await;

        let usage_data = usage_handle
            .lock()
            .expect("lock")
            .clone()
            .expect("usage present");
        assert_eq!(usage_data.prompt_tokens, 10);
        assert_eq!(usage_data.completion_tokens, 20);
        assert_eq!(usage_data.total_tokens, 30);
    }

    #[tokio::test]
    async fn relay_no_usage_returns_none() {
        let chunks: Vec<Result<ChatCompletionChunk, ProviderError>> =
            vec![Ok(make_chunk("Hello", None))];
        let chunk_stream: ChunkStream = Box::pin(tokio_stream::iter(chunks));

        let (byte_stream, usage_handle) = relay_with_usage(chunk_stream);
        let _: Vec<_> = tokio_stream::StreamExt::collect::<Vec<_>>(byte_stream).await;

        assert!(usage_handle.lock().expect("lock").is_none());
    }

    #[test]
    fn extract_non_streaming_usage_present() {
        let usage = crate::providers::traits::Usage {
            prompt_tokens: 5,
            completion_tokens: 10,
            total_tokens: 15,
        };
        let result = extract_non_streaming_usage(Some(&usage));
        let data = result.expect("should be Some");
        assert_eq!(data.prompt_tokens, 5);
        assert_eq!(data.total_tokens, 15);
    }

    #[test]
    fn extract_non_streaming_usage_absent() {
        let result = extract_non_streaming_usage(None);
        assert!(result.is_none());
    }
}
