//! LLMSmartGate -- Self-hosted LLM API Gateway library.
//!
//! Provides unified, secure, policy-governed access to multiple LLM providers.

pub mod api;
pub mod audit;
pub mod auth;
pub mod config;
pub mod error;
pub mod models;
pub mod observability;
pub mod policy;
pub mod providers;
pub mod routing;
pub mod server;
pub mod storage;
pub mod streaming;
pub mod types;
pub mod usage;
