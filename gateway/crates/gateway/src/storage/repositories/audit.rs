//! Audit-event and pricing-rule repositories.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::models::{Cursor, Page, clamp_limit};
use crate::storage::StorageError;
use crate::types::{AuditEventId, PricingRuleId, TenantId};

// ---------------------------------------------------------------------------
// Audit domain types
// ---------------------------------------------------------------------------

/// A single audit event as persisted in the `audit_events` table.
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
#[derive(Debug, Clone)]
pub struct CreateAuditEvent {
    pub tenant_id: Option<TenantId>,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub metadata_json: serde_json::Value,
}

/// Filter parameters for listing audit events.
#[derive(Debug, Default)]
pub struct AuditFilter {
    pub tenant_id: Option<TenantId>,
    pub action: Option<String>,
    pub target_type: Option<String>,
    pub actor_id: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Pricing domain types
// ---------------------------------------------------------------------------

/// A pricing rule as persisted in the `pricing_rules` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PricingRule {
    pub id: PricingRuleId,
    pub provider: String,
    pub model_name: String,
    pub input_price_per_1k: Decimal,
    pub output_price_per_1k: Decimal,
    pub effective_from: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new pricing rule.
#[derive(Debug, Clone)]
pub struct CreatePricingRule {
    pub provider: String,
    pub model_name: String,
    pub input_price_per_1k: Decimal,
    pub output_price_per_1k: Decimal,
    pub effective_from: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// AuditRepo
// ---------------------------------------------------------------------------

/// Repository for `audit_events` table operations (append-only).
#[derive(Clone)]
pub struct AuditRepo {
    pool: PgPool,
}

impl AuditRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a single audit event, returning the persisted row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or constraint failures.
    pub async fn insert(&self, input: &CreateAuditEvent) -> Result<AuditEvent, StorageError> {
        let tenant_uuid = input.tenant_id.map(|t| t.0);

        let row = sqlx::query_as::<_, AuditEvent>(
            r"
            INSERT INTO audit_events (
                tenant_id, actor_type, actor_id, action,
                target_type, target_id, metadata_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            ",
        )
        .bind(tenant_uuid)
        .bind(&input.actor_type)
        .bind(&input.actor_id)
        .bind(&input.action)
        .bind(&input.target_type)
        .bind(&input.target_id)
        .bind(&input.metadata_json)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// List audit events in reverse chronological order with cursor-based
    /// pagination.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failures.
    pub async fn list(
        &self,
        filter: &AuditFilter,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page<AuditEvent>, StorageError> {
        let tenant_uuid = filter.tenant_id.map(|t| t.0);

        // When a cursor is present, filter for rows strictly before it.
        let (cursor_ts, cursor_id) = match cursor {
            Some(c) => {
                let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
                (Some(ts), Some(id))
            }
            None => (None, None),
        };

        let (limit, fetch_limit) = clamp_limit(limit);

        let rows = sqlx::query_as::<_, AuditEvent>(
            r"
            SELECT *
            FROM audit_events
            WHERE ($1::UUID IS NULL OR tenant_id = $1)
              AND ($2::TEXT IS NULL OR action = $2)
              AND ($3::TEXT IS NULL OR target_type = $3)
              AND ($4::TEXT IS NULL OR actor_id = $4)
              AND ($5::TIMESTAMPTZ IS NULL OR created_at >= $5)
              AND ($6::TIMESTAMPTZ IS NULL OR created_at <= $6)
              AND (
                  $7::TIMESTAMPTZ IS NULL
                  OR (created_at, id) < ($7, $8::UUID)
              )
            ORDER BY created_at DESC, id DESC
            LIMIT $9
            ",
        )
        .bind(tenant_uuid)
        .bind(&filter.action)
        .bind(&filter.target_type)
        .bind(&filter.actor_id)
        .bind(filter.start_date)
        .bind(filter.end_date)
        .bind(cursor_ts)
        .bind(cursor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(Page::from_rows(rows, limit, |e| (e.created_at, e.id.0)))
    }
}

// ---------------------------------------------------------------------------
// PricingRepo
// ---------------------------------------------------------------------------

/// Repository for `pricing_rules` table operations.
#[derive(Clone)]
pub struct PricingRepo {
    pool: PgPool,
}

impl PricingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new pricing rule, returning the persisted row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Conflict`] if a rule with the same provider,
    /// model, and effective date already exists.
    /// Returns [`StorageError::Database`] on other failures.
    pub async fn create(&self, input: &CreatePricingRule) -> Result<PricingRule, StorageError> {
        let row = sqlx::query_as::<_, PricingRule>(
            r"
            INSERT INTO pricing_rules (
                provider, model_name, input_price_per_1k,
                output_price_per_1k, effective_from
            )
            VALUES ($1, $2, $3, $4, COALESCE($5, NOW()))
            RETURNING *
            ",
        )
        .bind(&input.provider)
        .bind(&input.model_name)
        .bind(input.input_price_per_1k)
        .bind(input.output_price_per_1k)
        .bind(input.effective_from)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                StorageError::Conflict {
                    entity: "pricing_rule".to_owned(),
                    detail: format!(
                        "pricing rule for {}/{} at given effective_from already exists",
                        input.provider, input.model_name
                    ),
                }
            }
            _ => StorageError::Database(e),
        })?;

        Ok(row)
    }

    /// Get the most recently effective pricing rule for a provider/model pair.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failures.
    pub async fn get_by_provider_model(
        &self,
        provider: &str,
        model_name: &str,
    ) -> Result<Option<PricingRule>, StorageError> {
        let row = sqlx::query_as::<_, PricingRule>(
            r"
            SELECT * FROM pricing_rules
            WHERE provider = $1 AND model_name = $2 AND effective_from <= NOW()
            ORDER BY effective_from DESC
            LIMIT 1
            ",
        )
        .bind(provider)
        .bind(model_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// List all pricing rules ordered by provider, model, and effective date.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failures.
    pub async fn list_all(&self) -> Result<Vec<PricingRule>, StorageError> {
        let rows = sqlx::query_as::<_, PricingRule>(
            r"
            SELECT * FROM pricing_rules
            ORDER BY provider, model_name, effective_from DESC
            ",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use sqlx::PgPool;
    use uuid::Uuid;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    async fn seed_tenant(pool: &PgPool) -> TenantId {
        let id = Uuid::new_v4();
        let slug = format!("test-tenant-{}", &id.to_string()[..8]);
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(id)
            .bind("Test Tenant")
            .bind(&slug)
            .execute(pool)
            .await
            .expect("seed tenant");
        TenantId::from_uuid(id)
    }

    fn make_audit(
        tenant_id: Option<TenantId>,
        action: &str,
        target_type: &str,
    ) -> CreateAuditEvent {
        CreateAuditEvent {
            tenant_id,
            actor_type: "admin_user".to_owned(),
            actor_id: format!("user-{}", Uuid::new_v4()),
            action: action.to_owned(),
            target_type: target_type.to_owned(),
            target_id: Uuid::new_v4().to_string(),
            metadata_json: serde_json::json!({}),
        }
    }

    // -----------------------------------------------------------------------
    // insert_audit_event
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn insert_audit_event(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let repo = AuditRepo::new(pool);

        let input = CreateAuditEvent {
            tenant_id: Some(tenant_id),
            actor_type: "admin_user".to_owned(),
            actor_id: "user-123".to_owned(),
            action: "service_account.created".to_owned(),
            target_type: "service_account".to_owned(),
            target_id: "sa-456".to_owned(),
            metadata_json: serde_json::json!({"name": "test-sa"}),
        };

        let event = repo.insert(&input).await.expect("insert should succeed");

        assert_eq!(event.tenant_id, Some(tenant_id));
        assert_eq!(event.actor_type, "admin_user");
        assert_eq!(event.actor_id, "user-123");
        assert_eq!(event.action, "service_account.created");
        assert_eq!(event.target_type, "service_account");
        assert_eq!(event.target_id, "sa-456");
        assert_eq!(event.metadata_json, serde_json::json!({"name": "test-sa"}));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn insert_audit_event_system_no_tenant(pool: PgPool) {
        let repo = AuditRepo::new(pool);

        let input = CreateAuditEvent {
            tenant_id: None,
            actor_type: "system".to_owned(),
            actor_id: "system".to_owned(),
            action: "startup".to_owned(),
            target_type: "gateway".to_owned(),
            target_id: "self".to_owned(),
            metadata_json: serde_json::json!({}),
        };

        let event = repo.insert(&input).await.expect("insert should succeed");
        assert!(event.tenant_id.is_none());
    }

    // -----------------------------------------------------------------------
    // list_audit_with_filters
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_audit_with_filters(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let repo = AuditRepo::new(pool.clone());

        // Insert events with different actions and target types
        let events = vec![
            make_audit(
                Some(tenant_id),
                "service_account.created",
                "service_account",
            ),
            make_audit(Some(tenant_id), "key.revoked", "key"),
            make_audit(Some(tenant_id), "policy.updated", "policy"),
        ];

        for e in &events {
            repo.insert(e).await.expect("insert should succeed");
        }

        // Filter by action
        let page = repo
            .list(
                &AuditFilter {
                    action: Some("key.revoked".to_owned()),
                    ..Default::default()
                },
                None,
                10,
            )
            .await
            .expect("list should succeed");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].action, "key.revoked");

        // Filter by target_type
        let page = repo
            .list(
                &AuditFilter {
                    target_type: Some("policy".to_owned()),
                    ..Default::default()
                },
                None,
                10,
            )
            .await
            .expect("list should succeed");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].target_type, "policy");

        // Filter by tenant
        let page = repo
            .list(
                &AuditFilter {
                    tenant_id: Some(tenant_id),
                    ..Default::default()
                },
                None,
                10,
            )
            .await
            .expect("list should succeed");
        assert_eq!(page.items.len(), 3);
    }

    // -----------------------------------------------------------------------
    // list_audit_reverse_chronological
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_audit_reverse_chronological(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let repo = AuditRepo::new(pool.clone());

        // Insert events with slight delay to ensure distinct timestamps
        let actions = ["first", "second", "third", "fourth", "fifth"];
        for action in &actions {
            repo.insert(&make_audit(Some(tenant_id), action, "test"))
                .await
                .expect("insert should succeed");
        }

        // Fetch all -- most recent first
        let page = repo
            .list(&AuditFilter::default(), None, 10)
            .await
            .expect("list should succeed");
        assert_eq!(page.items.len(), 5);
        assert!(page.next_cursor.is_none());

        // Verify reverse chronological order
        for window in page.items.windows(2) {
            assert!(window[0].created_at >= window[1].created_at);
        }
        // The last inserted should be first
        assert_eq!(page.items[0].action, "fifth");

        // Test pagination: fetch 2 at a time
        let page1 = repo
            .list(&AuditFilter::default(), None, 2)
            .await
            .expect("page1 should succeed");
        assert_eq!(page1.items.len(), 2);
        assert!(page1.next_cursor.is_some());

        let page2 = repo
            .list(&AuditFilter::default(), page1.next_cursor.as_ref(), 2)
            .await
            .expect("page2 should succeed");
        assert_eq!(page2.items.len(), 2);
        assert!(page2.next_cursor.is_some());

        let page3 = repo
            .list(&AuditFilter::default(), page2.next_cursor.as_ref(), 2)
            .await
            .expect("page3 should succeed");
        assert_eq!(page3.items.len(), 1);
        assert!(page3.next_cursor.is_none());

        // Verify no overlaps between pages
        let all_ids: Vec<_> = page1
            .items
            .iter()
            .chain(page2.items.iter())
            .chain(page3.items.iter())
            .map(|e| e.id)
            .collect();
        let unique_ids: std::collections::HashSet<_> = all_ids.iter().collect();
        assert_eq!(all_ids.len(), unique_ids.len());
    }

    // -----------------------------------------------------------------------
    // create_pricing_rule
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_pricing_rule(pool: PgPool) {
        let repo = PricingRepo::new(pool);

        let input = CreatePricingRule {
            provider: "openai".to_owned(),
            model_name: "gpt-4".to_owned(),
            input_price_per_1k: dec!(0.03),
            output_price_per_1k: dec!(0.06),
            effective_from: None,
        };

        let rule = repo.create(&input).await.expect("create should succeed");
        assert_eq!(rule.provider, "openai");
        assert_eq!(rule.model_name, "gpt-4");
        assert_eq!(rule.input_price_per_1k, dec!(0.03));
        assert_eq!(rule.output_price_per_1k, dec!(0.06));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_pricing_rule_conflict(pool: PgPool) {
        let repo = PricingRepo::new(pool);
        let now = Utc::now();

        let input = CreatePricingRule {
            provider: "openai".to_owned(),
            model_name: "gpt-4".to_owned(),
            input_price_per_1k: dec!(0.03),
            output_price_per_1k: dec!(0.06),
            effective_from: Some(now),
        };

        repo.create(&input)
            .await
            .expect("first create should succeed");

        let result = repo.create(&input).await;
        assert!(
            matches!(result, Err(StorageError::Conflict { .. })),
            "duplicate should return Conflict"
        );
    }

    // -----------------------------------------------------------------------
    // get_pricing_by_provider_model
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_pricing_by_provider_model(pool: PgPool) {
        let repo = PricingRepo::new(pool);

        // No rules yet
        let result = repo
            .get_by_provider_model("openai", "gpt-4")
            .await
            .expect("query should succeed");
        assert!(result.is_none());

        // Create a rule
        let input = CreatePricingRule {
            provider: "openai".to_owned(),
            model_name: "gpt-4".to_owned(),
            input_price_per_1k: dec!(0.03),
            output_price_per_1k: dec!(0.06),
            effective_from: None,
        };
        repo.create(&input).await.expect("create should succeed");

        // Now it should be found
        let rule = repo
            .get_by_provider_model("openai", "gpt-4")
            .await
            .expect("query should succeed")
            .expect("rule should exist");
        assert_eq!(rule.provider, "openai");
        assert_eq!(rule.model_name, "gpt-4");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_pricing_returns_most_recent_effective(pool: PgPool) {
        let repo = PricingRepo::new(pool);

        // Create an older rule
        let old = CreatePricingRule {
            provider: "anthropic".to_owned(),
            model_name: "claude-3".to_owned(),
            input_price_per_1k: dec!(0.01),
            output_price_per_1k: dec!(0.02),
            effective_from: Some(Utc::now() - chrono::Duration::days(30)),
        };
        repo.create(&old).await.expect("create old should succeed");

        // Create a newer rule
        let new = CreatePricingRule {
            provider: "anthropic".to_owned(),
            model_name: "claude-3".to_owned(),
            input_price_per_1k: dec!(0.015),
            output_price_per_1k: dec!(0.03),
            effective_from: Some(Utc::now() - chrono::Duration::days(1)),
        };
        repo.create(&new).await.expect("create new should succeed");

        let rule = repo
            .get_by_provider_model("anthropic", "claude-3")
            .await
            .expect("query should succeed")
            .expect("rule should exist");

        // Should return the newer rule
        assert_eq!(rule.input_price_per_1k, dec!(0.015));
        assert_eq!(rule.output_price_per_1k, dec!(0.03));
    }

    // -----------------------------------------------------------------------
    // list_all_pricing
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_all_pricing(pool: PgPool) {
        let repo = PricingRepo::new(pool);

        let rules = vec![
            CreatePricingRule {
                provider: "openai".to_owned(),
                model_name: "gpt-4".to_owned(),
                input_price_per_1k: dec!(0.03),
                output_price_per_1k: dec!(0.06),
                effective_from: None,
            },
            CreatePricingRule {
                provider: "anthropic".to_owned(),
                model_name: "claude-3".to_owned(),
                input_price_per_1k: dec!(0.01),
                output_price_per_1k: dec!(0.02),
                effective_from: None,
            },
        ];

        for r in &rules {
            repo.create(r).await.expect("create should succeed");
        }

        let all = repo.list_all().await.expect("list_all should succeed");
        assert_eq!(all.len(), 2);
    }
}
