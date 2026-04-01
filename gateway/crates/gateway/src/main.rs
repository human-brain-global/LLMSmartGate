//! LLMSmartGate -- Self-hosted LLM API Gateway
//!
//! Provides unified, secure, policy-governed access to multiple LLM providers.

mod api;
mod audit;
mod auth;
mod config;
mod observability;
mod policy;
mod providers;
mod routing;
mod storage;
mod streaming;
mod usage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    tracing::info!("LLMSmartGate starting");

    // TODO: Initialize configuration, database, cache, and start server

    Ok(())
}
