//! Service account CRUD handlers for the admin API.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::canonical::compute_body_hash;
use crate::auth::verifier::parse_public_key;
use crate::error::GatewayError;
use crate::models::{CreateServiceAccount, Cursor, RegisterKey, UpdateServiceAccount};
use crate::server::AppState;
use crate::storage::repositories::keys::KeyRepo;
use crate::storage::repositories::service_accounts::ServiceAccountRepo;
use crate::types::{Environment, ServiceAccountId, ServiceAccountStatus, TenantId};

use super::{AdminContext, map_storage_error, require_admin_role, validate_slug};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateServiceAccountRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub environment: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateServiceAccountRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListServiceAccountsQuery {
    pub tenant_id: Uuid,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GetServiceAccountQuery {
    pub tenant_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct RegisterKeyRequest {
    pub key_id: String,
    pub public_key_pem: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response for key registration, excludes full PEM for security.
#[derive(Debug, Serialize)]
pub struct KeyRegistrationResponse {
    pub id: Uuid,
    pub key_id: String,
    pub fingerprint: String,
    pub algorithm: String,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Parse an environment string into the `Environment` enum.
fn parse_environment(env: &str) -> Result<Environment, GatewayError> {
    match env {
        "dev" => Ok(Environment::Dev),
        "staging" => Ok(Environment::Staging),
        "prod" => Ok(Environment::Prod),
        _ => Err(GatewayError::validation(
            "invalid_environment",
            format!("environment must be one of: dev, staging, prod; got: {env}"),
        )),
    }
}

/// Compute SHA-256 fingerprint of public key bytes as hex string.
fn compute_key_fingerprint(key_bytes: &[u8; 32]) -> String {
    compute_body_hash(key_bytes)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /admin/v1/service-accounts
///
/// # Errors
///
/// Returns `GatewayError` on validation failure, auth failure, or storage error.
pub async fn create_service_account(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Json(body): Json<CreateServiceAccountRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;
    validate_slug(&body.slug)?;
    let environment = parse_environment(&body.environment)?;

    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let input = CreateServiceAccount {
        id: ServiceAccountId::new(),
        tenant_id: TenantId::from_uuid(body.tenant_id),
        name: body.name,
        slug: body.slug,
        environment,
        description: body.description,
        status: ServiceAccountStatus::Active,
        default_policy_id: None,
    };

    let sa = repo.create(&input).await.map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let sa_tenant_id = sa.tenant_id;
    let target_id = sa.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(sa_tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "service_account.created".to_owned(),
                target_type: "service_account".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok((StatusCode::CREATED, Json(sa)))
}

/// GET /admin/v1/service-accounts
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_service_accounts(
    Extension(_admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Query(query): Query<ListServiceAccountsQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = query.limit.unwrap_or(50);

    let page = repo
        .list_by_tenant(TenantId::from_uuid(query.tenant_id), cursor.as_ref(), limit)
        .await
        .map_err(map_storage_error)?;

    Ok(Json(page))
}

/// GET /admin/v1/service-accounts/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or if the service account is not found.
pub async fn get_service_account(
    Extension(_admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetServiceAccountQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let sa = repo
        .get_by_id(
            TenantId::from_uuid(query.tenant_id),
            ServiceAccountId::from_uuid(id),
        )
        .await
        .map_err(map_storage_error)?;

    Ok(Json(sa))
}

/// PATCH /admin/v1/service-accounts/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn update_service_account(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetServiceAccountQuery>,
    Json(body): Json<UpdateServiceAccountRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let input = UpdateServiceAccount {
        name: body.name,
        description: body.description,
        ..Default::default()
    };

    let sa = repo
        .update(
            TenantId::from_uuid(query.tenant_id),
            ServiceAccountId::from_uuid(id),
            &input,
        )
        .await
        .map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let sa_tenant_id = sa.tenant_id;
    let target_id = sa.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(sa_tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "service_account.updated".to_owned(),
                target_type: "service_account".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok(Json(sa))
}

/// POST /admin/v1/service-accounts/:id/suspend
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn suspend_service_account(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetServiceAccountQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let sa = repo
        .suspend(
            TenantId::from_uuid(query.tenant_id),
            ServiceAccountId::from_uuid(id),
        )
        .await
        .map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let sa_tenant_id = sa.tenant_id;
    let target_id = sa.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(sa_tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "service_account.suspended".to_owned(),
                target_type: "service_account".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok(Json(sa))
}

/// POST /admin/v1/service-accounts/:id/activate
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn activate_service_account(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<GetServiceAccountQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let sa = repo
        .activate(
            TenantId::from_uuid(query.tenant_id),
            ServiceAccountId::from_uuid(id),
        )
        .await
        .map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let sa_tenant_id = sa.tenant_id;
    let target_id = sa.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: Some(sa_tenant_id),
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "service_account.activated".to_owned(),
                target_type: "service_account".to_owned(),
                target_id,
                metadata_json: serde_json::json!({}),
            })
            .await;
    });

    Ok(Json(sa))
}

/// POST /admin/v1/service-accounts/:id/keys
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, invalid PEM, or storage error.
pub async fn register_key(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<RegisterKeyRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    // Validate PEM and extract public key bytes
    let verifying_key = parse_public_key(&body.public_key_pem).map_err(|_| {
        GatewayError::validation(
            "invalid_public_key",
            "public_key_pem is not a valid Ed25519 public key in PEM format",
        )
    })?;

    let fingerprint = compute_key_fingerprint(verifying_key.as_bytes());

    let pool = state.require_db()?;
    let key_repo = KeyRepo::new(pool.clone());

    let input = RegisterKey {
        service_account_id: ServiceAccountId::from_uuid(id),
        key_id: body.key_id,
        algorithm: "ed25519".to_owned(),
        public_key_pem: body.public_key_pem,
        fingerprint,
        expires_at: body.expires_at,
    };

    let key = key_repo
        .register_key(&input)
        .await
        .map_err(map_storage_error)?;

    let response = KeyRegistrationResponse {
        id: key.id.0,
        key_id: key.key_id,
        fingerprint: key.fingerprint,
        algorithm: key.algorithm,
        status: format!("{:?}", key.status).to_lowercase(),
        expires_at: key.expires_at,
        created_at: key.created_at,
    };

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    let target_id = key.id.0.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: None,
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "key.registered".to_owned(),
                target_type: "key".to_owned(),
                target_id,
                metadata_json: serde_json::json!({"service_account_id": id.to_string()}),
            })
            .await;
    });

    Ok((StatusCode::CREATED, Json(response)))
}

/// DELETE /admin/v1/service-accounts/:id/keys/:key_id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure, not found, or storage error.
pub async fn revoke_key(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path((sa_id, key_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, GatewayError> {
    require_admin_role(&admin_ctx)?;

    let pool = state.require_db()?;
    let key_repo = KeyRepo::new(pool.clone());

    let key = key_repo
        .revoke_scoped(
            crate::types::KeyId::from_uuid(key_id),
            ServiceAccountId::from_uuid(sa_id),
        )
        .await
        .map_err(map_storage_error)?;

    let audit_pool = pool.clone();
    let actor = admin_ctx.admin_id.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(audit_pool);
        let _ = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id: None,
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: "key.revoked".to_owned(),
                target_type: "key".to_owned(),
                target_id: key_id.to_string(),
                metadata_json: serde_json::json!({"service_account_id": sa_id.to_string()}),
            })
            .await;
    });

    Ok(Json(key))
}

