//! Service account repository -- CRUD operations for the `service_accounts` table.

use sqlx::PgPool;

use crate::models::{
    CreateServiceAccount, Cursor, Page, ServiceAccount, UpdateServiceAccount, clamp_limit,
};
use crate::storage::StorageError;
use crate::types::{ServiceAccountId, ServiceAccountStatus, TenantId};

/// Repository for service account persistence.
#[derive(Clone)]
pub struct ServiceAccountRepo {
    pool: PgPool,
}

impl ServiceAccountRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new service account and return the created row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Conflict`] if the (tenant_id, slug) pair already exists,
    /// [`StorageError::ReferenceError`] if the tenant does not exist,
    /// or [`StorageError::Database`] on connection / query failure.
    pub async fn create(
        &self,
        input: &CreateServiceAccount,
    ) -> Result<ServiceAccount, StorageError> {
        sqlx::query_as::<_, ServiceAccount>(
            r"INSERT INTO service_accounts (id, tenant_id, name, slug, environment, description, status, default_policy_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, tenant_id, name, slug, environment, description,
                         status, default_policy_id, created_at, updated_at",
        )
        .bind(input.id)
        .bind(input.tenant_id)
        .bind(&input.name)
        .bind(&input.slug)
        .bind(input.environment)
        .bind(&input.description)
        .bind(input.status)
        .bind(input.default_policy_id)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Fetch a single service account by primary key without tenant scoping.
    ///
    /// Used internally by the auth middleware where the caller only has a
    /// `service_account_id` (from the key record) and no tenant context yet.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the service account does not exist.
    pub async fn get_by_id_unscoped(
        &self,
        id: ServiceAccountId,
    ) -> Result<ServiceAccount, StorageError> {
        sqlx::query_as::<_, ServiceAccount>(
            r"SELECT id, tenant_id, name, slug, environment, description,
                      status, default_policy_id, created_at, updated_at
               FROM service_accounts
               WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("service_account", "id", id))
    }

    /// Fetch a single service account by primary key, scoped to tenant.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the service account does not exist
    /// within the given tenant.
    pub async fn get_by_id(
        &self,
        tenant_id: TenantId,
        id: ServiceAccountId,
    ) -> Result<ServiceAccount, StorageError> {
        sqlx::query_as::<_, ServiceAccount>(
            r"SELECT id, tenant_id, name, slug, environment, description,
                      status, default_policy_id, created_at, updated_at
               FROM service_accounts
               WHERE id = $1 AND tenant_id = $2",
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("service_account", "id", id))
    }

    /// List service accounts for a tenant with cursor-based pagination.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection / query failure.
    pub async fn list_by_tenant(
        &self,
        tenant_id: TenantId,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page<ServiceAccount>, StorageError> {
        let (limit, fetch_limit) = clamp_limit(limit);

        let rows = if let Some(c) = cursor {
            let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
            sqlx::query_as::<_, ServiceAccount>(
                r"SELECT id, tenant_id, name, slug, environment, description,
                          status, default_policy_id, created_at, updated_at
                   FROM service_accounts
                   WHERE tenant_id = $1 AND (created_at, id) < ($2, $3)
                   ORDER BY created_at DESC, id DESC
                   LIMIT $4",
            )
            .bind(tenant_id)
            .bind(ts)
            .bind(id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ServiceAccount>(
                r"SELECT id, tenant_id, name, slug, environment, description,
                          status, default_policy_id, created_at, updated_at
                   FROM service_accounts
                   WHERE tenant_id = $1
                   ORDER BY created_at DESC, id DESC
                   LIMIT $2",
            )
            .bind(tenant_id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(Page::from_rows(rows, limit, |sa| (sa.created_at, sa.id.0)))
    }

    /// Update mutable fields on a service account, scoped to tenant.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the service account does not exist
    /// within the given tenant.
    pub async fn update(
        &self,
        tenant_id: TenantId,
        id: ServiceAccountId,
        input: &UpdateServiceAccount,
    ) -> Result<ServiceAccount, StorageError> {
        // Read-before-write is intentional: we merge Optional fields with
        // existing values before the UPDATE (the SQL does not use COALESCE).
        let existing = self.get_by_id(tenant_id, id).await?;

        let name = input.name.as_deref().unwrap_or(&existing.name);
        let description = input
            .description
            .as_deref()
            .or(existing.description.as_deref());

        sqlx::query_as::<_, ServiceAccount>(
            r"UPDATE service_accounts
               SET name = $1, description = $2
               WHERE id = $3 AND tenant_id = $4
               RETURNING id, tenant_id, name, slug, environment, description,
                         status, default_policy_id, created_at, updated_at",
        )
        .bind(name)
        .bind(description)
        .bind(id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Suspend a service account.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the service account does not exist
    /// within the given tenant.
    pub async fn suspend(
        &self,
        tenant_id: TenantId,
        id: ServiceAccountId,
    ) -> Result<ServiceAccount, StorageError> {
        sqlx::query_as::<_, ServiceAccount>(
            r"UPDATE service_accounts
               SET status = $1
               WHERE id = $2 AND tenant_id = $3
               RETURNING id, tenant_id, name, slug, environment, description,
                         status, default_policy_id, created_at, updated_at",
        )
        .bind(ServiceAccountStatus::Suspended)
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("service_account", "id", id))
    }

    /// Activate a (previously suspended) service account.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the service account does not exist
    /// within the given tenant.
    pub async fn activate(
        &self,
        tenant_id: TenantId,
        id: ServiceAccountId,
    ) -> Result<ServiceAccount, StorageError> {
        sqlx::query_as::<_, ServiceAccount>(
            r"UPDATE service_accounts
               SET status = $1
               WHERE id = $2 AND tenant_id = $3
               RETURNING id, tenant_id, name, slug, environment, description,
                         status, default_policy_id, created_at, updated_at",
        )
        .bind(ServiceAccountStatus::Active)
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("service_account", "id", id))
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateTenant;
    use crate::storage::repositories::tenants::TenantRepo;
    use crate::types::Environment;

    async fn create_test_tenant(pool: &PgPool) -> crate::models::Tenant {
        let repo = TenantRepo::new(pool.clone());
        repo.create(&CreateTenant {
            id: TenantId::new(),
            name: format!("Test {}", uuid::Uuid::new_v4()),
            slug: format!("test-{}", uuid::Uuid::new_v4()),
            status: crate::types::TenantStatus::Active,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("create tenant")
    }

    async fn create_test_service_account(pool: &PgPool, tenant_id: TenantId) -> ServiceAccount {
        let repo = ServiceAccountRepo::new(pool.clone());
        repo.create(&CreateServiceAccount {
            id: ServiceAccountId::new(),
            tenant_id,
            name: format!("SA {}", uuid::Uuid::new_v4()),
            slug: format!("sa-{}", uuid::Uuid::new_v4()),
            environment: Environment::Dev,
            description: Some("test service account".to_owned()),
            status: ServiceAccountStatus::Active,
            default_policy_id: None,
        })
        .await
        .expect("create service account")
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_and_get(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let repo = ServiceAccountRepo::new(pool.clone());

        let input = CreateServiceAccount {
            id: ServiceAccountId::new(),
            tenant_id: tenant.id,
            name: "My SA".to_owned(),
            slug: "my-sa".to_owned(),
            environment: Environment::Prod,
            description: Some("production account".to_owned()),
            status: ServiceAccountStatus::Active,
            default_policy_id: None,
        };

        let created = repo.create(&input).await.expect("create should succeed");
        assert_eq!(created.name, "My SA");
        assert_eq!(created.tenant_id, tenant.id);
        assert_eq!(created.environment, Environment::Prod);
        assert_eq!(created.status, ServiceAccountStatus::Active);
        assert_eq!(created.description.as_deref(), Some("production account"));

        let fetched = repo
            .get_by_id(tenant.id, created.id)
            .await
            .expect("get_by_id should succeed");
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_by_tenant(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;

        for _ in 0..4 {
            create_test_service_account(&pool, tenant.id).await;
        }

        let repo = ServiceAccountRepo::new(pool.clone());

        // First page
        let page1 = repo
            .list_by_tenant(tenant.id, None, 2)
            .await
            .expect("list should succeed");
        assert_eq!(page1.items.len(), 2);
        assert!(page1.has_more);

        // Second page
        let page2 = repo
            .list_by_tenant(tenant.id, page1.next_cursor.as_ref(), 2)
            .await
            .expect("list page 2 should succeed");
        assert_eq!(page2.items.len(), 2);
        assert!(!page2.has_more);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn suspend_and_activate(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_service_account(&pool, tenant.id).await;
        assert_eq!(sa.status, ServiceAccountStatus::Active);

        let repo = ServiceAccountRepo::new(pool.clone());

        let suspended = repo
            .suspend(tenant.id, sa.id)
            .await
            .expect("suspend should succeed");
        assert_eq!(suspended.status, ServiceAccountStatus::Suspended);

        let activated = repo
            .activate(tenant.id, sa.id)
            .await
            .expect("activate should succeed");
        assert_eq!(activated.status, ServiceAccountStatus::Active);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn cross_tenant_isolation(pool: PgPool) {
        let tenant_a = create_test_tenant(&pool).await;
        let tenant_b = create_test_tenant(&pool).await;

        let sa = create_test_service_account(&pool, tenant_a.id).await;

        let repo = ServiceAccountRepo::new(pool.clone());

        // Querying from tenant B should not find tenant A's service account.
        let result = repo.get_by_id(tenant_b.id, sa.id).await;
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "expected NotFound for cross-tenant query, got {result:?}"
        );

        // List for tenant B should be empty.
        let page = repo
            .list_by_tenant(tenant_b.id, None, 100)
            .await
            .expect("list should succeed");
        assert!(
            page.items.is_empty(),
            "tenant B should have no service accounts"
        );
    }
}
