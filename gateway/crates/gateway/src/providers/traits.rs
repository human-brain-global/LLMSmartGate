//! Provider adapter trait, error types, normalized request/response types, and registry.
//!
//! The [`ProviderAdapter`] trait is the abstraction that all upstream LLM provider
//! implementations must satisfy.  Normalized types follow the OpenAI chat completions
//! schema so that the gateway presents a single, consistent API regardless of the
//! upstream provider.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use futures_core::Stream;
use serde::{Deserialize, Serialize};

use crate::models::ProviderRoute;
use crate::routing::retry::AttemptOutcome;
use crate::types::Provider;

// ===========================================================================
// ProviderError
// ===========================================================================

/// Errors returned by provider adapters.
///
/// Each variant carries the provider name for diagnostics.  The [`From`] impl
/// converts these into [`crate::error::GatewayError`] with the appropriate HTTP status code.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider '{provider}' timed out")]
    Timeout { provider: String },

    #[error("provider '{provider}' rate limited")]
    RateLimited {
        provider: String,
        retry_after: Option<u32>,
    },

    #[error("provider '{provider}' authentication failed")]
    AuthFailure { provider: String },

    #[error("provider '{provider}' bad request: {message}")]
    BadRequest { provider: String, message: String },

    #[error("provider '{provider}' server error (HTTP {status})")]
    ServerError { provider: String, status: u16 },

    #[error("failed to connect to provider '{provider}'")]
    ConnectionFailed { provider: String },
}

/// Convert a [`ProviderError`] into an [`AttemptOutcome`] for the retry/fallback
/// engine.  Transient errors are retryable; client errors are fatal.
pub fn classify_provider_error<T>(err: ProviderError) -> AttemptOutcome<T> {
    match &err {
        ProviderError::Timeout { .. }
        | ProviderError::RateLimited { .. }
        | ProviderError::ServerError { .. }
        | ProviderError::ConnectionFailed { .. } => AttemptOutcome::Retryable(err.into()),
        ProviderError::AuthFailure { .. } | ProviderError::BadRequest { .. } => {
            AttemptOutcome::Fatal(err.into())
        }
    }
}

// ===========================================================================
// Normalized request / response types (OpenAI-compatible)
// ===========================================================================

/// Normalized chat completions request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<serde_json::Value>,
    /// Pass-through for provider-specific fields not modelled above.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Normalized chat completions response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single choice in a chat completions response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// Token usage statistics (OpenAI chat completions format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

/// Unified token usage data extracted from any provider response.
///
/// This is the gateway-internal representation used by the metering pipeline.
#[derive(Debug, Clone, Default)]
pub struct UsageData {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl From<Usage> for UsageData {
    fn from(u: Usage) -> Self {
        Self {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }
    }
}

impl From<ResponsesUsage> for UsageData {
    fn from(u: ResponsesUsage) -> Self {
        Self {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.total_tokens,
        }
    }
}

impl From<EmbeddingsUsage> for UsageData {
    fn from(u: EmbeddingsUsage) -> Self {
        Self {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: 0,
            total_tokens: u.total_tokens,
        }
    }
}

/// Streaming chunk in the chat completions SSE stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    #[serde(default)]
    pub choices: Vec<ChunkChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single choice in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// Delta content in a streaming chunk choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
}

// ===========================================================================
// Responses API types
// ===========================================================================

/// Normalized Responses API request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Normalized Responses API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesResponse {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub output: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponsesUsage>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Token usage for the Responses API (uses `input_tokens` / `output_tokens`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

/// A single SSE event from a Responses API streaming response.
///
/// The `event_type` corresponds to the `event:` field in the SSE stream
/// (e.g. `response.created`, `response.output_text.delta`, `response.completed`).
#[derive(Debug, Clone)]
pub struct ResponsesStreamEvent {
    /// The SSE event type (e.g. `"response.output_text.delta"`).
    pub event_type: String,
    /// The raw JSON data payload.
    pub data: serde_json::Value,
}

