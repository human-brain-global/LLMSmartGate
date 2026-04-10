//! Shared helpers for OpenAI-compatible provider adapters.
//!
//! Both [`super::openai::OpenAIAdapter`] and [`super::vllm::VllmAdapter`] (and
//! future Azure OpenAI) share the same request/response format.  This module
//! extracts the common HTTP logic to avoid duplication.

// These are crate-internal helpers; full error docs are on the trait methods.
#![allow(clippy::missing_errors_doc)]

use std::time::Duration;

use tokio_stream::StreamExt as _;

use crate::models::ProviderRoute;
use crate::providers::traits::{
    ChatCompletionChunk, ChatCompletionsRequest, ChatCompletionsResponse, ChunkStream,
    ProviderError,
};
use crate::streaming::parser::parse_sse_stream;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `ProviderRoute.timeout_ms` to a `Duration`, defaulting to 30 s.
pub fn route_timeout(route: &ProviderRoute) -> Duration {
    Duration::from_millis(u64::try_from(route.timeout_ms).unwrap_or(30_000))
}

/// Build a JSON request body for an OpenAI-compatible endpoint, overriding
/// the `model` and `stream` fields.
pub fn build_chat_body(
    request: &ChatCompletionsRequest,
    route: &ProviderRoute,
    stream: bool,
    provider: &str,
) -> Result<serde_json::Value, ProviderError> {
    let mut body = serde_json::to_value(request).map_err(|e| ProviderError::BadRequest {
        provider: provider.into(),
        message: format!("failed to serialize request: {e}"),
    })?;

    body["model"] = serde_json::Value::String(route.provider_model_name.clone());
    body["stream"] = serde_json::Value::Bool(stream);

    // For streaming, inject stream_options to get usage in the final chunk.
    if stream {
        body["stream_options"] = serde_json::json!({ "include_usage": true });
    }

    Ok(body)
}

/// Build a JSON request body from an arbitrary serializable value, overriding
/// the `model` field.
pub fn build_generic_body(
    request: &impl serde::Serialize,
    route: &ProviderRoute,
    provider: &str,
) -> Result<serde_json::Value, ProviderError> {
    let mut body = serde_json::to_value(request).map_err(|e| ProviderError::BadRequest {
        provider: provider.into(),
        message: format!("failed to serialize request: {e}"),
    })?;
    body["model"] = serde_json::Value::String(route.provider_model_name.clone());
    Ok(body)
}

/// Send a POST request to an OpenAI-compatible endpoint.
///
/// Optionally includes an `Authorization: Bearer` header if `api_key` is provided.
pub async fn send_request(
    client: &reqwest::Client,
    url: &str,
    api_key: Option<&str>,
    body: &serde_json::Value,
    timeout: Duration,
    provider: &str,
) -> Result<reqwest::Response, ProviderError> {
    let mut builder = client
        .post(url)
        .header("Content-Type", "application/json")
        .timeout(timeout)
        .json(body);

    if let Some(key) = api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }

    builder.send().await.map_err(|e| {
        if e.is_timeout() {
            ProviderError::Timeout {
                provider: provider.into(),
            }
        } else {
            ProviderError::ConnectionFailed {
                provider: provider.into(),
            }
        }
    })
}

/// Check the response status, extracting the `Retry-After` header on 429.
/// Returns the response on success, or a [`ProviderError`] on non-2xx.
pub async fn check_response(
    response: reqwest::Response,
    provider: &str,
    extract_error_message: fn(&str) -> String,
) -> Result<reqwest::Response, ProviderError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    // Extract Retry-After header before consuming the body.
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok());

    let body = response.text().await.unwrap_or_else(|e| {
        tracing::debug!(error = %e, "failed to read error response body");
        String::new()
    });

    Err(map_error(
        status.as_u16(),
        &body,
        provider,
        retry_after,
        extract_error_message,
    ))
}

/// Map an HTTP error status + body to a [`ProviderError`].
pub fn map_error(
    status: u16,
    body: &str,
    provider: &str,
    retry_after: Option<u32>,
    extract_error_message: fn(&str) -> String,
) -> ProviderError {
    match status {
        401 | 403 => ProviderError::AuthFailure {
            provider: provider.into(),
        },
        429 => ProviderError::RateLimited {
            provider: provider.into(),
            retry_after,
        },
        400 | 404 => ProviderError::BadRequest {
            provider: provider.into(),
            message: extract_error_message(body),
        },
        s => ProviderError::ServerError {
            provider: provider.into(),
            status: s,
        },
    }
}

/// Read a response body as text, returning a [`ProviderError`] on failure.
pub async fn read_response_body(
    response: reqwest::Response,
    provider: &str,
) -> Result<String, ProviderError> {
    response
        .text()
        .await
        .map_err(|_| ProviderError::ConnectionFailed {
            provider: provider.into(),
        })
}

/// Parse a response body as JSON, logging a warning on failure.
pub fn parse_response<T: serde::de::DeserializeOwned>(
    body: &str,
    provider: &str,
    endpoint: &str,
) -> Result<T, ProviderError> {
    serde_json::from_str(body).map_err(|e| {
        tracing::warn!(
            error = %e,
            body_prefix = &body[..body.len().min(200)],
            "failed to parse {provider} {endpoint} response"
        );
        ProviderError::ServerError {
            provider: provider.into(),
            status: 500,
        }
    })
}

