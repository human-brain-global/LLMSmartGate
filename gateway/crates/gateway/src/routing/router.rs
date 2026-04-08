//! Route cache and execution plan types.
//!
//! The [`RouteCache`] provides an in-memory cache for resolved provider routes,
//! keyed by `(tenant_id, model_alias)`. The [`ExecutionPlan`] represents an
//! ordered sequence of route attempts with retry configuration.

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

use crate::error::GatewayError;
use crate::models::ProviderRoute;
use crate::types::TenantId;

// ---------------------------------------------------------------------------
// RouteCache
// ---------------------------------------------------------------------------

/// Default maximum capacity for the route cache.
const ROUTE_CACHE_MAX_CAPACITY: u64 = 10_000;

/// In-memory cache for resolved provider routes, keyed by `(tenant_id, model_alias)`.
///
/// TTL-based expiry (default 60s) ensures eventual consistency with the
/// database. Active invalidation is triggered by admin route mutations
/// for immediate consistency.
#[derive(Clone)]
pub struct RouteCache {
    cache: Cache<String, Arc<Vec<ProviderRoute>>>,
}

impl RouteCache {
    /// Create a new route cache with the given TTL.
    pub fn new(ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(ROUTE_CACHE_MAX_CAPACITY)
            .time_to_live(Duration::from_secs(ttl_secs))
            .build();
        Self { cache }
    }

    /// Build the cache key for a tenant + model alias pair.
    fn cache_key(tenant_id: TenantId, model_alias: &str) -> String {
        format!("routes:{tenant_id}:{model_alias}")
    }

    /// Get cached routes or load them using the provided async loader.
    ///
    /// Uses moka's `try_get_with` to coalesce concurrent cache misses for the
    /// same key into a single load (prevents cache stampede).
    ///
    /// # Errors
    ///
    /// Returns the error produced by `loader` if the load fails.
    pub async fn get_or_load<F, Fut>(
        &self,
        tenant_id: TenantId,
        model_alias: &str,
        loader: F,
    ) -> Result<Arc<Vec<ProviderRoute>>, GatewayError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<ProviderRoute>, GatewayError>>,
    {
        let key = Self::cache_key(tenant_id, model_alias);
        self.cache
            .try_get_with(key, async {
                tracing::debug!(
                    %tenant_id,
                    model_alias,
                    "route cache miss — loading from DB"
                );
                let routes = loader().await?;
                Ok::<_, GatewayError>(Arc::new(routes))
            })
            .await
            .map_err(|e| GatewayError::routing("route_cache_load_failed", e.to_string()))
    }

    /// Invalidate all cached entries and drain pending operations.
    ///
    /// Used when a route is created, updated, or deleted and we don't know
    /// exactly which cache entries are affected (simpler than tracking all
    /// tenant/alias combinations). Awaits `run_pending_tasks` to ensure
    /// invalidation is immediately visible.
    pub async fn invalidate_all(&self) {
        self.cache.invalidate_all();
        self.cache.run_pending_tasks().await;
    }
}

// ---------------------------------------------------------------------------
// ExecutionPlan + RouteAttempt
// ---------------------------------------------------------------------------

/// Default backoff cap (maximum backoff duration per retry).
const DEFAULT_BACKOFF_CAP: Duration = Duration::from_secs(10);

/// A single route to attempt, with its retry configuration.
#[derive(Debug, Clone)]
pub struct RouteAttempt {
    /// The provider route to call.
    pub route: ProviderRoute,
    /// Maximum number of retries (0 = no retries, just the initial attempt).
    pub max_retries: u32,
    /// Timeout for a single provider call.
    pub timeout: Duration,
    /// Base backoff duration for exponential backoff.
    pub backoff_base: Duration,
    /// Maximum backoff duration (cap).
    pub backoff_cap: Duration,
}

/// An ordered execution plan: the sequence of routes to try with fallback.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// The model alias being resolved.
    pub model_alias: String,
    /// Ordered list of route attempts (primary first, then fallbacks).
    pub attempts: Vec<RouteAttempt>,
}

