# LLMSmartGate -- Low-Level Technical Solution Design (Backend)

> **Doc ID:** `LLD-BE-001`  
> **Version:** 1.0  
> **Date:** 2026-04-01  
> **Status:** Draft  
> **Classification:** Internal -- Engineering

---

## Document Persona & Guidelines

| | |
|---|---|
| **Author Role** | Senior Rust Developer |
| **Perspective** | Implementation-level design: traits, structs, algorithms, performance |
| **Primary Audience** | Rust Developers implementing the gateway |
| **Secondary Audience** | Solution Architect (review), QA (test design), DevOps (config) |

### How to Read This Document

| Symbol | Meaning |
|--------|---------|
| `trait XxxRepository` | Database access interface -- implemented in `src/storage/` |
| `trait XxxService` | Business logic interface -- orchestrates repositories |
| `#[instrument]` | OpenTelemetry span -- every public function is traced |
| `-> Result<T, XxxError>` | All fallible operations return typed errors via `thiserror` |
| `// perf:` | Performance-critical note -- read carefully |
| `// security:` | Security-sensitive implementation detail |
| `[Lua]` | Redis Lua script -- atomic operation |
| `[SQL]` | PostgreSQL query -- compile-time checked via `sqlx` |

### Code Conventions in This Document

```rust
// Pseudocode / design-level Rust -- not copy-paste ready
// Focus on: signatures, types, flow, algorithms
// Actual implementation may differ in error handling verbosity
```

### Document Relationships

```
PRD-001                -- User stories this code fulfills
  ├── ARC-001          -- Tech stack & standards this code follows
  ├── HLD-001          -- Component interactions this code implements
  ├── LLD-BE-001 (this)-- Rust backend implementation design
  ├── LLD-FE-001       -- Frontend calls APIs defined here
  └── TST-001          -- Test cases verify behavior defined here
```

---

## Table of Contents

1. [Cargo Workspace Structure](#1-cargo-workspace-structure)
2. [API Layer](#2-api-layer)
3. [Auth Module](#3-auth-module)
4. [Policy Engine](#4-policy-engine)
5. [Routing Engine](#5-routing-engine)
6. [Provider Adapters](#6-provider-adapters)
7. [Streaming Engine](#7-streaming-engine)
8. [Usage Metering](#8-usage-metering)
9. [Storage Layer](#9-storage-layer)
10. [Observability](#10-observability)
11. [Configuration](#11-configuration)
12. [Testing Strategy](#12-testing-strategy)

---

## 1. Cargo Workspace Structure

### 1.1 Workspace Layout

```text
llmsmartgate/
+-- Cargo.toml                    # Workspace root
+-- Cargo.lock
+-- .sqlx/                        # sqlx offline query cache
+-- migrations/                   # SQL migration files
|   +-- 001_create_tenants.sql
|   +-- 002_create_policies.sql
|   +-- 003_create_service_accounts.sql
|   +-- 004_create_service_account_keys.sql
|   +-- 005_create_service_account_policy_bindings.sql
|   +-- 006_create_provider_routes.sql
|   +-- 007_create_budgets.sql
|   +-- 008_create_usage_events.sql
|   +-- 009_create_audit_events.sql
+-- config/
|   +-- default.toml              # Default configuration
|   +-- development.toml
|   +-- production.toml
+-- crates/
|   +-- smartgate-server/         # Binary crate: main entry point
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- main.rs
|   |       +-- cli.rs            # CLI argument parsing
|   |       +-- server.rs         # Server bootstrap
|   |       +-- shutdown.rs       # Graceful shutdown logic
|   +-- smartgate-api/            # Library crate: Axum routes + handlers
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- data_plane/
|   |       |   +-- mod.rs
|   |       |   +-- chat_completions.rs
|   |       |   +-- responses.rs
|   |       |   +-- embeddings.rs
|   |       |   +-- models.rs
|   |       +-- admin/
|   |       |   +-- mod.rs
|   |       |   +-- tenants.rs
|   |       |   +-- service_accounts.rs
|   |       |   +-- keys.rs
|   |       |   +-- policies.rs
|   |       |   +-- routes.rs
|   |       |   +-- usage.rs
|   |       |   +-- audit.rs
|   |       +-- middleware/
|   |       |   +-- mod.rs
|   |       |   +-- request_id.rs
|   |       |   +-- tracing_mw.rs
|   |       |   +-- auth.rs
|   |       |   +-- rate_limit.rs
|   |       +-- extractors/
|   |       |   +-- mod.rs
|   |       |   +-- auth_context.rs
|   |       |   +-- validated_json.rs
|   |       +-- error.rs          # GatewayError -> IntoResponse
|   |       +-- response.rs       # Common response helpers
|   +-- smartgate-auth/           # Library crate: authentication
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- verifier.rs
|   |       +-- canonical.rs
|   |       +-- nonce.rs
|   |       +-- key_cache.rs
|   |       +-- context.rs
|   |       +-- error.rs
|   +-- smartgate-policy/         # Library crate: policy engine
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- engine.rs
|   |       +-- model_access.rs
|   |       +-- quotas.rs
|   |       +-- budget.rs
|   |       +-- features.rs
|   |       +-- merge.rs
|   |       +-- error.rs
|   +-- smartgate-routing/        # Library crate: routing engine
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- resolver.rs
|   |       +-- retry.rs
|   |       +-- fallback.rs
|   |       +-- circuit_breaker.rs
|   |       +-- execution.rs
|   |       +-- error.rs
|   +-- smartgate-providers/      # Library crate: provider adapters
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- traits.rs
|   |       +-- openai.rs
|   |       +-- anthropic.rs
|   |       +-- gemini.rs
|   |       +-- azure_openai.rs
|   |       +-- vllm.rs
|   |       +-- error.rs
|   |       +-- types.rs          # Normalized request/response types
|   +-- smartgate-streaming/      # Library crate: SSE relay
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- relay.rs
|   |       +-- events.rs
|   |       +-- parser.rs
|   |       +-- backpressure.rs
|   +-- smartgate-usage/          # Library crate: usage metering
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- meter.rs
|   |       +-- pricing.rs
|   |       +-- aggregator.rs
|   |       +-- error.rs
|   +-- smartgate-storage/        # Library crate: PostgreSQL + Redis
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- postgres/
|   |       |   +-- mod.rs
|   |       |   +-- pool.rs
|   |       |   +-- tenants.rs
|   |       |   +-- service_accounts.rs
|   |       |   +-- keys.rs
|   |       |   +-- policies.rs
|   |       |   +-- routes.rs
|   |       |   +-- usage.rs
|   |       |   +-- audit.rs
|   |       |   +-- budgets.rs
|   |       +-- redis/
|   |       |   +-- mod.rs
|   |       |   +-- pool.rs
|   |       |   +-- nonce.rs
|   |       |   +-- rate_limit.rs
|   |       |   +-- cache.rs
|   |       |   +-- health.rs
|   |       |   +-- circuit_breaker.rs
|   |       +-- traits.rs         # Repository trait definitions
|   |       +-- error.rs
|   +-- smartgate-types/          # Library crate: shared types
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- ids.rs            # Type-safe ID wrappers
|   |       +-- models.rs         # Domain models
|   |       +-- request.rs        # Normalized request types
|   |       +-- response.rs       # Normalized response types
|   |       +-- events.rs         # SSE event types
|   |       +-- error.rs          # Error types hierarchy
|   |       +-- enums.rs          # Status enums, provider types
|   +-- smartgate-config/         # Library crate: configuration
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs
|   |       +-- settings.rs
|   |       +-- validation.rs
|   +-- smartgate-observability/  # Library crate: tracing + metrics
|       +-- Cargo.toml
|       +-- src/
|           +-- lib.rs
|           +-- tracing_setup.rs
|           +-- metrics.rs
|           +-- logging.rs
+-- tests/                        # Integration tests
|   +-- common/
|   |   +-- mod.rs
|   |   +-- fixtures.rs
|   |   +-- test_server.rs
|   |   +-- mock_provider.rs
|   +-- auth_test.rs
|   +-- chat_completions_test.rs
|   +-- streaming_test.rs
|   +-- routing_test.rs
|   +-- admin_test.rs
+-- benches/                      # Benchmarks
    +-- auth_bench.rs
    +-- routing_bench.rs
    +-- json_bench.rs
```

### 1.2 Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/smartgate-server",
    "crates/smartgate-api",
    "crates/smartgate-auth",
    "crates/smartgate-policy",
    "crates/smartgate-routing",
    "crates/smartgate-providers",
    "crates/smartgate-streaming",
    "crates/smartgate-usage",
    "crates/smartgate-storage",
    "crates/smartgate-types",
    "crates/smartgate-config",
    "crates/smartgate-observability",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "Apache-2.0"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.44", features = ["full"] }

# Web framework
axum = { version = "0.8", features = ["ws", "macros"] }
axum-extra = { version = "0.10", features = ["typed-header"] }
tower = { version = "0.5", features = ["full"] }
tower-http = { version = "0.6", features = [
    "cors", "trace", "request-id", "timeout",
    "compression-gzip", "limit", "set-header"
] }
hyper = { version = "1.6", features = ["full"] }

# HTTP client
reqwest = { version = "0.12", features = [
    "json", "stream", "rustls-tls", "gzip"
] }

# Database
sqlx = { version = "0.8", features = [
    "runtime-tokio-rustls", "postgres", "uuid",
    "chrono", "json", "migrate"
] }

# Redis
fred = { version = "10.1", features = ["tokio-runtime", "pool-prefer-active"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Cryptography
ed25519-dalek = { version = "2.1", features = ["rand_core", "zeroize"] }
sha2 = "0.10"
base64 = "0.22"

# Tracing / Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = [
    "env-filter", "json", "fmt"
] }
tracing-opentelemetry = "0.28"
opentelemetry = { version = "0.28", features = ["trace", "metrics"] }
opentelemetry-otlp = { version = "0.28", features = ["tonic"] }
opentelemetry_sdk = { version = "0.28", features = [
    "rt-tokio", "trace", "metrics"
] }

# Utilities
uuid = { version = "1", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
bytes = "1"
thiserror = "1.6"
moka = { version = "0.12", features = ["future"] }
zeroize = { version = "1", features = ["derive"] }
rand = "0.8"
url = "2"
config = "0.14"
rustls = "0.23"

# Testing
tokio-test = "0.4"
wiremock = "0.6"
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres", "redis"] }
criterion = { version = "0.5", features = ["async_tokio"] }
fake = { version = "3", features = ["derive", "uuid", "chrono"] }
proptest = "1"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
strip = "symbols"
panic = "abort"
```

### 1.3 Crate Dependency Graph

```text
smartgate-server
  +-- smartgate-api
  |     +-- smartgate-auth
  |     |     +-- smartgate-types
  |     |     +-- smartgate-storage
  |     +-- smartgate-policy
  |     |     +-- smartgate-types
  |     |     +-- smartgate-storage
  |     +-- smartgate-routing
  |     |     +-- smartgate-types
  |     |     +-- smartgate-providers
  |     |     +-- smartgate-storage
  |     +-- smartgate-providers
  |     |     +-- smartgate-types
  |     |     +-- smartgate-streaming
  |     +-- smartgate-streaming
  |     |     +-- smartgate-types
  |     +-- smartgate-usage
  |     |     +-- smartgate-types
  |     |     +-- smartgate-storage
  |     +-- smartgate-types
  +-- smartgate-config
  +-- smartgate-observability
  +-- smartgate-storage
        +-- smartgate-types
```

---

## 2. API Layer

### 2.1 Application State

```rust
// crates/smartgate-api/src/lib.rs

use std::sync::Arc;

/// Shared application state passed to all handlers via Axum's State extractor.
/// Wrapped in Arc for cheap cloning across request tasks.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<smartgate_config::Settings>,
    pub pg_pool: sqlx::PgPool,
    pub redis_pool: fred::clients::Pool,
    pub http_client: reqwest::Client,
    pub key_cache: moka::future::Cache<String, smartgate_types::ServiceAccountKey>,
    pub policy_cache: moka::future::Cache<uuid::Uuid, Vec<smartgate_types::Policy>>,
    pub route_cache: moka::future::Cache<String, Vec<smartgate_types::ProviderRoute>>,
    pub usage_sender: tokio::sync::mpsc::Sender<smartgate_types::UsageEvent>,
    pub audit_sender: tokio::sync::mpsc::Sender<smartgate_types::AuditEvent>,
    pub shutdown_token: tokio_util::sync::CancellationToken,
}
```

### 2.2 Router Setup

```rust
// crates/smartgate-api/src/lib.rs

use axum::{
    Router,
    middleware,
};
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    timeout::TimeoutLayer,
    limit::RequestBodyLimitLayer,
    cors::CorsLayer,
    compression::CompressionLayer,
    set_header::SetResponseHeaderLayer,
};
use std::time::Duration;

pub fn build_router(state: AppState) -> Router {
    let data_plane = Router::new()
        .route("/v1/chat/completions", axum::routing::post(data_plane::chat_completions::handler))
        .route("/v1/responses", axum::routing::post(data_plane::responses::handler))
        .route("/v1/embeddings", axum::routing::post(data_plane::embeddings::handler))
        .route("/v1/models", axum::routing::get(data_plane::models::handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::auth_middleware,
        ));

    let admin_plane = Router::new()
        .route("/admin/tenants", axum::routing::get(admin::tenants::list).post(admin::tenants::create))
        .route("/admin/tenants/{id}", axum::routing::get(admin::tenants::get))
        .route("/admin/service-accounts", axum::routing::get(admin::service_accounts::list).post(admin::service_accounts::create))
        .route("/admin/service-accounts/{id}", axum::routing::get(admin::service_accounts::get))
        .route("/admin/service-accounts/{sa_id}/keys", axum::routing::get(admin::keys::list).post(admin::keys::register))
        .route("/admin/service-accounts/{sa_id}/keys/{key_id}/revoke", axum::routing::post(admin::keys::revoke))
        .route("/admin/policies", axum::routing::get(admin::policies::list).post(admin::policies::create))
        .route("/admin/policies/{id}", axum::routing::get(admin::policies::get).put(admin::policies::update))
        .route("/admin/routes", axum::routing::get(admin::routes::list).post(admin::routes::create))
        .route("/admin/routes/{id}", axum::routing::get(admin::routes::get).put(admin::routes::update))
        .route("/admin/usage", axum::routing::get(admin::usage::query))
        .route("/admin/audit", axum::routing::get(admin::audit::query))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::admin_auth_middleware,
        ));

    let health = Router::new()
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .route("/readyz", axum::routing::get(health_check));

    let metrics_route = Router::new()
        .route("/metrics", axum::routing::get(metrics_handler));

    Router::new()
        .merge(data_plane)
        .merge(admin_plane)
        .merge(health)
        .merge(metrics_route)
        .layer(
            ServiceBuilder::new()
                // Layer ordering: outermost first, innermost last.
                // Execution order: top-to-bottom on request, bottom-to-top on response.
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::HeaderName::from_static("x-content-type-options"),
                    http::HeaderValue::from_static("nosniff"),
                ))
                .layer(CompressionLayer::new())
                .layer(CorsLayer::permissive()) // Tighten in production
                .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB max body
                .layer(TimeoutLayer::new(Duration::from_secs(120))) // Global timeout
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn(middleware::request_id::set_request_id))
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    middleware::rate_limit::ip_rate_limit,
                ))
        )
        .with_state(state)
}
```

### 2.3 Middleware Chain (Execution Order)

```text
REQUEST FLOW (top to bottom):

1. IP Rate Limit         -- Reject abusive IPs before any processing
2. Request ID            -- Generate UUIDv7, set X-Request-Id header
3. Tracing               -- Create root span, attach trace context
4. Timeout               -- Enforce global 120s timeout
5. Body Limit            -- Reject >10MB bodies
6. CORS                  -- Handle preflight, set CORS headers
7. Compression           -- Negotiate response compression
8. Security Headers      -- Set X-Content-Type-Options, etc.
9. [Data Plane only] Auth Middleware  -- Ed25519 signature verification
10.[Admin only] Admin Auth Middleware -- JWT Bearer verification
11. Handler              -- Business logic

RESPONSE FLOW (bottom to top):

11. Handler response
10. Auth context cleanup
9.  Auth headers stripped
8.  Security headers added
7.  Response compressed
6.  CORS headers added
5.  (no-op on response)
4.  (timeout cancel)
3.  Span closed, trace emitted
2.  X-Request-Id header set on response
1.  (no-op on response)
```

### 2.4 Request/Response Types

```rust
// crates/smartgate-types/src/request.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// OpenAI-compatible chat completion request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub tools: Option<Vec<Tool>>,
    #[serde(default)]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub n: Option<u32>,
    #[serde(default)]
    pub stop: Option<StopSequence>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
    #[serde(default)]
    pub user: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StopSequence {
    Single(String),
    Multiple(Vec<String>),
}
```

```rust
// crates/smartgate-types/src/response.rs

use serde::{Deserialize, Serialize};

/// OpenAI-compatible chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: AssistantMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Streaming chunk (OpenAI-compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}
```

### 2.5 Custom Extractors

```rust
// crates/smartgate-api/src/extractors/auth_context.rs

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use crate::AppState;
use smartgate_types::AuthContext;

/// Extractor that provides the authenticated principal context.
/// Must be used after the auth middleware has run.
impl FromRequestParts<AppState> for AuthContext {
    type Rejection = crate::error::GatewayError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or(crate::error::GatewayError::Internal(
                "AuthContext not found in extensions; auth middleware may not have run".into()
            ))
    }
}
```

```rust
// crates/smartgate-api/src/extractors/validated_json.rs

use axum::{
    extract::{rejection::JsonRejection, FromRequest, Request},
    Json,
};
use serde::de::DeserializeOwned;
use crate::{error::GatewayError, AppState};

/// A JSON extractor that provides better error messages than axum's default.
pub struct ValidatedJson<T>(pub T);

impl<T> FromRequest<AppState> for ValidatedJson<T>
where
    T: DeserializeOwned,
{
    type Rejection = GatewayError;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ValidatedJson(value)),
            Err(rejection) => {
                let message = match rejection {
                    JsonRejection::JsonDataError(e) => {
                        format!("Invalid JSON data: {e}")
                    }
                    JsonRejection::JsonSyntaxError(e) => {
                        format!("Invalid JSON syntax: {e}")
                    }
                    JsonRejection::MissingJsonContentType(_) => {
                        "Missing Content-Type: application/json".to_string()
                    }
                    JsonRejection::BytesRejection(e) => {
                        format!("Failed to read request body: {e}")
                    }
                    _ => "Unknown JSON parsing error".to_string(),
                };
                Err(GatewayError::Validation(
                    smartgate_types::ValidationError::InvalidJson { details: message },
                ))
            }
        }
    }
}
```

### 2.6 Error Types and IntoResponse

```rust
// crates/smartgate-types/src/error.rs

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error(transparent)]
    Auth(#[from] AuthError),

    #[error(transparent)]
    Policy(#[from] PolicyError),

    #[error(transparent)]
    Routing(#[from] RoutingError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(String),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing required header: {header}")]
    MissingHeader { header: &'static str },

    #[error("Invalid timestamp format")]
    InvalidTimestamp,

    #[error("Timestamp outside allowed window")]
    TimestampExpired,

    #[error("Unknown service account")]
    UnknownServiceAccount,

    #[error("Service account is suspended")]
    ServiceAccountSuspended,

    #[error("Unknown key ID")]
    UnknownKey,

    #[error("Key has been revoked")]
    KeyRevoked,

    #[error("Key has expired")]
    KeyExpired,

    #[error("Body hash does not match X-Body-SHA256")]
    BodyHashMismatch,

    #[error("Signature verification failed")]
    SignatureInvalid,

    #[error("Nonce has already been used")]
    NonceReplay,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Model '{model}' is not allowed by policy")]
    ModelDenied { model: String },

    #[error("Token limit exceeded: requested {requested}, limit {limit}")]
    TokenLimitExceeded { limit: u32, requested: u32 },

    #[error("Streaming is not allowed by policy")]
    StreamingNotAllowed,

    #[error("Tool usage is not allowed by policy")]
    ToolsNotAllowed,

    #[error("File input is not allowed by policy")]
    FilesNotAllowed,

    #[error("Budget exhausted for budget {budget_id}")]
    BudgetExhausted { budget_id: Uuid },

    #[error("Rate limit exceeded, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u32 },
}

#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("No routes found for model alias '{model_alias}'")]
    NoRoutesFound { model_alias: String },

    #[error("All providers failed after {attempts} attempt(s)")]
    AllProvidersFailed { attempts: usize },
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider '{provider}' timed out")]
    Timeout { provider: String },

    #[error("Provider '{provider}' rate limited")]
    RateLimited {
        provider: String,
        retry_after: Option<u32>,
    },

    #[error("Provider '{provider}' authentication failed")]
    AuthFailure { provider: String },

    #[error("Provider '{provider}' bad request: {message}")]
    BadRequest { provider: String, message: String },

    #[error("Provider '{provider}' server error (HTTP {status})")]
    ServerError { provider: String, status: u16 },

    #[error("Failed to connect to provider '{provider}'")]
    ConnectionFailed { provider: String },
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid JSON: {details}")]
    InvalidJson { details: String },

    #[error("Missing required field: {field}")]
    MissingField { field: &'static str },

    #[error("Invalid field '{field}': {reason}")]
    InvalidField {
        field: &'static str,
        reason: String,
    },
}
```

```rust
// crates/smartgate-api/src/error.rs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use smartgate_types::{
    AuthError, GatewayError, PolicyError, ProviderError, RoutingError, ValidationError,
};

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            // Auth errors -> 401
            GatewayError::Auth(e) => {
                let code = match e {
                    AuthError::MissingHeader { .. } => "auth.missing_header",
                    AuthError::InvalidTimestamp => "auth.invalid_timestamp",
                    AuthError::TimestampExpired => "auth.timestamp_expired",
                    AuthError::UnknownServiceAccount => "auth.unknown_service_account",
                    AuthError::ServiceAccountSuspended => "auth.service_account_suspended",
                    AuthError::UnknownKey => "auth.unknown_key",
                    AuthError::KeyRevoked => "auth.key_revoked",
                    AuthError::KeyExpired => "auth.key_expired",
                    AuthError::BodyHashMismatch => "auth.body_hash_mismatch",
                    AuthError::SignatureInvalid => "auth.signature_invalid",
                    AuthError::NonceReplay => "auth.nonce_replay",
                };
                let status = match e {
                    AuthError::ServiceAccountSuspended => StatusCode::FORBIDDEN,
                    _ => StatusCode::UNAUTHORIZED,
                };
                (status, code, e.to_string())
            }

            // Policy errors -> 403 or 429
            GatewayError::Policy(e) => {
                let (status, code) = match e {
                    PolicyError::RateLimited { .. } => {
                        (StatusCode::TOO_MANY_REQUESTS, "policy.rate_limited")
                    }
                    PolicyError::ModelDenied { .. } => {
                        (StatusCode::FORBIDDEN, "policy.model_denied")
                    }
                    PolicyError::TokenLimitExceeded { .. } => {
                        (StatusCode::FORBIDDEN, "policy.token_limit_exceeded")
                    }
                    PolicyError::StreamingNotAllowed => {
                        (StatusCode::FORBIDDEN, "policy.streaming_not_allowed")
                    }
                    PolicyError::ToolsNotAllowed => {
                        (StatusCode::FORBIDDEN, "policy.tools_not_allowed")
                    }
                    PolicyError::FilesNotAllowed => {
                        (StatusCode::FORBIDDEN, "policy.files_not_allowed")
                    }
                    PolicyError::BudgetExhausted { .. } => {
                        (StatusCode::FORBIDDEN, "policy.budget_exhausted")
                    }
                };
                (status, code, e.to_string())
            }

            // Routing errors -> 502
            GatewayError::Routing(e) => {
                let code = match e {
                    RoutingError::NoRoutesFound { .. } => "routing.no_routes",
                    RoutingError::AllProvidersFailed { .. } => "routing.all_providers_failed",
                };
                (StatusCode::BAD_GATEWAY, code, e.to_string())
            }

            // Provider errors -> 502 or 504
            GatewayError::Provider(e) => {
                let (status, code) = match e {
                    ProviderError::Timeout { .. } => {
                        (StatusCode::GATEWAY_TIMEOUT, "provider.timeout")
                    }
                    _ => (StatusCode::BAD_GATEWAY, "provider.error"),
                };
                (status, code, e.to_string())
            }

            // Validation errors -> 400 or 422
            GatewayError::Validation(e) => {
                let (status, code) = match e {
                    ValidationError::InvalidJson { .. } => {
                        (StatusCode::BAD_REQUEST, "validation.invalid_json")
                    }
                    ValidationError::MissingField { .. } => {
                        (StatusCode::UNPROCESSABLE_ENTITY, "validation.missing_field")
                    }
                    ValidationError::InvalidField { .. } => {
                        (StatusCode::UNPROCESSABLE_ENTITY, "validation.invalid_field")
                    }
                };
                (status, code, e.to_string())
            }

            // Internal errors -> 500
            GatewayError::Internal(_)
            | GatewayError::Database(_)
            | GatewayError::Redis(_) => {
                tracing::error!(error = %self, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal.server_error",
                    "An internal error occurred".to_string(),
                )
            }
        };

        let body = json!({
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}
```

### 2.7 Data Plane Handler Example

```rust
// crates/smartgate-api/src/data_plane/chat_completions.rs

use axum::{
    extract::State,
    response::{IntoResponse, Response, Sse},
};
use smartgate_types::{AuthContext, ChatCompletionsRequest, GatewayError};
use crate::{extractors::ValidatedJson, AppState};
use std::time::Instant;

#[tracing::instrument(
    name = "gateway.chat_completions",
    skip(state, body),
    fields(
        model = %body.0.model,
        stream = body.0.stream.unwrap_or(false),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    auth: AuthContext,
    ValidatedJson(request): ValidatedJson<ChatCompletionsRequest>,
) -> Result<Response, GatewayError> {
    let start = Instant::now();
    let is_streaming = request.stream.unwrap_or(false);

    // 1. Evaluate policies
    let policy_decision = smartgate_policy::evaluate(
        &state.pg_pool,
        &state.redis_pool,
        &state.policy_cache,
        &auth,
        &request,
    )
    .await?;

    // 2. Resolve route
    let execution_plan = smartgate_routing::resolve(
        &state.pg_pool,
        &state.redis_pool,
        &state.route_cache,
        &auth,
        &request.model,
    )
    .await?;

    // 3. Execute request (streaming or non-streaming)
    if is_streaming {
        let sse_stream = smartgate_routing::execute_streaming(
            &state.http_client,
            &state.redis_pool,
            execution_plan,
            request,
            &auth,
            state.usage_sender.clone(),
        )
        .await?;

        Ok(Sse::new(sse_stream)
            .keep_alive(
                axum::response::sse::KeepAlive::new()
                    .interval(std::time::Duration::from_secs(15))
                    .text(""),
            )
            .into_response())
    } else {
        let (response, usage_meta) = smartgate_routing::execute(
            &state.http_client,
            &state.redis_pool,
            execution_plan,
            request,
            &auth,
        )
        .await?;

        // 4. Record usage (async, non-blocking)
        let usage_event = smartgate_usage::build_event(
            &auth,
            &usage_meta,
            start.elapsed(),
        );
        let _ = state.usage_sender.try_send(usage_event);

        Ok(axum::Json(response).into_response())
    }
}
```

---

## 3. Auth Module

### 3.1 Core Types

```rust
// crates/smartgate-types/src/models.rs (relevant parts)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authenticated principal context, produced by auth middleware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub tenant_id: Uuid,
    pub service_account_id: Uuid,
    pub key_id: String,
    pub service_account_name: String,
    pub environment: String,
    pub policy_ids: Vec<Uuid>,
}

/// Represents a service account key stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceAccountKey {
    pub id: Uuid,
    pub service_account_id: Uuid,
    pub key_id: String,
    pub algorithm: String,
    pub public_key_pem: String,
    pub fingerprint: String,
    pub status: KeyStatus,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "key_status", rename_all = "lowercase")]
