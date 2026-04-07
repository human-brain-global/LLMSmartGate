//! Domain model structs that map to database rows, plus input types for
//! INSERT / UPDATE operations and cursor-based pagination helpers.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{
    AuditEventId, BudgetId, BudgetPeriodType, BudgetStatus, Environment, KeyId, KeyStatus,
    PolicyId, Provider, RouteId, ServiceAccountId, ServiceAccountStatus, TenantId, TenantStatus,
    UsageEventId,
};

// ===========================================================================
// Cursor pagination
// ===========================================================================

/// Opaque cursor token for keyset pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor(pub String);

#[derive(Serialize, Deserialize)]
struct CursorPayload {
    ts: String, // RFC 3339 timestamp
    id: Uuid,
}

impl Cursor {
    /// Encode a `(created_at, id)` pair into an opaque cursor string.
    pub fn encode(created_at: DateTime<Utc>, id: Uuid) -> Self {
        let payload = CursorPayload {
            ts: created_at.to_rfc3339(),
            id,
        };
        // serde_json::to_vec cannot fail for this simple struct.
        let json = serde_json::to_vec(&payload).unwrap_or_default();
        Self(URL_SAFE_NO_PAD.encode(json))
    }

    /// Decode the cursor back into `(created_at, id)`.
    ///
    /// # Errors
    ///
    /// Returns an error string if the cursor is malformed (bad base64, invalid
    /// JSON, or unparseable timestamp).
    pub fn decode(&self) -> Result<(DateTime<Utc>, Uuid), String> {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.0)
            .map_err(|e| format!("base64 decode: {e}"))?;
        let payload: CursorPayload =
            serde_json::from_slice(&bytes).map_err(|e| format!("json decode: {e}"))?;
        let ts = DateTime::parse_from_rfc3339(&payload.ts)
            .map_err(|e| format!("timestamp parse: {e}"))?
            .with_timezone(&Utc);
        Ok((ts, payload.id))
    }
}

/// A page of results with an optional cursor for the next page.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub has_more: bool,
}

impl<T> Page<T> {
    /// Build a `Page` from a result set that was fetched with `limit + 1` rows.
    ///
    /// `rows`: the result set (may contain one extra row beyond `limit`)
    /// `limit`: the requested page size (already clamped)
    /// `cursor_fn`: extracts `(created_at, id)` from the last item for the next cursor
    pub fn from_rows(
        rows: Vec<T>,
        limit: usize,
        cursor_fn: impl Fn(&T) -> (DateTime<Utc>, Uuid),
    ) -> Self {
        let has_more = rows.len() > limit;
        let items: Vec<T> = rows.into_iter().take(limit).collect();
        let next_cursor = if has_more {
            items.last().map(|item| {
                let (ts, id) = cursor_fn(item);
                Cursor::encode(ts, id)
            })
        } else {
            None
        };
        Page {
            items,
            next_cursor,
            has_more,
        }
    }
}

/// Clamp a pagination limit to a safe range and return `(clamped_limit, fetch_limit)`.
/// `fetch_limit` is `clamped_limit + 1` to detect `has_more`.
pub fn clamp_limit(limit: i64) -> (usize, i64) {
    let clamped = limit.clamp(1, 200);
    // SAFETY: clamped is 1..=200, cast is lossless.
    #[allow(clippy::cast_sign_loss)]
    let limit_usize = clamped as usize;
    (limit_usize, clamped + 1)
}

// ===========================================================================
// Tenant
// ===========================================================================

/// Row struct for the `tenants` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: TenantId,
    pub name: String,
    pub slug: String,
    pub status: TenantStatus,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenant {
    pub id: TenantId,
    pub name: String,
    pub slug: String,
    pub status: TenantStatus,
    pub metadata: serde_json::Value,
}

/// Input for updating an existing tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTenant {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub status: Option<TenantStatus>,
    pub metadata: Option<serde_json::Value>,
}

// ===========================================================================
// Service Account
// ===========================================================================

/// Row struct for the `service_accounts` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceAccount {
    pub id: ServiceAccountId,
    pub tenant_id: TenantId,
    pub name: String,
    pub slug: String,
    pub environment: Environment,
    pub description: Option<String>,
    pub status: ServiceAccountStatus,
    pub default_policy_id: Option<PolicyId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new service account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServiceAccount {
    pub id: ServiceAccountId,
    pub tenant_id: TenantId,
    pub name: String,
    pub slug: String,
    pub environment: Environment,
    pub description: Option<String>,
    pub status: ServiceAccountStatus,
    pub default_policy_id: Option<PolicyId>,
}

/// Input for updating an existing service account.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateServiceAccount {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub environment: Option<Environment>,
    pub description: Option<String>,
    pub status: Option<ServiceAccountStatus>,
    pub default_policy_id: Option<PolicyId>,
}

// ===========================================================================
// Service Account Key
// ===========================================================================

/// Row struct for the `service_account_keys` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceAccountKey {
    pub id: KeyId,
    pub service_account_id: ServiceAccountId,
    pub key_id: String,
    pub algorithm: String,
    pub public_key_pem: String,
    pub fingerprint: String,
    pub status: KeyStatus,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Input for registering a new service account key (simplified form used by
/// the key repository).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterKey {
    pub service_account_id: ServiceAccountId,
    pub key_id: String,
    pub algorithm: String,
    pub public_key_pem: String,
    pub fingerprint: String,
    pub expires_at: Option<DateTime<Utc>>,
}

