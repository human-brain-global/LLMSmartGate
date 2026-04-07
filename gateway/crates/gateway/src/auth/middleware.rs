//! Auth middleware for data plane routes.
//!
//! Extracts Ed25519 signed request headers, verifies the signature against
//! the registered public key, checks nonce replay, and injects [`AuthContext`]
//! into request extensions for downstream handlers.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Request};
use axum::middleware::Next;
use axum::response::Response;
use http_body_util::BodyExt as _;

use crate::auth::canonical;
use crate::auth::context::AuthContext;
use crate::auth::key_store::KeyStore;
use crate::auth::nonce as nonce_mod;
use crate::auth::verifier;
use crate::error::GatewayError;
use crate::models::{ServiceAccount, ServiceAccountKey};
use crate::server::AppState;
use crate::storage::redis::RedisClient;
use crate::storage::repositories::keys::KeyRepo;
use crate::storage::repositories::service_accounts::ServiceAccountRepo;
use crate::types::{KeyStatus, ServiceAccountStatus};

// ---------------------------------------------------------------------------
// Header extraction
// ---------------------------------------------------------------------------

/// The 6 required auth headers parsed from the request.
struct AuthHeaders {
    sa_id_str: String,
    key_id: String,
    timestamp: String,
    nonce: String,
    body_hash: String,
    signature: String,
}

/// Extract and validate all required auth headers from the request.
fn extract_auth_headers(headers: &HeaderMap) -> Result<AuthHeaders, GatewayError> {
    Ok(AuthHeaders {
        sa_id_str: extract_header(headers, "x-service-account-id")?,
        key_id: extract_header(headers, "x-key-id")?,
        timestamp: extract_header(headers, "x-timestamp")?,
        nonce: extract_header(headers, "x-nonce")?,
        body_hash: extract_header(headers, "x-body-sha256")?,
        signature: extract_header(headers, "x-signature")?,
    })
}

/// Extract a single required header value as a UTF-8 string.
fn extract_header(headers: &HeaderMap, name: &str) -> Result<String, GatewayError> {
    headers
        .get(name)
        .ok_or_else(|| {
            GatewayError::auth(
                "missing_auth_header",
                format!("missing required header: {name}"),
            )
        })?
        .to_str()
        .map(ToOwned::to_owned)
        .map_err(|_| {
            GatewayError::auth(
                "invalid_header",
                format!("header '{name}' contains non-ASCII characters"),
            )
        })
}

// ---------------------------------------------------------------------------
// Key and service account verification
// ---------------------------------------------------------------------------

/// Look up a key via the 3-tier cache, verify its status and expiry.
async fn verify_key(
    key_store: &KeyStore,
    key_id: &str,
) -> Result<Arc<ServiceAccountKey>, GatewayError> {
    let key = key_store.get_by_key_id(key_id).await?;

    match key.status {
        KeyStatus::Revoked => {
            tracing::warn!(key_id = %key_id, "auth: revoked key used");
            return Err(GatewayError::auth("key_revoked", "this key has been revoked"));
        }
        KeyStatus::Expired => {
            tracing::warn!(key_id = %key_id, "auth: expired key used");
            return Err(GatewayError::auth("key_expired", "this key has expired"));
        }
        KeyStatus::Active | KeyStatus::Rotating => {}
    }

    if let Some(expires_at) = key.expires_at {
        if expires_at < chrono::Utc::now() {
            tracing::warn!(key_id = %key_id, expires_at = %expires_at, "auth: key past expiry date");
            return Err(GatewayError::auth("key_expired", "this key has expired"));
        }
    }

    Ok(key)
}

/// Look up the service account (unscoped) and verify it is active.
async fn verify_service_account(
    sa_repo: &ServiceAccountRepo,
    key: &ServiceAccountKey,
) -> Result<ServiceAccount, GatewayError> {
    let sa = sa_repo
        .get_by_id_unscoped(key.service_account_id)
        .await
        .map_err(|e| {
            tracing::warn!(service_account_id = %key.service_account_id, error = %e, "auth: service account lookup failed");
            GatewayError::auth("unknown_service_account", "service account not found")
        })?;

    if sa.status != ServiceAccountStatus::Active {
        tracing::warn!(
            service_account_id = %sa.id,
            status = %sa.status,
            "auth: inactive service account"
        );
        return Err(GatewayError::auth(
            "service_account_suspended",
            format!("service account '{}' is {}", sa.name, sa.status),
        ));
    }

    Ok(sa)
}

// ---------------------------------------------------------------------------
// Middleware entry point
// ---------------------------------------------------------------------------