pub enum KeyStatus {
    Active,
    Rotating,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceAccount {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub environment: String,
    pub description: Option<String>,
    pub status: ServiceAccountStatus,
    pub default_policy_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "service_account_status", rename_all = "lowercase")]
pub enum ServiceAccountStatus {
    Active,
    Suspended,
    Revoked,
}
```

### 3.2 Canonical String Construction

```rust
// crates/smartgate-auth/src/canonical.rs

use sha2::{Sha256, Digest};

/// Construct the canonical string for signature verification.
///
/// Format:
/// ```text
/// {HTTP_METHOD}\n{REQUEST_PATH}\n{TIMESTAMP}\n{NONCE}\n{BODY_SHA256}
/// ```
///
/// All components are UTF-8 strings joined by newline characters.
/// The resulting bytes are what gets signed/verified.
pub fn build_canonical_string(
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body_sha256: &str,
) -> Vec<u8> {
    let canonical = format!("{method}\n{path}\n{timestamp}\n{nonce}\n{body_sha256}");
    canonical.into_bytes()
}

/// Compute SHA-256 hash of the request body bytes.
/// Returns lowercase hex-encoded string.
pub fn compute_body_sha256(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_string_format() {
        let canonical = build_canonical_string(
            "POST",
            "/v1/chat/completions",
            "2026-04-01T12:00:00Z",
            "abc123",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        let expected = "POST\n/v1/chat/completions\n2026-04-01T12:00:00Z\nabc123\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(canonical, expected.as_bytes());
    }

    #[test]
    fn test_body_sha256_empty() {
        let hash = compute_body_sha256(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
```

### 3.3 Signature Verification

```rust
// crates/smartgate-auth/src/verifier.rs

use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use smartgate_types::AuthError;

/// Verify an Ed25519 signature against a canonical message.
///
/// # Arguments
/// * `public_key_pem` - PEM-encoded Ed25519 public key
/// * `canonical_bytes` - The canonical string bytes to verify
/// * `signature_b64` - Base64-encoded Ed25519 signature
///
/// # Returns
/// * `Ok(())` if signature is valid
/// * `Err(AuthError::SignatureInvalid)` otherwise
#[tracing::instrument(name = "auth.verify_signature", skip_all)]
pub fn verify_signature(
    public_key_bytes: &[u8; 32],
    canonical_bytes: &[u8],
    signature_b64: &str,
) -> Result<(), AuthError> {
    // Decode the base64 signature
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|_| AuthError::SignatureInvalid)?;

    // Parse the signature (64 bytes for Ed25519)
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| AuthError::SignatureInvalid)?;

    // Parse the public key (32 bytes for Ed25519)
    let verifying_key = VerifyingKey::from_bytes(public_key_bytes)
        .map_err(|_| AuthError::SignatureInvalid)?;

    // Verify
    verifying_key
        .verify(canonical_bytes, &signature)
        .map_err(|_| AuthError::SignatureInvalid)
}
```

### 3.4 Nonce Check with Redis

```rust
// crates/smartgate-auth/src/nonce.rs

use fred::prelude::*;
use smartgate_types::AuthError;

const NONCE_TTL_SECONDS: i64 = 300; // 5 minutes

/// Check if a nonce has already been used, and if not, record it.
///
/// Uses Redis SETNX with TTL for atomic check-and-set.
///
/// Redis command sequence:
/// ```
/// SET nonce:{service_account_id}:{nonce} 1 NX EX 300
/// ```
///
/// If the key already exists (SETNX returns false/nil), the nonce
/// has been replayed.
///
/// # Arguments
/// * `redis` - Redis client pool
/// * `service_account_id` - The service account making the request
/// * `nonce` - The nonce value from X-Nonce header
///
/// # Returns
/// * `Ok(())` if nonce is fresh
/// * `Err(AuthError::NonceReplay)` if nonce was already used
#[tracing::instrument(name = "auth.check_nonce", skip(redis))]
pub async fn check_and_record_nonce(
    redis: &fred::clients::Pool,
    service_account_id: &uuid::Uuid,
    nonce: &str,
) -> Result<(), AuthError> {
    let key = format!("nonce:{service_account_id}:{nonce}");

    // SET key value NX EX ttl
    // Returns true if key was set (nonce is fresh)
    // Returns false if key already existed (replay)
    let was_set: bool = redis
        .set(
            &key,
            "1",
            Some(Expiration::EX(NONCE_TTL_SECONDS)),
            Some(SetPolicy::NX),
            false,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Redis error during nonce check");
            AuthError::NonceReplay // Fail closed: treat Redis errors as replay
        })?;

    if was_set {
        Ok(())
    } else {
        Err(AuthError::NonceReplay)
    }
}
```

### 3.5 Key Caching Strategy

```rust
// crates/smartgate-auth/src/key_cache.rs

use moka::future::Cache;
use smartgate_types::{AuthError, ServiceAccountKey, KeyStatus};
use std::time::Duration;

/// Two-tier key cache: in-process (moka) + Redis + PostgreSQL.
///
/// Lookup order:
/// 1. In-process moka cache (TTL 60s, max 10_000 entries)
/// 2. Redis cache (TTL 300s)
/// 3. PostgreSQL (source of truth)
///
/// On DB fetch, both caches are populated.
/// On key revocation, Redis key is deleted; moka expires via TTL.
pub struct KeyCache {
    local: Cache<String, ServiceAccountKey>,
}

impl KeyCache {
    pub fn new() -> Self {
        Self {
            local: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(60))
                .build(),
        }
    }

    #[tracing::instrument(name = "auth.fetch_key", skip(self, redis, pg_pool))]
    pub async fn get_active_key(
        &self,
        redis: &fred::clients::Pool,
        pg_pool: &sqlx::PgPool,
        key_id: &str,
    ) -> Result<ServiceAccountKey, AuthError> {
        // 1. Check in-process cache
        if let Some(key) = self.local.get(key_id).await {
            tracing::debug!(key_id, "Key found in local cache");
            return validate_key_status(&key);
        }

        // 2. Check Redis cache
        if let Some(key) = self.get_from_redis(redis, key_id).await? {
            tracing::debug!(key_id, "Key found in Redis cache");
            self.local.insert(key_id.to_string(), key.clone()).await;
            return validate_key_status(&key);
        }

        // 3. Fetch from PostgreSQL
        let key = self.get_from_db(pg_pool, key_id).await?;
        tracing::debug!(key_id, "Key fetched from PostgreSQL");

        // Populate both caches
        self.set_redis_cache(redis, key_id, &key).await;
        self.local.insert(key_id.to_string(), key.clone()).await;

        validate_key_status(&key)
    }

    async fn get_from_redis(
        &self,
        redis: &fred::clients::Pool,
        key_id: &str,
    ) -> Result<Option<ServiceAccountKey>, AuthError> {
        let cache_key = format!("key:{key_id}");
        let value: Option<String> = redis
            .get(&cache_key)
            .await
            .map_err(|_| AuthError::UnknownKey)?;

        match value {
            Some(json_str) => {
                let key: ServiceAccountKey = serde_json::from_str(&json_str)
                    .map_err(|_| AuthError::UnknownKey)?;
                Ok(Some(key))
            }
            None => Ok(None),
        }
    }

    async fn get_from_db(
        &self,
        pg_pool: &sqlx::PgPool,
        key_id: &str,
    ) -> Result<ServiceAccountKey, AuthError> {
        sqlx::query_as::<_, ServiceAccountKey>(
            r#"
            SELECT id, service_account_id, key_id, algorithm,
                   public_key_pem, fingerprint, status, expires_at,
                   created_at, revoked_at, last_used_at
            FROM service_account_keys
            WHERE key_id = $1
            "#,
        )
        .bind(key_id)
        .fetch_optional(pg_pool)
        .await
        .map_err(|_| AuthError::UnknownKey)?
        .ok_or(AuthError::UnknownKey)
    }

    async fn set_redis_cache(
        &self,
        redis: &fred::clients::Pool,
        key_id: &str,
        key: &ServiceAccountKey,
    ) {
        let cache_key = format!("key:{key_id}");
        if let Ok(json_str) = serde_json::to_string(key) {
            let _: Result<(), _> = redis
                .set(
                    &cache_key,
                    &json_str,
                    Some(fred::types::Expiration::EX(300)),
                    None,
                    false,
                )
                .await;
        }
    }
}

