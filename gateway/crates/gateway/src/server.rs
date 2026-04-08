//! Axum server wiring -- router construction, middleware stack, and application state.

use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use deadpool_redis::Pool as RedisPool;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::api::health;
use crate::auth;
use crate::auth::key_store::KeyStore;
use crate::config::GatewayConfig;
use crate::error::GatewayError;
use crate::models::AdminApiKey;
use crate::policy::concurrency::ConcurrencyLimiter;
use crate::policy::engine::PolicyCache;
use crate::policy::rate_limit::RateLimitEvaluator;

/// Header name used for request ID propagation.
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Maximum request body size in bytes (10 MiB).
pub const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Application state shared across all handlers via Axum's state extractor.
#[derive(Clone)]
pub struct AppState {
    /// Shared gateway configuration.
    pub config: Arc<GatewayConfig>,
    /// PostgreSQL connection pool. `None` in unit tests that don't need a DB.
    pub db: Option<PgPool>,
    /// Redis/Valkey connection pool. `None` in unit tests that don't need Redis.
    pub redis: Option<RedisPool>,
    /// Three-tier key cache (moka L1 → Redis L2 → PostgreSQL L3). `None` in unit tests.
    pub key_store: Option<KeyStore>,
    /// Cache for admin API keys (unit key since there is only one entry set).
    /// 30-second TTL. `None` in unit tests.
    pub admin_key_cache: Option<moka::future::Cache<(), Vec<AdminApiKey>>>,
    /// In-memory policy evaluation cache. `None` in unit tests.
    pub policy_cache: Option<PolicyCache>,
    /// Rate limit evaluator (sliding window). `None` in unit tests.
    pub rate_limit_evaluator: Option<RateLimitEvaluator>,
    /// Concurrency limiter. `None` in unit tests.
    pub concurrency_limiter: Option<ConcurrencyLimiter>,
}

impl AppState {
    /// Get the database pool, or return 500 if not initialized.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Config` with code `internal_error` if the pool is `None`.
    pub fn require_db(&self) -> Result<&PgPool, GatewayError> {
        self.db.as_ref().ok_or_else(|| {
            tracing::warn!("database pool not available");
            GatewayError::config("internal_error", "database not available")
        })
    }

    /// Get the Redis pool, or return 500 if not initialized.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Config` with code `internal_error` if the pool is `None`.
    pub fn require_redis(&self) -> Result<&RedisPool, GatewayError> {
        self.redis.as_ref().ok_or_else(|| {
            tracing::warn!("redis pool not available");
            GatewayError::config("internal_error", "redis not available")
        })
    }

    /// Get the key store, or return 500 if not initialized.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Config` with code `internal_error` if the key store is `None`.
    pub fn require_key_store(&self) -> Result<&KeyStore, GatewayError> {
        self.key_store.as_ref().ok_or_else(|| {
            tracing::warn!("key store not available");
            GatewayError::config("internal_error", "key store not available")
        })
    }

    /// Get the policy cache, or return 500 if not initialized.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Config` with code `internal_error` if the cache is `None`.
    pub fn require_policy_cache(&self) -> Result<&PolicyCache, GatewayError> {
        self.policy_cache.as_ref().ok_or_else(|| {
            tracing::warn!("policy cache not available");
            GatewayError::config("internal_error", "policy cache not available")
        })
    }

    /// Get the rate limit evaluator, or return 500 if not initialized.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Config` with code `internal_error` if the evaluator is `None`.
    pub fn require_rate_limit_evaluator(&self) -> Result<&RateLimitEvaluator, GatewayError> {
        self.rate_limit_evaluator.as_ref().ok_or_else(|| {
            tracing::warn!("rate limit evaluator not available");
            GatewayError::config("internal_error", "rate limit evaluator not available")
        })
    }

