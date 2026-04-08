//! Admin API authentication middleware and role-based access control.
//!
//! Admin endpoints are protected by bearer token authentication. Each admin
//! API key is stored as a bcrypt hash in the `admin_api_keys` table. On each
//! request, the middleware extracts the bearer token, verifies it against all
//! active keys, and injects an [`AdminContext`] into request extensions.

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::GatewayError;
use crate::server::AppState;
use crate::storage::repositories::admin_api_keys::AdminApiKeyRepo;

// ---------------------------------------------------------------------------
// AdminRole
// ---------------------------------------------------------------------------

/// Role assigned to an admin API key, controlling access to admin operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdminRole {
    Admin,
    Viewer,
}

impl AdminRole {
    /// Parse a role string from the database into an `AdminRole`.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Auth` if the role string is not recognised.
    fn from_db_str(s: &str) -> Result<Self, GatewayError> {
        match s {
            "admin" => Ok(Self::Admin),
            "viewer" => Ok(Self::Viewer),
            _ => {
                tracing::warn!(role = %s, "unknown admin role in database");
                Err(GatewayError::auth(
                    "invalid_role",
                    "unrecognized admin role",
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AdminContext
// ---------------------------------------------------------------------------

/// Context injected into request extensions after successful admin authentication.
#[derive(Debug, Clone)]
pub struct AdminContext {
    pub admin_id: Uuid,
    pub admin_name: String,
    pub role: AdminRole,
}

// ---------------------------------------------------------------------------
// Bearer token extraction
// ---------------------------------------------------------------------------

/// Extract the bearer token from an `Authorization: Bearer <token>` header value.
///
/// # Errors
///
/// Returns `GatewayError::Auth` if the header is missing, not valid UTF-8,
/// or does not use the `Bearer` scheme.
fn extract_bearer_token(
    header_value: Option<&axum::http::HeaderValue>,
) -> Result<&str, GatewayError> {
    let value = header_value.ok_or_else(|| {
        GatewayError::auth("missing_authorization", "Authorization header is required")
    })?;

    let value_str = value.to_str().map_err(|_| {
        GatewayError::auth(
            "invalid_authorization",
            "Authorization header is not valid UTF-8",
        )
    })?;

    let token = value_str.strip_prefix("Bearer ").ok_or_else(|| {
        GatewayError::auth(
            "invalid_authorization",
            "Authorization header must use Bearer scheme",
        )
    })?;

    if token.is_empty() {
        return Err(GatewayError::auth(
            "missing_token",
            "Bearer token must not be empty",
        ));
    }

    Ok(token)
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// Admin authentication middleware.
///
/// Extracts a bearer token from the `Authorization` header, verifies it against
/// all active admin API keys using bcrypt, and injects [`AdminContext`] into
/// request extensions.
///
/// # Errors
///
/// Returns 401 if the token is missing, invalid, or does not match any active key.
pub async fn admin_auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, GatewayError> {
    let token = extract_bearer_token(request.headers().get(axum::http::header::AUTHORIZATION))?;

    let pool = state.require_db()?;

    // Try cache first, fall back to DB.
    // NOTE: The admin key cache has a ~30s TTL. Revoked keys remain valid until
    // the cache entry expires. When admin key create/revoke endpoints are added,
    // they MUST call `admin_key_cache.invalidate(()).await` to flush stale entries.
    let keys = if let Some(ref cache) = state.admin_key_cache {
        match cache
            .try_get_with((), async {
                AdminApiKeyRepo::list_active(pool)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
        {
            Ok(keys) => keys,
            Err(e) => {
                tracing::error!(error = %e, "failed to load admin API keys");
                return Err(GatewayError::auth(
                    "internal_error",
                    "failed to load admin keys",
                ));
            }
        }
    } else {
        AdminApiKeyRepo::list_active(pool).await.map_err(|e| {
            tracing::error!(error = %e, "failed to load admin API keys");
            GatewayError::auth("internal_error", "failed to load admin keys")
        })?
    };

    if keys.is_empty() {
        tracing::warn!("no active admin API keys configured");
        return Err(GatewayError::auth(
            "no_admin_keys",
            "no active admin API keys configured",
        ));
    }

    // Verify the provided token against ALL keys' bcrypt hashes concurrently
    // to prevent timing side-channels that leak which key slot matched.
    // bcrypt::verify is CPU-bound, so we use spawn_blocking.
    // All verifications run in parallel and complete before checking results.
    // TODO: For large key sets, add key_prefix column for O(1) lookup.
    let token_owned = token.to_owned();
    let mut join_set = tokio::task::JoinSet::new();
    for (idx, key) in keys.iter().enumerate() {
        let provided = token_owned.clone();
        let hash = key.key_hash.clone();
        join_set.spawn_blocking(move || (idx, bcrypt::verify(provided, &hash)));
    }

    // Drain all tasks before checking results — timing-safe, no orphaned tasks.
    let mut matched_idx = None;
    let mut auth_error: Option<GatewayError> = None;
    while let Some(result) = join_set.join_next().await {
        match result {
            Err(e) => {
                tracing::error!(error = %e, "bcrypt task panicked");
                if auth_error.is_none() {
                    auth_error = Some(GatewayError::auth(
                        "internal_error",
                        "authentication verification failed",
                    ));
                }
            }
            Ok((_idx, Err(e))) => {
                tracing::error!(error = %e, "bcrypt verify error");
                if auth_error.is_none() {
                    auth_error = Some(GatewayError::auth(
                        "internal_error",
                        "authentication verification failed",
                    ));
                }
            }
            Ok((idx, Ok(is_match))) => {
                if is_match && matched_idx.is_none() {
                    matched_idx = Some(idx);
                }
            }
        }
    }

    if let Some(err) = auth_error {
        return Err(err);
    }

    let key = matched_idx.and_then(|idx| keys.get(idx)).ok_or_else(|| {
        tracing::warn!("admin auth failed: no matching key");
        GatewayError::auth("invalid_api_key", "invalid admin API key")
    })?;

    let role = AdminRole::from_db_str(&key.role)?;
    let ctx = AdminContext {
        admin_id: key.id,
        admin_name: key.name.clone(),
        role,
    };

    // Fire-and-forget: update last_used_at
    let pool_clone = pool.clone();
    let key_id = key.id;
    tokio::spawn(async move {
        if let Err(e) = AdminApiKeyRepo::touch_last_used(&pool_clone, key_id).await {
            tracing::warn!(error = %e, key_id = %key_id, "failed to update admin key last_used_at");
        }
    });

    request.extensions_mut().insert(ctx);
    Ok(next.run(request).await)
}

// ---------------------------------------------------------------------------
// Role check helper
// ---------------------------------------------------------------------------

/// Reject the request if the admin is not in the Admin role.
///
/// # Errors
///
/// Returns `GatewayError::Policy` with code `insufficient_permissions` if the
/// admin has the Viewer role.
pub fn require_admin_role(ctx: &AdminContext) -> Result<(), GatewayError> {
    if ctx.role != AdminRole::Admin {
        return Err(GatewayError::policy(
            "insufficient_permissions",
            "this operation requires admin role",
        ));
    }
    Ok(())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    // -----------------------------------------------------------------------
    // AdminRole serialization
    // -----------------------------------------------------------------------

    #[test]
    fn admin_role_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&AdminRole::Admin).expect("serialize"),
            "\"admin\""
        );
        assert_eq!(
            serde_json::to_string(&AdminRole::Viewer).expect("serialize"),
            "\"viewer\""
        );
    }

    #[test]
    fn admin_role_deserializes_lowercase() {
        let role: AdminRole = serde_json::from_str("\"admin\"").expect("deserialize");
        assert_eq!(role, AdminRole::Admin);
        let role: AdminRole = serde_json::from_str("\"viewer\"").expect("deserialize");
        assert_eq!(role, AdminRole::Viewer);
    }

    #[test]
    fn admin_role_from_db_str_valid() {
        assert_eq!(
            AdminRole::from_db_str("admin").expect("should parse"),
            AdminRole::Admin
        );
        assert_eq!(
            AdminRole::from_db_str("viewer").expect("should parse"),
            AdminRole::Viewer
        );
    }

    #[test]
    fn admin_role_from_db_str_unknown() {
        let err = AdminRole::from_db_str("superadmin").unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // require_admin_role
    // -----------------------------------------------------------------------

    #[test]
    fn require_admin_role_allows_admin() {
        let ctx = AdminContext {
            admin_id: Uuid::new_v4(),
            admin_name: "test-admin".to_owned(),
            role: AdminRole::Admin,
        };
        assert!(require_admin_role(&ctx).is_ok());
    }

    #[test]
    fn require_admin_role_rejects_viewer() {
        let ctx = AdminContext {
            admin_id: Uuid::new_v4(),
            admin_name: "test-viewer".to_owned(),
            role: AdminRole::Viewer,
        };
        let err = require_admin_role(&ctx).unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::FORBIDDEN);
    }

    // -----------------------------------------------------------------------
    // extract_bearer_token
    // -----------------------------------------------------------------------

    #[test]
    fn extract_bearer_token_valid() {
        let hv = HeaderValue::from_static("Bearer my-secret-key");
        let token = extract_bearer_token(Some(&hv)).expect("should extract");
        assert_eq!(token, "my-secret-key");
    }

    #[test]
    fn extract_bearer_token_missing() {
        let err = extract_bearer_token(None).unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn extract_bearer_token_wrong_scheme() {
        let hv = HeaderValue::from_static("Basic dXNlcjpwYXNz");
        let err = extract_bearer_token(Some(&hv)).unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn extract_bearer_token_empty_token() {
        let hv = HeaderValue::from_static("Bearer ");
        let err = extract_bearer_token(Some(&hv)).unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // Integration tests (require a running PostgreSQL instance)
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn admin_auth_valid_key(pool: sqlx::PgPool) {
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use tower::ServiceExt as _;

        let test_key = "test-admin-key-12345";
        let hash = bcrypt::hash(test_key, 4).expect("bcrypt hash");

        sqlx::query(
            "INSERT INTO admin_api_keys (id, name, key_hash, role, is_active)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind("test-key")
        .bind(&hash)
        .bind("admin")
        .bind(true)
        .execute(&pool)
        .await
        .expect("insert admin key");

        let state = AppState {
            config: Arc::new(crate::config::GatewayConfig::default()),
            db: Some(pool),
            redis: None,
            key_store: None,
            admin_key_cache: None,
            policy_cache: None,
            rate_limit_evaluator: None,
            concurrency_limiter: None,
            route_cache: None,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                admin_auth_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("Authorization", format!("Bearer {test_key}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn admin_auth_invalid_key(pool: sqlx::PgPool) {
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use tower::ServiceExt as _;

        let test_key = "test-admin-key-12345";
        let hash = bcrypt::hash(test_key, 4).expect("bcrypt hash");

        sqlx::query(
            "INSERT INTO admin_api_keys (id, name, key_hash, role, is_active)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind("test-key")
        .bind(&hash)
        .bind("admin")
        .bind(true)
        .execute(&pool)
        .await
        .expect("insert admin key");

        let state = AppState {
            config: Arc::new(crate::config::GatewayConfig::default()),
            db: Some(pool),
            redis: None,
            key_store: None,
            admin_key_cache: None,
            policy_cache: None,
            rate_limit_evaluator: None,
            concurrency_limiter: None,
            route_cache: None,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                admin_auth_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("Authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn admin_auth_missing_header(pool: sqlx::PgPool) {
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use tower::ServiceExt as _;

        let state = AppState {
            config: Arc::new(crate::config::GatewayConfig::default()),
            db: Some(pool),
            redis: None,
            key_store: None,
            admin_key_cache: None,
            policy_cache: None,
            rate_limit_evaluator: None,
            concurrency_limiter: None,
            route_cache: None,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                admin_auth_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(Request::get("/test").body(Body::empty()).expect("request"))
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn admin_auth_inactive_key_rejected(pool: sqlx::PgPool) {
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use tower::ServiceExt as _;

        let test_key = "test-inactive-key";
        let hash = bcrypt::hash(test_key, 4).expect("bcrypt hash");

        // Insert an inactive key
        sqlx::query(
            "INSERT INTO admin_api_keys (id, name, key_hash, role, is_active)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind("inactive-key")
        .bind(&hash)
        .bind("admin")
        .bind(false)
        .execute(&pool)
        .await
        .expect("insert admin key");

        let state = AppState {
            config: Arc::new(crate::config::GatewayConfig::default()),
            db: Some(pool),
            redis: None,
            key_store: None,
            admin_key_cache: None,
            policy_cache: None,
            rate_limit_evaluator: None,
            concurrency_limiter: None,
            route_cache: None,
        };

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                admin_auth_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("Authorization", format!("Bearer {test_key}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        // Should be 401 because inactive keys are not loaded
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn admin_auth_injects_context(pool: sqlx::PgPool) {
        use axum::Router;
        use axum::routing::get;
        use std::sync::Arc;
        use tower::ServiceExt as _;

        // Handler that checks the AdminContext -- must be defined before use.
        async fn check_context(request: Request<Body>) -> axum::http::StatusCode {
            let ctx = request.extensions().get::<AdminContext>();
            match ctx {
                Some(c) if c.role == AdminRole::Viewer && c.admin_name == "context-test" => {
                    axum::http::StatusCode::OK
                }
                _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            }
        }

        let test_key = "test-context-key";
        let hash = bcrypt::hash(test_key, 4).expect("bcrypt hash");
        let key_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO admin_api_keys (id, name, key_hash, role, is_active)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(key_id)
        .bind("context-test")
        .bind(&hash)
        .bind("viewer")
        .bind(true)
        .execute(&pool)
        .await
        .expect("insert admin key");

        let state = AppState {
            config: Arc::new(crate::config::GatewayConfig::default()),
            db: Some(pool),
            redis: None,
            key_store: None,
            admin_key_cache: None,
            policy_cache: None,
            rate_limit_evaluator: None,
            concurrency_limiter: None,
            route_cache: None,
        };

        let app = Router::new()
            .route("/test", get(check_context))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                admin_auth_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::get("/test")
                    .header("Authorization", format!("Bearer {test_key}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
