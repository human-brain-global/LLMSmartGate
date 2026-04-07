//! Provider route repository -- CRUD and hot-path route resolution.

use sqlx::PgPool;

use crate::models::{CreateRoute, Cursor, Page, ProviderRoute, UpdateRoute, clamp_limit};
use crate::storage::StorageError;
use crate::types::{RouteId, TenantId};

/// Repository for the `provider_routes` table.
#[derive(Clone)]
pub struct RouteRepo {
    pool: PgPool,
}

impl RouteRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// List global routes (tenant_id IS NULL) with cursor-based pagination.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn list_global(
        &self,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page<ProviderRoute>, StorageError> {
        let (limit, fetch_limit) = clamp_limit(limit);

        let rows = if let Some(c) = cursor {
            let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
            sqlx::query_as::<_, ProviderRoute>(
                r"
                SELECT * FROM provider_routes
                WHERE tenant_id IS NULL
                  AND (created_at, id) > ($1, $2)
                ORDER BY created_at ASC, id ASC
                LIMIT $3
                ",
            )
            .bind(ts)
            .bind(id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ProviderRoute>(
                r"
                SELECT * FROM provider_routes
                WHERE tenant_id IS NULL
                ORDER BY created_at ASC, id ASC
                LIMIT $1
                ",
            )
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(Page::from_rows(rows, limit, |r| (r.created_at, r.id.0)))
    }

    /// Insert a new route and return the full row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn create(&self, input: &CreateRoute) -> Result<ProviderRoute, StorageError> {
        sqlx::query_as::<_, ProviderRoute>(
            r"
            INSERT INTO provider_routes (
                tenant_id, model_alias, provider, provider_model_name,
                priority, enabled, timeout_ms, max_retries, retry_backoff_ms, config
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            ",
        )
        .bind(input.tenant_id)
        .bind(&input.model_alias)
        .bind(&input.provider)
        .bind(&input.provider_model_name)
        .bind(input.priority.unwrap_or(100))
        .bind(input.enabled.unwrap_or(true))
        .bind(input.timeout_ms.unwrap_or(30000))
        .bind(input.max_retries.unwrap_or(1))
        .bind(input.retry_backoff_ms.unwrap_or(250))
        .bind(input.config.as_ref().unwrap_or(&serde_json::json!({})))
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Fetch a single route by id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists.
    pub async fn get_by_id(&self, id: RouteId) -> Result<ProviderRoute, StorageError> {
        sqlx::query_as::<_, ProviderRoute>("SELECT * FROM provider_routes WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::not_found("route", "id", id))
    }

    /// List routes for a tenant with cursor-based pagination.
    ///
    /// Results are ordered by `(created_at, id)` ascending.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn list_by_tenant(
        &self,
        tenant_id: TenantId,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page<ProviderRoute>, StorageError> {
        let (limit, fetch_limit) = clamp_limit(limit);

        let rows = if let Some(c) = cursor {
            let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
            sqlx::query_as::<_, ProviderRoute>(
                r"
                SELECT * FROM provider_routes
                WHERE tenant_id = $1
                  AND (created_at, id) > ($2, $3)
                ORDER BY created_at ASC, id ASC
                LIMIT $4
                ",
            )
            .bind(tenant_id)
            .bind(ts)
            .bind(id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ProviderRoute>(
                r"
                SELECT * FROM provider_routes
                WHERE tenant_id = $1
                ORDER BY created_at ASC, id ASC
                LIMIT $2
                ",
            )
            .bind(tenant_id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(Page::from_rows(rows, limit, |r| (r.created_at, r.id.0)))
    }

    /// Update mutable fields of a route. Only non-`None` fields are applied.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists.
    pub async fn update(
        &self,
        id: RouteId,
        input: &UpdateRoute,
    ) -> Result<ProviderRoute, StorageError> {
        sqlx::query_as::<_, ProviderRoute>(
            r"
            UPDATE provider_routes SET
                model_alias         = COALESCE($2, model_alias),
                provider            = COALESCE($3, provider),
                provider_model_name = COALESCE($4, provider_model_name),
                priority            = COALESCE($5, priority),
                enabled             = COALESCE($6, enabled),
                timeout_ms          = COALESCE($7, timeout_ms),
                max_retries         = COALESCE($8, max_retries),
                retry_backoff_ms    = COALESCE($9, retry_backoff_ms),
                config              = COALESCE($10, config)
            WHERE id = $1
            RETURNING *
            ",
        )
        .bind(id)
        .bind(&input.model_alias)
        .bind(&input.provider)
        .bind(&input.provider_model_name)
        .bind(input.priority)
        .bind(input.enabled)
        .bind(input.timeout_ms)
        .bind(input.max_retries)
        .bind(input.retry_backoff_ms)
        .bind(&input.config)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if matches!(&e, sqlx::Error::RowNotFound) {
                StorageError::not_found("route", "id", id)
            } else {
                StorageError::from(e)
            }
        })
    }

    /// Delete a route by id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists.
    pub async fn delete(&self, id: RouteId) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM provider_routes WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::not_found("route", "id", id));
        }
        Ok(())
    }

    /// **HOT PATH** -- Resolve enabled routes for a model alias.
    ///
    /// Combines tenant-scoped and global (`tenant_id IS NULL`) routes, ordering
    /// tenant-scoped routes first, then by ascending priority.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection or query failure.
    pub async fn resolve_routes(
        &self,
        tenant_id: TenantId,
        model_alias: &str,
    ) -> Result<Vec<ProviderRoute>, StorageError> {
        let rows = sqlx::query_as::<_, ProviderRoute>(
            r"
            SELECT id, tenant_id, model_alias, provider, provider_model_name,
                   priority, enabled, timeout_ms, max_retries, retry_backoff_ms,
                   config, created_at, updated_at
            FROM provider_routes
            WHERE model_alias = $1
              AND enabled = true
              AND (tenant_id = $2 OR tenant_id IS NULL)
            ORDER BY
              CASE WHEN tenant_id IS NOT NULL THEN 0 ELSE 1 END,
              priority ASC
            ",
        )
        .bind(model_alias)
        .bind(tenant_id)
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
    use crate::models::Tenant;
    use crate::types::Provider;
    use sqlx::PgPool;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    async fn create_test_tenant(pool: &PgPool) -> Tenant {
        sqlx::query_as::<_, Tenant>("INSERT INTO tenants (name, slug) VALUES ($1, $2) RETURNING *")
            .bind(format!("Test {}", uuid::Uuid::new_v4()))
            .bind(format!("test-{}", uuid::Uuid::new_v4()))
            .fetch_one(pool)
            .await
            .expect("create tenant")
    }

    fn test_route_input(tenant_id: Option<TenantId>) -> CreateRoute {
        CreateRoute {
            tenant_id,
            model_alias: "gpt-4".into(),
            provider: "openai".into(),
            provider_model_name: "gpt-4-turbo".into(),
            priority: Some(100),
            enabled: Some(true),
            timeout_ms: Some(30000),
            max_retries: Some(2),
            retry_backoff_ms: Some(500),
            config: Some(serde_json::json!({"api_version": "2024-01"})),
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_and_get(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);

        let input = test_route_input(Some(tenant.id));
        let created = repo.create(&input).await.expect("create route");

        assert_eq!(created.model_alias, "gpt-4");
        assert_eq!(created.provider, Provider::Openai);
        assert_eq!(created.tenant_id, Some(tenant.id));
        assert_eq!(created.priority, 100);
        assert!(created.enabled);

        let fetched = repo.get_by_id(created.id).await.expect("get route");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.model_alias, created.model_alias);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_routes_priority_order(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);

        // Create routes with different priorities
        let mut low_priority = test_route_input(Some(tenant.id));
        low_priority.priority = Some(200);
        low_priority.provider_model_name = "gpt-4-low".into();
        repo.create(&low_priority).await.expect("create low");

        let mut high_priority = test_route_input(Some(tenant.id));
        high_priority.priority = Some(10);
        high_priority.provider_model_name = "gpt-4-high".into();
        repo.create(&high_priority).await.expect("create high");

        let mut mid_priority = test_route_input(Some(tenant.id));
        mid_priority.priority = Some(50);
        mid_priority.provider_model_name = "gpt-4-mid".into();
        repo.create(&mid_priority).await.expect("create mid");

        let routes = repo
            .resolve_routes(tenant.id, "gpt-4")
            .await
            .expect("resolve routes");

        assert_eq!(routes.len(), 3);
        assert_eq!(routes[0].provider_model_name, "gpt-4-high");
        assert_eq!(routes[1].provider_model_name, "gpt-4-mid");
        assert_eq!(routes[2].provider_model_name, "gpt-4-low");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_routes_tenant_before_global(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);

        // Global route (tenant_id = NULL) with higher priority number (lower priority)
        let mut global = test_route_input(None);
        global.priority = Some(10); // Even though priority is better...
        global.provider_model_name = "gpt-4-global".into();
        repo.create(&global).await.expect("create global");

        // Tenant-scoped route with lower priority number
        let mut scoped = test_route_input(Some(tenant.id));
        scoped.priority = Some(50);
        scoped.provider_model_name = "gpt-4-scoped".into();
        repo.create(&scoped).await.expect("create scoped");

        let routes = repo
            .resolve_routes(tenant.id, "gpt-4")
            .await
            .expect("resolve routes");

        assert_eq!(routes.len(), 2);
        // Tenant-scoped should come first even though global has better priority
        assert_eq!(routes[0].provider_model_name, "gpt-4-scoped");
        assert_eq!(routes[1].provider_model_name, "gpt-4-global");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolve_routes_excludes_disabled(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);

        let mut enabled = test_route_input(Some(tenant.id));
        enabled.provider_model_name = "gpt-4-enabled".into();
        repo.create(&enabled).await.expect("create enabled");

        let mut disabled = test_route_input(Some(tenant.id));
        disabled.enabled = Some(false);
        disabled.provider_model_name = "gpt-4-disabled".into();
        repo.create(&disabled).await.expect("create disabled");

        let routes = repo
            .resolve_routes(tenant.id, "gpt-4")
            .await
            .expect("resolve routes");

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].provider_model_name, "gpt-4-enabled");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn update_route(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);

        let created = repo
            .create(&test_route_input(Some(tenant.id)))
            .await
            .expect("create route");

        let update = UpdateRoute {
            model_alias: None,
            provider: None,
            provider_model_name: Some("gpt-4o".into()),
            priority: Some(50),
            enabled: None,
            timeout_ms: None,
            max_retries: None,
            retry_backoff_ms: None,
            config: None,
        };

        let updated = repo
            .update(created.id, &update)
            .await
            .expect("update route");
        assert_eq!(updated.provider_model_name, "gpt-4o");
        assert_eq!(updated.priority, 50);
        // Unchanged fields should keep original values
        assert_eq!(updated.provider, Provider::Openai);
        assert!(updated.enabled);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn delete_route(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = RouteRepo::new(pool);

        let created = repo
            .create(&test_route_input(Some(tenant.id)))
            .await
            .expect("create route");

        repo.delete(created.id).await.expect("delete route");

        let err = repo.get_by_id(created.id).await;
        assert!(
            matches!(err, Err(StorageError::NotFound { .. })),
            "should be not found after deletion"
        );
    }
}