/// Execute a non-streaming chat completion against an OpenAI-compatible endpoint.
pub async fn chat_completion(
    client: &reqwest::Client,
    route: &ProviderRoute,
    request: &ChatCompletionsRequest,
    url: &str,
    api_key: Option<&str>,
    provider: &str,
    extract_error_msg: fn(&str) -> String,
) -> Result<ChatCompletionsResponse, ProviderError> {
    let body = build_chat_body(request, route, false, provider)?;
    let timeout = route_timeout(route);

    let response = send_request(client, url, api_key, &body, timeout, provider).await?;
    let response = check_response(response, provider, extract_error_msg).await?;
    let text = read_response_body(response, provider).await?;
    parse_response(&text, provider, "chat/completions")
}

/// Execute a streaming chat completion against an OpenAI-compatible endpoint.
pub async fn chat_completion_stream(
    client: &reqwest::Client,
    route: &ProviderRoute,
    request: &ChatCompletionsRequest,
    url: &str,
    api_key: Option<&str>,
    provider: &str,
    extract_error_msg: fn(&str) -> String,
) -> Result<ChunkStream, ProviderError> {
    let body = build_chat_body(request, route, true, provider)?;
    let timeout = route_timeout(route);

    let response = send_request(client, url, api_key, &body, timeout, provider).await?;
    let response = check_response(response, provider, extract_error_msg).await?;

    let byte_stream = response.bytes_stream();
    let sse_stream = parse_sse_stream(byte_stream, provider.to_owned());
    let provider_name = provider.to_owned();

    let chunk_stream = sse_stream.filter_map(move |result| match result {
        Ok(event) => {
            if event.is_done() {
                None
            } else {
                match serde_json::from_str::<ChatCompletionChunk>(&event.data) {
                    Ok(chunk) => Some(Ok(chunk)),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            data_prefix = &event.data[..event.data.len().min(200)],
                            provider = %provider_name,
                            "failed to parse streaming chunk"
                        );
                        // Fail-safe (AP-5): surface parse errors to the client
                        // rather than silently dropping chunks.
                        Some(Err(ProviderError::ServerError {
                            provider: provider_name.clone(),
                            status: 500,
                        }))
                    }
                }
            }
        }
        Err(e) => Some(Err(e)),
    });

    Ok(Box::pin(chunk_stream) as ChunkStream)
}

/// Extract an error message from an OpenAI JSON error body.
///
/// Falls back to the raw body if the JSON structure is not recognised.
pub fn extract_openai_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or_else(|| body.to_string())
}

/// Identity error message extractor — returns the raw body as-is.
pub fn identity_error_message(body: &str) -> String {
    body.to_string()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_timeout_positive() {
        let route = crate::models::ProviderRoute {
            id: crate::types::RouteId::new(),
            tenant_id: None,
            model_alias: "test".into(),
            provider: crate::types::Provider::Openai,
            provider_model_name: "gpt-4".into(),
            priority: 100,
            enabled: true,
            timeout_ms: 5000,
            max_retries: 1,
            retry_backoff_ms: 250,
            config: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(route_timeout(&route), Duration::from_millis(5000));
    }

    #[test]
    fn route_timeout_negative_uses_default() {
        let route = crate::models::ProviderRoute {
            id: crate::types::RouteId::new(),
            tenant_id: None,
            model_alias: "test".into(),
            provider: crate::types::Provider::Openai,
            provider_model_name: "gpt-4".into(),
            priority: 100,
            enabled: true,
            timeout_ms: -1,
            max_retries: 1,
            retry_backoff_ms: 250,
            config: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(route_timeout(&route), Duration::from_millis(30_000));
    }

    #[test]
    fn map_error_429_with_retry_after() {
        let err = map_error(429, "", "test", Some(30), identity_error_message);
        match err {
            ProviderError::RateLimited { retry_after, .. } => {
                assert_eq!(retry_after, Some(30));
            }
            other => panic!("expected RateLimited, got: {other:?}"),
        }
    }

    #[test]
    fn map_error_400_uses_extractor() {
        let body = r#"{"error":{"message":"bad model"}}"#;
        let err = map_error(400, body, "test", None, extract_openai_error_message);
        match err {
            ProviderError::BadRequest { message, .. } => {
                assert_eq!(message, "bad model");
            }
            other => panic!("expected BadRequest, got: {other:?}"),
        }
    }

    #[test]
    fn map_error_500_single_arm() {
        let err = map_error(500, "", "test", None, identity_error_message);
        assert!(matches!(
            err,
            ProviderError::ServerError { status: 500, .. }
        ));

        // Non-standard status also maps to ServerError.
        let err2 = map_error(418, "", "test", None, identity_error_message);
        assert!(matches!(
            err2,
            ProviderError::ServerError { status: 418, .. }
        ));
    }

    #[test]
    fn extract_openai_error_message_valid() {
        let body = r#"{"error":{"message":"Rate limit exceeded"}}"#;
        assert_eq!(extract_openai_error_message(body), "Rate limit exceeded");
    }

    #[test]
    fn extract_openai_error_message_fallback() {
        assert_eq!(extract_openai_error_message("plain"), "plain");
    }
}
