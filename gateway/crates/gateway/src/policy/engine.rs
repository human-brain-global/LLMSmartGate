//! Policy evaluation engine -- load, merge, and enforce policies.
//!
//! Combines model access control, token limits, and feature flags into a
//! single evaluation pass. Multiple policies are merged using the
//! "most restrictive wins" strategy.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

use crate::auth::context::AuthContext;
use crate::error::GatewayError;
use crate::models::Policy;
use crate::policy::features::{self, FeatureFlags, RequestFeatures};
use crate::policy::model_access::{self, ModelAccessRules};
use crate::policy::quotas::{self, TokenCheckResult, TokenLimits};
use crate::storage::repositories::policies::PolicyRepo;
use crate::types::ServiceAccountId;

// ---------------------------------------------------------------------------
// EvaluatedPolicy -- the merged result of all applicable policies
// ---------------------------------------------------------------------------

/// The result of merging all applicable policies for a service account.
///
/// This is the "merged view" computed once per request (or served from cache)
/// and used by each enforcement check.
#[derive(Debug, Clone)]
pub struct EvaluatedPolicy {
    pub model_access: ModelAccessRules,
    pub token_limits: TokenLimits,
    pub feature_flags: FeatureFlags,
    /// Requests-per-minute limit from merged policies (most restrictive wins).
    pub rpm_limit: Option<u32>,
    /// Max concurrent in-flight requests from merged policies.
    pub concurrency_limit: Option<u32>,
}

impl Default for EvaluatedPolicy {
    /// Default for unbound service accounts: fail-safe restrictive defaults.
    ///
    /// An SA with no policy bindings gets no model restrictions (allowlist/denylist
    /// empty) but tools and files are denied per LLD fail-safe requirements.
    fn default() -> Self {
        Self {
            model_access: ModelAccessRules::default(),
            token_limits: TokenLimits::default(),
            feature_flags: FeatureFlags {
                allow_streaming: true,
                allow_tools: false,
                allow_files: false,
            },
            rpm_limit: None,
            concurrency_limit: None,
        }
    }
}

// ---------------------------------------------------------------------------
// PolicyDecision -- the outcome of a full evaluation
// ---------------------------------------------------------------------------

/// The outcome of evaluating all policies against a specific request.
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    /// Token check result (may contain clamped output tokens).
    pub token_check: TokenCheckResult,
    /// The merged policy (for downstream use by rate limiter, etc.).
    /// Wrapped in `Arc` to avoid cloning `Vec<String>` on every cache hit.
    pub evaluated: Arc<EvaluatedPolicy>,
}

// ---------------------------------------------------------------------------
// Merge logic
// ---------------------------------------------------------------------------

/// Validate a positive i32 limit and merge as minimum into an accumulator.
///
/// Returns `Err` with `policy_config_invalid` if the value is <= 0 (fail-safe).
fn merge_positive_limit(
    current: &mut Option<u32>,
    value: Option<i32>,
    policy_id: crate::types::PolicyId,
    field_name: &str,
) -> Result<(), GatewayError> {
    if let Some(limit) = value {
        if limit <= 0 {
            tracing::error!(
                policy_id = %policy_id,
                %field_name,
                limit,
                "policy: negative or zero {field_name} — denying access (fail-safe)"
            );
            return Err(GatewayError::policy(
                "policy_config_invalid",
                format!("policy has invalid {field_name} configuration"),
            ));
        }
        #[allow(clippy::cast_sign_loss)] // limit > 0 is guaranteed by the guard above
        let limit = limit as u32;
        *current = Some(current.map_or(limit, |cur| cur.min(limit)));
    }
    Ok(())
}

