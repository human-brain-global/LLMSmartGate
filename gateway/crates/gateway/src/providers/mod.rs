//! LLM provider adapters -- common trait and per-provider implementations.

pub mod anthropic;
pub mod azure_openai;
pub mod gemini;
pub mod openai;
pub mod openai_compat;
pub mod traits;
pub mod vllm;

pub use traits::{
    ChatCompletionChunk, ChatCompletionsRequest, ChatCompletionsResponse, ChatMessage, ChunkStream,
    EmbeddingsRequest, EmbeddingsResponse, ProviderAdapter, ProviderError, ProviderRegistry,
    ResponsesChunkStream, ResponsesRequest, ResponsesResponse, ResponsesStreamEvent, UsageData,
};