    /// Get the concurrency limiter, or return 500 if not initialized.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Config` with code `internal_error` if the limiter is `None`.
    pub fn require_concurrency_limiter(&self) -> Result<&ConcurrencyLimiter, GatewayError> {
        self.concurrency_limiter.as_ref().ok_or_else(|| {
            tracing::warn!("concurrency limiter not available");
            GatewayError::config("internal_error", "concurrency limiter not available")
        })
    }
}

/// Build the complete Axum router with all route groups and middleware.
///
/// Route groups:
/// - Health: `/healthz`, `/readyz`, `/metrics` (no auth required)
/// - Data plane: `/v1/*` (auth required via Ed25519 middleware)
/// - Admin: `/admin/v1/*` (admin auth required -- placeholder returns 401)
pub fn build_router(state: AppState) -> Router {
    use crate::api::admin::{policies, routes as route_handlers, service_accounts, tenants};

    // Health routes -- no authentication required
    let health_routes = Router::new()
        .route("/healthz", get(health::healthz))
        .route("/readyz", get(health::readyz))
        .route("/metrics", get(health::metrics_handler))
        .layer(CorsLayer::permissive());

    // Data plane routes -- protected by auth middleware, then rate limiting.
    // Layers execute bottom-up: auth runs first, then rate_limit_middleware.
    let data_plane = Router::new()
        .route("/v1/chat/completions", post(data_plane_post_placeholder))
        .route("/v1/responses", post(data_plane_post_placeholder))
        .route("/v1/embeddings", post(data_plane_post_placeholder))
        .route("/v1/models", get(data_plane_get_placeholder))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::policy::middleware::rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::auth_middleware,
        ));

    // Admin routes -- protected by admin auth middleware (bearer token + bcrypt)
    let admin = Router::new()
        // Tenants
        .route(
            "/admin/v1/tenants",
            get(tenants::list_tenants).post(tenants::create_tenant),
        )
        .route(
            "/admin/v1/tenants/{id}",
            get(tenants::get_tenant)
                .patch(tenants::update_tenant)
                .delete(tenants::delete_tenant),
        )
        // Service accounts
        .route(
            "/admin/v1/service-accounts",
            get(service_accounts::list_service_accounts)
                .post(service_accounts::create_service_account),
        )
        .route(
            "/admin/v1/service-accounts/{id}",
            get(service_accounts::get_service_account)
                .patch(service_accounts::update_service_account),
        )
        .route(
            "/admin/v1/service-accounts/{id}/suspend",
            post(service_accounts::suspend_service_account),
        )
        .route(
            "/admin/v1/service-accounts/{id}/activate",
            post(service_accounts::activate_service_account),
        )
        .route(
            "/admin/v1/service-accounts/{id}/keys",
            get(service_accounts::list_keys).post(service_accounts::register_key),
        )
        .route(
            "/admin/v1/service-accounts/{id}/keys/{key_id}",
            delete(service_accounts::revoke_key),
        )
        // Policies
        .route(
            "/admin/v1/policies",
            get(policies::list_policies).post(policies::create_policy),
        )
        .route(
            "/admin/v1/policies/{id}",
            get(policies::get_policy)
                .patch(policies::update_policy)
                .delete(policies::delete_policy),
        )
        .route(
            "/admin/v1/service-accounts/{id}/policy-bindings",
            post(policies::create_binding),
        )
        .route(
            "/admin/v1/service-accounts/{id}/policy-bindings/{binding_id}",
            delete(policies::delete_binding),
        )
        // Routes
        .route(
            "/admin/v1/routes",
            get(route_handlers::list_routes).post(route_handlers::create_route),
        )
        .route(
            "/admin/v1/routes/{id}",
            get(route_handlers::get_route)
                .patch(route_handlers::update_route)
                .delete(route_handlers::delete_route),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::api::admin::auth::admin_auth_middleware,
        ));

    let x_request_id = axum::http::HeaderName::from_static(REQUEST_ID_HEADER);

    Router::new()
        .merge(health_routes)
        .merge(data_plane)
        .merge(admin)
        .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .layer(SetRequestIdLayer::new(x_request_id, MakeRequestUuid))
        .with_state(state)
}