fn validate_key_status(key: &ServiceAccountKey) -> Result<ServiceAccountKey, AuthError> {
    match key.status {
        KeyStatus::Active => {
            // Check expiry
            if let Some(expires_at) = key.expires_at {
                if expires_at < chrono::Utc::now() {
                    return Err(AuthError::KeyExpired);
                }
            }
            Ok(key.clone())
        }
        KeyStatus::Rotating => Ok(key.clone()), // Still valid during rotation
        KeyStatus::Revoked => Err(AuthError::KeyRevoked),
        KeyStatus::Expired => Err(AuthError::KeyExpired),
    }
}
```

### 3.6 Auth Middleware

```rust
// crates/smartgate-api/src/middleware/auth.rs

use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};
use smartgate_auth::{canonical, nonce, verifier};
use smartgate_types::{AuthContext, AuthError, GatewayError, ServiceAccountStatus};
use crate::AppState;

#[tracing::instrument(name = "auth.verify", skip_all)]
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, GatewayError> {
    // 1. Extract required headers
    let headers = request.headers();
    let service_account_id = extract_header(headers, "x-service-account-id")?;
    let key_id = extract_header(headers, "x-key-id")?;
    let timestamp = extract_header(headers, "x-timestamp")?;
    let nonce_value = extract_header(headers, "x-nonce")?;
    let body_sha256 = extract_header(headers, "x-body-sha256")?;
    let signature = extract_header(headers, "x-signature")?;

    // 2. Validate timestamp window
    let ts = chrono::DateTime::parse_from_rfc3339(timestamp)
        .map_err(|_| AuthError::InvalidTimestamp)?;
    let now = chrono::Utc::now();
    let skew = (now - ts.with_timezone(&chrono::Utc)).num_seconds().unsigned_abs();
    if skew > 300 {
        return Err(AuthError::TimestampExpired.into());
    }

    // 3. Fetch and validate service account
    let sa_id: uuid::Uuid = service_account_id
        .parse()
        .map_err(|_| AuthError::UnknownServiceAccount)?;

    let service_account = smartgate_storage::postgres::service_accounts::get_by_id(
        &state.pg_pool,
        sa_id,
    )
    .await
    .map_err(|_| AuthError::UnknownServiceAccount)?;

    if service_account.status != ServiceAccountStatus::Active {
        return Err(AuthError::ServiceAccountSuspended.into());
    }

    // 4. Fetch public key (with caching)
    let key = state
        .key_cache
        .get(key_id.to_string())
        .await
        .ok_or(GatewayError::Auth(AuthError::UnknownKey))?;

    // 5. Verify body hash
    // We need the body bytes -- read them, then put them back
    let (parts, body) = request.into_parts();
    let body_bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|_| GatewayError::Internal("Failed to read request body".into()))?;

    let computed_hash = canonical::compute_body_sha256(&body_bytes);
    if computed_hash != body_sha256 {
        return Err(AuthError::BodyHashMismatch.into());
    }

    // 6. Reconstruct canonical string and verify signature
    let method = parts.method.as_str();
    let path = parts.uri.path();
    let canonical_bytes = canonical::build_canonical_string(
        method, path, timestamp, nonce_value, body_sha256,
    );

    // Decode PEM to raw 32-byte public key
    let pub_key_bytes = decode_ed25519_public_key(&key.public_key_pem)
        .map_err(|_| AuthError::SignatureInvalid)?;

    verifier::verify_signature(&pub_key_bytes, &canonical_bytes, signature)?;

    // 7. Check nonce
    nonce::check_and_record_nonce(&state.redis_pool, &sa_id, nonce_value).await?;

    // 8. Async update last_used_at (fire-and-forget)
    let pg_pool = state.pg_pool.clone();
    let kid = key_id.to_string();
    tokio::spawn(async move {
        let _ = sqlx::query("UPDATE service_account_keys SET last_used_at = NOW() WHERE key_id = $1")
            .bind(&kid)
            .execute(&pg_pool)
            .await;
    });

    // 9. Build AuthContext and attach to request extensions
    let policy_ids = smartgate_storage::postgres::policies::get_policy_ids_for_service_account(
        &state.pg_pool,
        sa_id,
        service_account.default_policy_id,
    )
    .await
    .map_err(|_| GatewayError::Internal("Failed to fetch policies".into()))?;

    let auth_context = AuthContext {
        tenant_id: service_account.tenant_id,
        service_account_id: sa_id,
        key_id: key_id.to_string(),
        service_account_name: service_account.name,
        environment: service_account.environment,
        policy_ids,
    };

    // Reconstruct request with body and auth context in extensions
    let mut request = Request::from_parts(parts, Body::from(body_bytes));
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

fn extract_header<'a>(
    headers: &'a axum::http::HeaderMap,
    name: &'static str,
) -> Result<&'a str, GatewayError> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GatewayError::Auth(AuthError::MissingHeader { header: name }))
}

fn decode_ed25519_public_key(pem: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    // Strip PEM headers and decode base64 content
    let base64_content = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    let der_bytes = base64::engine::general_purpose::STANDARD.decode(&base64_content)?;

    // Ed25519 public key in PKCS#8 SubjectPublicKeyInfo is 44 bytes:
    // 12 bytes header + 32 bytes key
    if der_bytes.len() == 44 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&der_bytes[12..44]);
        Ok(key)
    } else if der_bytes.len() == 32 {
        // Raw 32-byte key
        let mut key = [0u8; 32];
        key.copy_from_slice(&der_bytes);
        Ok(key)
    } else {
        Err("Invalid Ed25519 public key length".into())
    }
}
```

---

## 4. Policy Engine

### 4.1 Policy Evaluation Algorithm

```rust
// crates/smartgate-policy/src/engine.rs

use smartgate_types::{
    AuthContext, ChatCompletionsRequest, GatewayError, Policy, PolicyError,
};
use uuid::Uuid;

/// Policy evaluation result.
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    /// Effective maximum input tokens (most restrictive across all policies).
    pub max_input_tokens: Option<u32>,
    /// Effective maximum output tokens.
    pub max_output_tokens: Option<u32>,
    /// Whether streaming is permitted.
    pub allow_streaming: bool,
    /// Whether tool use is permitted.
    pub allow_tools: bool,
    /// Whether file input is permitted.
    pub allow_files: bool,
}

/// Evaluate all applicable policies for a request.
///
/// Evaluation order:
/// 1. Merge all bound policies (most restrictive wins)
/// 2. Check model access (allowlist/denylist)
/// 3. Check token limits
/// 4. Check feature flags (streaming, tools, files)
/// 5. Check rate limits (Redis)
/// 6. Check budget
///
/// Returns `PolicyDecision` on success, or `PolicyError` on denial.
#[tracing::instrument(name = "policy.evaluate", skip_all, fields(
    service_account_id = %auth.service_account_id,
    model = %request.model,
))]
pub async fn evaluate(
    pg_pool: &sqlx::PgPool,
    redis: &fred::clients::Pool,
    policy_cache: &moka::future::Cache<Uuid, Vec<Policy>>,
    auth: &AuthContext,
    request: &ChatCompletionsRequest,
) -> Result<PolicyDecision, GatewayError> {
    // 1. Load policies (from cache or DB)
    let policies = load_policies(pg_pool, policy_cache, &auth.policy_ids).await?;

    if policies.is_empty() {
        // No policies bound = default allow with no constraints
        return Ok(PolicyDecision {
            max_input_tokens: None,
            max_output_tokens: None,
            allow_streaming: true,
            allow_tools: false,
            allow_files: false,
        });
    }

    // 2. Merge policies (most restrictive wins)
    let merged = merge_policies(&policies);

    // 3. Check model access
    check_model_access(&merged, &request.model)?;

    // 4. Check token limits
    check_token_limits(&merged, request)?;

    // 5. Check feature flags
    check_features(&merged, request)?;

    // 6. Check rate limits
    check_rate_limit(redis, auth, &merged).await?;

    // 7. Check budget
    check_budget(pg_pool, redis, auth, &merged).await?;

    Ok(PolicyDecision {
        max_input_tokens: merged.max_input_tokens,
        max_output_tokens: merged.max_output_tokens,
        allow_streaming: merged.allow_streaming,
        allow_tools: merged.allow_tools,
        allow_files: merged.allow_files,
    })
}

async fn load_policies(
    pg_pool: &sqlx::PgPool,
    cache: &moka::future::Cache<Uuid, Vec<Policy>>,
    policy_ids: &[Uuid],
) -> Result<Vec<Policy>, GatewayError> {
    let mut policies = Vec::with_capacity(policy_ids.len());
    for id in policy_ids {
        if let Some(cached) = cache.get(id).await {
            policies.extend(cached);
        } else {
            let p = sqlx::query_as::<_, Policy>(
                "SELECT * FROM policies WHERE id = $1"
            )
            .bind(id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| GatewayError::Internal(format!("DB error: {e}")))?;

            if let Some(policy) = p {
                cache.insert(*id, vec![policy.clone()]).await;
                policies.push(policy);
            }
        }
    }
    Ok(policies)
}
```

### 4.2 Policy Merge Logic

```rust
// crates/smartgate-policy/src/merge.rs

use smartgate_types::Policy;

/// Merged policy: the result of combining multiple policies.
/// Uses "most restrictive wins" strategy.
pub struct MergedPolicy {
    pub allowed_models: Vec<String>,
    pub denied_models: Vec<String>,
    pub max_input_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub allow_streaming: bool,
    pub allow_tools: bool,
    pub allow_files: bool,
    pub rpm_limit: Option<u32>,
    pub concurrency_limit: Option<u32>,
    pub daily_budget: Option<rust_decimal::Decimal>,
    pub monthly_budget: Option<rust_decimal::Decimal>,
}

/// Merge multiple policies using "most restrictive wins" logic.
///
/// Rules:
/// - `allowed_models`: Intersection of all allowlists (empty = allow all)
/// - `denied_models`: Union of all denylists
/// - `max_input_tokens`: Minimum across all policies
/// - `max_output_tokens`: Minimum across all policies
/// - `allow_streaming`: AND (all must allow)
/// - `allow_tools`: AND (all must allow)
/// - `allow_files`: AND (all must allow)
/// - `rpm_limit`: Minimum across all policies
/// - `daily_budget`: Minimum across all policies
/// - `monthly_budget`: Minimum across all policies
pub fn merge_policies(policies: &[Policy]) -> MergedPolicy {
    let mut merged = MergedPolicy {
        allowed_models: Vec::new(),
        denied_models: Vec::new(),
        max_input_tokens: None,
        max_output_tokens: None,
        allow_streaming: true,
        allow_tools: true,
        allow_files: true,
        rpm_limit: None,
        concurrency_limit: None,
        daily_budget: None,
        monthly_budget: None,
    };

    let mut first_allowlist = true;

    for policy in policies {
        // Allowed models: intersection
        let policy_allowed: Vec<String> = serde_json::from_value(
            policy.allowed_models_json.clone()
        ).unwrap_or_default();

        if !policy_allowed.is_empty() {
            if first_allowlist {
                merged.allowed_models = policy_allowed;
                first_allowlist = false;
            } else {
                merged.allowed_models.retain(|m| policy_allowed.contains(m));
            }
        }

        // Denied models: union
        let policy_denied: Vec<String> = serde_json::from_value(
            policy.denied_models_json.clone()
        ).unwrap_or_default();
        for model in policy_denied {
            if !merged.denied_models.contains(&model) {
                merged.denied_models.push(model);
            }
        }

        // Token limits: minimum
        if let Some(limit) = policy.max_input_tokens {
            let limit = limit as u32;
            merged.max_input_tokens = Some(
                merged.max_input_tokens.map_or(limit, |current| current.min(limit))
            );
        }
        if let Some(limit) = policy.max_output_tokens {
            let limit = limit as u32;
            merged.max_output_tokens = Some(
                merged.max_output_tokens.map_or(limit, |current| current.min(limit))
            );
        }

        // Booleans: AND
        merged.allow_streaming = merged.allow_streaming && policy.allow_streaming;
        merged.allow_tools = merged.allow_tools && policy.allow_tools;
        merged.allow_files = merged.allow_files && policy.allow_files;

        // Rate limits: minimum
        if let Some(rpm) = policy.rpm_limit {
            let rpm = rpm as u32;
            merged.rpm_limit = Some(
                merged.rpm_limit.map_or(rpm, |current| current.min(rpm))
            );
        }

        // Budgets: minimum
        if let Some(daily) = &policy.daily_budget {
            merged.daily_budget = Some(
                merged.daily_budget.map_or(*daily, |current| current.min(*daily))
            );
        }
        if let Some(monthly) = &policy.monthly_budget {
            merged.monthly_budget = Some(
                merged.monthly_budget.map_or(*monthly, |current| current.min(*monthly))
            );
        }
    }

    merged
}
```

### 4.3 Model Access Check

```rust
// crates/smartgate-policy/src/model_access.rs

use crate::merge::MergedPolicy;
use smartgate_types::PolicyError;

/// Check if a model alias is allowed by the merged policy.
///
/// Logic:
/// 1. If denied_models is non-empty and contains the model -> DENY
/// 2. If allowed_models is non-empty and does NOT contain the model -> DENY
/// 3. Otherwise -> ALLOW
///
/// Supports glob patterns: "gpt-4*" matches "gpt-4", "gpt-4-turbo", etc.
pub fn check_model_access(merged: &MergedPolicy, model: &str) -> Result<(), PolicyError> {
    // Check denylist first
    for denied in &merged.denied_models {
        if matches_pattern(denied, model) {
            return Err(PolicyError::ModelDenied {
                model: model.to_string(),
            });
        }
    }

    // Check allowlist (empty = allow all)
    if !merged.allowed_models.is_empty() {
        let allowed = merged.allowed_models.iter().any(|pattern| {
            matches_pattern(pattern, model)
        });
        if !allowed {
            return Err(PolicyError::ModelDenied {
                model: model.to_string(),
            });
        }
    }

    Ok(())
}

/// Simple glob pattern matching for model names.
/// Supports trailing wildcard only: "gpt-4*" matches "gpt-4-turbo".
fn matches_pattern(pattern: &str, value: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}
```

### 4.4 Budget Check Flow

```rust
// crates/smartgate-policy/src/budget.rs

use smartgate_types::{AuthContext, PolicyError, GatewayError};
use crate::merge::MergedPolicy;

/// Check if the service account or tenant has exceeded budget limits.
///
/// Flow:
/// 1. Check Redis for cached budget usage snapshot
/// 2. If cache miss, compute from PostgreSQL usage_events aggregate
/// 3. Compare usage against policy budget limits
/// 4. If over budget, deny with BudgetExhausted
///
/// Budget periods:
/// - Daily: sum of estimated_cost where created_at >= start of UTC day
/// - Monthly: sum of estimated_cost where created_at >= start of UTC month
#[tracing::instrument(name = "policy.check_budget", skip_all)]
pub async fn check_budget(
    pg_pool: &sqlx::PgPool,
    redis: &fred::clients::Pool,
    auth: &AuthContext,
    merged: &MergedPolicy,
) -> Result<(), GatewayError> {
    // Check daily budget
    if let Some(daily_limit) = &merged.daily_budget {
        let daily_usage = get_daily_usage(pg_pool, redis, &auth.service_account_id).await?;
        if daily_usage >= *daily_limit {
            return Err(PolicyError::BudgetExhausted {
                budget_id: uuid::Uuid::nil(), // Simplified; real impl would track budget ID
            }.into());
        }
    }

    // Check monthly budget
    if let Some(monthly_limit) = &merged.monthly_budget {
        let monthly_usage = get_monthly_usage(pg_pool, redis, &auth.service_account_id).await?;
        if monthly_usage >= *monthly_limit {
            return Err(PolicyError::BudgetExhausted {
                budget_id: uuid::Uuid::nil(),
            }.into());
        }
    }

    Ok(())
}

async fn get_daily_usage(
    pg_pool: &sqlx::PgPool,
    redis: &fred::clients::Pool,
    service_account_id: &uuid::Uuid,
) -> Result<rust_decimal::Decimal, GatewayError> {
    let cache_key = format!(
        "budget:daily:{}:{}",
        service_account_id,
        chrono::Utc::now().format("%Y-%m-%d")
    );

    // Try Redis first
    let cached: Option<String> = redis.get(&cache_key).await
        .unwrap_or(None);
    if let Some(val) = cached {
        if let Ok(amount) = val.parse::<rust_decimal::Decimal>() {
            return Ok(amount);
        }
    }

    // Fallback to PostgreSQL
    let row = sqlx::query_scalar::<_, rust_decimal::Decimal>(
        r#"
        SELECT COALESCE(SUM(estimated_cost), 0)
        FROM usage_events
        WHERE service_account_id = $1
          AND created_at >= date_trunc('day', NOW() AT TIME ZONE 'UTC')
        "#,
    )
    .bind(service_account_id)
    .fetch_one(pg_pool)
    .await
    .map_err(|e| GatewayError::Internal(format!("Budget query failed: {e}")))?;

    // Cache in Redis with 60s TTL
    let _: Result<(), _> = redis.set(
        &cache_key,
        row.to_string(),
        Some(fred::types::Expiration::EX(60)),
        None,
        false,
    ).await;

    Ok(row)
}

async fn get_monthly_usage(
    pg_pool: &sqlx::PgPool,
    redis: &fred::clients::Pool,
    service_account_id: &uuid::Uuid,
) -> Result<rust_decimal::Decimal, GatewayError> {
    let cache_key = format!(
        "budget:monthly:{}:{}",
        service_account_id,
        chrono::Utc::now().format("%Y-%m")
    );

    let cached: Option<String> = redis.get(&cache_key).await.unwrap_or(None);
    if let Some(val) = cached {
        if let Ok(amount) = val.parse::<rust_decimal::Decimal>() {
            return Ok(amount);
        }
    }

    let row = sqlx::query_scalar::<_, rust_decimal::Decimal>(
        r#"
        SELECT COALESCE(SUM(estimated_cost), 0)
        FROM usage_events
        WHERE service_account_id = $1
          AND created_at >= date_trunc('month', NOW() AT TIME ZONE 'UTC')
        "#,
    )
    .bind(service_account_id)
    .fetch_one(pg_pool)
    .await
    .map_err(|e| GatewayError::Internal(format!("Budget query failed: {e}")))?;

    let _: Result<(), _> = redis.set(
        &cache_key,
        row.to_string(),
        Some(fred::types::Expiration::EX(60)),
        None,
        false,
    ).await;

    Ok(row)
}
```

### 4.5 Rate Limiting

```rust
// crates/smartgate-storage/src/redis/rate_limit.rs

use fred::prelude::*;
use smartgate_types::PolicyError;

