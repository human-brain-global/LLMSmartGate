//! Shared domain types: strongly-typed IDs, status enums, and provider enum.
//!
//! All ID newtypes wrap a [`uuid::Uuid`] and are `Copy`, `Eq`, and `Hash` so
//! they can be used as map keys with zero overhead.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ID newtypes (macro-generated)
// ---------------------------------------------------------------------------

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[serde(transparent)]
        #[sqlx(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a new random (v4) ID.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wrap an existing UUID.
            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }
    };
}

define_id!(
    /// Unique identifier for a tenant (organisation / workspace).
    TenantId
);
define_id!(
    /// Unique identifier for a service account within a tenant.
    ServiceAccountId
);
define_id!(
    /// Unique identifier for an API key.
    KeyId
);
define_id!(
    /// Unique identifier for a policy rule.
    PolicyId
);
define_id!(
    /// Unique identifier for a routing rule.
    RouteId
);
define_id!(
    /// Unique identifier for a budget allocation.
    BudgetId
);
define_id!(
    /// Unique identifier for a usage event.
    UsageEventId
);
define_id!(
    /// Unique identifier for an audit log entry.
    AuditEventId
);
define_id!(
    /// Unique identifier for a pricing rule.
    PricingRuleId
);

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Deployment environment tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Environment {
    Dev,
    Staging,
    Prod,
}

/// Lifecycle status of a service account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "service_account_status", rename_all = "lowercase")]
pub enum ServiceAccountStatus {
    Active,
    Suspended,
    Deleted,
}

impl fmt::Display for ServiceAccountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// Lifecycle status of an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "key_status", rename_all = "lowercase")]
pub enum KeyStatus {
    Active,
    Rotating,
    Revoked,
    Expired,
}

impl fmt::Display for KeyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Rotating => write!(f, "rotating"),
            Self::Revoked => write!(f, "revoked"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

/// Lifecycle status of a tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "tenant_status", rename_all = "lowercase")]
pub enum TenantStatus {
    Active,
    Suspended,
    Deleted,
}

/// Supported upstream LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Provider {
    Openai,
    Anthropic,
    Gemini,
    #[serde(rename = "azure_openai")]
    #[sqlx(rename = "azure_openai")]
    AzureOpenai,
    Vllm,
}

/// Budget period type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "budget_period_type", rename_all = "lowercase")]
pub enum BudgetPeriodType {
    Daily,
    Monthly,
    Custom,
}

/// Lifecycle status of a budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "budget_status", rename_all = "lowercase")]
pub enum BudgetStatus {
    Active,
    Exhausted,
    Expired,
    Disabled,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // ID newtypes
    // -----------------------------------------------------------------------

    #[test]
    fn id_from_uuid_roundtrip() {
        let uuid = Uuid::new_v4();
        let tenant = TenantId::from_uuid(uuid);
        assert_eq!(tenant.0, uuid);
    }

    #[test]
    fn id_from_trait() {
        let uuid = Uuid::new_v4();
        let key_id: KeyId = uuid.into();
        assert_eq!(key_id.0, uuid);
    }

