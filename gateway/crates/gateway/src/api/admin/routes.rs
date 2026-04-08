//! Provider route CRUD handlers for the admin API.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::models::{CreateRoute, Cursor, UpdateRoute};
use crate::server::AppState;
use crate::storage::repositories::routes::RouteRepo;
use crate::types::{RouteId, TenantId};

use super::{
    AdminContext, emit_audit, map_storage_error, parse_enum_str, require_admin_role,
    validate_limit, validate_model_identifier,
};

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateRouteRequest {
    pub tenant_id: Option<Uuid>,
    pub model_alias: String,
    pub provider: String,
    pub provider_model_name: String,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub timeout_ms: Option<i32>,
    pub max_retries: Option<i32>,
    pub retry_backoff_ms: Option<i32>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRouteRequest {
    pub model_alias: Option<String>,
    pub provider: Option<String>,
    pub provider_model_name: Option<String>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub timeout_ms: Option<i32>,
    pub max_retries: Option<i32>,
    pub retry_backoff_ms: Option<i32>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListRoutesQuery {
    pub tenant_id: Option<Uuid>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate the provider string against known providers.
fn validate_provider(provider: &str) -> Result<(), GatewayError> {
    parse_enum_str::<crate::types::Provider>(
        provider,
        "provider",
        "openai, anthropic, gemini, azure_openai, vllm",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /admin/v1/routes
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, validation failure, or storage error.
pub async fn create_route(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Json(body): Json<CreateRouteRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;
    validate_provider(&body.provider)?;

    validate_model_identifier(&body.model_alias, "model_alias")?;
    validate_model_identifier(&body.provider_model_name, "provider_model_name")?;

    let pool = state.require_db()?;
    let repo = RouteRepo::new(pool.clone());

    let input = CreateRoute {
        tenant_id: body.tenant_id.map(TenantId::from_uuid),
        model_alias: body.model_alias,
        provider: body.provider,
        provider_model_name: body.provider_model_name,
        priority: body.priority,
        enabled: body.enabled,
        timeout_ms: body.timeout_ms,
        max_retries: body.max_retries,
        retry_backoff_ms: body.retry_backoff_ms,
        config: body.config,
    };

    let route = repo.create(&input).await.map_err(map_storage_error)?;

    // Synchronous invalidation: ensures the response is not sent before the
    // cache is clean, preventing a stale-read race on this instance.
    if let Some(ref cache) = state.route_cache {
        cache.invalidate_all().await;
        tracing::debug!(route_id = %route.id, "route cache invalidated: route created");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, route_id = %route.id, action = "route.created", "admin operation");
    emit_audit(
        pool,
        route.tenant_id,
        admin_ctx.admin_id,
        "route.created",
        "route",
        route.id.0.to_string(),
        serde_json::json!({}),
    );

    Ok((StatusCode::CREATED, Json(route)))
}

/// GET /admin/v1/routes
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_routes(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Query(query): Query<ListRoutesQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = RouteRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = validate_limit(query.limit)?;

    // When tenant_id is provided, list tenant-scoped routes.
    // When absent, list global routes (tenant_id IS NULL).
    let page = match query.tenant_id.map(TenantId::from_uuid) {
        Some(tid) => repo
            .list_by_tenant(tid, cursor.as_ref(), limit)
            .await
            .map_err(map_storage_error)?,
        None => repo
            .list_global(cursor.as_ref(), limit)
            .await
            .map_err(map_storage_error)?,
    };

    tracing::debug!(admin_id = %admin_ctx.admin_id, action = "route.list", "admin operation");
    Ok(Json(page))
}

/// GET /admin/v1/routes/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or if the route is not found.
pub async fn get_route(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = RouteRepo::new(pool.clone());

    let route = repo
        .get_by_id(RouteId::from_uuid(id))
        .await
        .map_err(map_storage_error)?;

    tracing::debug!(admin_id = %admin_ctx.admin_id, route_id = %id, action = "route.get", "admin operation");
    Ok(Json(route))
}

/// PATCH /admin/v1/routes/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, validation failure, not found, or storage error.
pub async fn update_route(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRouteRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    // Validate fields if being updated
    if let Some(ref provider) = body.provider {
        validate_provider(provider)?;
    }
    if let Some(ref model_alias) = body.model_alias {
        validate_model_identifier(model_alias, "model_alias")?;
    }
    if let Some(ref provider_model_name) = body.provider_model_name {
        validate_model_identifier(provider_model_name, "provider_model_name")?;
    }

    let pool = state.require_db()?;
    let repo = RouteRepo::new(pool.clone());

    let input = UpdateRoute {
        model_alias: body.model_alias,
        provider: body.provider,
        provider_model_name: body.provider_model_name,
        priority: body.priority,
        enabled: body.enabled,
        timeout_ms: body.timeout_ms,
        max_retries: body.max_retries,
        retry_backoff_ms: body.retry_backoff_ms,
        config: body.config,
    };

    let route = repo
        .update(RouteId::from_uuid(id), &input)
        .await
        .map_err(map_storage_error)?;

    // Synchronous invalidation: ensures the response is not sent before the
    // cache is clean, preventing a stale-read race on this instance.
    if let Some(ref cache) = state.route_cache {
        cache.invalidate_all().await;
        tracing::debug!(route_id = %id, "route cache invalidated: route updated");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, route_id = %id, action = "route.updated", "admin operation");
    emit_audit(
        pool,
        route.tenant_id,
        admin_ctx.admin_id,
        "route.updated",
        "route",
        route.id.0.to_string(),
        serde_json::json!({}),
    );

    Ok(Json(route))
}

/// DELETE /admin/v1/routes/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn delete_route(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = RouteRepo::new(pool.clone());

    repo.delete(RouteId::from_uuid(id))
        .await
        .map_err(map_storage_error)?;

    // Synchronous invalidation: ensures the response is not sent before the
    // cache is clean, preventing a stale-read race on this instance.
    if let Some(ref cache) = state.route_cache {
        cache.invalidate_all().await;
        tracing::debug!(route_id = %id, "route cache invalidated: route deleted");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, route_id = %id, action = "route.deleted", "admin operation");
    emit_audit(
        pool,
        None,
        admin_ctx.admin_id,
        "route.deleted",
        "route",
        id.to_string(),
        serde_json::json!({}),
    );

    Ok(StatusCode::NO_CONTENT)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Provider validation
    // -----------------------------------------------------------------------

    #[test]
    fn valid_providers() {
        assert!(validate_provider("openai").is_ok());
        assert!(validate_provider("anthropic").is_ok());
        assert!(validate_provider("gemini").is_ok());
        assert!(validate_provider("azure_openai").is_ok());
        assert!(validate_provider("vllm").is_ok());
    }

    #[test]
    fn invalid_providers() {
        assert!(validate_provider("unknown").is_err());
        assert!(validate_provider("OpenAI").is_err());
        assert!(validate_provider("").is_err());
    }

    // -----------------------------------------------------------------------
    // Request deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn create_route_request_deserializes_minimal() {
        let json = serde_json::json!({
            "model_alias": "gpt-4",
            "provider": "openai",
            "provider_model_name": "gpt-4-turbo"
        });
        let req: CreateRouteRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.model_alias, "gpt-4");
        assert!(req.tenant_id.is_none());
        assert!(req.priority.is_none());
    }

    #[test]
    fn create_route_request_deserializes_full() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "model_alias": "gpt-4",
            "provider": "openai",
            "provider_model_name": "gpt-4-turbo",
            "priority": 50,
            "enabled": true,
            "timeout_ms": 30000,
            "max_retries": 3,
            "retry_backoff_ms": 500,
            "config": {"api_version": "2024-01"}
        });
        let req: CreateRouteRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.priority, Some(50));
        assert!(req.tenant_id.is_some());
    }

    #[test]
    fn update_route_request_deserializes_partial() {
        let json = serde_json::json!({
            "priority": 10,
            "enabled": false
        });
        let req: UpdateRouteRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.priority, Some(10));
        assert_eq!(req.enabled, Some(false));
        assert!(req.model_alias.is_none());
    }

    #[test]
    fn list_routes_query_no_tenant() {
        let json = serde_json::json!({});
        let query: ListRoutesQuery = serde_json::from_value(json).expect("should deserialize");
        assert!(query.tenant_id.is_none());
        assert!(query.cursor.is_none());
        assert!(query.limit.is_none());
    }

    #[test]
    fn list_routes_query_with_tenant() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "limit": 25
        });
        let query: ListRoutesQuery = serde_json::from_value(json).expect("should deserialize");
        assert!(query.tenant_id.is_some());
        assert_eq!(query.limit, Some(25));
    }
}