/// Auth middleware for data plane routes.
///
/// Designed for use with `axum::middleware::from_fn_with_state`. Performs the
/// full Ed25519 signature verification pipeline and injects [`AuthContext`].
///
/// # Errors
///
/// Returns `GatewayError::Auth` (401) for any authentication failure.
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, GatewayError> {
    // 1. Extract required auth headers
    let h = extract_auth_headers(request.headers())?;

    // 2. Read body bytes for hash verification
    let (parts, body) = request.into_parts();
    let body_bytes: bytes::Bytes = body
        .collect()
        .await
        .map_err(|e| GatewayError::auth("body_read_error", format!("failed to read body: {e}")))?
        .to_bytes();

    // 3. Verify body hash + timestamp + nonce format
    canonical::verify_body_hash(&body_bytes, &h.body_hash)?;
    nonce_mod::validate_timestamp(&h.timestamp, state.config.auth.timestamp_skew_secs)?;
    nonce_mod::validate_nonce_length(&h.nonce)?;

    // 4. Look up key (3-tier cache) and verify status
    let key = verify_key(state.require_key_store()?, &h.key_id).await?;

    // 5. Look up service account and verify status
    let db = state.require_db()?;
    let sa = verify_service_account(&ServiceAccountRepo::new(db.clone()), &key).await?;

    // 6. Verify Ed25519 signature
    let public_key = verifier::parse_public_key(&key.public_key_pem)?;
    let path = parts
        .uri
        .path_and_query()
        .map_or_else(|| parts.uri.path().to_owned(), ToString::to_string);
    verifier::verify_signature(
        &public_key,
        &canonical::build_canonical_string(parts.method.as_str(), &path, &h.timestamp, &h.nonce, &h.body_hash),
        &h.signature,
    )?;

    // 7. Check nonce replay (Redis)
    let redis_client = RedisClient::new(state.require_redis()?.clone());
    nonce_mod::check_nonce(&redis_client, &h.sa_id_str, &h.nonce, state.config.auth.timestamp_skew_secs).await?;

    // 8. Fire-and-forget: update last_used_at
    let key_id_for_update = key.id;
    let db_for_update = db.clone();
    tokio::spawn(async move {
        let repo = KeyRepo::new(db_for_update);
        if let Err(e) = repo.update_last_used(key_id_for_update).await {
            tracing::warn!(error = %e, "failed to update key last_used_at");
        }
    });

    // 9. Build AuthContext and pass to next handler
    let auth_ctx = AuthContext {
        tenant_id: sa.tenant_id,
        service_account_id: sa.id,
        key_id: h.key_id,
        service_account_name: sa.name,
        environment: sa.environment,
        default_policy_id: sa.default_policy_id,
    };

    let mut request = Request::from_parts(parts, Body::from(body_bytes));
    request.extensions_mut().insert(auth_ctx);
    Ok(next.run(request).await)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn extract_header_present() {
        let mut headers = HeaderMap::new();
        headers.insert("x-key-id", HeaderValue::from_static("my-key"));
        let result = extract_header(&headers, "x-key-id");
        assert!(result.is_ok());
        assert_eq!(result.expect("should be ok"), "my-key");
    }

    #[test]
    fn extract_header_missing() {
        let headers = HeaderMap::new();
        let result = extract_header(&headers, "x-key-id");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status_code(), axum::http::StatusCode::UNAUTHORIZED);
        let msg = err.to_string();
        assert!(msg.contains("x-key-id"), "error should mention header name: {msg}");
    }

    #[test]
    fn extract_header_non_ascii() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-key-id",
            HeaderValue::from_bytes(b"value\xff").expect("raw bytes"),
        );
        let result = extract_header(&headers, "x-key-id");
        assert!(result.is_err());
    }

    #[test]
    fn extract_header_empty_value() {
        let mut headers = HeaderMap::new();
        headers.insert("x-key-id", HeaderValue::from_static(""));
        let result = extract_header(&headers, "x-key-id");
        assert!(result.is_ok());
    }

    #[test]
    fn extract_all_headers_succeeds() {
        let mut headers = HeaderMap::new();
        headers.insert("x-service-account-id", HeaderValue::from_static("sa-1"));
        headers.insert("x-key-id", HeaderValue::from_static("key-1"));
        headers.insert("x-timestamp", HeaderValue::from_static("123"));
        headers.insert("x-nonce", HeaderValue::from_static("nonce-1234567890"));
        headers.insert("x-body-sha256", HeaderValue::from_static("hash"));
        headers.insert("x-signature", HeaderValue::from_static("sig"));

        let result = extract_auth_headers(&headers);
        assert!(result.is_ok());
        let ah = result.expect("should parse");
        assert_eq!(ah.sa_id_str, "sa-1");
        assert_eq!(ah.key_id, "key-1");
    }

    #[test]
    fn extract_all_headers_missing_one_fails() {
        let mut headers = HeaderMap::new();
        headers.insert("x-service-account-id", HeaderValue::from_static("sa-1"));
        let result = extract_auth_headers(&headers);
        assert!(result.is_err());
    }
}
