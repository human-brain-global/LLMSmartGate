//! vLLM / local-model provider adapter.
//!
//! vLLM, Ollama, and similar local inference servers expose an OpenAI-compatible
//! API.  This adapter reuses the shared HTTP/SSE logic from [`super::openai_compat`]
//! with key differences:
//!
//! - **Base URL is required** (no default `api.openai.com`).
//! - **Auth is optional** — many local deployments run without API keys.
//! - **Lenient deserialization** — vLLM may omit fields that OpenAI always includes.

use std::pin::Pin;

use serde::Deserialize;

use crate::models::ProviderRoute;
use crate::providers::openai_compat::{self, identity_error_message};
use crate::providers::traits::{
    ChatCompletionsRequest, ChatCompletionsResponse, ChunkStream, ProviderAdapter, ProviderError,
};

// ---------------------------------------------------------------------------
// Route configuration
// ---------------------------------------------------------------------------

/// Per-route configuration for vLLM / local model providers.
///
/// Unlike [`super::openai::OpenAIRouteConfig`], `base_url` is **required**
/// (no default) and `api_key` is **optional**.
#[derive(Debug, Deserialize)]
pub struct VllmRouteConfig {
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// Extract [`VllmRouteConfig`] from the route's JSON config blob.
///
/// # Errors
///
/// Returns [`ProviderError::BadRequest`] if the config JSON is missing the
/// required `base_url` field.
fn parse_route_config(route: &ProviderRoute) -> Result<VllmRouteConfig, ProviderError> {
    serde_json::from_value(route.config.clone()).map_err(|e| ProviderError::BadRequest {
        provider: PROVIDER.into(),
        message: format!("invalid route config: {e}"),
    })
}

// ---------------------------------------------------------------------------
// VllmAdapter
// ---------------------------------------------------------------------------

const PROVIDER: &str = "vllm";

/// Adapter for vLLM, Ollama, and other OpenAI-compatible local inference servers.
pub struct VllmAdapter;

#[allow(clippy::manual_async_fn)]
impl ProviderAdapter for VllmAdapter {
    fn name(&self) -> &'static str {
        PROVIDER
    }