/// Merge multiple policies using the "most restrictive wins" strategy.
///
/// Rules:
/// - **allowed_models**: intersection (empty = "no constraint from this policy")
/// - **denied_models**: union
/// - **max_input_tokens / max_output_tokens / rpm_limit / concurrency_limit**: minimum of all `Some` values
/// - **allow_streaming / allow_tools / allow_files**: AND
///
/// # Errors
///
/// Returns `GatewayError::Policy` if JSON fields are malformed or numeric limits
/// are non-positive (fail-safe: deny access rather than silently granting it).
pub fn merge_policies(policies: &[Policy]) -> Result<EvaluatedPolicy, GatewayError> {
    if policies.is_empty() {
        return Ok(EvaluatedPolicy::default());
    }

    let mut allowed_models: Vec<String> = Vec::new();
    let mut denied_models_set: HashSet<String> = HashSet::new();
    let mut max_input_tokens: Option<u32> = None;
    let mut max_output_tokens: Option<u32> = None;
    let mut allow_streaming = true;
    let mut allow_tools = true;
    let mut allow_files = true;
    let mut rpm_limit: Option<u32> = None;
    let mut concurrency_limit: Option<u32> = None;
    let mut first_allowlist = true;

    for policy in policies {
        // Allowed models: intersection
        let policy_allowed: Vec<String> =
            serde_json::from_value(policy.allowed_models_json.clone()).map_err(|e| {
                tracing::error!(
                    policy_id = %policy.id,
                    error = %e,
                    "policy: malformed allowed_models_json — denying access (fail-safe)"
                );
                GatewayError::policy(
                    "policy_config_invalid",
                    "policy has invalid allowed_models configuration",
                )
            })?;

        if !policy_allowed.is_empty() {
            if first_allowlist {
                allowed_models = policy_allowed;
                first_allowlist = false;
            } else {
                // Keep only patterns that appear in both sets (O(n) via HashSet)
                let policy_set: HashSet<String> = policy_allowed.into_iter().collect();
                allowed_models.retain(|m| policy_set.contains(m));
            }
        }

        // Denied models: union (HashSet for O(1) dedup)
        let policy_denied: Vec<String> = serde_json::from_value(policy.denied_models_json.clone())
            .map_err(|e| {
                tracing::error!(
                    policy_id = %policy.id,
                    error = %e,
                    "policy: malformed denied_models_json — denying access (fail-safe)"
                );
                GatewayError::policy(
                    "policy_config_invalid",
                    "policy has invalid denied_models configuration",
                )
            })?;
        denied_models_set.extend(policy_denied);

        // Token limits: minimum (reject negative values at read boundary — fail-safe)
        merge_positive_limit(
            &mut max_input_tokens,
            policy.max_input_tokens,
            policy.id,
            "max_input_tokens",
        )?;
        merge_positive_limit(
            &mut max_output_tokens,
            policy.max_output_tokens,
            policy.id,
            "max_output_tokens",
        )?;

        // Feature flags: AND
        allow_streaming = allow_streaming && policy.allow_streaming;
        allow_tools = allow_tools && policy.allow_tools;
        allow_files = allow_files && policy.allow_files;

        // Rate limits: minimum (same pattern as token limits)
        merge_positive_limit(&mut rpm_limit, policy.rpm_limit, policy.id, "rpm_limit")?;
        merge_positive_limit(
            &mut concurrency_limit,
            policy.concurrency_limit,
            policy.id,
            "concurrency_limit",
        )?;
    }

    Ok(EvaluatedPolicy {
        model_access: ModelAccessRules {
            allowed_models,
            denied_models: denied_models_set.into_iter().collect(),
        },
        token_limits: TokenLimits {
            max_input_tokens,
            max_output_tokens,
        },
        feature_flags: FeatureFlags {
            allow_streaming,
            allow_tools,
            allow_files,
        },
        rpm_limit,
        concurrency_limit,
    })
}

// ---------------------------------------------------------------------------
// Policy loading
// ---------------------------------------------------------------------------

/// Load all applicable policies for a service account from the database.
///
/// Loads explicitly bound policies + the default policy (if set and not
/// already bound). Fails the request if the DB query fails (fail-safe, AP-5).
async fn load_policies(repo: &PolicyRepo, auth: &AuthContext) -> Result<Vec<Policy>, GatewayError> {
    repo.list_policies_for_service_account(auth.service_account_id, auth.default_policy_id)
        .await
        .map_err(|e| {
            tracing::error!(
                service_account_id = %auth.service_account_id,
                error = %e,
                "policy: failed to load policies"
            );
            GatewayError::policy(
                "policy_load_failed",
                "failed to load policies for service account",
            )
        })
}

// ---------------------------------------------------------------------------
// Full evaluation
// ---------------------------------------------------------------------------

