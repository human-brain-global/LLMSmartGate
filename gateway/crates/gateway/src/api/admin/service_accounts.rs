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

use super::{
    AdminContext, emit_audit, map_storage_error, parse_enum_str, require_admin_role,
    validate_key_id, validate_limit, validate_name, validate_slug,
};

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

/// Response type for key listing -- omits the full public key PEM.
#[derive(Debug, Serialize)]
pub struct KeySummaryResponse {
    pub id: Uuid,
    pub key_id: String,
    pub fingerprint: String,
    pub algorithm: String,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<crate::models::ServiceAccountKey> for KeySummaryResponse {
    fn from(k: crate::models::ServiceAccountKey) -> Self {
        Self {
            id: k.id.0,
            key_id: k.key_id,
            fingerprint: k.fingerprint,
            algorithm: k.algorithm,
            status: k.status.to_string(),
            expires_at: k.expires_at,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
        }
    }
}

/// Parse an environment string into the `Environment` enum.
fn parse_environment(env: &str) -> Result<Environment, GatewayError> {
    parse_enum_str(env, "environment", "dev, staging, prod")
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
    validate_name(&body.name)?;
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

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %sa.id, action = "service_account.created", "admin operation");
    emit_audit(
        pool,
        Some(sa.tenant_id),
        admin_ctx.admin_id,
        "service_account.created",
        "service_account",
        sa.id.0.to_string(),
        serde_json::json!({}),
    );

    Ok((StatusCode::CREATED, Json(sa)))
}

/// GET /admin/v1/service-accounts
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_service_accounts(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Query(query): Query<ListServiceAccountsQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let repo = ServiceAccountRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = validate_limit(query.limit)?;

    let page = repo
        .list_by_tenant(TenantId::from_uuid(query.tenant_id), cursor.as_ref(), limit)
        .await
        .map_err(map_storage_error)?;

    tracing::debug!(admin_id = %admin_ctx.admin_id, tenant_id = %query.tenant_id, action = "service_account.list", "admin operation");
    Ok(Json(page))
}

/// GET /admin/v1/service-accounts/:id
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or if the service account is not found.
pub async fn get_service_account(
    Extension(admin_ctx): Extension<AdminContext>,
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

    tracing::debug!(admin_id = %admin_ctx.admin_id, service_account_id = %id, action = "service_account.get", "admin operation");
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
    if let Some(ref name) = body.name {
        validate_name(name)?;
    }

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

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %id, action = "service_account.updated", "admin operation");
    emit_audit(
        pool,
        Some(sa.tenant_id),
        admin_ctx.admin_id,
        "service_account.updated",
        "service_account",
        sa.id.0.to_string(),
        serde_json::json!({}),
    );

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

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %id, action = "service_account.suspended", "admin operation");
    emit_audit(
        pool,
        Some(sa.tenant_id),
        admin_ctx.admin_id,
        "service_account.suspended",
        "service_account",
        sa.id.0.to_string(),
        serde_json::json!({}),
    );

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

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %id, action = "service_account.activated", "admin operation");
    emit_audit(
        pool,
        Some(sa.tenant_id),
        admin_ctx.admin_id,
        "service_account.activated",
        "service_account",
        sa.id.0.to_string(),
        serde_json::json!({}),
    );

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
    validate_key_id(&body.key_id)?;

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
        status: key.status.to_string(),
        expires_at: key.expires_at,
        created_at: key.created_at,
    };

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %id, action = "key.registered", "admin operation");
    emit_audit(
        pool,
        None,
        admin_ctx.admin_id,
        "key.registered",
        "key",
        key.id.0.to_string(),
        serde_json::json!({"service_account_id": id.to_string()}),
    );

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

    // Invalidate the 3-tier key cache so the revoked key stops authenticating immediately.
    if let Some(ref key_store) = state.key_store {
        key_store.invalidate(&key.key_id).await;
    }

    tracing::info!(admin_id = %admin_ctx.admin_id, service_account_id = %sa_id, key_id = %key_id, action = "key.revoked", "admin operation");
    emit_audit(
        pool,
        None,
        admin_ctx.admin_id,
        "key.revoked",
        "key",
        key_id.to_string(),
        serde_json::json!({"service_account_id": sa_id.to_string()}),
    );

    Ok(Json(key))
}

/// Query parameters for listing keys.
#[derive(Debug, Deserialize)]
pub struct ListKeysQuery {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

/// GET /admin/v1/service-accounts/:id/keys
///
/// # Errors
///
/// Returns `GatewayError` on auth failure or storage error.
pub async fn list_keys(
    Extension(admin_ctx): Extension<AdminContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListKeysQuery>,
) -> Result<impl IntoResponse, GatewayError> {
    let pool = state.require_db()?;
    let key_repo = KeyRepo::new(pool.clone());

    let cursor = query.cursor.map(Cursor);
    let limit = validate_limit(query.limit)?;

    let page = key_repo
        .list_by_service_account(ServiceAccountId::from_uuid(id), cursor.as_ref(), limit)
        .await
        .map_err(map_storage_error)?;

    let response_page = crate::models::Page {
        items: page
            .items
            .into_iter()
            .map(KeySummaryResponse::from)
            .collect(),
        next_cursor: page.next_cursor,
        has_more: page.has_more,
    };

    tracing::debug!(admin_id = %admin_ctx.admin_id, service_account_id = %id, action = "key.list", "admin operation");
    Ok(Json(response_page))
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
    fn key_summary_response_omits_public_key_pem() {
        let resp = KeySummaryResponse {
            id: Uuid::new_v4(),
            key_id: "test-key-id".to_owned(),
            fingerprint: "abc123".to_owned(),
            algorithm: "ed25519".to_owned(),
            status: "active".to_owned(),
            expires_at: None,
            created_at: Utc::now(),
            last_used_at: None,
        };
        let json = serde_json::to_value(&resp).expect("serialization");
        assert!(
            json.get("public_key_pem").is_none(),
            "public_key_pem must not be in response"
        );
    }

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