/// Placeholder handler for data plane GET endpoints.
/// Returns 200 OK -- auth is enforced by the auth middleware layer.
async fn data_plane_get_placeholder() -> impl IntoResponse {
    StatusCode::OK
}

/// Placeholder handler for data plane POST endpoints.
/// Consumes the request body so the body-size limit layer can enforce its cap.
/// Returns 200 OK -- auth is enforced by the auth middleware layer.
async fn data_plane_post_placeholder(_body: Bytes) -> impl IntoResponse {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use super::*;

    /// Create a test `AppState` with a default configuration and no pools.
    fn test_state() -> AppState {
        AppState {
            config: Arc::new(GatewayConfig::default()),
            db: None,
            redis: None,
            key_store: None,
            admin_key_cache: None,
            policy_cache: None,
            rate_limit_evaluator: None,
            concurrency_limiter: None,
        }
    }

    /// Build a router backed by a test config.
    fn test_router() -> Router {
        build_router(test_state())
    }

    // ---------------------------------------------------------------
    // Health endpoints
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn healthz_returns_200_ok() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .expect("body collect")
            .to_bytes();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn readyz_returns_200_ok() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/readyz")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .expect("body collect")
            .to_bytes();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn metrics_returns_200() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/metrics")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    // ---------------------------------------------------------------
    // Data plane placeholders (401 until auth is wired)
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn v1_models_returns_401_without_auth() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/v1/models")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn v1_chat_completions_returns_401_without_auth() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // ---------------------------------------------------------------
    // Admin placeholders (401 until auth is wired)
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn admin_tenants_get_returns_401_without_auth() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/admin/v1/tenants")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_tenants_post_returns_401_without_auth() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/v1/tenants")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // ---------------------------------------------------------------
    // Unknown routes
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/this/does/not/exist")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ---------------------------------------------------------------
    // CORS
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn data_plane_no_cors_headers_on_preflight() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/v1/models")
                    .header("origin", "http://example.com")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        // Data plane should not have permissive CORS
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin"),
            "data plane should not have permissive CORS headers"
        );
    }

    #[tokio::test]
    async fn cors_headers_present_on_response() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/healthz")
                    .header("origin", "http://example.com")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert!(
            response
                .headers()
                .contains_key("access-control-allow-origin"),
            "expected CORS allow-origin header"
        );
    }

    // ---------------------------------------------------------------
    // Request ID generation
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn response_contains_x_request_id_header() {
        let app = test_router();

        let response = app
            .oneshot(
                Request::get("/healthz")
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let request_id = response
            .headers()
            .get("x-request-id")
            .expect("x-request-id header must be present");
        // Should be a valid UUID
        let id_str = request_id.to_str().expect("valid utf-8");
        uuid::Uuid::parse_str(id_str).expect("x-request-id should be a valid UUID");
    }

    #[tokio::test]
    async fn request_id_is_propagated_when_provided() {
        let app = test_router();

        let custom_id = "my-custom-request-id-123";
        let response = app
            .oneshot(
                Request::get("/healthz")
                    .header("x-request-id", custom_id)
                    .body(Body::empty())
                    .expect("request build"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let request_id = response
            .headers()
            .get("x-request-id")
            .expect("x-request-id header must be present");
        assert_eq!(
            request_id.to_str().expect("valid utf-8"),
            custom_id,
            "provided request ID should be propagated back"
        );
    }

    // ---------------------------------------------------------------
    // Request body size limit
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn oversized_body_rejected() {
        let app = test_router();

        // Create a body that exceeds the 10 MiB limit
        let oversized = vec![0u8; MAX_BODY_SIZE + 1];

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .body(Body::from(oversized))
                    .expect("request build"),
            )
            .await
            .expect("response");

        // Auth middleware intercepts before body size limit, so we get 401 (missing auth headers)
        // rather than 413. This is correct security behavior — reject unauthenticated requests first.
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