/// Evaluate all applicable policies for a request **without caching**.
///
/// **Data-plane handlers should use [`evaluate_cached`] instead.** This
/// function loads policies fresh from the DB on every call and is intended
/// for testing and offline/admin use only.
///
/// Steps:
/// 1. Load policies from DB (bound + default)
/// 2. Merge using most-restrictive-wins
/// 3. Check model access
/// 4. Check feature flags
/// 5. Check token limits (may clamp output)
///
/// # Errors
///
/// Returns `GatewayError::Policy` on any policy violation or load failure.
#[expect(
    dead_code,
    reason = "reserved for data-plane handlers (use evaluate_cached instead)"
)]
#[tracing::instrument(
    name = "policy.evaluate",
    skip_all,
    fields(
        service_account_id = %auth.service_account_id,
        model = %model,
    )
)]
pub(crate) async fn evaluate(
    repo: &PolicyRepo,
    auth: &AuthContext,
    model: &str,
    request_features: &RequestFeatures,
    estimated_input_tokens: Option<u32>,
    requested_max_output_tokens: Option<u32>,
) -> Result<PolicyDecision, GatewayError> {
    // 1. Load policies
    let policies = load_policies(repo, auth).await?;

    // 2. Merge
    let evaluated = Arc::new(merge_policies(&policies)?);

    // 3-5. Apply enforcement checks
    let token_check = apply_policy_checks(
        &evaluated,
        model,
        request_features,
        estimated_input_tokens,
        requested_max_output_tokens,
    )?;

    Ok(PolicyDecision {
        token_check,
        evaluated,
    })
}

/// Shared enforcement checks: model access, feature flags, token limits.
///
/// Extracted to avoid divergence between `evaluate` and `evaluate_cached`.
fn apply_policy_checks(
    evaluated: &EvaluatedPolicy,
    model: &str,
    request_features: &RequestFeatures,
    estimated_input_tokens: Option<u32>,
    requested_max_output_tokens: Option<u32>,
) -> Result<TokenCheckResult, GatewayError> {
    model_access::check_model_access(&evaluated.model_access, model)?;
    features::check_features(&evaluated.feature_flags, request_features)?;
    quotas::check_token_limits(
        &evaluated.token_limits,
        estimated_input_tokens,
        requested_max_output_tokens,
    )
}

// ---------------------------------------------------------------------------
// PolicyCache
// ---------------------------------------------------------------------------

/// Default maximum capacity for the policy cache.
const POLICY_CACHE_MAX_CAPACITY: u64 = 10_000;

/// In-memory cache for evaluated (merged) policies, keyed by service account ID.
///
/// TTL-based expiry (default 30s) ensures eventual consistency with the
/// database. Active invalidation is triggered by admin policy/binding mutations
/// for immediate consistency.
#[derive(Clone)]
pub struct PolicyCache {
    cache: Cache<ServiceAccountId, Arc<EvaluatedPolicy>>,
}

impl PolicyCache {
    /// Create a new policy cache with the given TTL.
    pub fn new(ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(POLICY_CACHE_MAX_CAPACITY)
            .time_to_live(Duration::from_secs(ttl_secs))
            .build();
        Self { cache }
    }

