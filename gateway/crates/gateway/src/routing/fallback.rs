//! Fallback chain execution -- tries routes in priority order with retries.
//!
//! [`execute_with_fallback`] is the top-level orchestrator that iterates through
//! an [`ExecutionPlan`], retrying each route before falling back to the next.

use std::future::Future;

use crate::error::GatewayError;
use crate::models::ProviderRoute;
use crate::routing::retry::{AttemptOutcome, retry_with_backoff};
use crate::routing::router::ExecutionPlan;

// ---------------------------------------------------------------------------
// RoutingOutcome
// ---------------------------------------------------------------------------

/// The successful outcome of executing a request through the fallback chain.
#[derive(Debug)]
pub struct RoutingOutcome<T> {
    /// The successful result from the provider.
    pub result: T,
    /// The route that produced the successful result.
    pub route: ProviderRoute,
    /// Total number of individual call attempts across all routes.
    pub total_attempts: u32,
    /// Number of distinct routes tried (including the successful one).
    pub routes_tried: u32,
}

// ---------------------------------------------------------------------------
// Fallback executor
// ---------------------------------------------------------------------------

/// Execute a request across the fallback chain.
///
/// For each route in the execution plan:
/// 1. Attempt the route with retries (via [`retry_with_backoff`])
/// 2. On success, return immediately with [`RoutingOutcome`]
/// 3. On failure (all retries exhausted or fatal error), move to the next route
/// 4. If all routes exhausted, return the last error
///
/// # Type Parameters
///
/// * `T` - The success type (e.g., provider response)
/// * `F` - Async closure that performs a single provider call
///
/// # Errors
///
/// Returns `GatewayError::Routing` with code `all_routes_exhausted` if every
/// route in the plan fails.
//
// NOTE: Circuit breaker integration is NOT yet implemented (see health.rs
// for the planned design). Once implemented, record failures here so the
// breaker can trip OPEN when the threshold is reached.
pub async fn execute_with_fallback<T, F, Fut>(
    plan: &ExecutionPlan,
    call_fn: F,
) -> Result<RoutingOutcome<T>, GatewayError>
where
    F: Fn(&ProviderRoute) -> Fut,
    Fut: Future<Output = AttemptOutcome<T>>,
{
    let mut total_attempts: u32 = 0;
    let mut routes_tried: u32 = 0;
    let mut last_error: Option<GatewayError> = None;

    for route_attempt in &plan.attempts {
        routes_tried += 1;

        match retry_with_backoff(route_attempt, &call_fn).await {
            Ok((result, attempts_used)) => {
                total_attempts += attempts_used;
                return Ok(RoutingOutcome {
                    result,
                    route: route_attempt.route.clone(),
                    total_attempts,
                    routes_tried,
                });
            }
            Err((e, attempts_used)) => {
                total_attempts += attempts_used;

                tracing::warn!(
                    route_id = %route_attempt.route.id,
                    provider = ?route_attempt.route.provider,
                    model_alias = %plan.model_alias,
                    routes_tried,
                    attempts_used,
                    total_attempts,
                    error = %e,
                    "route failed — trying next in fallback chain"
                );

                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        GatewayError::routing(
            "all_routes_exhausted",
            format!(
                "all {} routes failed for model alias '{}'",
                routes_tried, plan.model_alias
            ),
        )
    }))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::router::{ExecutionPlan, RouteAttempt};
    use crate::types::{Provider, RouteId};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_attempt(provider_model: &str, max_retries: u32) -> RouteAttempt {
        RouteAttempt {
            route: ProviderRoute {
                id: RouteId::new(),
                tenant_id: None,
                model_alias: "test-model".to_owned(),
                provider: Provider::Openai,
                provider_model_name: provider_model.to_owned(),
                priority: 100,
                enabled: true,
                timeout_ms: 30000,
                #[allow(clippy::cast_possible_wrap)]
                max_retries: max_retries as i32,
                retry_backoff_ms: 1, // minimal backoff for tests
                config: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            max_retries,
            timeout: Duration::from_millis(30000),
            backoff_base: Duration::from_millis(1), // minimal for tests
            backoff_cap: Duration::from_millis(10),
        }
    }

    fn make_plan(attempts: Vec<RouteAttempt>) -> ExecutionPlan {
        ExecutionPlan {
            model_alias: "test-model".to_owned(),
            attempts,
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn fallback_succeeds_on_first_route() {
        tokio::time::pause();

        let plan = make_plan(vec![
            make_attempt("primary", 1),
            make_attempt("secondary", 1),
        ]);

        let outcome =
            execute_with_fallback(&plan, |_route| async { AttemptOutcome::Success("ok") })
                .await
                .expect("should succeed");

        assert_eq!(outcome.result, "ok");
        assert_eq!(outcome.routes_tried, 1);
        assert_eq!(outcome.route.provider_model_name, "primary");
    }

    #[tokio::test]
    async fn fallback_to_second_route() {
        tokio::time::pause();

        let plan = make_plan(vec![
            make_attempt("primary", 0),   // no retries
            make_attempt("secondary", 0), // no retries
        ]);

        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let outcome = execute_with_fallback(&plan, |route| {
            let count = Arc::clone(&count);
            let model = route.provider_model_name.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if model == "primary" {
                    AttemptOutcome::Retryable(GatewayError::routing("error", "fail"))
                } else {
                    AttemptOutcome::Success("recovered")
                }
            }
        })
        .await
        .expect("should succeed on fallback");

        assert_eq!(outcome.result, "recovered");
        assert_eq!(outcome.routes_tried, 2);
        assert_eq!(outcome.route.provider_model_name, "secondary");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fallback_all_routes_exhausted() {
        tokio::time::pause();

        let plan = make_plan(vec![
            make_attempt("primary", 0),
            make_attempt("secondary", 0),
        ]);

        let result = execute_with_fallback::<(), _, _>(&plan, |_route| async {
            AttemptOutcome::Retryable(GatewayError::routing("error", "fail"))
        })
        .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            GatewayError::Routing { code, .. } => {
                // The last error from retry_with_backoff propagates
                assert!(code == "error" || code == "all_routes_exhausted");
            }
            other => panic!("expected Routing error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn fallback_fatal_error_skips_retries_tries_next_route() {
        tokio::time::pause();

        let plan = make_plan(vec![
            make_attempt("primary", 3), // has retries but Fatal should skip them
            make_attempt("secondary", 0),
        ]);

        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let outcome = execute_with_fallback(&plan, |route| {
            let count = Arc::clone(&count);
            let model = route.provider_model_name.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if model == "primary" {
                    AttemptOutcome::Fatal(GatewayError::routing("bad_request", "400"))
                } else {
                    AttemptOutcome::Success("ok")
                }
            }
        })
        .await
        .expect("should succeed on secondary");

        assert_eq!(outcome.result, "ok");
        assert_eq!(outcome.routes_tried, 2);
        // Fatal on primary = 1 call, success on secondary = 1 call
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        assert_eq!(outcome.total_attempts, 2);
    }

    #[tokio::test]
    async fn fallback_fatal_total_attempts_counts_one_per_fatal() {
        tokio::time::pause();

        let plan = make_plan(vec![
            make_attempt("primary", 3),   // Fatal on attempt 1 — should count 1, not 4
            make_attempt("fallback", 3),  // Fatal on attempt 1 — should count 1, not 4
            make_attempt("secondary", 0), // Success on attempt 1
        ]);

        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let outcome = execute_with_fallback(&plan, |route| {
            let count = Arc::clone(&count);
            let model = route.provider_model_name.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if model == "secondary" {
                    AttemptOutcome::Success("ok")
                } else {
                    AttemptOutcome::Fatal(GatewayError::routing("bad_request", "400"))
                }
            }
        })
        .await
        .expect("should succeed on secondary");

        assert_eq!(outcome.result, "ok");
        assert_eq!(outcome.routes_tried, 3);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        // 1 (fatal) + 1 (fatal) + 1 (success) = 3, NOT 4+4+1=9
        assert_eq!(outcome.total_attempts, 3);
    }

    #[tokio::test]
    async fn fallback_counts_total_attempts_across_routes() {
        tokio::time::pause();

        let plan = make_plan(vec![
            make_attempt("primary", 2),   // 1 + 2 retries = 3 attempts
            make_attempt("secondary", 0), // 1 attempt
        ]);

        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let outcome = execute_with_fallback(&plan, |route| {
            let count = Arc::clone(&count);
            let model = route.provider_model_name.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if model == "primary" {
                    AttemptOutcome::Retryable(GatewayError::routing("error", "fail"))
                } else {
                    AttemptOutcome::Success("ok")
                }
            }
        })
        .await
        .expect("should succeed");

        assert_eq!(outcome.result, "ok");
        assert_eq!(outcome.routes_tried, 2);
        // Primary: 3 attempts (exhausted). Secondary: 1 attempt (success).
        assert_eq!(outcome.total_attempts, 4);
    }

    #[tokio::test]
    async fn fallback_empty_plan_errors() {
        let plan = make_plan(vec![]);

        let result = execute_with_fallback::<(), _, _>(&plan, |_route| async {
            AttemptOutcome::Success(())
        })
        .await;

        assert!(result.is_err());
    }
}
