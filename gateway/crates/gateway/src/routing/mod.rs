//! Routing engine -- alias resolution, priority sorting, fallback chains, retry policy.

pub mod fallback;
pub mod health;
pub mod resolver;
pub mod retry;
pub mod router;