/// A stream of Responses API SSE events.
pub type ResponsesChunkStream =
    Pin<Box<dyn Stream<Item = Result<ResponsesStreamEvent, ProviderError>> + Send>>;

// ===========================================================================
// Embeddings API types
// ===========================================================================

/// Normalized embeddings request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Normalized embeddings response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingsUsage,
}

/// A single embedding vector in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub index: u32,
    pub embedding: serde_json::Value,
}

/// Token usage for embeddings (no completion tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

// ===========================================================================
// Stream type aliases
// ===========================================================================

/// A stream of chat completion chunks emitted by a provider during streaming.
pub type ChunkStream =
    Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send>>;

// ===========================================================================
// ProviderAdapter trait
// ===========================================================================

/// Abstraction over an upstream LLM provider.
///
/// Methods return boxed futures for object safety so the registry can store
/// `Box<dyn ProviderAdapter>`.
pub trait ProviderAdapter: Send + Sync + 'static {
    /// Provider identifier (e.g. `"openai"`, `"anthropic"`).
    fn name(&self) -> &'static str;

    /// Execute a non-streaming chat completion request.
    fn chat_completion<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a ChatCompletionsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ChatCompletionsResponse, ProviderError>> + Send + 'a>>;

    /// Execute a streaming chat completion request, returning a chunk stream.
    fn chat_completion_stream<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a ChatCompletionsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ChunkStream, ProviderError>> + Send + 'a>>;

    /// Execute a non-streaming Responses API request.
    ///
    /// Default implementation returns "not supported".
    fn responses<'a>(
        &'a self,
        _client: &'a reqwest::Client,
        _route: &'a ProviderRoute,
        _request: &'a ResponsesRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ResponsesResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async {
            Err(ProviderError::BadRequest {
                provider: "unknown".into(),
                message: "responses API not supported by this provider".into(),
            })
        })
    }

    /// Execute a streaming Responses API request.
    ///
    /// Default implementation returns "not supported".
    fn responses_stream<'a>(
        &'a self,
        _client: &'a reqwest::Client,
        _route: &'a ProviderRoute,
        _request: &'a ResponsesRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ResponsesChunkStream, ProviderError>> + Send + 'a>>
    {
        Box::pin(async {
            Err(ProviderError::BadRequest {
                provider: "unknown".into(),
                message: "responses streaming not supported by this provider".into(),
            })
        })
    }

    /// Execute an embeddings request.
    ///
    /// Default implementation returns "not supported".
    fn embeddings<'a>(
        &'a self,
        _client: &'a reqwest::Client,
        _route: &'a ProviderRoute,
        _request: &'a EmbeddingsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<EmbeddingsResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async {
            Err(ProviderError::BadRequest {
                provider: "unknown".into(),
                message: "embeddings not supported by this provider".into(),
            })
        })
    }

    /// Map a provider HTTP error status + body to a [`ProviderError`].
    fn map_error(&self, status: u16, body: &str) -> ProviderError;
}

// ===========================================================================
// ProviderRegistry
// ===========================================================================

/// Registry of available provider adapters, keyed by [`Provider`] enum.
pub struct ProviderRegistry {
    adapters: HashMap<Provider, Box<dyn ProviderAdapter>>,
}

