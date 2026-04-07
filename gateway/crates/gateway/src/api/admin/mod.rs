//! Admin control plane API -- tenant, service account, policy, and route management.

pub mod auth;
pub mod policies;
pub mod routes;
pub mod service_accounts;
pub mod tenants;

// Re-export auth types so handlers can use `super::AdminContext` etc.
pub(crate) use auth::{AdminContext, require_admin_role};

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::storage::StorageError;
use crate::types::TenantId;

/// Parse a string into a serde-deserializable enum, returning a validation
/// error with the field name and valid values on failure.
pub(crate) fn parse_enum_str<T: serde::de::DeserializeOwned>(
    s: &str,
    field: &str,
    valid_values: &str,
) -> Result<T, GatewayError> {
    serde_json::from_value::<T>(serde_json::Value::String(s.to_owned())).map_err(|_| {
        GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must be one of: {valid_values}; got: {s}"),
        )
    })
}

/// Map storage errors to gateway errors with appropriate HTTP semantics.
pub(crate) fn map_storage_error(err: StorageError) -> GatewayError {
    match err {
        StorageError::NotFound {
            entity,
            field,
            value,
        } => GatewayError::not_found(
            "not_found",
            format!("{entity} not found by {field} = {value}"),
        ),
        StorageError::Conflict { entity, detail } => {
            GatewayError::conflict("conflict", format!("duplicate {entity}: {detail}"))
        }
        StorageError::ReferenceError { detail } => {
            GatewayError::conflict("reference_error", detail)
        }
        StorageError::InvalidCursor(detail) => GatewayError::validation("invalid_cursor", detail),
        other => GatewayError::storage("database_error", other.to_string()),
    }
}

/// Validate that a pagination limit is within the allowed range [1, 200].
///
/// Returns the validated limit, defaulting to 50 if `None`.
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the limit is outside [1, 200].
pub(crate) fn validate_limit(limit: Option<i64>) -> Result<i64, GatewayError> {
    let limit = limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(GatewayError::validation(
            "invalid_limit",
            format!("limit must be between 1 and 200, got {limit}"),
        ));
    }
    Ok(limit)
}

/// Fire-and-forget audit event emission.
///
/// Spawns a background task to insert an audit event. Failures are logged at
/// WARN level but do not affect the caller.
pub(crate) fn emit_audit(
    pool: &PgPool,
    tenant_id: Option<TenantId>,
    admin_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: String,
    metadata: serde_json::Value,
) {
    let pool = pool.clone();
    let action = action.to_owned();
    let target_type = target_type.to_owned();
    let actor = admin_id.to_string();
    tokio::spawn(async move {
        let repo = crate::storage::repositories::audit::AuditRepo::new(pool);
        if let Err(e) = repo
            .insert(&crate::storage::repositories::audit::CreateAuditEvent {
                tenant_id,
                actor_type: "admin_user".to_owned(),
                actor_id: actor,
                action: action.clone(),
                target_type: target_type.clone(),
                target_id: target_id.clone(),
                metadata_json: metadata,
            })
            .await
        {
            tracing::warn!(
                error = %e,
                action = %action,
                target_type = %target_type,
                target_id = %target_id,
                "audit event insert failed"
            );
        }
    });
}

/// Validate that a resource name is non-empty, within length limits, and
/// contains no control characters.
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the name is empty, exceeds 128
/// characters, or contains control characters (NUL, newlines, etc.).
pub(crate) fn validate_name(name: &str) -> Result<(), GatewayError> {
    if name.is_empty() {
        return Err(GatewayError::validation(
            "invalid_name",
            "name must not be empty",
        ));
    }
    if name.len() > 128 {
        return Err(GatewayError::validation(
            "invalid_name",
            format!("name must be 128 characters or fewer, got {}", name.len()),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(GatewayError::validation(
            "invalid_name",
            "name must not contain control characters",
        ));
    }
    Ok(())
}

/// Validate that a model identifier (model_alias or provider_model_name)
/// conforms to a safe character set for use in cache keys and routing.
///
/// - 1-128 characters
/// - Alphanumeric, hyphens, underscores, dots, colons, and forward slashes only
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the identifier is invalid.
pub(crate) fn validate_model_identifier(value: &str, field: &str) -> Result<(), GatewayError> {
    if value.is_empty() {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must not be empty"),
        ));
    }
    if value.len() > 128 {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!(
                "{field} must be 128 characters or fewer, got {}",
                value.len()
            ),
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/'))
    {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!(
                "{field} must contain only alphanumeric characters, hyphens, underscores, dots, colons, and slashes"
            ),
        ));
    }
    Ok(())
}