/// Sliding window rate limiter using Redis.
///
/// Algorithm: Fixed-window counter with two windows for approximation.
///
/// Redis commands:
/// ```
/// -- Increment current window counter
/// INCR ratelimit:sa:{sa_id}:{current_window}
/// EXPIRE ratelimit:sa:{sa_id}:{current_window} {2 * window_seconds}
///
/// -- Get previous window counter
/// GET ratelimit:sa:{sa_id}:{previous_window}
///
/// -- Weighted count = previous_count * overlap_ratio + current_count
/// ```
///
/// This provides a smooth sliding window approximation without
/// the memory overhead of storing individual request timestamps.
pub async fn check_rate_limit(
    redis: &fred::clients::Pool,
    service_account_id: &uuid::Uuid,
    rpm_limit: u32,
) -> Result<(), PolicyError> {
    let window_seconds: u64 = 60; // 1 minute for RPM
    let now = chrono::Utc::now().timestamp() as u64;
    let current_window = now / window_seconds;
    let previous_window = current_window - 1;

    let current_key = format!("ratelimit:sa:{service_account_id}:{current_window}");
    let previous_key = format!("ratelimit:sa:{service_account_id}:{previous_window}");

    // Atomic pipeline: INCR current + GET previous
    let pipeline = redis.pipeline();
    pipeline.incr::<i64, _>(&current_key).await
        .map_err(|_| PolicyError::RateLimited { retry_after_secs: 1 })?;
    pipeline.expire::<bool, _>(&current_key, (window_seconds * 2) as i64).await
        .map_err(|_| PolicyError::RateLimited { retry_after_secs: 1 })?;
    let _results = pipeline.all::<Vec<fred::types::Resp3Frame>>().await;

    let current_count: i64 = redis.get(&current_key).await.unwrap_or(0);
    let previous_count: i64 = redis.get(&previous_key).await.unwrap_or(0);

    // Calculate weighted count (sliding window approximation)
    let elapsed_in_window = now % window_seconds;
    let overlap_ratio = 1.0 - (elapsed_in_window as f64 / window_seconds as f64);
    let weighted_count = (previous_count as f64 * overlap_ratio) + current_count as f64;

    if weighted_count > rpm_limit as f64 {
        let retry_after = window_seconds - elapsed_in_window;
        return Err(PolicyError::RateLimited {
            retry_after_secs: retry_after as u32,
        });
    }

    Ok(())
}
```

---

## 5. Routing Engine

### 5.1 Route Resolution Algorithm

```rust
// crates/smartgate-routing/src/resolver.rs

use smartgate_types::{AuthContext, GatewayError, ProviderRoute, RoutingError};
use uuid::Uuid;

/// An execution plan produced by the routing engine.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Ordered list of routes to try (primary first, then fallbacks).
    pub routes: Vec<RouteAttempt>,
}

#[derive(Debug, Clone)]
pub struct RouteAttempt {
    pub route: ProviderRoute,
    pub max_retries: u32,
    pub timeout: std::time::Duration,
    pub retry_backoff_base: std::time::Duration,
}

/// Resolve a model alias to an ordered execution plan.
///
/// Algorithm:
/// 1. Fetch routes for (model_alias, tenant_id) + global routes
/// 2. Filter: enabled = true
/// 3. Sort by: priority ASC (lower number = higher priority)
/// 4. Check circuit breaker state for each route
/// 5. Filter out OPEN circuit breakers
/// 6. Build ExecutionPlan from remaining routes
///
/// If no routes remain after filtering, return RoutingError::NoRoutesFound.
#[tracing::instrument(name = "routing.resolve", skip_all, fields(model_alias = %model_alias))]
pub async fn resolve(
    pg_pool: &sqlx::PgPool,
    redis: &fred::clients::Pool,
    route_cache: &moka::future::Cache<String, Vec<ProviderRoute>>,
    auth: &AuthContext,
    model_alias: &str,
) -> Result<ExecutionPlan, GatewayError> {
    // 1. Fetch routes (cache-first)
    let cache_key = format!("routes:{}:{}", auth.tenant_id, model_alias);
    let routes = match route_cache.get(&cache_key).await {
        Some(cached) => cached,
        None => {
            let fetched = fetch_routes(pg_pool, auth.tenant_id, model_alias).await?;
            route_cache.insert(cache_key, fetched.clone()).await;
            fetched
        }
    };

    // 2. Filter enabled and sort by priority
    let mut routes: Vec<ProviderRoute> = routes
        .into_iter()
        .filter(|r| r.enabled)
        .collect();
    routes.sort_by_key(|r| r.priority);

    if routes.is_empty() {
        return Err(RoutingError::NoRoutesFound {
            model_alias: model_alias.to_string(),
        }
        .into());
    }

    // 3. Check circuit breakers
    let mut available_routes = Vec::new();
    for route in routes {
        let cb_state = crate::circuit_breaker::get_state(
            redis,
            &route.provider,
            &route.provider_model_name,
        )
        .await;

        match cb_state {
            crate::circuit_breaker::CircuitState::Open => {
                tracing::debug!(
                    provider = %route.provider,
                    "Circuit breaker OPEN, skipping route"
                );
                continue;
            }
            crate::circuit_breaker::CircuitState::HalfOpen => {
                tracing::info!(
                    provider = %route.provider,
                    "Circuit breaker HALF_OPEN, allowing test request"
                );
            }
            crate::circuit_breaker::CircuitState::Closed => {}
        }

        available_routes.push(RouteAttempt {
            max_retries: route.max_retries as u32,
            timeout: std::time::Duration::from_millis(route.timeout_ms as u64),
            retry_backoff_base: std::time::Duration::from_millis(
                route.retry_backoff_ms as u64
            ),
            route,
        });
    }

    if available_routes.is_empty() {
        return Err(RoutingError::NoRoutesFound {
            model_alias: model_alias.to_string(),
        }
        .into());
    }

    Ok(ExecutionPlan {
        routes: available_routes,
    })
}

async fn fetch_routes(
    pg_pool: &sqlx::PgPool,
    tenant_id: Uuid,
    model_alias: &str,
) -> Result<Vec<ProviderRoute>, GatewayError> {
    let routes = sqlx::query_as::<_, ProviderRoute>(
        r#"
        SELECT id, tenant_id, model_alias, provider, provider_model_name,
               priority, enabled, timeout_ms, max_retries, retry_backoff_ms,
               fallback_group, region, created_at, updated_at
        FROM provider_routes
        WHERE model_alias = $1
          AND (tenant_id = $2 OR tenant_id IS NULL)
          AND enabled = true
        ORDER BY priority ASC
        "#,
    )
    .bind(model_alias)
    .bind(tenant_id)
    .fetch_all(pg_pool)
    .await
    .map_err(|e| GatewayError::Internal(format!("Route fetch failed: {e}")))?;

    Ok(routes)
}
```

### 5.2 Retry Logic with Exponential Backoff

```rust
// crates/smartgate-routing/src/retry.rs

use std::time::Duration;
use rand::Rng;

/// Calculate retry delay using exponential backoff with full jitter.
///
/// Formula: random(0, min(cap, base * 2^attempt))
///
/// References:
/// - AWS Architecture Blog: "Exponential Backoff And Jitter"
/// - The "full jitter" approach provides the best distribution
///   and prevents thundering herd effects.
///
/// # Arguments
/// * `base` - Base backoff duration (from route config, typically 250ms)
/// * `attempt` - Zero-indexed attempt number (0 = first retry)
/// * `cap` - Maximum backoff duration (default 30 seconds)
///
/// # Returns
/// Duration to wait before the next retry attempt.
pub fn calculate_backoff(base: Duration, attempt: u32, cap: Duration) -> Duration {
    let base_ms = base.as_millis() as u64;
    let cap_ms = cap.as_millis() as u64;

    // Exponential: base * 2^attempt, capped
    let exponential_ms = base_ms.saturating_mul(1u64 << attempt.min(16));
    let bounded_ms = exponential_ms.min(cap_ms);

    // Full jitter: random(0, bounded)
    let jitter_ms = if bounded_ms > 0 {
        rand::thread_rng().gen_range(0..=bounded_ms)
    } else {
        0
    };

    Duration::from_millis(jitter_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_increases() {
        let base = Duration::from_millis(250);
        let cap = Duration::from_secs(30);

        // First retry: max 250ms
        for _ in 0..100 {
            let delay = calculate_backoff(base, 0, cap);
            assert!(delay <= Duration::from_millis(250));
        }

        // Second retry: max 500ms
        for _ in 0..100 {
            let delay = calculate_backoff(base, 1, cap);
            assert!(delay <= Duration::from_millis(500));
        }
    }

    #[test]
    fn test_backoff_capped() {
        let base = Duration::from_millis(250);
        let cap = Duration::from_secs(30);

        for _ in 0..100 {
            let delay = calculate_backoff(base, 20, cap);
            assert!(delay <= cap);
        }
    }
}
```

### 5.3 Fallback Chain Execution

```rust
// crates/smartgate-routing/src/execution.rs

use crate::resolver::{ExecutionPlan, RouteAttempt};
use crate::retry::calculate_backoff;
use smartgate_types::{
    ChatCompletionsRequest, ChatCompletionsResponse, GatewayError,
    ProviderError, RoutingError, AuthContext,
};
use std::time::Duration;

/// Metadata about the provider call for usage tracking.
#[derive(Debug, Clone)]
pub struct ProviderCallMetadata {
    pub provider: String,
    pub provider_model_name: String,
    pub route_id: uuid::Uuid,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub retry_count: u32,
    pub final_status: String,
}

/// Record of a single provider attempt (for error reporting).
#[derive(Debug, Clone)]
struct AttemptRecord {
    provider: String,
    model: String,
    error: String,
    attempt: u32,
}

/// Execute a request through the execution plan with retry and fallback.
///
/// Algorithm:
/// ```text
/// for each route in execution_plan.routes:
///     for attempt in 0..=route.max_retries:
///         result = call_provider(route, request, timeout)
///         if success:
///             record_success(circuit_breaker)
///             return response
///         if non-retryable error (400, 401, 403):
///             record_failure(circuit_breaker)
///             break to next route
///         if retryable error (5xx, timeout, connection):
///             record_failure(circuit_breaker)
///             if attempt < max_retries:
///                 sleep(exponential_backoff)
///             else:
///                 break to next route
/// return AllProvidersFailed
/// ```
#[tracing::instrument(name = "routing.execute", skip_all)]
pub async fn execute(
    http_client: &reqwest::Client,
    redis: &fred::clients::Pool,
    plan: ExecutionPlan,
    request: ChatCompletionsRequest,
    auth: &AuthContext,
) -> Result<(ChatCompletionsResponse, ProviderCallMetadata), GatewayError> {
    let mut attempt_records: Vec<AttemptRecord> = Vec::new();
    let backoff_cap = Duration::from_secs(30);

    for route_attempt in &plan.routes {
        let route = &route_attempt.route;
        let provider_name = &route.provider;

        for attempt in 0..=route_attempt.max_retries {
            let span = tracing::info_span!(
                "provider.call",
                provider = %provider_name,
                model = %route.provider_model_name,
                attempt = attempt,
            );
            let _guard = span.enter();

            // Call the provider with timeout
            let result = tokio::time::timeout(
                route_attempt.timeout,
                smartgate_providers::call(
                    http_client,
                    &route.provider,
                    &route.provider_model_name,
                    &request,
                    route,
                ),
            )
            .await;

            match result {
                // Timeout
                Err(_elapsed) => {
                    tracing::warn!(
                        provider = %provider_name,
                        attempt = attempt,
                        "Provider call timed out"
                    );
                    attempt_records.push(AttemptRecord {
                        provider: provider_name.clone(),
                        model: route.provider_model_name.clone(),
                        error: "timeout".into(),
                        attempt,
                    });
                    crate::circuit_breaker::record_failure(
                        redis, provider_name, &route.provider_model_name
                    ).await;
                }

                // Provider returned a result
                Ok(Ok(response)) => {
                    // Success
                    crate::circuit_breaker::record_success(
                        redis, provider_name, &route.provider_model_name
                    ).await;

                    let meta = ProviderCallMetadata {
                        provider: provider_name.clone(),
                        provider_model_name: route.provider_model_name.clone(),
                        route_id: route.id,
                        prompt_tokens: response.usage.as_ref().map_or(0, |u| u.prompt_tokens),
                        completion_tokens: response.usage.as_ref().map_or(0, |u| u.completion_tokens),
                        total_tokens: response.usage.as_ref().map_or(0, |u| u.total_tokens),
                        retry_count: attempt,
                        final_status: "success".into(),
                    };
                    return Ok((response, meta));
                }

                Ok(Err(provider_err)) => {
                    let is_retryable = matches!(
                        &provider_err,
                        ProviderError::ServerError { .. }
                        | ProviderError::Timeout { .. }
                        | ProviderError::ConnectionFailed { .. }
                        | ProviderError::RateLimited { .. }
                    );

                    attempt_records.push(AttemptRecord {
                        provider: provider_name.clone(),
                        model: route.provider_model_name.clone(),
                        error: provider_err.to_string(),
                        attempt,
                    });

                    crate::circuit_breaker::record_failure(
                        redis, provider_name, &route.provider_model_name
                    ).await;

                    if !is_retryable {
                        tracing::warn!(
                            provider = %provider_name,
                            error = %provider_err,
                            "Non-retryable provider error, moving to fallback"
                        );
                        break; // Move to next route
                    }

                    if attempt < route_attempt.max_retries {
                        let delay = calculate_backoff(
                            route_attempt.retry_backoff_base,
                            attempt,
                            backoff_cap,
                        );
                        tracing::info!(
                            provider = %provider_name,
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Retrying after backoff"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
    }

    // All routes exhausted
    Err(RoutingError::AllProvidersFailed {
        attempts: attempt_records.len(),
    }
    .into())
}
```

### 5.4 Circuit Breaker State Machine

```rust
// crates/smartgate-routing/src/circuit_breaker.rs

use fred::prelude::*;
use std::time::Duration;

/// Circuit breaker states.
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Normal operation; requests flow through.
    Closed,
    /// Failures exceeded threshold; requests rejected immediately.
    Open,
    /// After wait duration; allowing a single test request.
    HalfOpen,
}

/// Configuration for circuit breaker behavior.
const FAILURE_THRESHOLD: u32 = 5;        // Failures before opening
const WINDOW_SECONDS: u64 = 60;          // Sliding window for counting failures
const OPEN_DURATION_SECONDS: u64 = 30;   // How long to stay open before half-open

/// Get current circuit breaker state for a provider.
///
/// Redis keys:
/// - `cb:failures:{provider}:{model}` -> failure count (expires with window)
/// - `cb:open:{provider}:{model}` -> timestamp when circuit opened (expires after open_duration)
///
/// State machine:
/// ```text
///   CLOSED   -- failures >= threshold --> OPEN
///   OPEN     -- open key expired -------> HALF_OPEN
///   HALF_OPEN -- success ----------------> CLOSED  (reset failures)
///   HALF_OPEN -- failure ----------------> OPEN    (reset open timer)
/// ```
pub async fn get_state(
    redis: &fred::clients::Pool,
    provider: &str,
    model: &str,
) -> CircuitState {
    let open_key = format!("cb:open:{provider}:{model}");

    // Check if circuit is open
    let is_open: bool = redis
        .exists::<bool, _>(&open_key)
        .await
        .unwrap_or(false);

    if is_open {
        return CircuitState::Open;
    }

    // Check failure count
    let failures_key = format!("cb:failures:{provider}:{model}");
    let failure_count: u32 = redis
        .get::<u32, _>(&failures_key)
        .await
        .unwrap_or(0);

    if failure_count >= FAILURE_THRESHOLD {
        // Transition to Open (the key may have just expired)
        CircuitState::HalfOpen
    } else {
        CircuitState::Closed
    }
}

/// Record a failure for circuit breaker tracking.
pub async fn record_failure(
    redis: &fred::clients::Pool,
    provider: &str,
    model: &str,
) {
    let failures_key = format!("cb:failures:{provider}:{model}");
    let open_key = format!("cb:open:{provider}:{model}");

    // Increment failure counter
    let count: u32 = redis
        .incr(&failures_key)
        .await
        .unwrap_or(1);

    // Set TTL on failures key if this is the first failure
    if count == 1 {
        let _: Result<(), _> = redis
            .expire(&failures_key, WINDOW_SECONDS as i64)
            .await;
    }

    // If threshold exceeded, open the circuit
    if count >= FAILURE_THRESHOLD {
        let _: Result<(), _> = redis
            .set(
                &open_key,
                chrono::Utc::now().timestamp().to_string(),
                Some(Expiration::EX(OPEN_DURATION_SECONDS as i64)),
                None,
                false,
            )
            .await;
        tracing::warn!(
            provider = provider,
            model = model,
            failures = count,
            "Circuit breaker OPENED"
        );
    }
}

/// Record a success, resetting the circuit breaker.
pub async fn record_success(
    redis: &fred::clients::Pool,
    provider: &str,
    model: &str,
) {
    let failures_key = format!("cb:failures:{provider}:{model}");
    let open_key = format!("cb:open:{provider}:{model}");

    // Reset both keys
    let _: Result<(), _> = redis.del(&failures_key).await;
    let _: Result<(), _> = redis.del(&open_key).await;
}
```

---

## 6. Provider Adapters

### 6.1 Provider Trait Definition

```rust
// crates/smartgate-providers/src/traits.rs

use async_trait::async_trait;
use smartgate_types::{
    ChatCompletionsRequest, ChatCompletionsResponse, ChatCompletionChunk,
    ProviderError, ProviderRoute,
};
use futures::Stream;
use std::pin::Pin;

/// Type alias for a boxed stream of SSE chunks.
pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send>>;

/// Trait that all provider adapters must implement.
///
/// Each provider translates between the gateway's normalized format
/// (OpenAI-compatible) and the provider's native API format.
#[async_trait]
pub trait ProviderAdapter: Send + Sync + 'static {
    /// Provider identifier (e.g., "openai", "anthropic").
    fn name(&self) -> &'static str;

    /// Execute a non-streaming chat completion request.
    ///
    /// # Arguments
    /// * `client` - Shared HTTP client (with connection pooling)
    /// * `route` - Provider route configuration (endpoint, credentials, etc.)
    /// * `request` - Normalized OpenAI-compatible request
    ///
    /// # Returns
    /// Normalized OpenAI-compatible response or provider error.
    async fn chat_completion(
        &self,
        client: &reqwest::Client,
        route: &ProviderRoute,
        request: &ChatCompletionsRequest,
    ) -> Result<ChatCompletionsResponse, ProviderError>;

    /// Execute a streaming chat completion request.
    ///
    /// Returns a stream of normalized chunks that can be relayed
    /// directly to the client as SSE events.
    async fn chat_completion_stream(
        &self,
        client: &reqwest::Client,
        route: &ProviderRoute,
        request: &ChatCompletionsRequest,
    ) -> Result<ChunkStream, ProviderError>;

    /// Map a provider-specific error response to a normalized ProviderError.
    fn map_error(&self, status: u16, body: &str) -> ProviderError;
}

/// Registry of provider adapters, keyed by provider name.
pub struct ProviderRegistry {
    adapters: std::collections::HashMap<String, Box<dyn ProviderAdapter>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut adapters: std::collections::HashMap<String, Box<dyn ProviderAdapter>> =
            std::collections::HashMap::new();

        adapters.insert("openai".into(), Box::new(super::openai::OpenAIAdapter));
        adapters.insert("anthropic".into(), Box::new(super::anthropic::AnthropicAdapter));
        // Future: gemini, azure_openai, vllm

        Self { adapters }
    }

    pub fn get(&self, provider: &str) -> Option<&dyn ProviderAdapter> {
        self.adapters.get(provider).map(|a| a.as_ref())
    }
}
```

### 6.2 OpenAI Adapter

```rust
// crates/smartgate-providers/src/openai.rs

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use smartgate_types::{
    ChatCompletionChunk, ChatCompletionsRequest, ChatCompletionsResponse,
    ProviderError, ProviderRoute,
};
use crate::traits::{ChunkStream, ProviderAdapter};

pub struct OpenAIAdapter;

#[async_trait]
impl ProviderAdapter for OpenAIAdapter {
    fn name(&self) -> &'static str {
        "openai"
    }

    #[tracing::instrument(name = "provider.openai.chat_completion", skip_all)]
    async fn chat_completion(
        &self,
        client: &reqwest::Client,
        route: &ProviderRoute,
        request: &ChatCompletionsRequest,
    ) -> Result<ChatCompletionsResponse, ProviderError> {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| ProviderError::AuthFailure {
                provider: "openai".into(),
            })?;

        // OpenAI uses the same format as our normalized request,
        // so we can forward mostly unchanged.
        let mut body = serde_json::to_value(request)
            .map_err(|e| ProviderError::BadRequest {
                provider: "openai".into(),
                message: e.to_string(),
            })?;

        // Override model with provider-specific model name
        body["model"] = serde_json::Value::String(
            route.provider_model_name.clone()
        );
        // Ensure stream is false
        body["stream"] = serde_json::Value::Bool(false);

        let response = client
            .post(format!("{base_url}/chat/completions"))
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionFailed {
                provider: "openai".into(),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body_text));
        }

        let result: ChatCompletionsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::BadRequest {
                provider: "openai".into(),
                message: format!("Failed to parse response: {e}"),
            })?;

        Ok(result)
    }

    #[tracing::instrument(name = "provider.openai.chat_completion_stream", skip_all)]
    async fn chat_completion_stream(
        &self,
        client: &reqwest::Client,
        route: &ProviderRoute,
        request: &ChatCompletionsRequest,
    ) -> Result<ChunkStream, ProviderError> {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| ProviderError::AuthFailure {
                provider: "openai".into(),
            })?;

        let mut body = serde_json::to_value(request)
            .map_err(|e| ProviderError::BadRequest {
                provider: "openai".into(),
                message: e.to_string(),
            })?;

        body["model"] = serde_json::Value::String(route.provider_model_name.clone());
        body["stream"] = serde_json::Value::Bool(true);

        let response = client
            .post(format!("{base_url}/chat/completions"))
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
            .map_err(|_| ProviderError::ConnectionFailed {
                provider: "openai".into(),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body_text));
        }

        // Parse SSE stream from OpenAI
        let byte_stream = response.bytes_stream();
        let chunk_stream = parse_openai_sse_stream(byte_stream);

        Ok(Box::pin(chunk_stream))
    }

    fn map_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            401 => ProviderError::AuthFailure { provider: "openai".into() },
            429 => {
                // Parse Retry-After from body if available
                ProviderError::RateLimited {
                    provider: "openai".into(),
                    retry_after: None,
                }
            }
            400 => ProviderError::BadRequest {
                provider: "openai".into(),
                message: extract_openai_error_message(body),
            },
            500..=599 => ProviderError::ServerError {
                provider: "openai".into(),
                status,
            },
            _ => ProviderError::ServerError {
                provider: "openai".into(),
                status,
            },
        }
    }
}

