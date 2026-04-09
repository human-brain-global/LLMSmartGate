//! OpenAI-compatible provider adapter.
//!
//! Supports chat completions, responses API, and embeddings against any
//! OpenAI-compatible endpoint via configurable base URL.

use std::pin::Pin;

use serde::Deserialize;
use tokio_stream::StreamExt as _;

use crate::models::ProviderRoute;
use crate::providers::openai_compat::{self, extract_openai_error_message};
use crate::providers::traits::{
    ChatCompletionsRequest, ChatCompletionsResponse, ChunkStream, EmbeddingsRequest,
    EmbeddingsResponse, ProviderAdapter, ProviderError, ResponsesChunkStream, ResponsesRequest,
    ResponsesResponse, ResponsesStreamEvent,
};
use crate::streaming::parser::parse_sse_stream;

// ---------------------------------------------------------------------------
// Route configuration
// ---------------------------------------------------------------------------

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_owned()
}

/// Per-route configuration extracted from `ProviderRoute.config`.
#[derive(Debug, Deserialize)]
pub struct OpenAIRouteConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    pub api_key: String,
}

/// Extract [`OpenAIRouteConfig`] from the route's JSON config blob.
///
/// # Errors
///
/// Returns [`ProviderError::BadRequest`] if the config JSON is missing required
/// fields or cannot be deserialized.
pub fn parse_route_config(route: &ProviderRoute) -> Result<OpenAIRouteConfig, ProviderError> {
    serde_json::from_value(route.config.clone()).map_err(|e| ProviderError::BadRequest {
        provider: "openai".into(),
        message: format!("invalid route config: {e}"),
    })
}

// ---------------------------------------------------------------------------
// OpenAIAdapter
// ---------------------------------------------------------------------------

const PROVIDER: &str = "openai";

/// Adapter for OpenAI and OpenAI-compatible providers.
pub struct OpenAIAdapter;