impl ExecutionPlan {
    /// Build an execution plan from resolved routes.
    ///
    /// Routes are already sorted by the resolver (tenant-scoped first,
    /// then priority ASC). This method converts `ProviderRoute` fields
    /// into typed `RouteAttempt` values.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Routing` with code `no_healthy_route` if no
    /// routes are provided, or `invalid_route_config` if any route has
    /// negative configuration values (fail-safe).
    //
    // NOTE: Circuit breaker filtering is NOT yet implemented (see health.rs
    // for the planned design). Once implemented, filter out routes whose
    // circuit breaker is in the OPEN state before building the plan.
    pub fn build(model_alias: &str, routes: &[ProviderRoute]) -> Result<Self, GatewayError> {
        if routes.is_empty() {
            return Err(GatewayError::routing(
                "no_healthy_route",
                format!("no available routes for model alias '{model_alias}'"),
            ));
        }

        let mut attempts = Vec::with_capacity(routes.len());
        for route in routes {
            // Fail-safe: reject negative values rather than silently accepting
            if route.max_retries < 0 || route.timeout_ms < 0 || route.retry_backoff_ms < 0 {
                tracing::error!(
                    route_id = %route.id,
                    max_retries = route.max_retries,
                    timeout_ms = route.timeout_ms,
                    retry_backoff_ms = route.retry_backoff_ms,
                    "invalid route configuration — negative values"
                );
                return Err(GatewayError::routing(
                    "invalid_route_config",
                    format!(
                        "route {} has invalid configuration (negative values)",
                        route.id
                    ),
                ));
            }

            #[allow(clippy::cast_sign_loss)]
            let attempt = RouteAttempt {
                route: route.clone(),
                max_retries: route.max_retries as u32,
                timeout: Duration::from_millis(route.timeout_ms as u64),
                backoff_base: Duration::from_millis(route.retry_backoff_ms as u64),
                backoff_cap: DEFAULT_BACKOFF_CAP,
            };
            attempts.push(attempt);
        }

        Ok(Self {
            model_alias: model_alias.to_owned(),
            attempts,
        })
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Provider, RouteId};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_route(overrides: impl FnOnce(&mut ProviderRoute)) -> ProviderRoute {
        let mut route = ProviderRoute {
            id: RouteId::new(),
            tenant_id: None,
            model_alias: "test-model".to_owned(),
            provider: Provider::Openai,
            provider_model_name: "gpt-4".to_owned(),
            priority: 100,
            enabled: true,
            timeout_ms: 30000,
            max_retries: 1,
            retry_backoff_ms: 250,
            config: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        overrides(&mut route);
        route
    }

    // -----------------------------------------------------------------------
    // RouteCache
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn cache_new_is_empty() {
        let cache = RouteCache::new(60);
        let tenant_id = TenantId::new();
        let key = RouteCache::cache_key(tenant_id, "gpt-4");
        assert!(cache.cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn cache_get_or_load_populates_on_miss() {
        let cache = RouteCache::new(60);
        let tenant_id = TenantId::new();
        let route = make_route(|_| {});

        let result = cache
            .get_or_load(tenant_id, "gpt-4", || async { Ok(vec![route.clone()]) })
            .await
            .expect("should load");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, route.id);

        // Second call should hit cache
        let key = RouteCache::cache_key(tenant_id, "gpt-4");
        assert!(cache.cache.get(&key).await.is_some());
    }

    #[tokio::test]
    async fn cache_stampede_protection() {
        let cache = RouteCache::new(60);
        let tenant_id = TenantId::new();
        let call_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let cache = cache.clone();
            let count = Arc::clone(&call_count);
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_load(tenant_id, "gpt-4", || {
                        let count = Arc::clone(&count);
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            // Simulate DB latency
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            Ok(vec![])
                        }
                    })
                    .await
                    .expect("should load");
            }));
        }

        for h in handles {
            h.await.expect("task should complete");
        }

        // Stampede protection: only 1 loader should have fired
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cache_invalidate_all_clears_entries() {
        let cache = RouteCache::new(60);
        let tenant_id = TenantId::new();

        cache
            .get_or_load(tenant_id, "gpt-4", || async { Ok(vec![]) })
            .await
            .expect("should load");

        cache
            .get_or_load(tenant_id, "claude-3", || async { Ok(vec![]) })
            .await
            .expect("should load");

        cache.invalidate_all().await;

        let key1 = RouteCache::cache_key(tenant_id, "gpt-4");
        let key2 = RouteCache::cache_key(tenant_id, "claude-3");
        assert!(cache.cache.get(&key1).await.is_none());
        assert!(cache.cache.get(&key2).await.is_none());
    }

    #[tokio::test]
    async fn cache_propagates_loader_error() {
        let cache = RouteCache::new(60);
        let tenant_id = TenantId::new();

        let result = cache
            .get_or_load(tenant_id, "gpt-4", || async {
                Err(GatewayError::routing("db_error", "connection lost"))
            })
            .await;

        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // ExecutionPlan
    // -----------------------------------------------------------------------

    #[test]
    fn execution_plan_from_routes() {
        let routes = vec![
            make_route(|r| {
                r.priority = 10;
                r.provider_model_name = "fast".to_owned();
            }),
            make_route(|r| {
                r.priority = 50;
                r.provider_model_name = "medium".to_owned();
            }),
            make_route(|r| {
                r.priority = 200;
                r.provider_model_name = "slow".to_owned();
            }),
        ];

        let plan = ExecutionPlan::build("test-model", &routes).expect("should build");
        assert_eq!(plan.model_alias, "test-model");
        assert_eq!(plan.attempts.len(), 3);
        assert_eq!(plan.attempts[0].route.provider_model_name, "fast");
        assert_eq!(plan.attempts[1].route.provider_model_name, "medium");
        assert_eq!(plan.attempts[2].route.provider_model_name, "slow");
    }

    #[test]
    fn execution_plan_empty_routes_errors() {
        let result = ExecutionPlan::build("test-model", &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            GatewayError::Routing { code, .. } => assert_eq!(code, "no_healthy_route"),
            other => panic!("expected Routing error, got: {other:?}"),
        }
    }

    #[test]
    fn execution_plan_maps_fields_correctly() {
        let route = make_route(|r| {
            r.max_retries = 3;
            r.timeout_ms = 15000;
            r.retry_backoff_ms = 500;
        });

        let plan = ExecutionPlan::build("test-model", &[route]).expect("should build");
        let attempt = &plan.attempts[0];

        assert_eq!(attempt.max_retries, 3);
        assert_eq!(attempt.timeout, Duration::from_millis(15000));
        assert_eq!(attempt.backoff_base, Duration::from_millis(500));
        assert_eq!(attempt.backoff_cap, DEFAULT_BACKOFF_CAP);
    }

    #[test]
    fn execution_plan_negative_retries_fails() {
        let route = make_route(|r| {
            r.max_retries = -1;
        });

        let result = ExecutionPlan::build("test-model", &[route]);
        assert!(result.is_err());
        match result.unwrap_err() {
            GatewayError::Routing { code, .. } => assert_eq!(code, "invalid_route_config"),
            other => panic!("expected Routing error, got: {other:?}"),
        }
    }

    #[test]
    fn execution_plan_negative_timeout_fails() {
        let route = make_route(|r| {
            r.timeout_ms = -1;
        });

        let result = ExecutionPlan::build("test-model", &[route]);
        assert!(result.is_err());
    }

    #[test]
    fn execution_plan_negative_backoff_fails() {
        let route = make_route(|r| {
            r.retry_backoff_ms = -1;
        });

        let result = ExecutionPlan::build("test-model", &[route]);
        assert!(result.is_err());
    }
}