/// Parse OpenAI SSE stream into normalized chunks.
///
/// OpenAI SSE format:
/// ```text
/// data: {"id":"chatcmpl-...","object":"chat.completion.chunk",...}
///
/// data: {"id":"chatcmpl-...","object":"chat.completion.chunk",...}
///
/// data: [DONE]
/// ```
fn parse_openai_sse_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send {
    let mut buffer = String::new();

    byte_stream
        .map(move |chunk_result| {
            match chunk_result {
                Err(_) => vec![Err(ProviderError::ConnectionFailed {
                    provider: "openai".into(),
                })],
                Ok(bytes) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    let mut events = Vec::new();

                    // Process complete SSE events (terminated by double newline)
                    while let Some(pos) = buffer.find("\n\n") {
                        let event = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();

                        for line in event.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    // Stream complete
                                    continue;
                                }
                                match serde_json::from_str::<ChatCompletionChunk>(data) {
                                    Ok(chunk) => events.push(Ok(chunk)),
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            data = data,
                                            "Failed to parse OpenAI SSE chunk"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    events
                }
            }
        })
        .flat_map(futures::stream::iter)
}

fn extract_openai_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or_else(|| body.to_string())
}
```

### 6.3 Anthropic Adapter

```rust
// crates/smartgate-providers/src/anthropic.rs

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use smartgate_types::{
    ChatCompletionChunk, ChatCompletionsRequest, ChatCompletionsResponse,
    ProviderError, ProviderRoute, ChatRole, MessageContent, Usage,
    Choice, AssistantMessage, Delta, ChunkChoice,
};
use crate::traits::{ChunkStream, ProviderAdapter};
use serde::{Deserialize, Serialize};

pub struct AnthropicAdapter;

/// Anthropic-specific request format.
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Anthropic-specific response format.
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContent>,
    usage: AnthropicUsage,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    #[tracing::instrument(name = "provider.anthropic.chat_completion", skip_all)]
    async fn chat_completion(
        &self,
        client: &reqwest::Client,
        route: &ProviderRoute,
        request: &ChatCompletionsRequest,
    ) -> Result<ChatCompletionsResponse, ProviderError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ProviderError::AuthFailure {
                provider: "anthropic".into(),
            })?;

        let anthropic_request = transform_to_anthropic(request, &route.provider_model_name, false);

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|_| ProviderError::ConnectionFailed {
                provider: "anthropic".into(),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body));
        }

        let anthropic_resp: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::BadRequest {
                provider: "anthropic".into(),
                message: format!("Parse error: {e}"),
            })?;

        Ok(normalize_anthropic_response(anthropic_resp))
    }

    #[tracing::instrument(name = "provider.anthropic.chat_completion_stream", skip_all)]
    async fn chat_completion_stream(
        &self,
        client: &reqwest::Client,
        route: &ProviderRoute,
        request: &ChatCompletionsRequest,
    ) -> Result<ChunkStream, ProviderError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ProviderError::AuthFailure {
                provider: "anthropic".into(),
            })?;

        let anthropic_request = transform_to_anthropic(request, &route.provider_model_name, true);

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|_| ProviderError::ConnectionFailed {
                provider: "anthropic".into(),
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body));
        }

        let byte_stream = response.bytes_stream();
        let chunk_stream = parse_anthropic_sse_stream(byte_stream);

        Ok(Box::pin(chunk_stream))
    }

    fn map_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            401 => ProviderError::AuthFailure { provider: "anthropic".into() },
            429 => ProviderError::RateLimited {
                provider: "anthropic".into(),
                retry_after: None,
            },
            400 => ProviderError::BadRequest {
                provider: "anthropic".into(),
                message: body.to_string(),
            },
            500..=599 => ProviderError::ServerError {
                provider: "anthropic".into(),
                status,
            },
            _ => ProviderError::ServerError {
                provider: "anthropic".into(),
                status,
            },
        }
    }
}