#[allow(clippy::manual_async_fn)]
impl ProviderAdapter for OpenAIAdapter {
    fn name(&self) -> &'static str {
        PROVIDER
    }

    #[tracing::instrument(name = "provider.openai.chat_completion", skip_all, fields(
        provider = PROVIDER,
        model = %route.provider_model_name,
    ))]
    fn chat_completion<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a ChatCompletionsRequest,
    ) -> Pin<
        Box<
            dyn std::future::Future<Output = Result<ChatCompletionsResponse, ProviderError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let config = parse_route_config(route)?;
            let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
            openai_compat::chat_completion(
                client,
                route,
                request,
                &url,
                Some(&config.api_key),
                PROVIDER,
                extract_openai_error_message,
            )
            .await
        })
    }

    #[tracing::instrument(name = "provider.openai.chat_completion_stream", skip_all, fields(
        provider = PROVIDER,
        model = %route.provider_model_name,
    ))]
    fn chat_completion_stream<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a ChatCompletionsRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<ChunkStream, ProviderError>> + Send + 'a>>
    {
        Box::pin(async move {
            let config = parse_route_config(route)?;
            let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
            openai_compat::chat_completion_stream(
                client,
                route,
                request,
                &url,
                Some(&config.api_key),
                PROVIDER,
                extract_openai_error_message,
            )
            .await
        })
    }

    #[tracing::instrument(name = "provider.openai.responses", skip_all, fields(
        provider = PROVIDER,
        model = %route.provider_model_name,
    ))]
    fn responses<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a ResponsesRequest,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<ResponsesResponse, ProviderError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let config = parse_route_config(route)?;
            let url = format!("{}/responses", config.base_url.trim_end_matches('/'));
            let timeout = openai_compat::route_timeout(route);

            let mut body = openai_compat::build_generic_body(request, route, PROVIDER)?;
            body["stream"] = serde_json::Value::Bool(false);

            let response = openai_compat::send_request(
                client,
                &url,
                Some(&config.api_key),
                &body,
                timeout,
                PROVIDER,
            )
            .await?;
            let response =
                openai_compat::check_response(response, PROVIDER, extract_openai_error_message)
                    .await?;
            let text = openai_compat::read_response_body(response, PROVIDER).await?;
            openai_compat::parse_response(&text, PROVIDER, "responses")
        })
    }

    #[tracing::instrument(name = "provider.openai.responses_stream", skip_all, fields(
        provider = PROVIDER,
        model = %route.provider_model_name,
    ))]
    fn responses_stream<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a ResponsesRequest,
    ) -> Pin<
        Box<
            dyn std::future::Future<Output = Result<ResponsesChunkStream, ProviderError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let config = parse_route_config(route)?;
            let url = format!("{}/responses", config.base_url.trim_end_matches('/'));
            let timeout = openai_compat::route_timeout(route);

            let mut body = openai_compat::build_generic_body(request, route, PROVIDER)?;
            body["stream"] = serde_json::Value::Bool(true);

            let response = openai_compat::send_request(
                client,
                &url,
                Some(&config.api_key),
                &body,
                timeout,
                PROVIDER,
            )
            .await?;
            let response =
                openai_compat::check_response(response, PROVIDER, extract_openai_error_message)
                    .await?;

            let byte_stream = response.bytes_stream();
            let sse_stream = parse_sse_stream(byte_stream, PROVIDER.to_owned());

            let event_stream = sse_stream.filter_map(|result| match result {
                Ok(event) => {
                    if event.is_done() {
                        return None;
                    }
                    let event_type = event.event_type.unwrap_or_default();
                    match serde_json::from_str::<serde_json::Value>(&event.data) {
                        Ok(data) => Some(Ok(ResponsesStreamEvent { event_type, data })),
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "failed to parse responses stream event — skipping"
                            );
                            None
                        }
                    }
                }
                Err(e) => Some(Err(e)),
            });

            Ok(Box::pin(event_stream) as ResponsesChunkStream)
        })
    }

    #[tracing::instrument(name = "provider.openai.embeddings", skip_all, fields(
        provider = PROVIDER,
        model = %route.provider_model_name,
    ))]
    fn embeddings<'a>(
        &'a self,
        client: &'a reqwest::Client,
        route: &'a ProviderRoute,
        request: &'a EmbeddingsRequest,
    ) -> Pin<
        Box<
            dyn std::future::Future<Output = Result<EmbeddingsResponse, ProviderError>> + Send + 'a,
        >,
    > {
        Box::pin(async move {
            let config = parse_route_config(route)?;
            let url = format!("{}/embeddings", config.base_url.trim_end_matches('/'));
            let timeout = openai_compat::route_timeout(route);

            let body = openai_compat::build_generic_body(request, route, PROVIDER)?;

            let response = openai_compat::send_request(
                client,
                &url,
                Some(&config.api_key),
                &body,
                timeout,
                PROVIDER,
            )
            .await?;
            let response =
                openai_compat::check_response(response, PROVIDER, extract_openai_error_message)
                    .await?;
            let text = openai_compat::read_response_body(response, PROVIDER).await?;
            openai_compat::parse_response(&text, PROVIDER, "embeddings")
        })
    }

    fn map_error(&self, status: u16, body: &str) -> ProviderError {
        openai_compat::map_error(status, body, PROVIDER, None, extract_openai_error_message)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Provider, RouteId};
    use std::collections::HashMap;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn test_route(base_url: &str) -> ProviderRoute {
        ProviderRoute {
            id: RouteId::new(),
            tenant_id: None,
            model_alias: "gpt-4".into(),
            provider: Provider::Openai,
            provider_model_name: "gpt-4-turbo".into(),
            priority: 100,
            enabled: true,
            timeout_ms: 5000,
            max_retries: 1,
            retry_backoff_ms: 250,
            config: serde_json::json!({
                "base_url": base_url,
                "api_key": "sk-test-key"
            }),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn test_request() -> ChatCompletionsRequest {
        ChatCompletionsRequest {
            model: "gpt-4".into(),
            messages: vec![crate::providers::traits::ChatMessage {
                role: "user".into(),
                content: Some(serde_json::Value::String("hello".into())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                extra: HashMap::new(),
            }],
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            tools: None,
            tool_choice: None,
            n: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
            stream_options: None,
            extra: HashMap::new(),
        }
    }

    fn sample_response_json() -> serde_json::Value {
        serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4-turbo",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        })
    }

    fn sample_streaming_body() -> String {
        let chunk1 = serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4-turbo",
            "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]
        });
        let chunk2 = serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4-turbo",
            "choices": [{"index": 0, "delta": {"content": "Hello"}, "finish_reason": null}]
        });
        let chunk3 = serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4-turbo",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 1, "total_tokens": 11}
        });
        format!("data: {chunk1}\n\ndata: {chunk2}\n\ndata: {chunk3}\n\ndata: [DONE]\n\n")
    }

    // -----------------------------------------------------------------------
    // Route config parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_route_config_defaults_base_url() {
        let config = serde_json::json!({"api_key": "sk-test"});
        let parsed: OpenAIRouteConfig = serde_json::from_value(config).expect("should parse");
        assert_eq!(parsed.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn parse_route_config_custom_base_url() {
        let config =
            serde_json::json!({"base_url": "http://localhost:8000/v1", "api_key": "sk-local"});
        let parsed: OpenAIRouteConfig = serde_json::from_value(config).expect("should parse");
        assert_eq!(parsed.base_url, "http://localhost:8000/v1");
    }

    #[test]
    fn parse_route_config_missing_api_key_errors() {
        let result = serde_json::from_value::<OpenAIRouteConfig>(serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn parse_route_config_from_provider_route() {
        let route = test_route("http://localhost:8000/v1");
        let cfg = parse_route_config(&route).expect("should parse");
        assert_eq!(cfg.api_key, "sk-test-key");
    }

    // -----------------------------------------------------------------------
    // Non-streaming chat completion (wiremock)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn chat_completion_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer sk-test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response_json()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let route = test_route(&server.uri());
        let adapter = OpenAIAdapter;
        let response = adapter
            .chat_completion(&client, &route, &test_request())
            .await
            .expect("ok");

        assert_eq!(response.id, "chatcmpl-123");
        let usage = response.usage.expect("usage present");
        assert_eq!(usage.total_tokens, 18);
    }

    #[tokio::test]
    async fn chat_completion_model_override() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response_json()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let route = test_route(&server.uri());
        let _ = OpenAIAdapter
            .chat_completion(&client, &route, &test_request())
            .await
            .expect("ok");

        let received = server.received_requests().await.expect("requests");
        let sent_body: serde_json::Value =
            serde_json::from_slice(&received[0].body).expect("parse body");
        assert_eq!(sent_body["model"], "gpt-4-turbo");
        assert_eq!(sent_body["stream"], false);
    }

    #[tokio::test]
    async fn chat_completion_401_returns_auth_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string(
                r#"{"error":{"message":"Invalid API key","type":"authentication_error"}}"#,
            ))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let err = OpenAIAdapter
            .chat_completion(&client, &test_route(&server.uri()), &test_request())
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::AuthFailure { .. }));
    }

    #[tokio::test]
    async fn chat_completion_429_returns_rate_limited_with_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("retry-after", "30")
                    .set_body_string(r#"{"error":{"message":"Rate limit exceeded"}}"#),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let err = OpenAIAdapter
            .chat_completion(&client, &test_route(&server.uri()), &test_request())
            .await
            .unwrap_err();
        match err {
            ProviderError::RateLimited { retry_after, .. } => {
                assert_eq!(retry_after, Some(30));
            }
            other => panic!("expected RateLimited, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn chat_completion_500_returns_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let err = OpenAIAdapter
            .chat_completion(&client, &test_route(&server.uri()), &test_request())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ProviderError::ServerError { status: 500, .. }
        ));
    }

    #[tokio::test]
    async fn chat_completion_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_delay(std::time::Duration::from_secs(10)),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let mut route = test_route(&server.uri());
        route.timeout_ms = 100;

        let err = OpenAIAdapter
            .chat_completion(&client, &route, &test_request())
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Timeout { .. }));
    }

    // -----------------------------------------------------------------------
    // Streaming chat completion (wiremock)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn chat_completion_stream_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sample_streaming_body()),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let route = test_route(&server.uri());
        let stream = OpenAIAdapter
            .chat_completion_stream(&client, &route, &test_request())
            .await
            .expect("ok");

        let chunks: Vec<_> = tokio_stream::StreamExt::collect::<Vec<_>>(stream)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("all chunks ok");

        assert_eq!(chunks.len(), 3);
        assert_eq!(
            chunks[0].choices[0].delta.role.as_deref(),
            Some("assistant")
        );
        assert_eq!(chunks[1].choices[0].delta.content.as_deref(), Some("Hello"));
        assert_eq!(chunks[2].choices[0].finish_reason.as_deref(), Some("stop"));
        let usage = chunks[2].usage.as_ref().expect("usage in final chunk");
        assert_eq!(usage.total_tokens, 11);
    }

    #[tokio::test]
    async fn chat_completion_stream_injects_stream_options() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string("data: [DONE]\n\n"),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let _ = OpenAIAdapter
            .chat_completion_stream(&client, &test_route(&server.uri()), &test_request())
            .await
            .expect("ok");

        let received = server.received_requests().await.expect("requests");
        let sent_body: serde_json::Value =
            serde_json::from_slice(&received[0].body).expect("parse body");
        assert_eq!(sent_body["stream"], true);
        assert_eq!(sent_body["stream_options"]["include_usage"], true);
    }

    // -----------------------------------------------------------------------
    // Responses API (wiremock)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn responses_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "resp_123",
                "object": "response",
                "output": [],
                "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let request = ResponsesRequest {
            model: "gpt-4".into(),
            input: serde_json::Value::String("hello".into()),
            instructions: None,
            stream: None,
            extra: HashMap::new(),
        };
        let response = OpenAIAdapter
            .responses(&client, &test_route(&server.uri()), &request)
            .await
            .expect("ok");
        assert_eq!(response.id, "resp_123");
    }

    #[tokio::test]
    async fn responses_stream_success() {
        let server = MockServer::start().await;
        let sse_body = "event: response.created\ndata: {\"id\":\"resp_1\"}\n\ndata: [DONE]\n\n";
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_body),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let request = ResponsesRequest {
            model: "gpt-4".into(),
            input: serde_json::Value::String("hello".into()),
            instructions: None,
            stream: Some(true),
            extra: HashMap::new(),
        };
        let stream = OpenAIAdapter
            .responses_stream(&client, &test_route(&server.uri()), &request)
            .await
            .expect("ok");

        let events: Vec<_> = tokio_stream::StreamExt::collect::<Vec<_>>(stream)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("all ok");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "response.created");
    }

    // -----------------------------------------------------------------------
    // Embeddings API (wiremock)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn embeddings_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2]}],
                "model": "text-embedding-ada-002",
                "usage": {"prompt_tokens": 5, "total_tokens": 5}
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let request = EmbeddingsRequest {
            model: "text-embedding-ada-002".into(),
            input: serde_json::Value::String("hello".into()),
            encoding_format: None,
            extra: HashMap::new(),
        };
        let response = OpenAIAdapter
            .embeddings(&client, &test_route(&server.uri()), &request)
            .await
            .expect("ok");
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.usage.prompt_tokens, 5);
    }
}
