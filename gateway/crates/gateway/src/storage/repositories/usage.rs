//! Usage-event repository -- insert, batch-insert, and aggregation queries.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::storage::StorageError;
use crate::types::{ServiceAccountId, TenantId, UsageEventId};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A single usage event as persisted in the `usage_events` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageEvent {
    pub id: UsageEventId,
    pub request_id: String,
    pub tenant_id: TenantId,
    pub service_account_id: ServiceAccountId,
    pub provider_route_id: Option<uuid::Uuid>,
    pub provider: String,
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
#[derive(Debug, Clone)]
pub struct CreateUsageEvent {
    pub request_id: String,
    pub tenant_id: TenantId,
    pub service_account_id: ServiceAccountId,
    pub provider_route_id: Option<uuid::Uuid>,
    pub provider: String,
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

/// Aggregated usage summary returned by [`UsageRepo::query_summary`].
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageSummary {
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: Decimal,
    pub avg_latency_ms: f64,
    pub error_count: i64,
}

/// Filter parameters for usage aggregation queries.
#[derive(Debug, Default)]
pub struct UsageFilter {
    pub tenant_id: Option<TenantId>,
    pub service_account_id: Option<ServiceAccountId>,
    pub model_alias: Option<String>,
    pub provider: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

/// Repository for `usage_events` table operations.
#[derive(Clone)]
pub struct UsageRepo {
    pool: PgPool,
}

impl UsageRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a single usage event, returning the persisted row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or constraint failures.
    pub async fn insert_one(&self, input: &CreateUsageEvent) -> Result<UsageEvent, StorageError> {
        let row = sqlx::query_as::<_, UsageEvent>(
            r"
            INSERT INTO usage_events (
                request_id, tenant_id, service_account_id, provider_route_id,
                provider, model_alias, provider_model_name,
                prompt_tokens, completion_tokens, total_tokens,
                estimated_cost, currency, latency_ms, retry_count,
                final_status, is_streaming
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING *
            ",
        )
        .bind(&input.request_id)
        .bind(input.tenant_id)
        .bind(input.service_account_id)
        .bind(input.provider_route_id)
        .bind(&input.provider)
        .bind(&input.model_alias)
        .bind(&input.provider_model_name)
        .bind(input.prompt_tokens)
        .bind(input.completion_tokens)
        .bind(input.total_tokens)
        .bind(input.estimated_cost)
        .bind(&input.currency)
        .bind(input.latency_ms)
        .bind(input.retry_count)
        .bind(&input.final_status)
        .bind(input.is_streaming)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Batch-insert multiple usage events, returning the number of rows inserted.
    ///
    /// Uses `QueryBuilder::push_values` for a single multi-row INSERT.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or constraint failures.
    pub async fn batch_insert(&self, events: &[CreateUsageEvent]) -> Result<u64, StorageError> {
        if events.is_empty() {
            return Ok(0);
        }

        let mut builder = sqlx::QueryBuilder::new(
            "INSERT INTO usage_events (\
                request_id, tenant_id, service_account_id, provider_route_id, \
                provider, model_alias, provider_model_name, \
                prompt_tokens, completion_tokens, total_tokens, \
                estimated_cost, currency, latency_ms, retry_count, \
                final_status, is_streaming\
            ) ",
        );

        builder.push_values(events, |mut b, e| {
            b.push_bind(&e.request_id)
                .push_bind(e.tenant_id)
                .push_bind(e.service_account_id)
                .push_bind(e.provider_route_id)
                .push_bind(&e.provider)
                .push_bind(&e.model_alias)
                .push_bind(&e.provider_model_name)
                .push_bind(e.prompt_tokens)
                .push_bind(e.completion_tokens)
                .push_bind(e.total_tokens)
                .push_bind(e.estimated_cost)
                .push_bind(&e.currency)
                .push_bind(e.latency_ms)
                .push_bind(e.retry_count)
                .push_bind(&e.final_status)
                .push_bind(e.is_streaming);
        });

        let result = builder.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// Aggregate usage events matching the given filter.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failures.
    pub async fn query_summary(&self, filter: &UsageFilter) -> Result<UsageSummary, StorageError> {
        let tenant_id = filter.tenant_id.map(|t| t.0);
        let sa_id = filter.service_account_id.map(|s| s.0);

        let row = sqlx::query_as::<_, UsageSummary>(
            r"
            SELECT
                COUNT(*)::BIGINT                                              AS total_requests,
                COALESCE(SUM(prompt_tokens)::BIGINT, 0)                       AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens)::BIGINT, 0)                   AS total_completion_tokens,
                COALESCE(SUM(total_tokens)::BIGINT, 0)                        AS total_tokens,
                COALESCE(SUM(estimated_cost), 0)                              AS total_cost,
                COALESCE(AVG(latency_ms)::FLOAT8, 0)                          AS avg_latency_ms,
                COUNT(*) FILTER (WHERE final_status != 'success')::BIGINT     AS error_count
            FROM usage_events
            WHERE ($1::UUID IS NULL OR tenant_id = $1)
              AND ($2::UUID IS NULL OR service_account_id = $2)
              AND ($3::TEXT IS NULL OR model_alias = $3)
              AND ($4::TEXT IS NULL OR provider = $4)
              AND ($5::TIMESTAMPTZ IS NULL OR created_at >= $5)
              AND ($6::TIMESTAMPTZ IS NULL OR created_at <= $6)
            ",
        )
        .bind(tenant_id)
        .bind(sa_id)
        .bind(&filter.model_alias)
        .bind(&filter.provider)
        .bind(filter.start_date)
        .bind(filter.end_date)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
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

    /// Create a tenant row directly, returning its UUID.
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

    /// Create a service account row directly, returning its UUID.
    async fn seed_service_account(pool: &PgPool, tenant_id: TenantId) -> ServiceAccountId {
        let id = Uuid::new_v4();
        let slug = format!("test-sa-{}", &id.to_string()[..8]);
        sqlx::query(
            "INSERT INTO service_accounts (id, tenant_id, name, slug, environment) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(tenant_id.0)
        .bind("Test SA")
        .bind(&slug)
        .bind("dev")
        .execute(pool)
        .await
        .expect("seed service account");
        ServiceAccountId::from_uuid(id)
    }

    #[allow(clippy::too_many_arguments)]
    fn make_event(
        tenant_id: TenantId,
        sa_id: ServiceAccountId,
        provider: &str,
        model: &str,
        prompt_tokens: i32,
        completion_tokens: i32,
        cost: Decimal,
        latency_ms: i32,
        status: &str,
    ) -> CreateUsageEvent {
        CreateUsageEvent {
            request_id: format!("req-{}", Uuid::new_v4()),
            tenant_id,
            service_account_id: sa_id,
            provider_route_id: None,
            provider: provider.to_owned(),
            model_alias: model.to_owned(),
            provider_model_name: format!("{provider}/{model}"),
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            estimated_cost: cost,
            currency: "USD".to_owned(),
            latency_ms,
            retry_count: 0,
            final_status: status.to_owned(),
            is_streaming: false,
        }
    }

    // -----------------------------------------------------------------------
    // insert_one_and_query
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn insert_one_and_query(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let sa_id = seed_service_account(&pool, tenant_id).await;
        let repo = UsageRepo::new(pool);

        let input = make_event(
            tenant_id,
            sa_id,
            "openai",
            "gpt-4",
            100,
            50,
            dec!(0.015),
            200,
            "success",
        );

        let event = repo
            .insert_one(&input)
            .await
            .expect("insert should succeed");

        assert_eq!(event.request_id, input.request_id);
        assert_eq!(event.tenant_id, tenant_id);
        assert_eq!(event.service_account_id, sa_id);
        assert_eq!(event.provider, "openai");
        assert_eq!(event.model_alias, "gpt-4");
        assert_eq!(event.prompt_tokens, 100);
        assert_eq!(event.completion_tokens, 50);
        assert_eq!(event.total_tokens, 150);
        assert_eq!(event.estimated_cost, dec!(0.015));
        assert_eq!(event.latency_ms, 200);
        assert_eq!(event.final_status, "success");
    }

    // -----------------------------------------------------------------------
    // batch_insert
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn batch_insert(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let sa_id = seed_service_account(&pool, tenant_id).await;
        let repo = UsageRepo::new(pool);

        let events: Vec<CreateUsageEvent> = (0..5)
            .map(|i| {
                make_event(
                    tenant_id,
                    sa_id,
                    "anthropic",
                    "claude-3",
                    100 + i * 10,
                    50 + i * 5,
                    dec!(0.01),
                    100 + i * 20,
                    "success",
                )
            })
            .collect();

        let count = repo
            .batch_insert(&events)
            .await
            .expect("batch insert should succeed");
        assert_eq!(count, 5);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn batch_insert_empty(pool: PgPool) {
        let repo = UsageRepo::new(pool);
        let count = repo
            .batch_insert(&[])
            .await
            .expect("empty batch should succeed");
        assert_eq!(count, 0);
    }

    // -----------------------------------------------------------------------
    // query_summary_aggregation
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn query_summary_aggregation(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let sa_id = seed_service_account(&pool, tenant_id).await;
        let repo = UsageRepo::new(pool);

        // Insert 3 events: 2 success, 1 error
        let events = vec![
            make_event(
                tenant_id,
                sa_id,
                "openai",
                "gpt-4",
                100,
                50,
                dec!(0.010),
                200,
                "success",
            ),
            make_event(
                tenant_id,
                sa_id,
                "openai",
                "gpt-4",
                200,
                100,
                dec!(0.020),
                300,
                "success",
            ),
            make_event(
                tenant_id,
                sa_id,
                "openai",
                "gpt-4",
                50,
                25,
                dec!(0.005),
                100,
                "error",
            ),
        ];
        repo.batch_insert(&events)
            .await
            .expect("batch insert should succeed");

        let summary = repo
            .query_summary(&UsageFilter::default())
            .await
            .expect("summary should succeed");

        assert_eq!(summary.total_requests, 3);
        assert_eq!(summary.total_prompt_tokens, 350); // 100+200+50
        assert_eq!(summary.total_completion_tokens, 175); // 50+100+25
        assert_eq!(summary.total_tokens, 525); // 150+300+75
        assert_eq!(summary.total_cost, dec!(0.035)); // 0.010+0.020+0.005
        assert_eq!(summary.error_count, 1);
        // avg_latency_ms = (200+300+100) / 3 = 200.0
        assert!((summary.avg_latency_ms - 200.0).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // query_summary_with_filters
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn query_summary_with_filters(pool: PgPool) {
        let tenant_id = seed_tenant(&pool).await;
        let sa_id = seed_service_account(&pool, tenant_id).await;

        // Create a second tenant with its own events
        let tenant_id2 = seed_tenant(&pool).await;
        let sa_id2 = seed_service_account(&pool, tenant_id2).await;

        let repo = UsageRepo::new(pool);

        let events = vec![
            make_event(
                tenant_id,
                sa_id,
                "openai",
                "gpt-4",
                100,
                50,
                dec!(0.01),
                200,
                "success",
            ),
            make_event(
                tenant_id,
                sa_id,
                "anthropic",
                "claude-3",
                200,
                100,
                dec!(0.02),
                300,
                "success",
            ),
            make_event(
                tenant_id2,
                sa_id2,
                "openai",
                "gpt-4",
                300,
                150,
                dec!(0.03),
                400,
                "success",
            ),
        ];
        repo.batch_insert(&events)
            .await
            .expect("batch insert should succeed");

        // Filter by tenant
        let summary = repo
            .query_summary(&UsageFilter {
                tenant_id: Some(tenant_id),
                ..Default::default()
            })
            .await
            .expect("summary should succeed");
        assert_eq!(summary.total_requests, 2);

        // Filter by provider
        let summary = repo
            .query_summary(&UsageFilter {
                provider: Some("openai".to_owned()),
                ..Default::default()
            })
            .await
            .expect("summary should succeed");
        assert_eq!(summary.total_requests, 2);

        // Filter by model alias
        let summary = repo
            .query_summary(&UsageFilter {
                model_alias: Some("claude-3".to_owned()),
                ..Default::default()
            })
            .await
            .expect("summary should succeed");
        assert_eq!(summary.total_requests, 1);
        assert_eq!(summary.total_prompt_tokens, 200);

        // Filter by tenant + provider (intersection)
        let summary = repo
            .query_summary(&UsageFilter {
                tenant_id: Some(tenant_id),
                provider: Some("openai".to_owned()),
                ..Default::default()
            })
            .await
            .expect("summary should succeed");
        assert_eq!(summary.total_requests, 1);
    }

    // -----------------------------------------------------------------------
    // query_summary_empty
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn query_summary_empty(pool: PgPool) {
        let repo = UsageRepo::new(pool);

        let summary = repo
            .query_summary(&UsageFilter::default())
            .await
            .expect("empty summary should succeed");

        assert_eq!(summary.total_requests, 0);
        assert_eq!(summary.total_prompt_tokens, 0);
        assert_eq!(summary.total_completion_tokens, 0);
        assert_eq!(summary.total_tokens, 0);
        assert_eq!(summary.total_cost, dec!(0));
        assert!((summary.avg_latency_ms - 0.0).abs() < 0.01);
        assert_eq!(summary.error_count, 0);
    }
}