/// Validate that a key ID conforms to the required format:
/// - 1-128 characters
/// - Alphanumeric, hyphens, and underscores only
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the key ID is invalid.
pub(crate) fn validate_key_id(key_id: &str) -> Result<(), GatewayError> {
    if key_id.is_empty() {
        return Err(GatewayError::validation(
            "invalid_key_id",
            "key_id must not be empty",
        ));
    }
    if key_id.len() > 128 {
        return Err(GatewayError::validation(
            "invalid_key_id",
            format!(
                "key_id must be 128 characters or fewer, got {}",
                key_id.len()
            ),
        ));
    }
    if !key_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(GatewayError::validation(
            "invalid_key_id",
            "key_id must contain only alphanumeric characters, hyphens, and underscores",
        ));
    }
    Ok(())
}

/// Validate that a slug conforms to the required format:
/// - 3-63 characters
/// - Lowercase alphanumeric and hyphens only
/// - Must start with a lowercase letter
/// - Must not end with a hyphen
pub(crate) fn validate_slug(slug: &str) -> Result<(), GatewayError> {
    let len = slug.len();
    if !(3..=63).contains(&len) {
        return Err(GatewayError::validation(
            "invalid_slug",
            format!("slug must be 3-63 characters, got {len}"),
        ));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(GatewayError::validation(
            "invalid_slug",
            "slug must contain only lowercase letters, digits, and hyphens",
        ));
    }
    if !slug.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err(GatewayError::validation(
            "invalid_slug",
            "slug must start with a lowercase letter",
        ));
    }
    if slug.ends_with('-') {
        return Err(GatewayError::validation(
            "invalid_slug",
            "slug must not end with a hyphen",
        ));
    }
    Ok(())
}

/// Validate that a JSON value is an array of strings (for model allowlists/denylists).
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the value is not a JSON array of strings.
pub(crate) fn validate_models_json(
    value: &serde_json::Value,
    field: &str,
) -> Result<(), GatewayError> {
    let arr = value.as_array().ok_or_else(|| {
        GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must be a JSON array of strings"),
        )
    })?;
    for (i, item) in arr.iter().enumerate() {
        let s = item.as_str().ok_or_else(|| {
            GatewayError::validation(
                format!("invalid_{field}"),
                format!("{field}[{i}] must be a string"),
            )
        })?;
        // Strip at most one trailing glob wildcard, then validate the base.
        // validate_model_identifier rejects '*' via its charset check, so
        // interior wildcards (e.g. "gpt-4*-turbo") and double wildcards
        // ("**") are caught by that call.
        let base = s.strip_suffix('*').unwrap_or(s);
        if !base.is_empty() {
            validate_model_identifier(base, &format!("{field}[{i}]"))?;
        }
    }
    Ok(())
}

/// Maximum allowed token limit (1 million tokens).
const MAX_TOKEN_LIMIT: i32 = 1_000_000;

/// Validate that a token limit is within range (1..=1_000_000).
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the value is zero, negative, or
/// exceeds the maximum allowed limit.
pub(crate) fn validate_token_limit(value: i32, field: &str) -> Result<(), GatewayError> {
    if value <= 0 {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must be a positive integer, got {value}"),
        ));
    }
    if value > MAX_TOKEN_LIMIT {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must not exceed {MAX_TOKEN_LIMIT}, got {value}"),
        ));
    }
    Ok(())
}

/// Maximum allowed RPM limit.
const MAX_RPM_LIMIT: i32 = 100_000;
/// Maximum allowed concurrency limit.
const MAX_CONCURRENCY_LIMIT: i32 = 10_000;

