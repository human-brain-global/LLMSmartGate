//! Tenant CRUD endpoints for the admin API.
//!
//! All endpoints require an authenticated admin context (injected by middleware).
//! Mutating operations (POST, PATCH, DELETE) require `AdminRole::Admin`.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{AdminContext, map_storage_error, require_admin_role, validate_slug};
use crate::error::GatewayError;
use crate::models::{CreateTenant, Cursor, UpdateTenant};
use crate::server::AppState;
use crate::storage::repositories::tenants::TenantRepo;
use crate::types::{TenantId, TenantStatus};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Request body for creating a new tenant.
#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
    pub metadata: Option<serde_json::Value>,
}

/// Request body for updating an existing tenant.
#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    pub name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing tenants.
#[derive(Debug, Deserialize)]
pub struct ListTenantsQuery {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /admin/v1/tenants -- create a new tenant.
///
/// Requires `AdminRole::Admin`.
///
/// # Errors
///
/// Returns `GatewayError` on invalid slug, duplicate slug, or database failure.
pub async fn create_tenant(
    State(state): State<AppState>,
    Extension(admin_ctx): Extension<AdminContext>,
    Json(body): Json<CreateTenantRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;
    validate_slug(&body.slug)?;

    let pool = state.require_db()?;
    let repo = TenantRepo::new(pool.clone());

    let input = CreateTenant {
        id: TenantId::new(),
        name: body.name,
        slug: body.slug,
        status: TenantStatus::Active,
        metadata: body.metadata.unwrap_or(serde_json::json!({})),
    };

    let tenant = repo.create(&input).await.map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let tenant_id = tenant.id;
    let target_id = tenant.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "tenant.created".to_owned(),
                target_type: "tenant".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok((StatusCode::CREATED, Json(tenant)))
}

/// GET /admin/v1/tenants -- list tenants with cursor-based pagination.
///
/// # Errors
///
/// Returns `GatewayError` on database failure or invalid cursor.
pub async fn list_tenants(
    State(state): State<AppState>,
    Extension(_admin_ctx): Extension<AdminContext>,
    Query(query): Query<ListTenantsQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = TenantRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = query.limit.unwrap_or(50);

    let status = query
        .status
        .as_deref()
        .map(|s| {
            serde_json::from_value::<TenantStatus>(serde_json::Value::String(s.to_owned())).map_err(
                |_| {
                    GatewayError::validation(
                        "invalid_status",
                        format!("status must be one of: active, suspended, deleted; got: {s}"),
                    )
                },
            )
        })
        .transpose()?;

    let page = repo
        .list(cursor.as_ref(), limit, status)
        .await
        .map_err(map_storage_error)?;

    Ok(Json(page))
}

/// GET /admin/v1/tenants/{id} -- get a single tenant by ID.
///
/// # Errors
///
/// Returns `GatewayError` if the tenant is not found or on database failure.
pub async fn get_tenant(
    State(state): State<AppState>,
    Extension(_admin_ctx): Extension<AdminContext>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = TenantRepo::new(pool.clone());

    let tenant = repo
        .get_by_id(TenantId::from_uuid(id))
        .await
        .map_err(map_storage_error)?;

    Ok(Json(tenant))
}

/// PATCH /admin/v1/tenants/{id} -- update a tenant.
///
/// Requires `AdminRole::Admin`.
///
/// # Errors
///
/// Returns `GatewayError` if the tenant is not found, on permission failure,
/// or on database failure.
pub async fn update_tenant(
    State(state): State<AppState>,
    Extension(admin_ctx): Extension<AdminContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTenantRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = TenantRepo::new(pool.clone());

    let input = UpdateTenant {
        name: body.name,
        slug: None,
        status: None,
        metadata: body.metadata,
    };

    let tenant = repo
        .update(TenantId::from_uuid(id), &input)
        .await
        .map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let tenant_id = tenant.id;
    let target_id = tenant.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "tenant.updated".to_owned(),
                target_type: "tenant".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok(Json(tenant))
}

/// DELETE /admin/v1/tenants/{id} -- soft-delete a tenant.
///
/// Requires `AdminRole::Admin`.
///
/// # Errors
///
/// Returns `GatewayError` if the tenant is not found, on permission failure,
/// or on database failure.
pub async fn delete_tenant(
    State(state): State<AppState>,
    Extension(admin_ctx): Extension<AdminContext>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = TenantRepo::new(pool.clone());

    let tenant = repo
        .soft_delete(TenantId::from_uuid(id))
        .await
        .map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let tenant_id = tenant.id;
    let target_id = tenant.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "tenant.deleted".to_owned(),
                target_type: "tenant".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok(Json(tenant))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageError;

    // -----------------------------------------------------------------------
    // Slug validation
    // -----------------------------------------------------------------------

    #[test]
    fn valid_slugs() {
        assert!(validate_slug("my-tenant").is_ok());
        assert!(validate_slug("abc").is_ok());
        assert!(validate_slug("tenant-123").is_ok());
        assert!(validate_slug("a1b").is_ok());
    }

    #[test]
    fn valid_slug_max_length() {
        let slug = "a".repeat(63);
        assert!(validate_slug(&slug).is_ok());
    }

    #[test]
    fn valid_slug_min_length() {
        assert!(validate_slug("abc").is_ok());
    }

    #[test]
    fn invalid_slug_too_short() {
        assert!(validate_slug("ab").is_err());
        assert!(validate_slug("a").is_err());
        assert!(validate_slug("").is_err());
    }

    #[test]
    fn invalid_slug_too_long() {
        let slug = "a".repeat(64);
        assert!(validate_slug(&slug).is_err());
    }

    #[test]
    fn invalid_slug_uppercase() {
        assert!(validate_slug("MyTenant").is_err());
        assert!(validate_slug("ABC").is_err());
    }

    #[test]
    fn invalid_slug_special_chars() {
        assert!(validate_slug("my_tenant").is_err());
        assert!(validate_slug("my.tenant").is_err());
        assert!(validate_slug("my tenant").is_err());
    }

    #[test]
    fn invalid_slug_starts_with_hyphen() {
        assert!(validate_slug("-slug").is_err());
    }

    #[test]
    fn invalid_slug_ends_with_hyphen() {
        assert!(validate_slug("slug-").is_err());
    }

    #[test]
    fn invalid_slug_starts_and_ends_with_hyphen() {
        assert!(validate_slug("-slug-").is_err());
    }

    // -----------------------------------------------------------------------
    // Error mapping
    // -----------------------------------------------------------------------

    #[test]
    fn map_not_found_to_404() {
        let err = map_storage_error(StorageError::NotFound {
            entity: "tenant".to_owned(),
            field: "id".to_owned(),
            value: "abc".to_owned(),
        });
        match err {
            GatewayError::NotFound { code, message } => {
                assert_eq!(code, "not_found");
                assert!(message.contains("tenant"));
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn map_conflict_to_409() {
        let err = map_storage_error(StorageError::Conflict {
            entity: "tenants".to_owned(),
            detail: "slug already exists".to_owned(),
        });
        match err {
            GatewayError::Conflict { code, message } => {
                assert_eq!(code, "conflict");
                assert!(message.contains("duplicate"));
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn map_invalid_cursor_to_validation() {
        let err = map_storage_error(StorageError::InvalidCursor("bad".to_owned()));
        match err {
            GatewayError::Validation { code, .. } => {
                assert_eq!(code, "invalid_cursor");
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }
}
