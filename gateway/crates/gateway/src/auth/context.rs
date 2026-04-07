//! Authentication context injected into request extensions after successful verification.

use crate::types::{Environment, PolicyId, ServiceAccountId, TenantId};

/// Authenticated request context, available to downstream handlers via request extensions.
///
/// Populated by the auth middleware after successful Ed25519 signature verification.
/// Downstream handlers (policy engine, routing, etc.) use this to make authorization decisions.
///
/// Policy resolution is deferred to the policy engine (Item 006) — the auth context
/// carries the `default_policy_id` so the policy engine can load and merge policies.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// The tenant that owns the service account.
    pub tenant_id: TenantId,
    /// The authenticated service account.
    pub service_account_id: ServiceAccountId,
    /// The key_id string used for this request.
    pub key_id: String,
    /// Human-readable service account name (for logging and error messages).
    pub service_account_name: String,
    /// Deployment environment (dev/staging/prod) for environment-scoped policies.
    pub environment: Environment,
    /// Default policy ID (fallback if no explicit bindings). Policy engine loads the full set.
    pub default_policy_id: Option<PolicyId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_context_is_clone_and_debug() {
        let ctx = AuthContext {
            tenant_id: TenantId::new(),
            service_account_id: ServiceAccountId::new(),
            key_id: "test-key-id".to_owned(),
            service_account_name: "test-sa".to_owned(),
            environment: Environment::Dev,
            default_policy_id: Some(PolicyId::new()),
        };
        let cloned = ctx.clone();
        assert_eq!(cloned.tenant_id, ctx.tenant_id);
        assert_eq!(cloned.service_account_id, ctx.service_account_id);
        assert_eq!(cloned.key_id, ctx.key_id);
        assert_eq!(cloned.service_account_name, ctx.service_account_name);
        assert_eq!(cloned.environment, ctx.environment);
        assert!(cloned.default_policy_id.is_some());
        let _ = format!("{ctx:?}");
    }
}
