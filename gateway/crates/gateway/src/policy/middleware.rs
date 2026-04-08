//! Policy enforcement middleware for the data plane.
//!
//! Runs after auth middleware. Evaluates rate limits and concurrency limits,
//! then injects `X-RateLimit-*` headers into every response. Stores the
//! evaluated policy in request extensions so downstream handlers can read it
//! without a second cache lookup.

use std::sync::Arc;

use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::auth::context::AuthContext;
use crate::error::GatewayError;
use crate::server::AppState;

use super::concurrency::ConcurrencyPermit;
use super::engine::EvaluatedPolicy;
use super::headers::inject_rate_limit_headers;

/// Data-plane middleware that enforces rate limits and concurrency limits.
///
/// Must be layered **after** auth middleware (which injects `AuthContext`).
/// Layers execute bottom-up, so in the router this layer is added *before*
/// the auth layer.
///
/// Stores `Arc<EvaluatedPolicy>` in request extensions for downstream handlers.
///
/// # Errors
///
/// Returns `GatewayError::RateLimit` (429) if a rate or concurrency limit is exceeded,
/// or `GatewayError::Config` (500) if required state is not initialized.
#[tracing::instrument(name = "policy.rate_limit", skip_all)]
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, GatewayError> {
    // Extract auth context (set by auth middleware).
    let auth = request
        .extensions()
        .get::<AuthContext>()
        .ok_or_else(|| {
            tracing::error!("rate_limit_middleware: AuthContext missing from request extensions");
            GatewayError::config("internal_error", "auth context missing")
        })?
        .clone();

    let redis_pool = state.require_redis()?;
    let evaluator = state.require_rate_limit_evaluator()?;

    // Get the evaluated policy from the policy cache.
    let policy_cache = state.require_policy_cache()?;
    let db = state.require_db()?;
    let repo = crate::storage::repositories::policies::PolicyRepo::new(db.clone());
    let evaluated = policy_cache
        .get_or_evaluate(&repo, &auth)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "rate_limit_middleware: policy evaluation failed");
            e
        })?;

    // 1. Rate limit check (SA → global)
    let outcome = evaluator
        .evaluate(redis_pool, auth.service_account_id, &evaluated)
        .await?;

    // 2. Concurrency limit check (if configured)
    let permit: Option<ConcurrencyPermit> = if let Some(limit) = evaluated.concurrency_limit {
        let limiter = state.require_concurrency_limiter()?;
        Some(limiter.acquire(auth.service_account_id, limit).await?)
    } else {
        None
    };

    // 3. Store evaluated policy in extensions for downstream handlers
    request
        .extensions_mut()
        .insert::<Arc<EvaluatedPolicy>>(evaluated);

    // 4. Run the downstream handler
    let response = next.run(request).await;

    // 5. Release concurrency permit (explicit release preferred over Drop)
    if let Some(permit) = permit {
        permit.release().await;
    }

    // 6. Inject rate limit headers into every response
    Ok(inject_rate_limit_headers(response, &outcome))
}