/// Transform normalized OpenAI request to Anthropic format.
///
/// Key differences:
/// - System message is a top-level field, not in messages array
/// - Messages use "user"/"assistant" roles (no "system")
/// - Content is a string, not array (for text-only)
/// - max_tokens is required (default to 4096 if not specified)
fn transform_to_anthropic(
    request: &ChatCompletionsRequest,
    model: &str,
    stream: bool,
) -> AnthropicRequest {
    let mut system_message = None;
    let mut messages = Vec::new();

    for msg in &request.messages {
        match msg.role {
            ChatRole::System => {
                if let MessageContent::Text(text) = &msg.content {
                    system_message = Some(text.clone());
                }
            }
            ChatRole::User | ChatRole::Assistant | ChatRole::Tool => {
                let role = match msg.role {
                    ChatRole::User | ChatRole::Tool => "user",
                    ChatRole::Assistant => "assistant",
                    _ => "user",
                };
                let content = match &msg.content {
                    MessageContent::Text(text) => text.clone(),
                    MessageContent::Parts(parts) => {
                        parts
                            .iter()
                            .filter_map(|p| match p {
                                smartgate_types::ContentPart::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                };
                messages.push(AnthropicMessage {
                    role: role.to_string(),
                    content,
                });
            }
        }
    }

    AnthropicRequest {
        model: model.to_string(),
        messages,
        max_tokens: request.max_tokens.unwrap_or(4096),
        system: system_message,
        temperature: request.temperature,
        top_p: request.top_p,
        stream,
    }
}

/// Normalize Anthropic response to OpenAI format.
fn normalize_anthropic_response(resp: AnthropicResponse) -> ChatCompletionsResponse {
    let content = resp
        .content
        .iter()
        .filter_map(|c| c.text.as_ref())
        .cloned()
        .collect::<Vec<_>>()
        .join("");

    ChatCompletionsResponse {
        id: resp.id,
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: resp.model,
        choices: vec![Choice {
            index: 0,
            message: AssistantMessage {
                role: "assistant".to_string(),
                content: Some(content),
                tool_calls: None,
            },
            finish_reason: resp.stop_reason,
        }],
        usage: Some(Usage {
            prompt_tokens: resp.usage.input_tokens,
            completion_tokens: resp.usage.output_tokens,
            total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
        }),
        system_fingerprint: None,
    }
}

/// Parse Anthropic SSE stream into normalized OpenAI-compatible chunks.
///
/// Anthropic SSE events:
/// ```text
/// event: message_start
/// data: {"type":"message_start","message":{...}}
///
/// event: content_block_delta
/// data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}
///
/// event: message_delta
/// data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{...}}
///
/// event: message_stop
/// data: {"type":"message_stop"}
/// ```
fn parse_anthropic_sse_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send {
    let mut buffer = String::new();
    let mut message_id = String::new();
    let mut model = String::new();

    byte_stream
        .map(move |chunk_result| {
            match chunk_result {
                Err(_) => vec![Err(ProviderError::ConnectionFailed {
                    provider: "anthropic".into(),
                })],
                Ok(bytes) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    let mut events = Vec::new();

                    while let Some(pos) = buffer.find("\n\n") {
                        let raw_event = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();

                        // Parse SSE event type and data
                        let mut event_type = "";
                        let mut data = "";
                        for line in raw_event.lines() {
                            if let Some(t) = line.strip_prefix("event: ") {
                                event_type = t.trim();
                            }
                            if let Some(d) = line.strip_prefix("data: ") {
                                data = d.trim();
                            }
                        }

                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
                            match event_type {
                                "message_start" => {
                                    if let Some(msg) = value.get("message") {
                                        message_id = msg["id"].as_str().unwrap_or("").to_string();
                                        model = msg["model"].as_str().unwrap_or("").to_string();
                                    }
                                }
                                "content_block_delta" => {
                                    if let Some(delta) = value.get("delta") {
                                        if let Some(text) = delta["text"].as_str() {
                                            events.push(Ok(ChatCompletionChunk {
                                                id: message_id.clone(),
                                                object: "chat.completion.chunk".to_string(),
                                                created: chrono::Utc::now().timestamp(),
                                                model: model.clone(),
                                                choices: vec![ChunkChoice {
                                                    index: 0,
                                                    delta: Delta {
                                                        role: None,
                                                        content: Some(text.to_string()),
                                                        tool_calls: None,
                                                    },
                                                    finish_reason: None,
                                                }],
                                                usage: None,
                                            }));
                                        }
                                    }
                                }
                                "message_delta" => {
                                    if let Some(delta) = value.get("delta") {
                                        let stop_reason = delta["stop_reason"]
                                            .as_str()
                                            .map(|s| s.to_string());
                                        events.push(Ok(ChatCompletionChunk {
                                            id: message_id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            created: chrono::Utc::now().timestamp(),
                                            model: model.clone(),
                                            choices: vec![ChunkChoice {
                                                index: 0,
                                                delta: Delta {
                                                    role: None,
                                                    content: None,
                                                    tool_calls: None,
                                                },
                                                finish_reason: stop_reason,
                                            }],
                                            usage: None,
                                        }));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    events
                }
            }
        })
        .flat_map(futures::stream::iter)
}
```

### 6.4 Error Mapping Table

| HTTP Status | OpenAI Error | Anthropic Error | Gateway Error |
|-------------|-------------|----------------|---------------|
| 400 | `invalid_request_error` | `invalid_request_error` | `ProviderError::BadRequest` |
| 401 | `authentication_error` | `authentication_error` | `ProviderError::AuthFailure` |
| 403 | `permission_error` | `permission_denied` | `ProviderError::AuthFailure` |
| 404 | `not_found` | `not_found_error` | `ProviderError::BadRequest` |
| 429 | `rate_limit_error` | `rate_limit_error` | `ProviderError::RateLimited` |
| 500 | `server_error` | `api_error` | `ProviderError::ServerError` |
| 502 | N/A | N/A | `ProviderError::ServerError` |
| 503 | `overloaded` | `overloaded_error` | `ProviderError::ServerError` |
| Timeout | N/A | N/A | `ProviderError::Timeout` |
| Connection refused | N/A | N/A | `ProviderError::ConnectionFailed` |

---

## 7. Streaming Engine

### 7.1 SSE Event Format

```rust
// crates/smartgate-streaming/src/events.rs

use serde::Serialize;
use smartgate_types::ChatCompletionChunk;

/// Gateway SSE event types.
#[derive(Debug, Clone)]
pub enum SseEvent {
    /// A chunk of the completion response.
    Chunk(ChatCompletionChunk),
    /// Stream is complete. Sent as "data: [DONE]".
    Done,
    /// An error occurred during streaming.
    Error(StreamError),
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamError {
    pub code: String,
    pub message: String,
}

impl SseEvent {
    /// Convert to SSE wire format.
    ///
    /// Format:
    /// ```text
    /// data: {"id":"...","object":"chat.completion.chunk",...}\n\n
    /// ```
    /// or:
    /// ```text
    /// data: [DONE]\n\n
    /// ```
    pub fn to_sse_string(&self) -> String {
        match self {
            SseEvent::Chunk(chunk) => {
                let json = serde_json::to_string(chunk).unwrap_or_default();
                format!("data: {json}\n\n")
            }
            SseEvent::Done => "data: [DONE]\n\n".to_string(),
            SseEvent::Error(err) => {
                let json = serde_json::to_string(err).unwrap_or_default();
                format!("event: error\ndata: {json}\n\n")
            }
        }
    }
}
```

### 7.2 Streaming Relay with Backpressure

```rust
// crates/smartgate-streaming/src/relay.rs

use futures::{Stream, StreamExt};
use smartgate_types::{ChatCompletionChunk, ProviderError, Usage};
use crate::events::SseEvent;
use std::pin::Pin;
use tokio::sync::mpsc;

/// Configuration for the streaming relay.
const CHANNEL_CAPACITY: usize = 1024;

/// Accumulated usage during streaming (tokens are reported at end).
#[derive(Debug, Default)]
pub struct StreamUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub chunk_count: u32,
    pub is_complete: bool,
}

/// Create a streaming relay that bridges an upstream provider stream
/// to a downstream Axum SSE response.
///
/// Architecture:
/// ```text
/// Provider Stream --> [Upstream Consumer Task] --> bounded mpsc --> [Axum SSE Response]
///                                                 (cap 1024)
/// ```
///
/// Backpressure: If the client reads slowly, the mpsc channel fills up.
/// The upstream consumer task blocks on channel send, which in turn
/// stops reading from the provider TCP stream, applying TCP-level
/// backpressure to the provider.
///
/// Cancellation: If the client disconnects, the mpsc receiver is dropped.
/// The upstream consumer detects the closed channel and aborts the
/// provider request.
pub fn create_relay(
    upstream: Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send>>,
    usage_sender: mpsc::Sender<smartgate_types::UsageEvent>,
    request_meta: StreamRequestMeta,
) -> impl Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send {
    let (tx, rx) = mpsc::channel::<SseEvent>(CHANNEL_CAPACITY);

    // Spawn upstream consumer task
    tokio::spawn(upstream_consumer(upstream, tx, usage_sender, request_meta));

    // Convert receiver to Stream for Axum SSE
    tokio_stream::wrappers::ReceiverStream::new(rx).map(|event| {
        Ok(axum::response::sse::Event::default()
            .data(match &event {
                SseEvent::Chunk(chunk) => serde_json::to_string(chunk).unwrap_or_default(),
                SseEvent::Done => "[DONE]".to_string(),
                SseEvent::Error(err) => serde_json::to_string(err).unwrap_or_default(),
            }))
    })
}

/// Metadata needed for usage tracking after stream completes.
#[derive(Debug, Clone)]
pub struct StreamRequestMeta {
    pub tenant_id: uuid::Uuid,
    pub service_account_id: uuid::Uuid,
    pub provider: String,
    pub provider_model_name: String,
    pub model_alias: String,
    pub route_id: uuid::Uuid,
    pub request_id: String,
}

/// Background task that consumes the upstream provider stream,
/// normalizes events, and sends them through the bounded channel.
async fn upstream_consumer(
    mut upstream: Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send>>,
    tx: mpsc::Sender<SseEvent>,
    usage_sender: mpsc::Sender<smartgate_types::UsageEvent>,
    meta: StreamRequestMeta,
) {
    let mut accumulated_usage = StreamUsage::default();
    let start = std::time::Instant::now();

    loop {
        tokio::select! {
            // Check if downstream client disconnected
            _ = tx.closed() => {
                tracing::info!(
                    request_id = %meta.request_id,
                    chunks = accumulated_usage.chunk_count,
                    "Client disconnected, cancelling upstream"
                );
                // Drop upstream to cancel the provider request
                drop(upstream);
                // Record partial usage
                persist_usage(&usage_sender, &meta, &accumulated_usage, start.elapsed(), "partial").await;
                return;
            }

            // Read next chunk from upstream
            chunk = upstream.next() => {
                match chunk {
                    Some(Ok(chunk_data)) => {
                        accumulated_usage.chunk_count += 1;

                        // Track usage from final chunk (if provider reports it)
                        if let Some(usage) = &chunk_data.usage {
                            accumulated_usage.prompt_tokens = usage.prompt_tokens;
                            accumulated_usage.completion_tokens = usage.completion_tokens;
                            accumulated_usage.total_tokens = usage.total_tokens;
                        }

                        // Send to client (backpressure happens here)
                        if tx.send(SseEvent::Chunk(chunk_data)).await.is_err() {
                            // Receiver dropped (client disconnected)
                            tracing::info!("Channel closed while sending chunk");
                            persist_usage(&usage_sender, &meta, &accumulated_usage, start.elapsed(), "partial").await;
                            return;
                        }
                    }
                    Some(Err(err)) => {
                        tracing::error!(error = %err, "Upstream provider error during stream");
                        let _ = tx.send(SseEvent::Error(crate::events::StreamError {
                            code: "provider.stream_error".into(),
                            message: err.to_string(),
                        })).await;
                        persist_usage(&usage_sender, &meta, &accumulated_usage, start.elapsed(), "error").await;
                        return;
                    }
                    None => {
                        // Stream complete
                        accumulated_usage.is_complete = true;
                        let _ = tx.send(SseEvent::Done).await;
                        persist_usage(&usage_sender, &meta, &accumulated_usage, start.elapsed(), "success").await;
                        return;
                    }
                }
            }
        }
    }
}

async fn persist_usage(
    usage_sender: &mpsc::Sender<smartgate_types::UsageEvent>,
    meta: &StreamRequestMeta,
    usage: &StreamUsage,
    latency: std::time::Duration,
    status: &str,
) {
    let event = smartgate_types::UsageEvent {
        request_id: meta.request_id.clone(),
        tenant_id: meta.tenant_id,
        service_account_id: meta.service_account_id,
        provider_route_id: Some(meta.route_id),
        provider: meta.provider.clone(),
        model_alias: meta.model_alias.clone(),
        provider_model_name: meta.provider_model_name.clone(),
        prompt_tokens: usage.prompt_tokens as i32,
        completion_tokens: usage.completion_tokens as i32,
        total_tokens: usage.total_tokens as i32,
        estimated_cost: rust_decimal::Decimal::ZERO, // Calculated by usage module
        currency: "USD".to_string(),
        latency_ms: latency.as_millis() as i32,
        retry_count: 0,
        final_status: status.to_string(),
        is_streaming: true,
    };
    let _ = usage_sender.try_send(event);
}
```

---

## 8. Usage Metering

### 8.1 Usage Event Structure

```rust
// crates/smartgate-types/src/events.rs

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A usage event represents one completed LLM request for billing and analytics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub request_id: String,
    pub tenant_id: Uuid,
    pub service_account_id: Uuid,
    pub provider_route_id: Option<Uuid>,
    pub provider: String,
    pub model_alias: String,
    pub provider_model_name: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub estimated_cost: Decimal,
    pub currency: String,
    pub latency_ms: i32,
    pub retry_count: i32,
    pub final_status: String,
    pub is_streaming: bool,
}

/// An audit event records an administrative action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub tenant_id: Option<Uuid>,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub metadata: serde_json::Value,
}
```

### 8.2 Async Persistence Pattern

```rust
// crates/smartgate-usage/src/meter.rs

use smartgate_types::UsageEvent;
use tokio::sync::mpsc;

/// Batch size for bulk inserts.
const BATCH_SIZE: usize = 100;
/// Maximum wait time before flushing a partial batch.
const FLUSH_INTERVAL_MS: u64 = 1000;

/// Start the usage metering background task.
///
/// Pattern: mpsc channel -> batch accumulator -> bulk INSERT.
///
/// This decouples usage recording from the request hot path.
/// The handler sends a UsageEvent to the channel (non-blocking via try_send),
/// and this background task batches events and persists them.
///
/// Graceful shutdown: The task drains remaining events when the
/// sender half is dropped (all request handlers finished).
pub fn start_usage_writer(
    pg_pool: sqlx::PgPool,
    shutdown: tokio_util::sync::CancellationToken,
) -> mpsc::Sender<UsageEvent> {
    let (tx, mut rx) = mpsc::channel::<UsageEvent>(10_000);

    tokio::spawn(async move {
        let mut batch: Vec<UsageEvent> = Vec::with_capacity(BATCH_SIZE);
        let mut flush_interval = tokio::time::interval(
            std::time::Duration::from_millis(FLUSH_INTERVAL_MS)
        );

        loop {
            tokio::select! {
                // Receive events from handlers
                event = rx.recv() => {
                    match event {
                        Some(e) => {
                            batch.push(e);
                            if batch.len() >= BATCH_SIZE {
                                flush_batch(&pg_pool, &mut batch).await;
                            }
                        }
                        None => {
                            // Channel closed (all senders dropped)
                            if !batch.is_empty() {
                                flush_batch(&pg_pool, &mut batch).await;
                            }
                            tracing::info!("Usage writer shutting down, channel closed");
                            return;
                        }
                    }
                }

                // Periodic flush for partial batches
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        flush_batch(&pg_pool, &mut batch).await;
                    }
                }

                // Shutdown signal
                _ = shutdown.cancelled() => {
                    tracing::info!("Usage writer received shutdown signal, draining...");
                    // Drain remaining events from channel
                    while let Ok(event) = rx.try_recv() {
                        batch.push(event);
                    }
                    if !batch.is_empty() {
                        flush_batch(&pg_pool, &mut batch).await;
                    }
                    return;
                }
            }
        }
    });

    tx
}

/// Flush a batch of usage events to PostgreSQL using a single INSERT.
async fn flush_batch(pg_pool: &sqlx::PgPool, batch: &mut Vec<UsageEvent>) {
    if batch.is_empty() {
        return;
    }

    let span = tracing::info_span!("usage.flush_batch", count = batch.len());
    let _guard = span.enter();

    // Build bulk INSERT using unnest for performance
    let request_ids: Vec<&str> = batch.iter().map(|e| e.request_id.as_str()).collect();
    let tenant_ids: Vec<uuid::Uuid> = batch.iter().map(|e| e.tenant_id).collect();
    let sa_ids: Vec<uuid::Uuid> = batch.iter().map(|e| e.service_account_id).collect();
    let providers: Vec<&str> = batch.iter().map(|e| e.provider.as_str()).collect();
    let model_aliases: Vec<&str> = batch.iter().map(|e| e.model_alias.as_str()).collect();
    let provider_models: Vec<&str> = batch.iter().map(|e| e.provider_model_name.as_str()).collect();
    let prompt_tokens: Vec<i32> = batch.iter().map(|e| e.prompt_tokens).collect();
    let completion_tokens: Vec<i32> = batch.iter().map(|e| e.completion_tokens).collect();
    let total_tokens: Vec<i32> = batch.iter().map(|e| e.total_tokens).collect();
    let costs: Vec<rust_decimal::Decimal> = batch.iter().map(|e| e.estimated_cost).collect();
    let currencies: Vec<&str> = batch.iter().map(|e| e.currency.as_str()).collect();
    let latencies: Vec<i32> = batch.iter().map(|e| e.latency_ms).collect();
    let retry_counts: Vec<i32> = batch.iter().map(|e| e.retry_count).collect();
    let statuses: Vec<&str> = batch.iter().map(|e| e.final_status.as_str()).collect();
    let is_streaming: Vec<bool> = batch.iter().map(|e| e.is_streaming).collect();

    let result = sqlx::query(
        r#"
        INSERT INTO usage_events (
            request_id, tenant_id, service_account_id, provider,
            model_alias, provider_model_name, prompt_tokens,
            completion_tokens, total_tokens, estimated_cost, currency,
            latency_ms, retry_count, final_status, is_streaming
        )
        SELECT * FROM UNNEST(
            $1::text[], $2::uuid[], $3::uuid[], $4::text[],
            $5::text[], $6::text[], $7::int[], $8::int[], $9::int[],
            $10::numeric[], $11::text[], $12::int[], $13::int[],
            $14::text[], $15::bool[]
        )
        ON CONFLICT (request_id) DO NOTHING
        "#,
    )
    .bind(&request_ids)
    .bind(&tenant_ids)
    .bind(&sa_ids)
    .bind(&providers)
    .bind(&model_aliases)
    .bind(&provider_models)
    .bind(&prompt_tokens)
    .bind(&completion_tokens)
    .bind(&total_tokens)
    .bind(&costs)
    .bind(&currencies)
    .bind(&latencies)
    .bind(&retry_counts)
    .bind(&statuses)
    .bind(&is_streaming)
    .execute(pg_pool)
    .await;

    match result {
        Ok(rows) => {
            tracing::debug!(
                inserted = rows.rows_affected(),
                batch_size = batch.len(),
                "Usage batch flushed"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, batch_size = batch.len(), "Failed to flush usage batch");
            // TODO: Write to dead-letter queue or file for recovery
        }
    }

    batch.clear();
}
```

### 8.3 Cost Calculation

```rust
// crates/smartgate-usage/src/pricing.rs

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;

/// Pricing table: cost per 1M tokens.
/// These are configurable via admin API in v2; hardcoded defaults for v1.
#[derive(Debug, Clone)]
pub struct PricingTable {
    /// Map of "provider:model" -> (input_cost_per_1m, output_cost_per_1m)
    prices: HashMap<String, (Decimal, Decimal)>,
}

impl PricingTable {
    pub fn default_2026() -> Self {
        let mut prices = HashMap::new();
        // OpenAI pricing (approximate, as of early 2026)
        prices.insert("openai:gpt-4-turbo".into(),      (dec!(10.00),  dec!(30.00)));
        prices.insert("openai:gpt-4o".into(),            (dec!(2.50),   dec!(10.00)));
        prices.insert("openai:gpt-4o-mini".into(),       (dec!(0.15),   dec!(0.60)));
        // Anthropic pricing
        prices.insert("anthropic:claude-sonnet-4".into(), (dec!(3.00),   dec!(15.00)));
        prices.insert("anthropic:claude-haiku-3.5".into(), (dec!(0.80),  dec!(4.00)));
        // Default fallback
        prices.insert("default".into(),                   (dec!(1.00),   dec!(3.00)));

        Self { prices }
    }

    /// Estimate cost for a request.
    ///
    /// Formula:
    ///   cost = (prompt_tokens / 1_000_000 * input_rate)
    ///        + (completion_tokens / 1_000_000 * output_rate)
    pub fn estimate_cost(
        &self,
        provider: &str,
        model: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Decimal {
        let key = format!("{provider}:{model}");
        let (input_rate, output_rate) = self
            .prices
            .get(&key)
            .or_else(|| self.prices.get("default"))
            .copied()
            .unwrap_or((dec!(1.00), dec!(3.00)));

        let million = dec!(1_000_000);
        let input_cost = Decimal::from(prompt_tokens) / million * input_rate;
        let output_cost = Decimal::from(completion_tokens) / million * output_rate;

        input_cost + output_cost
    }
}
```

---

## 9. Storage Layer

### 9.1 Repository Trait Definitions

```rust
// crates/smartgate-storage/src/traits.rs

use async_trait::async_trait;
use smartgate_types::*;
use uuid::Uuid;

#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> Result<Tenant, StorageError>;
    async fn get_by_slug(&self, slug: &str) -> Result<Tenant, StorageError>;
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Tenant>, StorageError>;
    async fn create(&self, input: CreateTenantInput) -> Result<Tenant, StorageError>;
    async fn update_status(&self, id: Uuid, status: TenantStatus) -> Result<(), StorageError>;
}

#[async_trait]
pub trait ServiceAccountRepository: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> Result<ServiceAccount, StorageError>;
    async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ServiceAccount>, StorageError>;
    async fn create(&self, input: CreateServiceAccountInput) -> Result<ServiceAccount, StorageError>;
    async fn update_status(
        &self,
        id: Uuid,
        status: ServiceAccountStatus,
    ) -> Result<(), StorageError>;
}

#[async_trait]
pub trait KeyRepository: Send + Sync {
    async fn get_active_key(&self, key_id: &str) -> Result<ServiceAccountKey, StorageError>;
    async fn list_by_service_account(
        &self,
        service_account_id: Uuid,
    ) -> Result<Vec<ServiceAccountKey>, StorageError>;
    async fn create(&self, input: CreateKeyInput) -> Result<ServiceAccountKey, StorageError>;
    async fn revoke(&self, key_id: &str) -> Result<(), StorageError>;
    async fn touch_last_used(&self, key_id: &str) -> Result<(), StorageError>;
}

#[async_trait]
pub trait PolicyRepository: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> Result<Policy, StorageError>;
    async fn list_by_tenant(
        &self,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Policy>, StorageError>;
    async fn create(&self, input: CreatePolicyInput) -> Result<Policy, StorageError>;
    async fn update(&self, id: Uuid, input: UpdatePolicyInput) -> Result<Policy, StorageError>;
    async fn get_policies_for_service_account(
        &self,
        service_account_id: Uuid,
        default_policy_id: Option<Uuid>,
    ) -> Result<Vec<Policy>, StorageError>;
}

#[async_trait]
pub trait RouteRepository: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> Result<ProviderRoute, StorageError>;
    async fn list_for_alias(
        &self,
        tenant_id: Uuid,
        model_alias: &str,
    ) -> Result<Vec<ProviderRoute>, StorageError>;
    async fn list_by_tenant(
        &self,
        tenant_id: Option<Uuid>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ProviderRoute>, StorageError>;
    async fn create(&self, input: CreateRouteInput) -> Result<ProviderRoute, StorageError>;
    async fn update(&self, id: Uuid, input: UpdateRouteInput) -> Result<ProviderRoute, StorageError>;
}

#[async_trait]
pub trait UsageRepository: Send + Sync {
    async fn query(
        &self,
        filter: UsageQueryFilter,
    ) -> Result<Vec<UsageEvent>, StorageError>;
    async fn aggregate_cost(
        &self,
        service_account_id: Uuid,
        period_start: chrono::DateTime<chrono::Utc>,
    ) -> Result<rust_decimal::Decimal, StorageError>;
}

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn query(
        &self,
        filter: AuditQueryFilter,
    ) -> Result<Vec<AuditEventRecord>, StorageError>;
    async fn create(&self, event: AuditEvent) -> Result<(), StorageError>;
}

/// Storage-level error type.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Entity not found")]
    NotFound,

    #[error("Duplicate entity")]
    Duplicate,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(String),
}
```

### 9.2 Key Query Patterns (with SQL)

```rust
// crates/smartgate-storage/src/postgres/service_accounts.rs

use smartgate_types::{ServiceAccount, ServiceAccountStatus};
use uuid::Uuid;

/// Fetch a service account by ID.
/// Used in: auth middleware (every request).
/// Expected latency: <1ms (usually cached, ~5ms uncached).
pub async fn get_by_id(
    pool: &sqlx::PgPool,
    id: Uuid,
) -> Result<ServiceAccount, sqlx::Error> {
    sqlx::query_as::<_, ServiceAccount>(
        r#"
        SELECT id, tenant_id, name, slug, environment, description,
               status, default_policy_id, created_at, updated_at
        FROM service_accounts
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
}
```

```rust
// crates/smartgate-storage/src/postgres/policies.rs

use smartgate_types::Policy;
use uuid::Uuid;

/// Get all effective policy IDs for a service account.
/// Includes explicitly bound policies + default policy.
/// Used in: auth middleware (every request).
pub async fn get_policy_ids_for_service_account(
    pool: &sqlx::PgPool,
    service_account_id: Uuid,
    default_policy_id: Option<Uuid>,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let mut ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT policy_id
        FROM service_account_policy_bindings
        WHERE service_account_id = $1
        "#,
    )
    .bind(service_account_id)
    .fetch_all(pool)
    .await?;

    // Add default policy if not already bound
    if let Some(default_id) = default_policy_id {
        if !ids.contains(&default_id) {
            ids.push(default_id);
        }
    }

    Ok(ids)
}

