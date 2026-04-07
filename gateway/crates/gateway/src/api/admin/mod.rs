//! Admin control plane API -- tenant, service account, policy, and route management.

pub mod auth;
pub mod policies;
pub mod routes;
pub mod service_accounts;
pub mod tenants;

// Re-export auth types so handlers can use `super::AdminContext` etc.
pub(crate) use auth::{AdminContext, require_admin_role};

use crate::error::GatewayError;
use crate::storage::StorageError;

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