    #[test]
    fn id_display_matches_uuid() {
        let uuid = Uuid::new_v4();
        let id = ServiceAccountId::from_uuid(uuid);
        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn id_serde_roundtrip() {
        let original = PolicyId::new();
        let json = serde_json::to_string(&original).expect("serialize should succeed");
        let deserialized: PolicyId =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn id_serialises_as_plain_uuid_string() {
        let uuid = Uuid::new_v4();
        let id = RouteId::from_uuid(uuid);
        let json = serde_json::to_string(&id).expect("serialize should succeed");
        // Should be a plain quoted UUID, not an object
        assert_eq!(json, format!("\"{uuid}\""));
    }

    #[test]
    fn id_deserialises_from_plain_uuid_string() {
        let uuid = Uuid::new_v4();
        let json = format!("\"{uuid}\"");
        let id: BudgetId = serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(id.0, uuid);
    }

    #[test]
    fn id_equality_and_hash() {
        let uuid = Uuid::new_v4();
        let a = AuditEventId::from_uuid(uuid);
        let b = AuditEventId::from_uuid(uuid);
        assert_eq!(a, b);

        // Verify Hash works by inserting into a set.
        let mut set = std::collections::HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn id_copy_semantics() {
        let id = UsageEventId::new();
        let copy = id; // Copy, not move
        assert_eq!(id, copy);
    }

    #[test]
    fn id_default_generates_new() {
        let a = TenantId::default();
        let b = TenantId::default();
        // Vanishingly unlikely to collide
        assert_ne!(a, b);
    }

    // -----------------------------------------------------------------------
    // Enum serialisation
    // -----------------------------------------------------------------------

    #[test]
    fn environment_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&Environment::Dev).expect("serialize"),
            "\"dev\""
        );
        assert_eq!(
            serde_json::to_string(&Environment::Staging).expect("serialize"),
            "\"staging\""
        );
        assert_eq!(
            serde_json::to_string(&Environment::Prod).expect("serialize"),
            "\"prod\""
        );
    }

    #[test]
    fn environment_deserialises_lowercase() {
        let env: Environment = serde_json::from_str("\"staging\"").expect("deserialize");
        assert_eq!(env, Environment::Staging);
    }

    #[test]
    fn service_account_status_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&ServiceAccountStatus::Active).expect("serialize"),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&ServiceAccountStatus::Suspended).expect("serialize"),
            "\"suspended\""
        );
        assert_eq!(
            serde_json::to_string(&ServiceAccountStatus::Deleted).expect("serialize"),
            "\"deleted\""
        );
    }

    #[test]
    fn key_status_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&KeyStatus::Active).expect("serialize"),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&KeyStatus::Rotating).expect("serialize"),
            "\"rotating\""
        );
        assert_eq!(
            serde_json::to_string(&KeyStatus::Revoked).expect("serialize"),
            "\"revoked\""
        );
        assert_eq!(
            serde_json::to_string(&KeyStatus::Expired).expect("serialize"),
            "\"expired\""
        );
    }

    #[test]
    fn tenant_status_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&TenantStatus::Active).expect("serialize"),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&TenantStatus::Suspended).expect("serialize"),
            "\"suspended\""
        );
    }

    #[test]
    fn provider_serialises_lowercase() {
        assert_eq!(
            serde_json::to_string(&Provider::Openai).expect("serialize"),
            "\"openai\""
        );
        assert_eq!(
            serde_json::to_string(&Provider::Anthropic).expect("serialize"),
            "\"anthropic\""
        );
        assert_eq!(
            serde_json::to_string(&Provider::Gemini).expect("serialize"),
            "\"gemini\""
        );
        assert_eq!(
            serde_json::to_string(&Provider::AzureOpenai).expect("serialize"),
            "\"azure_openai\""
        );
        assert_eq!(
            serde_json::to_string(&Provider::Vllm).expect("serialize"),
            "\"vllm\""
        );
    }

    #[test]
    fn provider_deserialises_lowercase() {
        let p: Provider = serde_json::from_str("\"anthropic\"").expect("deserialize");
        assert_eq!(p, Provider::Anthropic);
    }

    #[test]
    fn provider_azure_openai_roundtrip() {
        let json = serde_json::to_string(&Provider::AzureOpenai).expect("serialize");
        let p: Provider = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, Provider::AzureOpenai);
    }

    #[test]
    fn key_status_display_matches_serde() {
        assert_eq!(KeyStatus::Active.to_string(), "active");
        assert_eq!(KeyStatus::Rotating.to_string(), "rotating");
        assert_eq!(KeyStatus::Revoked.to_string(), "revoked");
        assert_eq!(KeyStatus::Expired.to_string(), "expired");
    }

    #[test]
    fn key_status_roundtrip() {
        let json = serde_json::to_string(&KeyStatus::Rotating).expect("serialize");
        let status: KeyStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(status, KeyStatus::Rotating);
    }
}