    #[tracing::instrument(name = "provider.vllm.chat_completion", skip_all, fields(
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
                config.api_key.as_deref(),
                PROVIDER,
                identity_error_message,
            )
            .await
        })
    }

    #[tracing::instrument(name = "provider.vllm.chat_completion_stream", skip_all, fields(
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
                config.api_key.as_deref(),
                PROVIDER,
                identity_error_message,
            )
            .await
        })
    }

    fn map_error(&self, status: u16, body: &str) -> ProviderError {
        openai_compat::map_error(status, body, PROVIDER, None, identity_error_message)
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

    fn setup_tls() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    fn test_route_with_auth(server_uri: &str) -> ProviderRoute {
        ProviderRoute {
            id: RouteId::new(),
            tenant_id: None,
            model_alias: "llama-3".into(),
            provider: Provider::Vllm,
            provider_model_name: "meta-llama/Llama-3-8B-Instruct".into(),
            priority: 100,
            enabled: true,
            timeout_ms: 5000,
            max_retries: 1,
            retry_backoff_ms: 250,
            config: serde_json::json!({"base_url": format!("{server_uri}/v1"), "api_key": "vllm-key"}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn test_route_no_auth(server_uri: &str) -> ProviderRoute {
        ProviderRoute {
            id: RouteId::new(),
            tenant_id: None,
            model_alias: "llama-3".into(),
            provider: Provider::Vllm,
            provider_model_name: "meta-llama/Llama-3-8B-Instruct".into(),
            priority: 100,
            enabled: true,
            timeout_ms: 5000,
            max_retries: 1,
            retry_backoff_ms: 250,
            config: serde_json::json!({"base_url": format!("{server_uri}/v1")}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn test_request() -> ChatCompletionsRequest {
        ChatCompletionsRequest {
            model: "llama-3".into(),
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

    fn sample_vllm_response() -> serde_json::Value {
        serde_json::json!({
            "id": "cmpl-abc",
            "object": "chat.completion",
            "created": 1_700_000_000,
            "model": "meta-llama/Llama-3-8B-Instruct",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hi there!"},
                "finish_reason": "stop"
            }]
        })
    }

    // -----------------------------------------------------------------------
    // Route config
    // -----------------------------------------------------------------------

    #[test]
    fn parse_config_with_auth() {
        let config =
            serde_json::json!({"base_url": "http://localhost:8000/v1", "api_key": "my-key"});
        let parsed: VllmRouteConfig = serde_json::from_value(config).expect("parse");
        assert_eq!(parsed.api_key.as_deref(), Some("my-key"));
    }

    #[test]
    fn parse_config_without_auth() {
        let config = serde_json::json!({"base_url": "http://localhost:8000/v1"});
        let parsed: VllmRouteConfig = serde_json::from_value(config).expect("parse");
        assert!(parsed.api_key.is_none());
    }

    #[test]
    fn parse_config_missing_base_url_errors() {
        assert!(serde_json::from_value::<VllmRouteConfig>(serde_json::json!({})).is_err());
    }

    // -----------------------------------------------------------------------
    // Non-streaming (wiremock)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn chat_completion_success() {
        setup_tls();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer vllm-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_vllm_response()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let response = VllmAdapter
            .chat_completion(
                &client,
                &test_route_with_auth(&server.uri()),
                &test_request(),
            )
            .await
            .expect("ok");

        assert_eq!(response.id, "cmpl-abc");
        assert!(response.usage.is_none()); // vLLM may omit usage
    }

    #[tokio::test]
    async fn chat_completion_no_auth_header_when_no_key() {
        setup_tls();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_vllm_response()))
            .expect(1)
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let _ = VllmAdapter
            .chat_completion(&client, &test_route_no_auth(&server.uri()), &test_request())
            .await
            .expect("ok");

        let received = server.received_requests().await.expect("requests");
        assert!(
            !received[0].headers.contains_key("Authorization"),
            "should not send Authorization when api_key absent"
        );
    }

    #[tokio::test]
    async fn chat_completion_lenient_missing_usage_and_extra_fields() {
        setup_tls();
        let server = MockServer::start().await;
        let response_json = serde_json::json!({
            "id": "cmpl-xyz",
            "object": "chat.completion",
            "created": 1_700_000_000,
            "model": "llama-3",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "hi"}, "finish_reason": "stop"}],
            "extra_vllm_field": "some value"
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_json))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let response = VllmAdapter
            .chat_completion(&client, &test_route_no_auth(&server.uri()), &test_request())
            .await
            .expect("lenient deser");

        assert_eq!(response.id, "cmpl-xyz");
        assert!(response.usage.is_none());
        assert!(response.extra.contains_key("extra_vllm_field"));
    }

    // -----------------------------------------------------------------------
    // Streaming (wiremock)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn chat_completion_stream_success() {
        setup_tls();
        let server = MockServer::start().await;
        let chunk = serde_json::json!({
            "id": "cmpl-abc",
            "object": "chat.completion.chunk",
            "created": 1_700_000_000,
            "model": "llama-3",
            "choices": [{"index": 0, "delta": {"content": "Hi"}, "finish_reason": null}]
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(format!("data: {chunk}\n\ndata: [DONE]\n\n")),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let stream = VllmAdapter
            .chat_completion_stream(&client, &test_route_no_auth(&server.uri()), &test_request())
            .await
            .expect("ok");

        let chunks: Vec<_> = tokio_stream::StreamExt::collect::<Vec<_>>(stream)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("all ok");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].choices[0].delta.content.as_deref(), Some("Hi"));
    }

    #[tokio::test]
    async fn chat_completion_overrides_model() {
        setup_tls();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_vllm_response()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let _ = VllmAdapter
            .chat_completion(&client, &test_route_no_auth(&server.uri()), &test_request())
            .await
            .expect("ok");

        let received = server.received_requests().await.expect("requests");
        let sent_body: serde_json::Value =
            serde_json::from_slice(&received[0].body).expect("parse");
        assert_eq!(sent_body["model"], "meta-llama/Llama-3-8B-Instruct");
    }
}