/// Validate that a rate/concurrency limit is within a sensible range.
///
/// # Errors
///
/// Returns `GatewayError::Validation` if the value is zero, negative, or
/// exceeds the field-specific maximum.
pub(crate) fn validate_positive_limit(value: i32, field: &str) -> Result<(), GatewayError> {
    if value <= 0 {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must be a positive integer, got {value}"),
        ));
    }
    let max = if field.contains("concurrency") {
        MAX_CONCURRENCY_LIMIT
    } else {
        MAX_RPM_LIMIT
    };
    if value > max {
        return Err(GatewayError::validation(
            format!("invalid_{field}"),
            format!("{field} must not exceed {max}, got {value}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // validate_name
    // -----------------------------------------------------------------------

    #[test]
    fn validate_name_accepts_valid() {
        assert!(validate_name("Acme Corp").is_ok());
        assert!(validate_name("a").is_ok());
        assert!(validate_name(&"x".repeat(128)).is_ok());
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_name("").is_err());
    }

    #[test]
    fn validate_name_rejects_too_long() {
        assert!(validate_name(&"x".repeat(129)).is_err());
    }

    #[test]
    fn validate_name_rejects_control_characters() {
        assert!(validate_name("name\x00with_null").is_err());
        assert!(validate_name("name\nwith_newline").is_err());
        assert!(validate_name("name\twith_tab").is_err());
    }

    // -----------------------------------------------------------------------
    // validate_model_identifier
    // -----------------------------------------------------------------------

    #[test]
    fn validate_model_identifier_accepts_valid() {
        assert!(validate_model_identifier("gpt-4-turbo", "model_alias").is_ok());
        assert!(validate_model_identifier("claude-3.5-sonnet", "model_alias").is_ok());
        assert!(validate_model_identifier("meta/llama-3:8b", "model_alias").is_ok());
        assert!(validate_model_identifier("gpt_4", "model_alias").is_ok());
        assert!(validate_model_identifier(&"m".repeat(128), "model_alias").is_ok());
    }

    #[test]
    fn validate_model_identifier_rejects_empty() {
        assert!(validate_model_identifier("", "model_alias").is_err());
    }

    #[test]
    fn validate_model_identifier_rejects_too_long() {
        assert!(validate_model_identifier(&"m".repeat(129), "model_alias").is_err());
    }

    #[test]
    fn validate_model_identifier_rejects_spaces() {
        assert!(validate_model_identifier("gpt 4 turbo", "model_alias").is_err());
    }

    #[test]
    fn validate_model_identifier_rejects_special_chars() {
        assert!(validate_model_identifier("gpt-4@turbo", "model_alias").is_err());
        assert!(validate_model_identifier("model#1", "model_alias").is_err());
        assert!(validate_model_identifier("model\nnewline", "model_alias").is_err());
    }

    // -----------------------------------------------------------------------
    // validate_key_id
    // -----------------------------------------------------------------------

    #[test]
    fn validate_key_id_accepts_valid() {
        assert!(validate_key_id("key-123").is_ok());
        assert!(validate_key_id("my_key_id").is_ok());
        assert!(validate_key_id("abc123").is_ok());
        assert!(validate_key_id(&"k".repeat(128)).is_ok());
    }

    #[test]
    fn validate_key_id_rejects_empty() {
        assert!(validate_key_id("").is_err());
    }

    #[test]
    fn validate_key_id_rejects_too_long() {
        assert!(validate_key_id(&"k".repeat(129)).is_err());
    }

    #[test]
    fn validate_key_id_rejects_special_chars() {
        assert!(validate_key_id("key id").is_err());
        assert!(validate_key_id("key.id").is_err());
        assert!(validate_key_id("key/id").is_err());
    }

    // -----------------------------------------------------------------------
    // validate_limit
    // -----------------------------------------------------------------------

    #[test]
    fn validate_limit_defaults_to_50() {
        assert_eq!(validate_limit(None).expect("ok"), 50);
    }

    #[test]
    fn validate_limit_accepts_valid_values() {
        assert_eq!(validate_limit(Some(1)).expect("ok"), 1);
        assert_eq!(validate_limit(Some(25)).expect("ok"), 25);
        assert_eq!(validate_limit(Some(200)).expect("ok"), 200);
    }

    #[test]
    fn validate_limit_rejects_zero() {
        assert!(validate_limit(Some(0)).is_err());
    }

    #[test]
    fn validate_limit_rejects_negative() {
        assert!(validate_limit(Some(-1)).is_err());
    }

    #[test]
    fn validate_limit_rejects_too_high() {
        assert!(validate_limit(Some(201)).is_err());
    }

    // -----------------------------------------------------------------------
    // validate_models_json
    // -----------------------------------------------------------------------

    #[test]
    fn validate_models_json_accepts_valid() {
        assert!(validate_models_json(&serde_json::json!([]), "test").is_ok());
        assert!(validate_models_json(&serde_json::json!(["gpt-4", "claude-3*"]), "test").is_ok());
        assert!(validate_models_json(&serde_json::json!(["*"]), "test").is_ok());
        assert!(validate_models_json(&serde_json::json!(["meta/llama-3:8b*"]), "test").is_ok());
    }

    #[test]
    fn validate_models_json_rejects_non_array() {
        assert!(validate_models_json(&serde_json::json!("not-array"), "test").is_err());
        assert!(validate_models_json(&serde_json::json!(null), "test").is_err());
        assert!(validate_models_json(&serde_json::json!({}), "test").is_err());
    }

    #[test]
    fn validate_models_json_rejects_non_string_elements() {
        assert!(validate_models_json(&serde_json::json!([1, 2]), "test").is_err());
        assert!(validate_models_json(&serde_json::json!(["ok", null]), "test").is_err());
    }

    #[test]
    fn validate_models_json_rejects_invalid_model_content() {
        // Spaces
        assert!(validate_models_json(&serde_json::json!(["gpt 4"]), "test").is_err());
        // Control characters
        assert!(validate_models_json(&serde_json::json!(["gpt-4\x00bad"]), "test").is_err());
        // Special characters
        assert!(validate_models_json(&serde_json::json!(["model@evil"]), "test").is_err());
        // Double asterisk
        assert!(validate_models_json(&serde_json::json!(["**"]), "test").is_err());
        // Interior wildcard
        assert!(validate_models_json(&serde_json::json!(["gpt-4*-turbo"]), "test").is_err());
    }

    // -----------------------------------------------------------------------
    // validate_token_limit
    // -----------------------------------------------------------------------

    #[test]
    fn validate_token_limit_accepts_positive() {
        assert!(validate_token_limit(1, "test").is_ok());
        assert!(validate_token_limit(4096, "test").is_ok());
        assert!(validate_token_limit(1_000_000, "test").is_ok());
    }

    #[test]
    fn validate_token_limit_rejects_zero_and_negative() {
        assert!(validate_token_limit(0, "test").is_err());
        assert!(validate_token_limit(-1, "test").is_err());
        assert!(validate_token_limit(i32::MIN, "test").is_err());
    }

    #[test]
    fn validate_token_limit_rejects_exceeding_max() {
        assert!(validate_token_limit(1_000_001, "test").is_err());
        assert!(validate_token_limit(i32::MAX, "test").is_err());
    }

    // -----------------------------------------------------------------------
    // validate_positive_limit
    // -----------------------------------------------------------------------

    #[test]
    fn validate_positive_limit_accepts_valid_rpm() {
        assert!(validate_positive_limit(1, "rpm_limit").is_ok());
        assert!(validate_positive_limit(60, "rpm_limit").is_ok());
        assert!(validate_positive_limit(100_000, "rpm_limit").is_ok());
    }

    #[test]
    fn validate_positive_limit_accepts_valid_concurrency() {
        assert!(validate_positive_limit(1, "concurrency_limit").is_ok());
        assert!(validate_positive_limit(10_000, "concurrency_limit").is_ok());
    }

    #[test]
    fn validate_positive_limit_rejects_zero_and_negative() {
        assert!(validate_positive_limit(0, "rpm_limit").is_err());
        assert!(validate_positive_limit(-1, "rpm_limit").is_err());
        assert!(validate_positive_limit(i32::MIN, "concurrency_limit").is_err());
    }

    #[test]
    fn validate_positive_limit_rejects_exceeding_max() {
        assert!(validate_positive_limit(100_001, "rpm_limit").is_err());
        assert!(validate_positive_limit(10_001, "concurrency_limit").is_err());
        assert!(validate_positive_limit(i32::MAX, "rpm_limit").is_err());
    }
}
