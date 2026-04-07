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

use super::{AdminContext, map_storage_error, require_admin_role};

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

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let policy_tenant_id = policy.tenant_id;
    let target_id = policy.id.0.to_string();
    tokio::spawn(async move {
        let audit_repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = audit_repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(policy_tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "policy.created".to_owned(),
                target_type: "policy".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok((StatusCode::CREATED, Json(policy)))
}

/// GET /admin/v1/policies
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_policies(
    Extension(_admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Query(query): Query<ListPoliciesQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = PolicyRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = query.limit.unwrap_or(50);

    let page = repo
        .list_by_tenant(TenantId::from_uuid(query.tenant_id), cursor.as_ref(), limit)
        .await
        .map_err(map_storage_error)?;

    Ok(Json(page))
}

/// GET /admin/v1/policies/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or if the policy is not found.
pub async fn get_policy(
    Extension(_admin_ctx): Extension<AdminContext>,
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

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let policy_tenant_id = policy.tenant_id;
    let target_id = policy.id.0.to_string();
    tokio::spawn(async move {
        let audit_repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = audit_repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(policy_tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "policy.updated".to_owned(),
                target_type: "policy".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

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

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let target_id = id.to_string();
    let tenant_id = TenantId::from_uuid(query.tenant_id);
    tokio::spawn(async move {
        let audit_repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = audit_repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "policy.deleted".to_owned(),
                target_type: "policy".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

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

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let target_id = binding.id.to_string();
    let policy_id_str = body.policy_id.to_string();
    let sa_id_str = sa_id.to_string();
    tokio::spawn(async move {
        let audit_repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = audit_repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: None,
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "policy_binding.created".to_owned(),
                target_type: "policy_binding".to_owned(),
                target_id,
                metadata_json: serde_json::json!({"service_account_id": sa_id_str, "policy_id": policy_id_str}),
            })
            .await;
    });

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

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    tokio::spawn(async move {
        let audit_repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = audit_repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: None,
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "policy_binding.deleted".to_owned(),
                target_type: "policy_binding".to_owned(),
                target_id: format!("{sa_id}/{policy_id}"),
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

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