    /// Get a cached evaluated policy, or load and cache it.
    ///
    /// Uses moka's `try_get_with` to coalesce concurrent cache misses for the
    /// same SA into a single DB query (prevents cache stampede).
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Policy` if loading policies from the DB fails.
    pub async fn get_or_evaluate(
        &self,
        repo: &PolicyRepo,
        auth: &AuthContext,
    ) -> Result<Arc<EvaluatedPolicy>, GatewayError> {
        let sa_id = auth.service_account_id;
        self.cache
            .try_get_with(sa_id, async {
                tracing::debug!(service_account_id = %sa_id, "policy cache miss — loading from DB");
                let policies = load_policies(repo, auth).await?;
                let evaluated = merge_policies(&policies)?;
                Ok::<_, GatewayError>(Arc::new(evaluated))
            })
            .await
            .map_err(|e| {
                // try_get_with wraps the error in Arc — convert to string
                GatewayError::policy("policy_load_failed", e.to_string())
            })
    }

    /// Invalidate the cache entry for a specific service account.
    pub async fn invalidate(&self, sa_id: &ServiceAccountId) {
        self.cache.invalidate(sa_id).await;
    }

    /// Invalidate all cached entries and drain pending operations.
    ///
    /// Used when a policy is modified and we don't know which service accounts
    /// are affected (simpler than tracking all bindings, and TTL is short).
    /// Awaits `run_pending_tasks` to ensure invalidation is immediately
    /// visible to concurrent readers on the same process.
    pub async fn invalidate_all(&self) {
        self.cache.invalidate_all();
        self.cache.run_pending_tasks().await;
    }
}

/// Evaluate policies using the cache for the merge step.
///
/// This is the primary entry point for data-plane request processing.
///
/// # Errors
///
/// Returns `GatewayError::Policy` on any policy violation or load failure.
#[tracing::instrument(
    name = "policy.evaluate_cached",
    skip_all,
    fields(
        service_account_id = %auth.service_account_id,
        model = %model,
    )
)]
pub async fn evaluate_cached(
    cache: &PolicyCache,
    repo: &PolicyRepo,
    auth: &AuthContext,
    model: &str,
    request_features: &RequestFeatures,
    estimated_input_tokens: Option<u32>,
    requested_max_output_tokens: Option<u32>,
) -> Result<PolicyDecision, GatewayError> {
    // 1. Get merged policy from cache (or load + merge)
    let evaluated = cache.get_or_evaluate(repo, auth).await?;

    // 2-4. Apply enforcement checks (shared with evaluate())
    let token_check = apply_policy_checks(
        &evaluated,
        model,
        request_features,
        estimated_input_tokens,
        requested_max_output_tokens,
    )?;

    Ok(PolicyDecision {
        token_check,
        evaluated,
    })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PolicyId, TenantId};
    use chrono::Utc;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_policy(overrides: impl FnOnce(&mut Policy)) -> Policy {
        let mut p = Policy {
            id: PolicyId::new(),
            tenant_id: TenantId::new(),
            name: "test-policy".to_owned(),
            description: None,
            allowed_models_json: json!([]),
            denied_models_json: json!([]),
            max_input_tokens: None,
            max_output_tokens: None,
            allow_streaming: true,
            allow_tools: true,
            allow_files: true,
            rpm_limit: None,
            concurrency_limit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        overrides(&mut p);
        p
    }

    // -----------------------------------------------------------------------
    // merge_policies
    // -----------------------------------------------------------------------

    #[test]
    fn merge_no_policies_returns_defaults() {
        let result = merge_policies(&[]).unwrap();
        assert!(result.model_access.allowed_models.is_empty());
        assert!(result.model_access.denied_models.is_empty());
        assert!(result.token_limits.max_input_tokens.is_none());
        assert!(result.token_limits.max_output_tokens.is_none());
        assert!(result.feature_flags.allow_streaming);
        // Fail-safe: unbound SAs get tools/files denied
        assert!(!result.feature_flags.allow_tools);
        assert!(!result.feature_flags.allow_files);
    }

    #[test]
    fn merge_single_policy_passes_through() {
        let p = make_policy(|p| {
            p.allowed_models_json = json!(["gpt-4*"]);
            p.denied_models_json = json!(["gpt-4-32k"]);
            p.max_input_tokens = Some(1000);
            p.max_output_tokens = Some(500);
            p.allow_streaming = false;
            p.allow_tools = true;
            p.allow_files = false;
        });
        let result = merge_policies(&[p]).unwrap();

        assert_eq!(result.model_access.allowed_models, vec!["gpt-4*"]);
        assert_eq!(result.model_access.denied_models, vec!["gpt-4-32k"]);
        assert_eq!(result.token_limits.max_input_tokens, Some(1000));
        assert_eq!(result.token_limits.max_output_tokens, Some(500));
        assert!(!result.feature_flags.allow_streaming);
        assert!(result.feature_flags.allow_tools);
        assert!(!result.feature_flags.allow_files);
    }

    #[test]
    fn merge_two_policies_most_restrictive() {
        let p1 = make_policy(|p| {
            p.allowed_models_json = json!(["gpt-4*", "claude-3*"]);
            p.denied_models_json = json!(["gpt-4-32k"]);
            p.max_input_tokens = Some(2000);
            p.max_output_tokens = Some(1000);
            p.allow_streaming = true;
            p.allow_tools = true;
            p.allow_files = true;
        });
        let p2 = make_policy(|p| {
            p.allowed_models_json = json!(["gpt-4*"]);
            p.denied_models_json = json!(["gpt-4-turbo"]);
            p.max_input_tokens = Some(1000);
            p.max_output_tokens = Some(500);
            p.allow_streaming = false;
            p.allow_tools = true;
            p.allow_files = false;
        });

        let result = merge_policies(&[p1, p2]).unwrap();

        // Intersection of allowed: only "gpt-4*" (present in both)
        assert_eq!(result.model_access.allowed_models, vec!["gpt-4*"]);
        // Union of denied
        assert!(
            result
                .model_access
                .denied_models
                .contains(&"gpt-4-32k".to_owned())
        );
        assert!(
            result
                .model_access
                .denied_models
                .contains(&"gpt-4-turbo".to_owned())
        );
        // Minimum token limits
        assert_eq!(result.token_limits.max_input_tokens, Some(1000));
        assert_eq!(result.token_limits.max_output_tokens, Some(500));
        // AND of feature flags
        assert!(!result.feature_flags.allow_streaming);
        assert!(result.feature_flags.allow_tools);
        assert!(!result.feature_flags.allow_files);
    }

    #[test]
    fn merge_allowed_models_empty_means_no_constraint() {
        let p1 = make_policy(|p| {
            p.allowed_models_json = json!(["gpt-4*", "claude-3*"]);
        });
        let p2 = make_policy(|_| {
            // empty allowed_models = no constraint from this policy
        });

        let result = merge_policies(&[p1, p2]).unwrap();

        // p2 has no constraint, so p1's allowlist is preserved
        assert_eq!(
            result.model_access.allowed_models,
            vec!["gpt-4*", "claude-3*"]
        );
    }

    #[test]
    fn merge_denied_models_union() {
        let p1 = make_policy(|p| {
            p.denied_models_json = json!(["gpt-4-32k"]);
        });
        let p2 = make_policy(|p| {
            p.denied_models_json = json!(["gpt-4-32k", "claude-3-opus"]);
        });

        let result = merge_policies(&[p1, p2]).unwrap();

        assert_eq!(result.model_access.denied_models.len(), 2);
        assert!(
            result
                .model_access
                .denied_models
                .contains(&"gpt-4-32k".to_owned())
        );
        assert!(
            result
                .model_access
                .denied_models
                .contains(&"claude-3-opus".to_owned())
        );
    }

    #[test]
    fn merge_token_limits_one_none() {
        let p1 = make_policy(|p| {
            p.max_input_tokens = Some(2000);
            // no output limit
        });
        let p2 = make_policy(|p| {
            // no input limit
            p.max_output_tokens = Some(500);
        });

        let result = merge_policies(&[p1, p2]).unwrap();

        assert_eq!(result.token_limits.max_input_tokens, Some(2000));
        assert_eq!(result.token_limits.max_output_tokens, Some(500));
    }

    #[test]
    fn merge_feature_flags_and_logic() {
        let p1 = make_policy(|p| {
            p.allow_streaming = true;
            p.allow_tools = false;
            p.allow_files = true;
        });
        let p2 = make_policy(|p| {
            p.allow_streaming = true;
            p.allow_tools = true;
            p.allow_files = false;
        });

        let result = merge_policies(&[p1, p2]).unwrap();

        assert!(result.feature_flags.allow_streaming); // true AND true
        assert!(!result.feature_flags.allow_tools); // false AND true
        assert!(!result.feature_flags.allow_files); // true AND false
    }

    // -----------------------------------------------------------------------
    // PolicyCache
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn cache_new_creates_instance() {
        let cache = PolicyCache::new(30);
        let sa_id = ServiceAccountId::new();
        // Cache should be empty initially
        assert!(cache.cache.get(&sa_id).await.is_none());
    }

    #[tokio::test]
    async fn cache_insert_and_get() {
        let cache = PolicyCache::new(30);
        let sa_id = ServiceAccountId::new();
        let policy = Arc::new(EvaluatedPolicy::default());

        cache.cache.insert(sa_id, Arc::clone(&policy)).await;

        let cached = cache.cache.get(&sa_id).await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn cache_invalidate_removes_entry() {
        let cache = PolicyCache::new(30);
        let sa_id = ServiceAccountId::new();
        let policy = Arc::new(EvaluatedPolicy::default());

        cache.cache.insert(sa_id, Arc::clone(&policy)).await;
        assert!(cache.cache.get(&sa_id).await.is_some());

        cache.invalidate(&sa_id).await;
        assert!(cache.cache.get(&sa_id).await.is_none());
    }

    #[tokio::test]
    async fn cache_invalidate_all_clears_everything() {
        let cache = PolicyCache::new(30);
        let sa1 = ServiceAccountId::new();
        let sa2 = ServiceAccountId::new();
        let policy = Arc::new(EvaluatedPolicy::default());

        cache.cache.insert(sa1, Arc::clone(&policy)).await;
        cache.cache.insert(sa2, Arc::clone(&policy)).await;

        cache.invalidate_all().await;

        assert!(cache.cache.get(&sa1).await.is_none());
        assert!(cache.cache.get(&sa2).await.is_none());
    }

    // -----------------------------------------------------------------------
    // merge_policies -- rate limits
    // -----------------------------------------------------------------------

    #[test]
    fn merge_rpm_limit_most_restrictive() {
        let p1 = make_policy(|p| {
            p.rpm_limit = Some(1000);
        });
        let p2 = make_policy(|p| {
            p.rpm_limit = Some(500);
        });
        let result = merge_policies(&[p1, p2]).unwrap();
        assert_eq!(result.rpm_limit, Some(500));
    }

    #[test]
    fn merge_concurrency_limit_most_restrictive() {
        let p1 = make_policy(|p| {
            p.concurrency_limit = Some(10);
        });
        let p2 = make_policy(|p| {
            p.concurrency_limit = Some(5);
        });
        let result = merge_policies(&[p1, p2]).unwrap();
        assert_eq!(result.concurrency_limit, Some(5));
    }

    #[test]
    fn merge_rpm_limit_none_means_no_constraint() {
        let p1 = make_policy(|p| {
            p.rpm_limit = Some(1000);
        });
        let p2 = make_policy(|_| {}); // no rpm_limit
        let result = merge_policies(&[p1, p2]).unwrap();
        assert_eq!(result.rpm_limit, Some(1000));
    }

    #[test]
    fn merge_all_none_rate_limits() {
        let p1 = make_policy(|_| {});
        let result = merge_policies(&[p1]).unwrap();
        assert!(result.rpm_limit.is_none());
        assert!(result.concurrency_limit.is_none());
    }

    #[test]
    fn merge_rejects_zero_rpm_limit() {
        let p = make_policy(|p| {
            p.rpm_limit = Some(0);
        });
        let err = merge_policies(&[p]).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "policy_config_invalid"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn merge_rejects_negative_concurrency_limit() {
        let p = make_policy(|p| {
            p.concurrency_limit = Some(-1);
        });
        let err = merge_policies(&[p]).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "policy_config_invalid"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // merge_policies -- token limit errors
    // -----------------------------------------------------------------------

    #[test]
    fn merge_rejects_negative_token_limits() {
        let p = make_policy(|p| {
            p.max_input_tokens = Some(-1);
        });
        let err = merge_policies(&[p]).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "policy_config_invalid"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn merge_rejects_zero_token_limits() {
        let p = make_policy(|p| {
            p.max_output_tokens = Some(0);
        });
        let err = merge_policies(&[p]).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "policy_config_invalid"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn merge_rejects_malformed_allowed_models_json() {
        let p = make_policy(|p| {
            p.allowed_models_json = json!("not-an-array");
        });
        let err = merge_policies(&[p]).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "policy_config_invalid"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn merge_rejects_malformed_denied_models_json() {
        let p = make_policy(|p| {
            p.denied_models_json = json!({"not": "an-array"});
        });
        let err = merge_policies(&[p]).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "policy_config_invalid"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn cache_invalidate_one_preserves_others() {
        let cache = PolicyCache::new(30);
        let sa1 = ServiceAccountId::new();
        let sa2 = ServiceAccountId::new();
        let policy = Arc::new(EvaluatedPolicy::default());

        cache.cache.insert(sa1, Arc::clone(&policy)).await;
        cache.cache.insert(sa2, Arc::clone(&policy)).await;

        cache.invalidate(&sa1).await;

        assert!(cache.cache.get(&sa1).await.is_none());
        assert!(cache.cache.get(&sa2).await.is_some());
    }
}