/// Fetch a full policy by ID.
pub async fn get_by_id(pool: &sqlx::PgPool, id: Uuid) -> Result<Policy, sqlx::Error> {
    sqlx::query_as::<_, Policy>(
        r#"
        SELECT id, tenant_id, name, description,
               allowed_models_json, denied_models_json,
               max_input_tokens, max_output_tokens,
               allow_streaming, allow_tools, allow_files,
               rpm_limit, concurrency_limit,
               daily_budget, monthly_budget,
               allowed_regions_json,
               created_at, updated_at
        FROM policies
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
}
```

```rust
// crates/smartgate-storage/src/postgres/usage.rs

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Query filter for usage events.
pub struct UsageQueryFilter {
    pub tenant_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub model_alias: Option<String>,
    pub provider: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

/// Query usage events with dynamic filters.
/// Used in: admin API (/admin/usage).
pub async fn query_usage(
    pool: &sqlx::PgPool,
    filter: &UsageQueryFilter,
) -> Result<Vec<smartgate_types::UsageEvent>, sqlx::Error> {
    // Using dynamic query building with sqlx::QueryBuilder
    let mut qb = sqlx::QueryBuilder::new(
        r#"
        SELECT request_id, tenant_id, service_account_id, provider_route_id,
               provider, model_alias, provider_model_name,
               prompt_tokens, completion_tokens, total_tokens,
               estimated_cost, currency, latency_ms, retry_count,
               final_status, is_streaming, created_at
        FROM usage_events
        WHERE 1=1
        "#,
    );

    if let Some(tid) = filter.tenant_id {
        qb.push(" AND tenant_id = ");
        qb.push_bind(tid);
    }
    if let Some(said) = filter.service_account_id {
        qb.push(" AND service_account_id = ");
        qb.push_bind(said);
    }
    if let Some(from) = filter.from {
        qb.push(" AND created_at >= ");
        qb.push_bind(from);
    }
    if let Some(to) = filter.to {
        qb.push(" AND created_at <= ");
        qb.push_bind(to);
    }
    if let Some(model) = &filter.model_alias {
        qb.push(" AND model_alias = ");
        qb.push_bind(model);
    }
    if let Some(provider) = &filter.provider {
        qb.push(" AND provider = ");
        qb.push_bind(provider);
    }

    qb.push(" ORDER BY created_at DESC LIMIT ");
    qb.push_bind(filter.limit);
    qb.push(" OFFSET ");
    qb.push_bind(filter.offset);

    qb.build_query_as().fetch_all(pool).await
}
```

### 9.3 Connection Pool Configuration

```rust
// crates/smartgate-storage/src/postgres/pool.rs

use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use std::time::Duration;
use smartgate_config::DatabaseConfig;

/// Create a PostgreSQL connection pool.
///
/// Pool sizing guidance:
/// - connections = (2 * cpu_cores) + disk_spindles
/// - For SSD: connections ~ 2 * cpu_cores + 1
/// - Default: 20 connections for a 4-core machine
/// - Each connection uses ~10MB RAM on the PG server side
pub async fn create_pg_pool(config: &DatabaseConfig) -> Result<sqlx::PgPool, sqlx::Error> {
    let options = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.username)
        .password(&config.password)
        .database(&config.database)
        .application_name("llmsmartgate");

    PgPoolOptions::new()
        .max_connections(config.max_connections)      // Default: 20
        .min_connections(config.min_connections)       // Default: 5
        .acquire_timeout(Duration::from_secs(5))      // Wait for connection
        .idle_timeout(Duration::from_secs(300))        // Close idle connections
        .max_lifetime(Duration::from_secs(1800))       // Recycle connections
        .connect_with(options)
        .await
}
```

```rust
// crates/smartgate-storage/src/redis/pool.rs

use fred::prelude::*;
use smartgate_config::RedisConfig;

/// Create a Redis connection pool using Fred.
///
/// Fred provides built-in round-robin pooling.
/// Pool size: 4-8 connections for typical workload.
/// Each connection is multiplexed (multiple commands in flight).
pub async fn create_redis_pool(config: &RedisConfig) -> Result<fred::clients::Pool, fred::error::Error> {
    let redis_config = fred::types::config::Config::from_url(&config.url)?;

    let pool = fred::clients::Pool::new(
        redis_config,
        None,     // Use default performance config
        None,     // Use default connection config
        None,     // Use default reconnect policy
        config.pool_size,  // Default: 6
    )?;

    pool.init().await?;

    // Verify connectivity
    let _: String = pool.ping(None).await?;

    Ok(pool)
}
```

### 9.4 Migration Strategy

Migrations are managed via `sqlx-cli` and stored in the `migrations/` directory.

```text
Migration naming convention:
  {sequence}_{description}.sql
  Examples:
    001_create_tenants.sql
    002_create_policies.sql
    ...

Commands:
  sqlx migrate add create_tenants     # Create new migration
  sqlx migrate run                     # Apply pending migrations
  sqlx migrate revert                  # Revert last migration
  sqlx migrate info                    # Show migration status

CI/CD:
  - Migrations run automatically on startup via sqlx::migrate!()
  - All migrations are idempotent (CREATE TABLE IF NOT EXISTS)
  - Down migrations provided for development rollback
  - Production: migrations run in a separate init step before deployment
```

---

## 10. Observability

### 10.1 Span Hierarchy

```text
Span Name                          | Attributes
-----------------------------------|------------------------------------------
gateway.request                    | http.method, http.route, http.status_code,
                                   | gateway.request_id, gateway.tenant_id,
                                   | gateway.service_account_id, gateway.model_alias
  |
  +-- auth.verify                  | auth.service_account_id, auth.key_id
  |     +-- auth.fetch_key         | auth.key_id, auth.cache_hit (bool)
  |     +-- auth.verify_signature  | (no extra attributes)
  |     +-- auth.check_nonce       | (no extra attributes)
  |
  +-- policy.evaluate              | policy.decision (allow/deny)
  |     +-- policy.check_model     | policy.model, policy.result
  |     +-- policy.check_quotas    | policy.token_limit
  |     +-- policy.check_budget    | policy.budget_remaining
  |     +-- policy.check_rate      | policy.rpm_current, policy.rpm_limit
  |
  +-- routing.resolve              | routing.model_alias, routing.routes_found
  |     +-- routing.check_cb       | routing.provider, routing.cb_state
  |
  +-- provider.call                | provider.name, provider.model,
  |     |                          | provider.attempt, provider.status
  |     +-- provider.transform_req | (no extra attributes)
  |     +-- provider.http_call     | http.url, http.status_code,
  |     |                          | http.response_size
  |     +-- provider.transform_res | provider.prompt_tokens,
  |                                | provider.completion_tokens
  |
  +-- streaming.relay              | streaming.chunk_count,
  |                                | streaming.duration_ms,
  |                                | streaming.disconnect (bool)
  |
  +-- usage.persist                | usage.tokens, usage.cost
```

### 10.2 Metric Definitions

```rust
// crates/smartgate-observability/src/metrics.rs

use opentelemetry::{
    metrics::{Counter, Histogram, Meter, UpDownCounter},
    KeyValue,
};

/// All gateway metrics, initialized once at startup.
pub struct GatewayMetrics {
    // Request metrics
    pub requests_total: Counter<u64>,
    pub request_duration: Histogram<f64>,

    // Auth metrics
    pub auth_duration: Histogram<f64>,
    pub auth_failures_total: Counter<u64>,

    // Provider metrics
    pub provider_requests_total: Counter<u64>,
    pub provider_request_duration: Histogram<f64>,
    pub provider_retries_total: Counter<u64>,
    pub provider_fallbacks_total: Counter<u64>,

    // Circuit breaker
    pub circuit_breaker_state_changes: Counter<u64>,

    // Token and cost metrics
    pub tokens_total: Counter<u64>,
    pub estimated_cost_total: Counter<f64>,

    // Rate limiting
    pub rate_limit_rejections: Counter<u64>,

    // Streaming
    pub active_streams: UpDownCounter<i64>,

    // Infrastructure
    pub pg_pool_active: UpDownCounter<i64>,
    pub redis_pool_active: UpDownCounter<i64>,
}

impl GatewayMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            requests_total: meter
                .u64_counter("gateway.requests.total")
                .with_description("Total number of gateway requests")
                .build(),

            request_duration: meter
                .f64_histogram("gateway.request.duration")
                .with_description("Request duration in seconds")
                .with_unit("s")
                .build(),

            auth_duration: meter
                .f64_histogram("gateway.auth.duration")
                .with_description("Auth verification duration in seconds")
                .with_unit("s")
                .build(),

            auth_failures_total: meter
                .u64_counter("gateway.auth.failures.total")
                .with_description("Total auth failures")
                .build(),

            provider_requests_total: meter
                .u64_counter("gateway.provider.requests.total")
                .with_description("Total provider requests")
                .build(),

            provider_request_duration: meter
                .f64_histogram("gateway.provider.request.duration")
                .with_description("Provider request duration in seconds")
                .with_unit("s")
                .build(),

            provider_retries_total: meter
                .u64_counter("gateway.provider.retries.total")
                .with_description("Total provider retries")
                .build(),

            provider_fallbacks_total: meter
                .u64_counter("gateway.provider.fallbacks.total")
                .with_description("Total provider fallbacks")
                .build(),

            circuit_breaker_state_changes: meter
                .u64_counter("gateway.circuit_breaker.state_changes")
                .with_description("Circuit breaker state transitions")
                .build(),

            tokens_total: meter
                .u64_counter("gateway.tokens.total")
                .with_description("Total tokens processed")
                .build(),

            estimated_cost_total: meter
                .f64_counter("gateway.cost.total")
                .with_description("Total estimated cost in USD")
                .with_unit("usd")
                .build(),

            rate_limit_rejections: meter
                .u64_counter("gateway.rate_limit.rejections")
                .with_description("Total rate limit rejections")
                .build(),

            active_streams: meter
                .i64_up_down_counter("gateway.streams.active")
                .with_description("Currently active SSE streams")
                .build(),

            pg_pool_active: meter
                .i64_up_down_counter("gateway.pg.pool.active")
                .with_description("Active PostgreSQL connections")
                .build(),

            redis_pool_active: meter
                .i64_up_down_counter("gateway.redis.pool.active")
                .with_description("Active Redis connections")
                .build(),
        }
    }

    /// Record a completed request.
    pub fn record_request(
        &self,
        method: &str,
        route: &str,
        status: u16,
        duration_secs: f64,
    ) {
        let attrs = vec![
            KeyValue::new("http.method", method.to_string()),
            KeyValue::new("http.route", route.to_string()),
            KeyValue::new("http.status_code", status as i64),
        ];
        self.requests_total.add(1, &attrs);
        self.request_duration.record(duration_secs, &attrs);
    }
}
```

### 10.3 Tracing Setup

```rust
// crates/smartgate-observability/src/tracing_setup.rs

use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Initialize the complete observability stack:
/// - tracing-subscriber for structured logging
/// - tracing-opentelemetry for distributed tracing
/// - OpenTelemetry OTLP exporter for trace export
///
/// Environment variables:
/// - RUST_LOG: log level filter (default: "info,smartgate=debug")
/// - OTEL_EXPORTER_OTLP_ENDPOINT: OTLP collector endpoint
/// - OTEL_SERVICE_NAME: service name (default: "llmsmartgate")
pub fn init_observability(
    otlp_endpoint: &str,
    service_name: &str,
    log_format: LogFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Build OpenTelemetry tracer
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .build()?;

    let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(Resource::builder()
            .with_service_name(service_name)
            .build())
        .with_batch_exporter(exporter)
        .build();

    let tracer = tracer_provider.tracer("llmsmartgate");

    // 2. Build tracing-opentelemetry layer
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // 3. Build log format layer
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,smartgate=debug"));

    match log_format {
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().json())
                .with(otel_layer)
                .init();
        }
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .with(otel_layer)
                .init();
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Pretty,
}

/// Shutdown the observability stack gracefully.
/// Flushes pending spans and metrics.
pub fn shutdown_observability() {
    opentelemetry::global::shutdown_tracer_provider();
}
```

### 10.4 Log Format Specification

**JSON format (production)**:
```json
{
  "timestamp": "2026-04-01T12:00:00.123Z",
  "level": "INFO",
  "target": "smartgate_api::data_plane::chat_completions",
  "message": "Request completed",
  "span": {
    "name": "gateway.request",
    "request_id": "01JQX...",
    "trace_id": "abc123...",
    "span_id": "def456..."
  },
  "fields": {
    "http.method": "POST",
    "http.route": "/v1/chat/completions",
    "http.status_code": 200,
    "gateway.model_alias": "gpt-4",
    "gateway.provider": "openai",
    "gateway.latency_ms": 1234
  }
}
```

---

## 11. Configuration

### 11.1 Configuration Struct

```rust
// crates/smartgate-config/src/settings.rs

use serde::Deserialize;
use std::time::Duration;

/// Root configuration for LLMSmartGate.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub auth: AuthConfig,
    pub providers: ProvidersConfig,
    pub rate_limit: RateLimitConfig,
    pub usage: UsageConfig,
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Bind address (default: "0.0.0.0")
    #[serde(default = "default_host")]
    pub host: String,

    /// Listen port (default: 8080)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Enable TLS (default: false in dev, true in prod)
    #[serde(default)]
    pub tls_enabled: bool,

    /// Path to TLS certificate file
    pub tls_cert_path: Option<String>,

    /// Path to TLS private key file
    pub tls_key_path: Option<String>,

    /// Maximum request body size in bytes (default: 10MB)
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Global request timeout in seconds (default: 120)
    #[serde(default = "default_global_timeout")]
    pub global_timeout_secs: u64,

    /// Graceful shutdown timeout in seconds (default: 30)
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL host
    pub host: String,

    /// PostgreSQL port (default: 5432)
    #[serde(default = "default_pg_port")]
    pub port: u16,

    /// Database name
    pub database: String,

    /// Database username
    pub username: String,

    /// Database password (prefer env var LLMSG_DATABASE_PASSWORD)
    #[serde(default)]
    pub password: String,

    /// Maximum pool connections (default: 20)
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum pool connections (default: 5)
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Run migrations on startup (default: true)
    #[serde(default = "default_true")]
    pub run_migrations: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    /// Redis URL (redis://host:port or redis+sentinel://...)
    pub url: String,

    /// Connection pool size (default: 6)
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    /// Maximum timestamp skew in seconds (default: 300)
    #[serde(default = "default_timestamp_window")]
    pub timestamp_window_secs: u64,

    /// Nonce TTL in seconds (default: 300)
    #[serde(default = "default_nonce_ttl")]
    pub nonce_ttl_secs: u64,

    /// Key cache TTL in seconds for L1 (default: 60)
    #[serde(default = "default_key_cache_ttl")]
    pub key_cache_ttl_secs: u64,

    /// Key cache max entries (default: 10_000)
    #[serde(default = "default_key_cache_max")]
    pub key_cache_max_entries: u64,

    /// Admin JWT secret (prefer env var LLMSG_ADMIN_JWT_SECRET)
    #[serde(default)]
    pub admin_jwt_secret: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvidersConfig {
    /// Default timeout per provider call in ms (default: 30000)
    #[serde(default = "default_provider_timeout")]
    pub default_timeout_ms: u64,

    /// HTTP client pool: max idle connections per host (default: 32)
    #[serde(default = "default_pool_max_idle")]
    pub pool_max_idle_per_host: usize,

    /// HTTP client pool: idle timeout in seconds (default: 30)
    #[serde(default = "default_pool_idle_timeout")]
    pub pool_idle_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Default RPM limit when no policy specifies one (default: 1000)
    #[serde(default = "default_rpm_limit")]
    pub default_rpm: u32,

    /// IP-level rate limit (requests per minute, default: 600)
    #[serde(default = "default_ip_rpm")]
    pub ip_rpm: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsageConfig {
    /// Batch size for bulk insert (default: 100)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval in ms (default: 1000)
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,

    /// Channel buffer size (default: 10_000)
    #[serde(default = "default_channel_buffer")]
    pub channel_buffer_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    /// OTLP endpoint for traces and metrics
    #[serde(default = "default_otlp_endpoint")]
    pub otlp_endpoint: String,

    /// Service name for OTEL (default: "llmsmartgate")
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Log format: "json" or "pretty" (default: "json")
    #[serde(default = "default_log_format")]
    pub log_format: String,

    /// Enable Prometheus metrics endpoint (default: true)
    #[serde(default = "default_true")]
    pub prometheus_enabled: bool,
}

// Default value functions
fn default_host() -> String { "0.0.0.0".into() }
fn default_port() -> u16 { 8080 }
fn default_max_body_size() -> usize { 10 * 1024 * 1024 }
fn default_global_timeout() -> u64 { 120 }
fn default_shutdown_timeout() -> u64 { 30 }
fn default_pg_port() -> u16 { 5432 }
fn default_max_connections() -> u32 { 20 }
fn default_min_connections() -> u32 { 5 }
fn default_true() -> bool { true }
fn default_redis_pool_size() -> usize { 6 }
fn default_timestamp_window() -> u64 { 300 }
fn default_nonce_ttl() -> u64 { 300 }
fn default_key_cache_ttl() -> u64 { 60 }
fn default_key_cache_max() -> u64 { 10_000 }
fn default_provider_timeout() -> u64 { 30_000 }
fn default_pool_max_idle() -> usize { 32 }
fn default_pool_idle_timeout() -> u64 { 30 }
fn default_rpm_limit() -> u32 { 1000 }
fn default_ip_rpm() -> u32 { 600 }
fn default_batch_size() -> usize { 100 }
fn default_flush_interval() -> u64 { 1000 }
fn default_channel_buffer() -> usize { 10_000 }
fn default_otlp_endpoint() -> String { "http://localhost:4317".into() }
fn default_service_name() -> String { "llmsmartgate".into() }
fn default_log_format() -> String { "json".into() }
```

### 11.2 Environment Variable Mapping

```text
Environment Variable                 | Config Path                        | Type
-------------------------------------|------------------------------------|---------
LLMSG_SERVER_HOST                    | server.host                        | String
LLMSG_SERVER_PORT                    | server.port                        | u16
LLMSG_SERVER_TLS_ENABLED            | server.tls_enabled                 | bool
LLMSG_SERVER_TLS_CERT_PATH          | server.tls_cert_path               | String
LLMSG_SERVER_TLS_KEY_PATH           | server.tls_key_path                | String
LLMSG_DATABASE_HOST                  | database.host                      | String
LLMSG_DATABASE_PORT                  | database.port                      | u16
LLMSG_DATABASE_NAME                  | database.database                  | String
LLMSG_DATABASE_USERNAME              | database.username                  | String
LLMSG_DATABASE_PASSWORD              | database.password                  | String (secret)
LLMSG_DATABASE_MAX_CONNECTIONS       | database.max_connections           | u32
LLMSG_REDIS_URL                      | redis.url                          | String
LLMSG_REDIS_POOL_SIZE               | redis.pool_size                    | usize
LLMSG_AUTH_ADMIN_JWT_SECRET          | auth.admin_jwt_secret              | String (secret)
LLMSG_OTEL_ENDPOINT                  | observability.otlp_endpoint        | String
LLMSG_LOG_FORMAT                     | observability.log_format           | String
OPENAI_API_KEY                       | (direct env var)                   | String (secret)
ANTHROPIC_API_KEY                    | (direct env var)                   | String (secret)
GEMINI_API_KEY                       | (direct env var)                   | String (secret)
AZURE_OPENAI_API_KEY                 | (direct env var)                   | String (secret)
```

### 11.3 Config Loading and Validation

```rust
// crates/smartgate-config/src/lib.rs

use config::{Config, Environment, File};
use crate::settings::Settings;

/// Load configuration from files and environment variables.
///
/// Loading order (later overrides earlier):
/// 1. config/default.toml
/// 2. config/{RUN_MODE}.toml (e.g., development, production)
/// 3. Environment variables with LLMSG_ prefix
pub fn load_config() -> Result<Settings, config::ConfigError> {
    let run_mode = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

    let settings = Config::builder()
        .add_source(File::with_name("config/default").required(false))
        .add_source(File::with_name(&format!("config/{run_mode}")).required(false))
        .add_source(
            Environment::with_prefix("LLMSG")
                .separator("_")
                .try_parsing(true),
        )
        .build()?;

    let settings: Settings = settings.try_deserialize()?;
    validate_config(&settings)?;
    Ok(settings)
}

/// Validate configuration at startup.
fn validate_config(settings: &Settings) -> Result<(), config::ConfigError> {
    // TLS validation
    if settings.server.tls_enabled {
        if settings.server.tls_cert_path.is_none() || settings.server.tls_key_path.is_none() {
            return Err(config::ConfigError::Message(
                "TLS enabled but tls_cert_path or tls_key_path not set".into(),
            ));
        }
    }

    // Database validation
    if settings.database.host.is_empty() {
        return Err(config::ConfigError::Message(
            "database.host is required".into(),
        ));
    }

    // Redis validation
    if settings.redis.url.is_empty() {
        return Err(config::ConfigError::Message(
            "redis.url is required".into(),
        ));
    }

    // Pool size sanity
    if settings.database.max_connections < settings.database.min_connections {
        return Err(config::ConfigError::Message(
            "database.max_connections must be >= min_connections".into(),
        ));
    }

    Ok(())
}
```

---

## 12. Testing Strategy

### 12.1 Unit Test Patterns

```rust
// Example: unit test for canonical string construction
// crates/smartgate-auth/src/canonical.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_string_deterministic() {
        let a = build_canonical_string("POST", "/v1/chat/completions", "2026-04-01T00:00:00Z", "nonce1", "abc123");
        let b = build_canonical_string("POST", "/v1/chat/completions", "2026-04-01T00:00:00Z", "nonce1", "abc123");
        assert_eq!(a, b, "Same inputs must produce same canonical string");
    }

    #[test]
    fn test_canonical_string_changes_with_method() {
        let post = build_canonical_string("POST", "/v1/chat/completions", "ts", "n", "h");
        let get = build_canonical_string("GET", "/v1/chat/completions", "ts", "n", "h");
        assert_ne!(post, get);
    }

    #[test]
    fn test_body_sha256_known_value() {
        let hash = compute_body_sha256(b"hello");
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }
}
```

```rust
// Example: unit test for policy merge
// crates/smartgate-policy/src/merge.rs

