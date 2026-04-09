//! LLMSmartGate -- Self-hosted LLM API Gateway
//!
//! Provides unified, secure, policy-governed access to multiple LLM providers.

use std::net::SocketAddr;
use std::sync::Arc;

use llmsmartgate::auth::key_store::KeyStore;
use llmsmartgate::config::GatewayConfig;
use llmsmartgate::policy::concurrency::ConcurrencyLimiter;
use llmsmartgate::policy::engine::PolicyCache;
use llmsmartgate::policy::rate_limit::RateLimitEvaluator;
use llmsmartgate::providers::ProviderRegistry;
use llmsmartgate::providers::openai::OpenAIAdapter;
use llmsmartgate::providers::vllm::VllmAdapter;
use llmsmartgate::routing::router::RouteCache;
use llmsmartgate::server::{AppState, build_router};
use llmsmartgate::storage::postgres::create_pg_pool;
use llmsmartgate::storage::redis::{RedisClient, create_redis_pool};
use llmsmartgate::types::Provider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    tracing::info!("LLMSmartGate starting");

    let config = GatewayConfig::load()?;
    tracing::info!(
        host = %config.server.host,
        port = config.server.port,
        pg_pool_size = config.database.pool_size,
        valkey_pool_size = config.redis.pool_size,
        timestamp_skew_secs = config.auth.timestamp_skew_secs,
        admin_key_cache_ttl_secs = config.auth.admin_key_cache_ttl_secs,
        policy_cache_ttl_secs = config.policy.cache_ttl_secs,
        routing_cache_ttl_secs = config.routing.cache_ttl_secs,
        provider_timeout_ms = config.provider.timeout_ms,
        log_level = %config.observability.log_level,
        "Configuration loaded"
    );

    let addr = SocketAddr::from((config.server.host, config.server.port));

    // Initialize connection pools
    let db = create_pg_pool(&config.database).await?;
    let redis_pool = create_redis_pool(&config.redis)?;

    // Key store: moka L1 → Redis L2 → PostgreSQL L3
    let redis_client = RedisClient::new(redis_pool.clone());
    let key_store = KeyStore::new(db.clone(), Some(redis_client));

    // Admin key cache: configurable TTL, single entry
    let admin_key_cache = moka::future::Cache::builder()
        .time_to_live(std::time::Duration::from_secs(
            config.auth.admin_key_cache_ttl_secs,
        ))
        .max_capacity(1)
        .build();

    let policy_cache = PolicyCache::new(config.policy.cache_ttl_secs);
    let route_cache = RouteCache::new(config.routing.cache_ttl_secs);
    let rate_limit_evaluator = RateLimitEvaluator::new(&config.rate_limit);
    let concurrency_limiter = ConcurrencyLimiter::new(redis_pool.clone());

    // Install the ring crypto provider for rustls (reqwest uses rustls-no-provider).
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    // Per-route timeouts are set on each request by the provider adapter.
    // No client-level timeout — it would shadow the per-route value.
    let http_client = reqwest::Client::builder()
        .use_rustls_tls()
        .pool_max_idle_per_host(20)
        .build()
        .expect("failed to build HTTP client");

    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Provider::Openai, Box::new(OpenAIAdapter));
    provider_registry.register(Provider::Vllm, Box::new(VllmAdapter));

    let state = AppState {
        config: Arc::new(config),
        db: Some(db),
        redis: Some(redis_pool),
        key_store: Some(key_store),
        admin_key_cache: Some(admin_key_cache),
        policy_cache: Some(policy_cache),
        rate_limit_evaluator: Some(rate_limit_evaluator),
        concurrency_limiter: Some(concurrency_limiter),
        route_cache: Some(route_cache),
        http_client: Some(http_client),
        provider_registry: Some(Arc::new(provider_registry)),
    };
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "Server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Wait for SIGINT (ctrl-c) or SIGTERM to initiate graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = ctrl_c => { tracing::info!("Received SIGINT, shutting down"); }
                    _ = sigterm.recv() => { tracing::info!("Received SIGTERM, shutting down"); }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to install SIGTERM handler, falling back to SIGINT only");
                if let Err(e) = ctrl_c.await {
                    tracing::error!(error = %e, "failed to listen for ctrl-c");
                }
                tracing::info!("Received SIGINT, shutting down");
            }
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = ctrl_c.await {
            tracing::error!(error = %e, "failed to listen for ctrl-c");
        }
        tracing::info!("Received SIGINT, shutting down");
    }
}
