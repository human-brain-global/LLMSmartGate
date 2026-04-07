//! Policy engine -- model access, token limits, feature flags, and evaluation.

pub mod budgets;
pub mod engine;
pub mod features;
pub mod model_access;
pub mod quotas;

// Re-export key types for ergonomic imports.
pub use engine::{EvaluatedPolicy, PolicyCache, PolicyDecision, evaluate_cached, merge_policies};
pub use features::{FeatureFlags, RequestFeatures};
pub use model_access::{ModelAccessRules, check_model_access};
pub use quotas::{TokenCheckResult, TokenLimits, check_token_limits, estimate_input_tokens};