impl ProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Register an adapter for the given provider.
    pub fn register(&mut self, provider: Provider, adapter: Box<dyn ProviderAdapter>) {
        self.adapters.insert(provider, adapter);
    }

    /// Look up the adapter for a provider.
    pub fn get(&self, provider: &Provider) -> Option<&dyn ProviderAdapter> {
        self.adapters.get(provider).map(AsRef::as_ref)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("providers", &self.adapters.keys().collect::<Vec<_>>())
            .finish()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::GatewayError;
    use axum::http::StatusCode;

    // -----------------------------------------------------------------------
    // ProviderError Display
    // -----------------------------------------------------------------------

    #[test]
    fn provider_error_timeout_display() {
        let err = ProviderError::Timeout {
            provider: "openai".into(),
        };
        assert_eq!(err.to_string(), "provider 'openai' timed out");
    }

    #[test]
    fn provider_error_rate_limited_display() {
        let err = ProviderError::RateLimited {
            provider: "openai".into(),
            retry_after: Some(30),
        };
        assert_eq!(err.to_string(), "provider 'openai' rate limited");
    }

    #[test]
    fn provider_error_auth_failure_display() {
        let err = ProviderError::AuthFailure {
            provider: "openai".into(),
        };
        assert_eq!(err.to_string(), "provider 'openai' authentication failed");
    }

    #[test]
    fn provider_error_bad_request_display() {
        let err = ProviderError::BadRequest {
            provider: "openai".into(),
            message: "invalid model".into(),
        };
        assert_eq!(
            err.to_string(),
            "provider 'openai' bad request: invalid model"
        );
    }

    #[test]
    fn provider_error_server_error_display() {
        let err = ProviderError::ServerError {
            provider: "openai".into(),
            status: 500,
        };
        assert_eq!(err.to_string(), "provider 'openai' server error (HTTP 500)");
    }

    #[test]
    fn provider_error_connection_failed_display() {
        let err = ProviderError::ConnectionFailed {
            provider: "openai".into(),
        };
        assert_eq!(err.to_string(), "failed to connect to provider 'openai'");
    }

    // -----------------------------------------------------------------------
    // From<ProviderError> for GatewayError -- status codes
    // -----------------------------------------------------------------------

    #[test]
    fn provider_error_timeout_maps_to_gateway_timeout() {
        let gw: GatewayError = ProviderError::Timeout {
            provider: "test".into(),
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[test]
    fn provider_error_rate_limited_maps_to_429() {
        let gw: GatewayError = ProviderError::RateLimited {
            provider: "test".into(),
            retry_after: Some(60),
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn provider_error_auth_failure_maps_to_bad_gateway() {
        let gw: GatewayError = ProviderError::AuthFailure {
            provider: "test".into(),
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn provider_error_bad_request_maps_to_400() {
        let gw: GatewayError = ProviderError::BadRequest {
            provider: "test".into(),
            message: "bad".into(),
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn provider_error_server_error_maps_status() {
        let gw: GatewayError = ProviderError::ServerError {
            provider: "test".into(),
            status: 503,
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn provider_error_server_error_invalid_status_maps_to_bad_gateway() {
        // Status 0 is invalid (HTTP requires 100-999), so from_u16 fails
        // and the From impl falls back to BAD_GATEWAY.
        let gw: GatewayError = ProviderError::ServerError {
            provider: "test".into(),
            status: 0,
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn provider_error_connection_failed_maps_to_bad_gateway() {
        let gw: GatewayError = ProviderError::ConnectionFailed {
            provider: "test".into(),
        }
        .into();
        assert_eq!(gw.status_code(), StatusCode::BAD_GATEWAY);
    }

    // -----------------------------------------------------------------------
    // classify_provider_error
    // -----------------------------------------------------------------------

    #[test]
    fn classify_timeout_is_retryable() {
        let outcome: AttemptOutcome<()> = classify_provider_error(ProviderError::Timeout {
            provider: "test".into(),
        });
        assert!(matches!(outcome, AttemptOutcome::Retryable(_)));
    }

    #[test]
    fn classify_rate_limited_is_retryable() {
        let outcome: AttemptOutcome<()> = classify_provider_error(ProviderError::RateLimited {
            provider: "test".into(),
            retry_after: None,
        });
        assert!(matches!(outcome, AttemptOutcome::Retryable(_)));
    }

    #[test]
    fn classify_server_error_is_retryable() {
        let outcome: AttemptOutcome<()> = classify_provider_error(ProviderError::ServerError {
            provider: "test".into(),
            status: 500,
        });
        assert!(matches!(outcome, AttemptOutcome::Retryable(_)));
    }

    #[test]
    fn classify_connection_failed_is_retryable() {
        let outcome: AttemptOutcome<()> =
            classify_provider_error(ProviderError::ConnectionFailed {
                provider: "test".into(),
            });
        assert!(matches!(outcome, AttemptOutcome::Retryable(_)));
    }

    #[test]
    fn classify_auth_failure_is_fatal() {
        let outcome: AttemptOutcome<()> = classify_provider_error(ProviderError::AuthFailure {
            provider: "test".into(),
        });
        assert!(matches!(outcome, AttemptOutcome::Fatal(_)));
    }

    #[test]
    fn classify_bad_request_is_fatal() {
        let outcome: AttemptOutcome<()> = classify_provider_error(ProviderError::BadRequest {
            provider: "test".into(),
            message: "invalid".into(),
        });
        assert!(matches!(outcome, AttemptOutcome::Fatal(_)));
    }

    // -----------------------------------------------------------------------
    // ProviderRegistry
    // -----------------------------------------------------------------------

    /// Minimal adapter for testing the registry.
    struct FakeAdapter;

    impl ProviderAdapter for FakeAdapter {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn chat_completion<'a>(
            &'a self,
            _client: &'a reqwest::Client,
            _route: &'a ProviderRoute,
            _request: &'a ChatCompletionsRequest,
        ) -> Pin<Box<dyn Future<Output = Result<ChatCompletionsResponse, ProviderError>> + Send + 'a>>
        {
            Box::pin(async {
                Err(ProviderError::AuthFailure {
                    provider: "fake".into(),
                })
            })
        }

        fn chat_completion_stream<'a>(
            &'a self,
            _client: &'a reqwest::Client,
            _route: &'a ProviderRoute,
            _request: &'a ChatCompletionsRequest,
        ) -> Pin<Box<dyn Future<Output = Result<ChunkStream, ProviderError>> + Send + 'a>> {
            Box::pin(async {
                Err(ProviderError::AuthFailure {
                    provider: "fake".into(),
                })
            })
        }

        fn map_error(&self, status: u16, _body: &str) -> ProviderError {
            ProviderError::ServerError {
                provider: "fake".into(),
                status,
            }
        }
    }

    #[test]
    fn registry_register_and_get() {
        let mut registry = ProviderRegistry::new();
        registry.register(Provider::Openai, Box::new(FakeAdapter));

        let adapter = registry.get(&Provider::Openai);
        assert!(adapter.is_some());
        assert_eq!(adapter.expect("registered").name(), "fake");
    }

    #[test]
    fn registry_get_unregistered_returns_none() {
        let registry = ProviderRegistry::new();
        assert!(registry.get(&Provider::Anthropic).is_none());
    }

    #[test]
    fn registry_default_is_empty() {
        let registry = ProviderRegistry::default();
        assert!(registry.get(&Provider::Openai).is_none());
    }

    #[test]
    fn registry_debug_shows_providers() {
        let mut registry = ProviderRegistry::new();
        registry.register(Provider::Openai, Box::new(FakeAdapter));
        let debug = format!("{registry:?}");
        assert!(debug.contains("Openai"));
    }

    // -----------------------------------------------------------------------
    // Serde round-trip tests
    // -----------------------------------------------------------------------

    #[test]
    fn chat_completions_request_serde_round_trip() {
        let request = ChatCompletionsRequest {
            model: "gpt-4".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: Some(serde_json::Value::String("hello".into())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                extra: HashMap::new(),
            }],
            temperature: Some(0.7),
            top_p: None,
            max_tokens: Some(100),
            stream: Some(false),
            tools: None,
            tool_choice: None,
            n: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
            stream_options: None,
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let deser: ChatCompletionsRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.model, "gpt-4");
        assert_eq!(deser.messages.len(), 1);
        assert_eq!(deser.temperature, Some(0.7));
        assert_eq!(deser.max_tokens, Some(100));
    }

    #[test]
    fn chat_completions_request_flatten_preserves_unknown_fields() {
        let json = r#"{
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "hi"}],
            "custom_field": "custom_value",
            "another": 42
        }"#;

        let request: ChatCompletionsRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(
            request.extra.get("custom_field").and_then(|v| v.as_str()),
            Some("custom_value")
        );
        assert_eq!(
            request
                .extra
                .get("another")
                .and_then(serde_json::Value::as_i64),
            Some(42)
        );

        // Re-serialize and verify custom fields are preserved
        let out = serde_json::to_value(&request).expect("serialize");
        assert_eq!(out["custom_field"], "custom_value");
        assert_eq!(out["another"], 42);
    }

    #[test]
    fn chat_completions_response_serde_round_trip() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }"#;

        let response: ChatCompletionsResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].finish_reason.as_deref(), Some("stop"));
        let usage = response.usage.expect("usage present");
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn chat_completions_response_missing_optional_fields() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant"},
                "finish_reason": null
            }]
        }"#;

        let response: ChatCompletionsResponse = serde_json::from_str(json).expect("deserialize");
        assert!(response.usage.is_none());
        assert!(response.system_fingerprint.is_none());
        assert!(response.choices[0].finish_reason.is_none());
    }

    #[test]
    fn chat_completion_chunk_serde_round_trip() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        }"#;

        let chunk: ChatCompletionChunk = serde_json::from_str(json).expect("deserialize");
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("Hello"));
        assert!(chunk.usage.is_none());
    }

    #[test]
    fn chat_completion_chunk_with_usage_in_final_chunk() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4",
            "choices": [],
            "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30}
        }"#;

        let chunk: ChatCompletionChunk = serde_json::from_str(json).expect("deserialize");
        let usage = chunk.usage.expect("usage present in final chunk");
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn usage_defaults_to_zero() {
        let json = r"{}";
        let usage: Usage = serde_json::from_str(json).expect("deserialize");
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    // -----------------------------------------------------------------------
    // Responses API types
    // -----------------------------------------------------------------------

    #[test]
    fn responses_request_serde_round_trip() {
        let req = ResponsesRequest {
            model: "gpt-4".into(),
            input: serde_json::Value::String("hello".into()),
            instructions: Some("Be helpful".into()),
            stream: None,
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let deser: ResponsesRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.model, "gpt-4");
        assert_eq!(deser.instructions.as_deref(), Some("Be helpful"));
    }

    #[test]
    fn responses_response_serde() {
        let json = r#"{
            "id": "resp_123",
            "object": "response",
            "output": [{"type": "message"}],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        }"#;
        let resp: ResponsesResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.id, "resp_123");
        let usage = resp.usage.expect("usage");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn responses_usage_defaults() {
        let json = r"{}";
        let usage: ResponsesUsage = serde_json::from_str(json).expect("deserialize");
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    // -----------------------------------------------------------------------
    // Embeddings API types
    // -----------------------------------------------------------------------

    #[test]
    fn embeddings_request_serde_round_trip() {
        let req = EmbeddingsRequest {
            model: "text-embedding-ada-002".into(),
            input: serde_json::Value::String("hello world".into()),
            encoding_format: Some("float".into()),
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let deser: EmbeddingsRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.model, "text-embedding-ada-002");
        assert_eq!(deser.encoding_format.as_deref(), Some("float"));
    }

    #[test]
    fn embeddings_response_serde() {
        let json = r#"{
            "object": "list",
            "data": [{
                "object": "embedding",
                "index": 0,
                "embedding": [0.1, 0.2, 0.3]
            }],
            "model": "text-embedding-ada-002",
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        }"#;
        let resp: EmbeddingsResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].index, 0);
        assert_eq!(resp.usage.prompt_tokens, 5);
    }
}