#[cfg(test)]
mod tests {
    use super::*;
    use smartgate_types::Policy;

    fn make_policy(allowed: Vec<&str>, max_input: Option<i32>, allow_streaming: bool) -> Policy {
        Policy {
            id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            name: "test".into(),
            description: None,
            allowed_models_json: serde_json::to_value(allowed).unwrap(),
            denied_models_json: serde_json::json!([]),
            max_input_tokens: max_input,
            max_output_tokens: None,
            allow_streaming,
            allow_tools: false,
            allow_files: false,
            rpm_limit: None,
            concurrency_limit: None,
            daily_budget: None,
            monthly_budget: None,
            allowed_regions_json: serde_json::json!([]),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_merge_takes_minimum_token_limit() {
        let p1 = make_policy(vec![], Some(1000), true);
        let p2 = make_policy(vec![], Some(500), true);

        let merged = merge_policies(&[p1, p2]);
        assert_eq!(merged.max_input_tokens, Some(500));
    }

    #[test]
    fn test_merge_streaming_requires_all_allow() {
        let p1 = make_policy(vec![], None, true);
        let p2 = make_policy(vec![], None, false);

        let merged = merge_policies(&[p1, p2]);
        assert!(!merged.allow_streaming);
    }

    #[test]
    fn test_merge_models_intersection() {
        let p1 = make_policy(vec!["gpt-4", "gpt-3.5"], None, true);
        let p2 = make_policy(vec!["gpt-4", "claude-3"], None, true);

        let merged = merge_policies(&[p1, p2]);
        assert_eq!(merged.allowed_models, vec!["gpt-4"]);
    }
}
```

### 12.2 Integration Test Setup

```rust
// tests/common/mod.rs

use sqlx::PgPool;
use smartgate_api::AppState;
use std::sync::Arc;

/// Create a test server with real PostgreSQL and Redis connections
/// using testcontainers.
pub struct TestServer {
    pub addr: std::net::SocketAddr,
    pub pg_pool: PgPool,
    pub redis_pool: fred::clients::Pool,
    pub client: reqwest::Client,
}

impl TestServer {
    pub async fn new() -> Self {
        // Start containers
        let pg_container = testcontainers::runners::AsyncRunner::start(
            testcontainers_modules::postgres::Postgres::default()
        ).await.expect("Failed to start PostgreSQL container");

        let redis_container = testcontainers::runners::AsyncRunner::start(
            testcontainers_modules::redis::Redis::default()
        ).await.expect("Failed to start Redis container");

        let pg_url = format!(
            "postgres://postgres:postgres@localhost:{}/postgres",
            pg_container.get_host_port_ipv4(5432).await.unwrap()
        );
        let redis_url = format!(
            "redis://localhost:{}",
            redis_container.get_host_port_ipv4(6379).await.unwrap()
        );

        // Create pools
        let pg_pool = sqlx::PgPool::connect(&pg_url).await.unwrap();
        sqlx::migrate!("../../migrations").run(&pg_pool).await.unwrap();

        let redis_config = fred::types::config::Config::from_url(&redis_url).unwrap();
        let redis_pool = fred::clients::Pool::new(redis_config, None, None, None, 2).unwrap();
        redis_pool.init().await.unwrap();

        // Build app state and router
        let (usage_tx, _) = tokio::sync::mpsc::channel(1000);
        let (audit_tx, _) = tokio::sync::mpsc::channel(1000);

        let state = AppState {
            config: Arc::new(smartgate_config::load_config().unwrap()),
            pg_pool: pg_pool.clone(),
            redis_pool: redis_pool.clone(),
            http_client: reqwest::Client::new(),
            key_cache: moka::future::Cache::builder()
                .max_capacity(100)
                .build(),
            policy_cache: moka::future::Cache::builder()
                .max_capacity(100)
                .build(),
            route_cache: moka::future::Cache::builder()
                .max_capacity(100)
                .build(),
            usage_sender: usage_tx,
            audit_sender: audit_tx,
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        };

        let app = smartgate_api::build_router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            addr,
            pg_pool,
            redis_pool,
            client: reqwest::Client::new(),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}
```

### 12.3 Mock Provider for Testing

```rust
// tests/common/mock_provider.rs

use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};
use serde_json::json;

/// Start a mock OpenAI-compatible server for integration tests.
pub async fn start_mock_openai() -> MockServer {
    let server = MockServer::start().await;

    // Mock: POST /v1/chat/completions (non-streaming)
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-test123",
            "object": "chat.completion",
            "created": 1711929600,
            "model": "gpt-4-turbo",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 8,
                "total_tokens": 18
            }
        })))
        .mount(&server)
        .await;

    // Mock: POST /v1/chat/completions (streaming)
    let sse_body = [
        "data: {\"id\":\"chatcmpl-test123\",\"object\":\"chat.completion.chunk\",\"created\":1711929600,\"model\":\"gpt-4-turbo\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-test123\",\"object\":\"chat.completion.chunk\",\"created\":1711929600,\"model\":\"gpt-4-turbo\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"chatcmpl-test123\",\"object\":\"chat.completion.chunk\",\"created\":1711929600,\"model\":\"gpt-4-turbo\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    ].join("");

    // This mock is installed separately for streaming tests
    // using a custom header to differentiate

    server
}

/// Start a mock Anthropic server.
pub async fn start_mock_anthropic() -> MockServer {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-anthropic-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_test123",
            "type": "message",
            "role": "assistant",
            "model": "claude-sonnet-4-20250514",
            "content": [{
                "type": "text",
                "text": "Hello! How can I help you?"
            }],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 7
            }
        })))
        .mount(&server)
        .await;

    server
}
```

### 12.4 Example Integration Test

```rust
// tests/chat_completions_test.rs

mod common;

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};
use common::TestServer;

#[tokio::test]
async fn test_chat_completion_success() {
    let server = TestServer::new().await;

    // 1. Create tenant
    let tenant = server.client
        .post(server.url("/admin/tenants"))
        .json(&serde_json::json!({
            "name": "Test Tenant",
            "slug": "test-tenant"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(tenant.status(), 201);
    let tenant: serde_json::Value = tenant.json().await.unwrap();
    let tenant_id = tenant["id"].as_str().unwrap();

    // 2. Create service account
    // 3. Register Ed25519 key
    // 4. Create policy and bind
    // 5. Create route pointing to mock provider
    // 6. Sign and send chat completion request
    // 7. Assert response

    // (Full implementation follows the auth signing flow from Section 3)
}

#[tokio::test]
async fn test_nonce_replay_rejected() {
    let server = TestServer::new().await;

    // Setup: create tenant, SA, key, policy, route...

    // Send first request with nonce "test-nonce-1" -> should succeed
    // Send second request with same nonce "test-nonce-1" -> should get 401

    // Assert second response has error code "auth.nonce_replay"
}

#[tokio::test]
async fn test_rate_limit_enforced() {
    let server = TestServer::new().await;

    // Setup: create policy with rpm_limit = 2
    // Send 3 requests rapidly
    // Assert third request returns 429
}
```

### 12.5 Benchmark Structure

```rust
// benches/auth_bench.rs

use criterion::{criterion_group, criterion_main, Criterion, black_box};
use ed25519_dalek::{SigningKey, Signer, VerifyingKey, Verifier};
use rand::rngs::OsRng;
use smartgate_auth::canonical;

fn bench_ed25519_sign_verify(c: &mut Criterion) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let message = canonical::build_canonical_string(
        "POST",
        "/v1/chat/completions",
        "2026-04-01T12:00:00Z",
        "test-nonce-12345678",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );

    c.bench_function("ed25519_sign", |b| {
        b.iter(|| {
            let _sig = signing_key.sign(black_box(&message));
        });
    });

    let signature = signing_key.sign(&message);

    c.bench_function("ed25519_verify", |b| {
        b.iter(|| {
            let _ = verifying_key.verify(black_box(&message), &signature);
        });
    });
}

fn bench_sha256(c: &mut Criterion) {
    let body = r#"{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}]}"#;

    c.bench_function("sha256_request_body", |b| {
        b.iter(|| {
            canonical::compute_body_sha256(black_box(body.as_bytes()));
        });
    });
}

criterion_group!(benches, bench_ed25519_sign_verify, bench_sha256);
criterion_main!(benches);
```

---

## Graceful Shutdown

```rust
// crates/smartgate-server/src/shutdown.rs

use tokio_util::sync::CancellationToken;
use std::time::Duration;

/// Orchestrate graceful shutdown of all components.
///
/// Sequence:
/// 1. Receive shutdown signal (SIGTERM or Ctrl+C)
/// 2. Stop accepting new connections (axum's with_graceful_shutdown)
/// 3. Signal all background tasks via CancellationToken
/// 4. Wait for in-flight requests to complete (up to timeout)
/// 5. Flush usage writer (drain remaining events)
/// 6. Flush OpenTelemetry spans
/// 7. Close database and Redis connections
/// 8. Exit
pub async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown");
        }
    }

    // Signal all background tasks
    token.cancel();
}
```

```rust
// crates/smartgate-server/src/main.rs

use smartgate_api::{build_router, AppState};
use smartgate_config::load_config;
use smartgate_observability::tracing_setup;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load configuration
    let config = load_config()?;

    // 2. Initialize observability
    let log_format = if config.observability.log_format == "json" {
        tracing_setup::LogFormat::Json
    } else {
        tracing_setup::LogFormat::Pretty
    };
    tracing_setup::init_observability(
        &config.observability.otlp_endpoint,
        &config.observability.service_name,
        log_format,
    )?;

    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        "Starting LLMSmartGate"
    );

    // 3. Create connection pools
    let pg_pool = smartgate_storage::postgres::pool::create_pg_pool(&config.database).await?;
    let redis_pool = smartgate_storage::redis::pool::create_redis_pool(&config.redis).await?;

    // 4. Run migrations if enabled
    if config.database.run_migrations {
        tracing::info!("Running database migrations");
        sqlx::migrate!("./migrations").run(&pg_pool).await?;
    }

    // 5. Create HTTP client for provider calls
    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(config.providers.pool_max_idle_per_host)
        .pool_idle_timeout(std::time::Duration::from_secs(
            config.providers.pool_idle_timeout_secs,
        ))
        .timeout(std::time::Duration::from_millis(
            config.providers.default_timeout_ms,
        ))
        .build()?;

    // 6. Create shutdown token
    let shutdown_token = tokio_util::sync::CancellationToken::new();

    // 7. Start background tasks
    let usage_sender = smartgate_usage::meter::start_usage_writer(
        pg_pool.clone(),
        shutdown_token.clone(),
    );

    let (audit_sender, mut audit_rx) = tokio::sync::mpsc::channel(1000);
    {
        let pg = pg_pool.clone();
        let token = shutdown_token.clone();
        tokio::spawn(async move {
            // Audit writer (similar pattern to usage writer)
            loop {
                tokio::select! {
                    event = audit_rx.recv() => {
                        if let Some(event) = event {
                            let _ = smartgate_storage::postgres::audit::insert(&pg, event).await;
                        } else {
                            break;
                        }
                    }
                    _ = token.cancelled() => break,
                }
            }
        });
    }

    // 8. Build application state
    let state = AppState {
        config: Arc::new(config.clone()),
        pg_pool,
        redis_pool,
        http_client,
        key_cache: moka::future::Cache::builder()
            .max_capacity(config.auth.key_cache_max_entries)
            .time_to_live(std::time::Duration::from_secs(config.auth.key_cache_ttl_secs))
            .build(),
        policy_cache: moka::future::Cache::builder()
            .max_capacity(10_000)
            .time_to_live(std::time::Duration::from_secs(60))
            .build(),
        route_cache: moka::future::Cache::builder()
            .max_capacity(1_000)
            .time_to_live(std::time::Duration::from_secs(30))
            .build(),
        usage_sender,
        audit_sender,
        shutdown_token: shutdown_token.clone(),
    };

    // 9. Build router
    let app = build_router(state);

    // 10. Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr = %addr, "Server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(smartgate_server::shutdown::shutdown_signal(
            shutdown_token.clone(),
        ))
        .await?;

    // 11. Cleanup
    tracing::info!("Server stopped, cleaning up...");
    smartgate_observability::tracing_setup::shutdown_observability();
    tracing::info!("Shutdown complete");

    Ok(())
}
```

---

## References

### Crate Versions (as of April 2026)

| Crate | Version | Source |
|-------|---------|--------|
| axum | 0.8.5+ | [docs.rs/axum](https://docs.rs/axum/latest/axum/) |
| tokio | 1.44+ | [docs.rs/tokio](https://docs.rs/tokio) |
| tower | 0.5.x | [docs.rs/tower](https://docs.rs/tower) |
| tower-http | 0.6.x | [docs.rs/tower-http](https://docs.rs/tower-http) |
| reqwest | 0.12.x | [docs.rs/reqwest](https://docs.rs/reqwest) |
| sqlx | 0.8.6 | [docs.rs/sqlx](https://docs.rs/crate/sqlx/latest) |
| fred | 10.1.0 | [docs.rs/fred](https://docs.rs/fred) |
| ed25519-dalek | 2.1.x | [docs.rs/ed25519-dalek](https://docs.rs/ed25519-dalek/) |
| serde | 1.x | [docs.rs/serde](https://docs.rs/serde) |
| tracing | 0.1.x | [docs.rs/tracing](https://docs.rs/tracing) |
| tracing-opentelemetry | 0.28+ | [docs.rs/tracing-opentelemetry](https://docs.rs/tracing-opentelemetry) |
| opentelemetry | 0.28+ | [docs.rs/opentelemetry](https://docs.rs/opentelemetry) |
| moka | 0.12.x | [docs.rs/moka](https://docs.rs/moka) |
| thiserror | 1.6.x | [docs.rs/thiserror](https://docs.rs/thiserror) |

### Research Sources

- [Axum 0.8 Announcement -- Tokio Blog (January 2025)](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0)
- [Axum Middleware Documentation](https://docs.rs/axum/latest/axum/middleware/index.html)
- [Tower Service Trait](https://docs.rs/tower/latest/tower/trait.Service.html)
- [Tokio Graceful Shutdown Guide](https://tokio.rs/tokio/topics/shutdown)
- [Rust ORMs in 2026 Comparison](https://aarambhdevhub.medium.com/rust-orms-in-2026-diesel-vs-sqlx-vs-seaorm-vs-rusqlite-which-one-should-you-actually-use-706d0fe912f3)
- [SQLx Compile-Time Query Checking](https://docs.rs/sqlx/latest/sqlx/macro.query.html)
- [Fred Redis Client Documentation](https://docs.rs/fred)
- [Ed25519-Dalek Crate](https://docs.rs/ed25519-dalek/)
- [OpenTelemetry Rust SDK](https://opentelemetry.io/docs/languages/rust/)
- [Tracing-OpenTelemetry Integration](https://docs.rs/tracing-opentelemetry)
- [Error Handling with thiserror and anyhow (2026)](https://oneuptime.com/blog/post/2026-01-25-error-types-thiserror-anyhow-rust/view)
- [Rust Observability with OpenTelemetry and Tokio](https://dasroot.net/posts/2026/01/rust-observability-opentelemetry-tokio/)
- [Tower-CircuitBreaker](https://docs.rs/tower-circuitbreaker/latest/tower_circuitbreaker/)
- [High-Throughput HTTP Proxy in Rust (2026)](https://oneuptime.com/blog/post/2026-01-25-high-throughput-http-proxy-rust/view)
- [Axum SSE Backpressure Discussion](https://users.rust-lang.org/t/axum-sse-and-backpressure/133061)
- [Zero-Copy Data Parsing in Rust Web Services](https://leapcell.io/blog/achieving-zero-copy-data-parsing-in-rust-web-services-for-enhanced-performance)
- [Rust Async Practical Patterns (2026)](https://dasroot.net/posts/2026/02/rust-async-practical-patterns-high-performance-tools/)
