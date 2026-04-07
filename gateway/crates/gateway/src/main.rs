//! LLMSmartGate -- Self-hosted LLM API Gateway
//!
//! Provides unified, secure, policy-governed access to multiple LLM providers.

use std::net::SocketAddr;
use std::sync::Arc;

use llmsmartgate::config::GatewayConfig;
use llmsmartgate::server::{AppState, build_router};

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
        provider_timeout_ms = config.provider.timeout_ms,
        log_level = %config.observability.log_level,
        "Configuration loaded"
    );

    let addr = SocketAddr::from((config.server.host, config.server.port));
    let state = AppState {
        config: Arc::new(config),
        db: None,
        redis: None,
        key_store: None,
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
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");

        tokio::select! {
            _ = ctrl_c => { tracing::info!("Received SIGINT, shutting down"); }
            _ = sigterm.recv() => { tracing::info!("Received SIGTERM, shutting down"); }
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("failed to listen for ctrl-c");
        tracing::info!("Received SIGINT, shutting down");
    }
}
