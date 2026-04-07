//! Policy CRUD handlers and policy-binding management for the admin API.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::models::{CreatePolicy, Cursor, UpdatePolicy};
use crate::server::AppState;
use crate::storage::repositories::policies::PolicyRepo;
use crate::types::{PolicyId, ServiceAccountId, TenantId};

use super::{
    AdminContext, emit_audit, map_storage_error, require_admin_role, validate_limit,
    validate_models_json, validate_name, validate_positive_limit, validate_token_limit,
};

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub allowed_models_json: Option<serde_json::Value>,
    pub denied_models_json: Option<serde_json::Value>,
    pub max_input_tokens: Option<i32>,
    pub max_output_tokens: Option<i32>,
    pub allow_streaming: Option<bool>,
    pub allow_tools: Option<bool>,
    pub allow_files: Option<bool>,
    pub rpm_limit: Option<i32>,
    pub concurrency_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_models_json: Option<serde_json::Value>,
    pub denied_models_json: Option<serde_json::Value>,
    pub max_input_tokens: Option<i32>,
    pub max_output_tokens: Option<i32>,
    pub allow_streaming: Option<bool>,
    pub allow_tools: Option<bool>,
    pub allow_files: Option<bool>,
    pub rpm_limit: Option<i32>,
    pub concurrency_limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ListPoliciesQuery {
    pub tenant_id: Uuid,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GetPolicyQuery {
    pub tenant_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateBindingRequest {
    pub policy_id: Uuid,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /admin/v1/policies
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn create_policy(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Json(body): Json<CreatePolicyRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;
    validate_name(&body.name)?;
    if let Some(ref v) = body.allowed_models_json {
        validate_models_json(v, "allowed_models_json")?;
    }
    if let Some(ref v) = body.denied_models_json {
        validate_models_json(v, "denied_models_json")?;
    }
    if let Some(v) = body.max_input_tokens {
        validate_token_limit(v, "max_input_tokens")?;
    }
    if let Some(v) = body.max_output_tokens {
        validate_token_limit(v, "max_output_tokens")?;
    }
    if let Some(v) = body.rpm_limit {
        validate_positive_limit(v, "rpm_limit")?;
    }
    if let Some(v) = body.concurrency_limit {
        validate_positive_limit(v, "concurrency_limit")?;
    }

    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    let input = CreatePolicy {
        id: PolicyId::new(),
        tenant_id: TenantId::from_uuid(body.tenant_id),
        name: body.name,
        description: body.description,
        allowed_models_json: body.allowed_models_json.unwrap_or(serde_json::json!([])),
        denied_models_json: body.denied_models_json.unwrap_or(serde_json::json!([])),
        max_input_tokens: body.max_input_tokens,
        max_output_tokens: body.max_output_tokens,
        allow_streaming: body.allow_streaming.unwrap_or(true),
        allow_tools: body.allow_tools.unwrap_or(true),
        allow_files: body.allow_files.unwrap_or(true),
        rpm_limit: body.rpm_limit,
        concurrency_limit: body.concurrency_limit,
    };

    let policy = repo.create(&input).await.map_err(map_storage_error)?;

    // Invalidate policy cache -- new policy may be a default_policy_id target
    if let Some(ref cache) = state.policy_cache {
        cache.invalidate_all().await;
        tracing::debug!(policy_id = %policy.id, "policy cache invalidated: policy created");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, policy_id = %policy.id, action = "policy.created", "admin operation");
    emit_audit(
        pool,
        Some(policy.tenant_id),
        admin_ctx.admin_id,
        "policy.created",
        "policy",
        policy.id.0.to_string(),
        serde_json::json!({"name": policy.name, "tenant_id": policy.tenant_id.to_string()}),
    );

    Ok((StatusCode::CREATED, Json(policy)))
}

/// GET /admin/v1/policies
///
/// Both Admin and Viewer roles are permitted (read-only operation).
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_policies(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Query(query): Query<ListPoliciesQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = validate_limit(query.limit)?;

    let page = repo
        .list_by_tenant(TenantId::from_uuid(query.tenant_id), cursor.as_ref(), limit)
        .await
        .map_err(map_storage_error)?;

    tracing::debug!(admin_id = %admin_ctx.admin_id, tenant_id = %query.tenant_id, action = "policy.list", "admin operation");
    Ok(Json(page))
}

/// GET /admin/v1/policies/:id
///
/// Both Admin and Viewer roles are permitted (read-only operation).
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or if the policy is not found.
pub async fn get_policy(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetPolicyQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    let policy = repo
        .get_by_id(
            TenantId::from_uuid(query.tenant_id),
            PolicyId::from_uuid(id),
        )
        .await
        .map_err(map_storage_error)?;

    tracing::debug!(admin_id = %admin_ctx.admin_id, policy_id = %id, action = "policy.get", "admin operation");
    Ok(Json(policy))
}

/// PATCH /admin/v1/policies/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn update_policy(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetPolicyQuery>,
    Json(body): Json<UpdatePolicyRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;
    if let Some(ref name) = body.name {
        validate_name(name)?;
    }
    if let Some(ref v) = body.allowed_models_json {
        validate_models_json(v, "allowed_models_json")?;
    }
    if let Some(ref v) = body.denied_models_json {
        validate_models_json(v, "denied_models_json")?;
    }
    if let Some(v) = body.max_input_tokens {
        validate_token_limit(v, "max_input_tokens")?;
    }
    if let Some(v) = body.max_output_tokens {
        validate_token_limit(v, "max_output_tokens")?;
    }
    if let Some(v) = body.rpm_limit {
        validate_positive_limit(v, "rpm_limit")?;
    }
    if let Some(v) = body.concurrency_limit {
        validate_positive_limit(v, "concurrency_limit")?;
    }

    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    let input = UpdatePolicy {
        name: body.name,
        description: body.description,
        allowed_models_json: body.allowed_models_json,
        denied_models_json: body.denied_models_json,
        max_input_tokens: body.max_input_tokens,
        max_output_tokens: body.max_output_tokens,
        allow_streaming: body.allow_streaming,
        allow_tools: body.allow_tools,
        allow_files: body.allow_files,
        rpm_limit: body.rpm_limit,
        concurrency_limit: body.concurrency_limit,
    };

    let policy = repo
        .update(
            TenantId::from_uuid(query.tenant_id),
            PolicyId::from_uuid(id),
            &input,
        )
        .await
        .map_err(map_storage_error)?;

    // Invalidate policy cache -- we don't know which SAs use this policy
    if let Some(ref cache) = state.policy_cache {
        cache.invalidate_all().await;
        tracing::debug!(policy_id = %id, "policy cache invalidated: policy updated");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, policy_id = %id, action = "policy.updated", "admin operation");
    emit_audit(
        pool,
        Some(policy.tenant_id),
        admin_ctx.admin_id,
        "policy.updated",
        "policy",
        policy.id.0.to_string(),
        serde_json::json!({"name": policy.name}),
    );

    Ok(Json(policy))
}

/// DELETE /admin/v1/policies/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, conflict (has bindings), or storage error.
pub async fn delete_policy(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetPolicyQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    repo.delete(
        TenantId::from_uuid(query.tenant_id),
        PolicyId::from_uuid(id),
    )
    .await
    .map_err(|e| match &e {
        crate::storage::StorageError::ReferenceError { .. } => GatewayError::conflict(
            "conflict",
            "cannot delete policy: it still has active bindings",
        ),
        _ => map_storage_error(e),
    })?;

    // Invalidate policy cache -- policy was deleted
    if let Some(ref cache) = state.policy_cache {
        cache.invalidate_all().await;
        tracing::debug!(policy_id = %id, "policy cache invalidated: policy deleted");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, policy_id = %id, action = "policy.deleted", "admin operation");
    emit_audit(
        pool,
        Some(TenantId::from_uuid(query.tenant_id)),
        admin_ctx.admin_id,
        "policy.deleted",
        "policy",
        id.to_string(),
        serde_json::json!({"tenant_id": query.tenant_id.to_string()}),
    );

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Policy binding handlers
// ---------------------------------------------------------------------------

/// POST /admin/v1/service-accounts/:id/policy-bindings
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, conflict, or storage error.
pub async fn create_binding(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(sa_id): Path<Uuid>,
    Json(body): Json<CreateBindingRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    let binding = repo
        .create_binding(
            ServiceAccountId::from_uuid(sa_id),
            PolicyId::from_uuid(body.policy_id),
        )
        .await
        .map_err(map_storage_error)?;

    // Invalidate this SA's cached policy evaluation
    if let Some(ref cache) = state.policy_cache {
        cache.invalidate(&ServiceAccountId::from_uuid(sa_id)).await;
        tracing::debug!(service_account_id = %sa_id, "policy cache invalidated: binding created");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %sa_id, policy_id = %body.policy_id, action = "policy_binding.created", "admin operation");
    emit_audit(
        pool,
        None,
        admin_ctx.admin_id,
        "policy_binding.created",
        "policy_binding",
        format!("{sa_id}/{}", body.policy_id),
        serde_json::json!({"service_account_id": sa_id.to_string(), "policy_id": body.policy_id.to_string()}),
    );

    Ok((StatusCode::CREATED, Json(binding)))
}

/// DELETE /admin/v1/service-accounts/:id/policy-bindings/:binding_id
///
/// Note: `binding_id` here is the policy_id used in the binding, since
/// the repository identifies bindings by the `(service_account_id, policy_id)` pair.
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn delete_binding(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path((sa_id, policy_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    repo.delete_binding(
        ServiceAccountId::from_uuid(sa_id),
        PolicyId::from_uuid(policy_id),
    )
    .await
    .map_err(map_storage_error)?;

    // Invalidate this SA's cached policy evaluation
    if let Some(ref cache) = state.policy_cache {
        cache.invalidate(&ServiceAccountId::from_uuid(sa_id)).await;
        tracing::debug!(service_account_id = %sa_id, "policy cache invalidated: binding deleted");
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %sa_id, policy_id = %policy_id, action = "policy_binding.deleted", "admin operation");
    emit_audit(
        pool,
        None,
        admin_ctx.admin_id,
        "policy_binding.deleted",
        "policy_binding",
        format!("{sa_id}/{policy_id}"),
        serde_json::json!({"service_account_id": sa_id.to_string(), "policy_id": policy_id.to_string()}),
    );

    Ok(StatusCode::NO_CONTENT)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_policy_request_deserializes_minimal() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "name": "Default Policy",
        });
        let req: CreatePolicyRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.name, "Default Policy");
        assert!(req.description.is_none());
        assert!(req.allowed_models_json.is_none());
        assert!(req.allow_streaming.is_none());
    }

    #[test]
    fn create_policy_request_deserializes_full() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "name": "Strict Policy",
            "description": "Very restrictive",
            "allowed_models_json": ["gpt-4", "gpt-3.5-turbo"],
            "denied_models_json": ["gpt-4-vision"],
            "max_input_tokens": 4096,
            "max_output_tokens": 2048,
            "allow_streaming": false,
            "allow_tools": false,
            "allow_files": false,
            "rpm_limit": 60,
            "concurrency_limit": 5
        });
        let req: CreatePolicyRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.name, "Strict Policy");
        assert_eq!(req.allow_streaming, Some(false));
        assert_eq!(req.rpm_limit, Some(60));
    }

    #[test]
    fn update_policy_request_deserializes_partial() {
        let json = serde_json::json!({
            "name": "Updated Name",
            "max_input_tokens": 8192
        });
        let req: UpdatePolicyRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.name.as_deref(), Some("Updated Name"));
        assert_eq!(req.max_input_tokens, Some(8192));
        assert!(req.description.is_none());
        assert!(req.allow_streaming.is_none());
    }

    #[test]
    fn create_binding_request_deserializes() {
        let json = serde_json::json!({
            "policy_id": "00000000-0000-0000-0000-000000000002"
        });
        let req: CreateBindingRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(
            req.policy_id,
            Uuid::parse_str("00000000-0000-0000-0000-000000000002").expect("uuid")
        );
    }

    #[test]
    fn list_policies_query_deserializes() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "limit": 10,
            "cursor": "abc123"
        });
        let query: ListPoliciesQuery = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(query.limit, Some(10));
        assert_eq!(query.cursor.as_deref(), Some("abc123"));
    }
}