// ===========================================================================
// Policy
// ===========================================================================

/// Row struct for the `policies` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Policy {
    pub id: PolicyId,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: Option<String>,
    pub allowed_models_json: serde_json::Value,
    pub denied_models_json: serde_json::Value,
    pub max_input_tokens: Option<i32>,
    pub max_output_tokens: Option<i32>,
    pub allow_streaming: bool,
    pub allow_tools: bool,
    pub allow_files: bool,
    pub rpm_limit: Option<i32>,
    pub concurrency_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePolicy {
    pub id: PolicyId,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: Option<String>,
    pub allowed_models_json: serde_json::Value,
    pub denied_models_json: serde_json::Value,
    pub max_input_tokens: Option<i32>,
    pub max_output_tokens: Option<i32>,
    pub allow_streaming: bool,
    pub allow_tools: bool,
    pub allow_files: bool,
    pub rpm_limit: Option<i32>,
    pub concurrency_limit: Option<i32>,
}

/// Input for updating an existing policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePolicy {
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

// ===========================================================================
// Service Account Policy Binding
// ===========================================================================

/// Row struct for the `service_account_policy_bindings` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServiceAccountPolicyBinding {
    pub id: Uuid,
    pub service_account_id: ServiceAccountId,
    pub policy_id: PolicyId,
    pub created_at: DateTime<Utc>,
}

// ===========================================================================
// Provider Route
// ===========================================================================

/// Row struct for the `provider_routes` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderRoute {
    pub id: RouteId,
    pub tenant_id: Option<TenantId>,
    pub model_alias: String,
    pub provider: Provider,
    pub provider_model_name: String,
    pub priority: i32,
    pub enabled: bool,
    pub timeout_ms: i32,
    pub max_retries: i32,
    pub retry_backoff_ms: i32,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Simplified input for creating a new provider route (used by route
/// repository with defaults applied at insert time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoute {
    pub tenant_id: Option<TenantId>,
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

/// Simplified input for updating an existing provider route (all fields
/// optional so callers only supply what they want to change).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateRoute {
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

// ===========================================================================
// Budget
// ===========================================================================

/// Row struct for the `budgets` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Budget {
    pub id: BudgetId,
    pub tenant_id: TenantId,
    pub service_account_id: Option<ServiceAccountId>,
    pub period_type: BudgetPeriodType,
    pub amount_limit: Decimal,
    pub currency: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub status: BudgetStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ===========================================================================
// Usage Event
// ===========================================================================

/// Row struct for the `usage_events` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageEvent {
    pub id: UsageEventId,
    pub request_id: String,
    pub tenant_id: TenantId,
    pub service_account_id: ServiceAccountId,
    pub provider_route_id: Option<RouteId>,
    pub provider: Provider,
    pub model_alias: String,
    pub provider_model_name: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub estimated_cost: Decimal,
    pub currency: String,
    pub latency_ms: i32,
    pub retry_count: i32,
    pub final_status: String,
    pub is_streaming: bool,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new usage event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUsageEvent {
    pub id: UsageEventId,
    pub request_id: String,
    pub tenant_id: TenantId,
    pub service_account_id: ServiceAccountId,
    pub provider_route_id: Option<RouteId>,
    pub provider: Provider,
    pub model_alias: String,
    pub provider_model_name: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub estimated_cost: Decimal,
    pub currency: String,
    pub latency_ms: i32,
    pub retry_count: i32,
    pub final_status: String,
    pub is_streaming: bool,
}

// ===========================================================================
// Audit Event
// ===========================================================================

/// Row struct for the `audit_events` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub tenant_id: Option<TenantId>,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub metadata_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuditEvent {
    pub id: AuditEventId,
    pub tenant_id: Option<TenantId>,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub metadata_json: serde_json::Value,
}

// ===========================================================================
// Pricing Rule
// ===========================================================================

/// Row struct for the `pricing_rules` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PricingRule {
    pub id: Uuid,
    pub provider: Provider,
    pub model_name: String,
    pub input_price_per_1k: Decimal,
    pub output_price_per_1k: Decimal,
    pub effective_from: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new pricing rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePricingRule {
    pub id: Uuid,
    pub provider: Provider,
    pub model_name: String,
    pub input_price_per_1k: Decimal,
    pub output_price_per_1k: Decimal,
    pub effective_from: DateTime<Utc>,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_encode_decode_roundtrip() {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let cursor = Cursor::encode(now, id);
        let (decoded_ts, decoded_id) = cursor.decode().expect("decode should succeed");
        assert_eq!(decoded_id, id);
        // Compare at millisecond precision (RFC 3339 can lose sub-ms).
        assert_eq!(
            decoded_ts.timestamp_millis(),
            now.timestamp_millis(),
        );
    }

    #[test]
    fn cursor_decode_invalid_base64() {
        let cursor = Cursor("!!!invalid!!!".to_owned());
        assert!(cursor.decode().is_err());
    }

    #[test]
    fn cursor_decode_invalid_json() {
        let cursor = Cursor(URL_SAFE_NO_PAD.encode(b"not json"));
        assert!(cursor.decode().is_err());
    }

    #[test]
    fn page_serialises() {
        let page: Page<String> = Page {
            items: vec!["a".to_owned(), "b".to_owned()],
            next_cursor: None,
            has_more: false,
        };
        let json = serde_json::to_value(&page).expect("serialize");
        assert_eq!(json["has_more"], false);
        assert_eq!(json["items"].as_array().expect("array").len(), 2);
    }
}
