//! Circuit breaker for provider route health tracking.
//!
//! # Planned Design
//!
//! States: Closed (normal) → Open (failing) → HalfOpen (probing)
//!
//! - **Closed → Open**: 5 failures in a 60s sliding window
//! - **Open → HalfOpen**: after 30s cooldown
//! - **HalfOpen → Closed**: 1 success resets
//! - **HalfOpen → Open**: 1 failure re-opens
//!
//! Redis keys:
//! - `cb:failures:{provider}:{model}` — failure count (TTL = window)
//! - `cb:open:{provider}:{model}` — open timestamp (TTL = cooldown)
//!
//! Integration points:
//! - `ExecutionPlan::build()` filters out OPEN routes
//! - `execute_with_fallback()` records success/failure after each route