/// GET /admin/v1/service-accounts/:id/keys
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_keys(
    Extension(_admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let key_repo = KeyRepo::new(pool.clone());

    let keys = key_repo
        .list_by_service_account(ServiceAccountId::from_uuid(id))
        .await
        .map_err(map_storage_error)?;

    Ok(Json(keys))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Slug validation
    // -----------------------------------------------------------------------

    #[test]
    fn valid_slugs() {
        assert!(validate_slug("abc").is_ok());
        assert!(validate_slug("my-service-account").is_ok());
        assert!(validate_slug("sa-123").is_ok());
        assert!(validate_slug("a".repeat(63).as_str()).is_ok());
    }

    #[test]
    fn slug_too_short() {
        assert!(validate_slug("ab").is_err());
    }

    #[test]
    fn slug_too_long() {
        let long = "a".repeat(64);
        assert!(validate_slug(&long).is_err());
    }

    #[test]
    fn slug_must_start_with_letter() {
        assert!(validate_slug("1-abc").is_err());
        assert!(validate_slug("-abc").is_err());
    }

    #[test]
    fn slug_no_uppercase() {
        assert!(validate_slug("My-Slug").is_err());
    }

    #[test]
    fn slug_no_special_chars() {
        assert!(validate_slug("my_slug").is_err());
        assert!(validate_slug("my.slug").is_err());
    }

    // -----------------------------------------------------------------------
    // Environment parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_valid_environments() {
        assert_eq!(parse_environment("dev").expect("dev"), Environment::Dev);
        assert_eq!(
            parse_environment("staging").expect("staging"),
            Environment::Staging
        );
        assert_eq!(parse_environment("prod").expect("prod"), Environment::Prod);
    }

    #[test]
    fn parse_invalid_environment() {
        assert!(parse_environment("production").is_err());
        assert!(parse_environment("DEV").is_err());
        assert!(parse_environment("").is_err());
    }

    // -----------------------------------------------------------------------
    // Key fingerprint
    // -----------------------------------------------------------------------

    #[test]
    fn fingerprint_is_consistent() {
        let key_bytes = [0u8; 32];
        let fp1 = compute_key_fingerprint(&key_bytes);
        let fp2 = compute_key_fingerprint(&key_bytes);
        assert_eq!(fp1, fp2);
        // SHA-256 hex is 64 characters
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn fingerprint_differs_for_different_keys() {
        let key_a = [0u8; 32];
        let mut key_b = [0u8; 32];
        key_b[0] = 1;
        assert_ne!(
            compute_key_fingerprint(&key_a),
            compute_key_fingerprint(&key_b)
        );
    }

    // -----------------------------------------------------------------------
    // Request deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn create_request_deserializes() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "name": "My SA",
            "slug": "my-sa",
            "environment": "prod",
            "description": "A test service account"
        });
        let req: CreateServiceAccountRequest =
            serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.name, "My SA");
        assert_eq!(req.environment, "prod");
    }

    #[test]
    fn register_key_request_deserializes() {
        let json = serde_json::json!({
            "key_id": "key-123",
            "public_key_pem": "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEA...\n-----END PUBLIC KEY-----",
        });
        let req: RegisterKeyRequest = serde_json::from_value(json).expect("should deserialize");
        assert_eq!(req.key_id, "key-123");
        assert!(req.expires_at.is_none());
    }

    #[test]
    fn list_query_deserializes() {
        let json = serde_json::json!({
            "tenant_id": "00000000-0000-0000-0000-000000000001",
            "limit": 25
        });
        let query: ListServiceAccountsQuery =
            serde_json::from_value(json).expect("should deserialize");
        assert_eq!(query.limit, Some(25));
        assert!(query.cursor.is_none());
    }

    // -----------------------------------------------------------------------
    // Key registration response serialization
    // -----------------------------------------------------------------------

    #[test]
    fn key_registration_response_serializes() {
        let resp = KeyRegistrationResponse {
            id: Uuid::nil(),
            key_id: "key-1".to_owned(),
            fingerprint: "abc123".to_owned(),
            algorithm: "ed25519".to_owned(),
            status: "active".to_owned(),
            expires_at: None,
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&resp).expect("should serialize");
        assert_eq!(json["key_id"], "key-1");
        assert_eq!(json["algorithm"], "ed25519");
        assert!(json["expires_at"].is_null());
    }
}
