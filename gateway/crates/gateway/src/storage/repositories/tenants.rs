//! Tenant repository -- CRUD operations for the `tenants` table.

use sqlx::PgPool;

use crate::models::{CreateTenant, Cursor, Page, Tenant, UpdateTenant, clamp_limit};
use crate::storage::StorageError;
use crate::types::{TenantId, TenantStatus};

/// Repository for tenant persistence.
#[derive(Clone)]
pub struct TenantRepo {
    pool: PgPool,
}

impl TenantRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a new tenant and return the created row.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Conflict`] if the slug already exists,
    /// or [`StorageError::Database`] on connection / query failure.
    pub async fn create(&self, input: &CreateTenant) -> Result<Tenant, StorageError> {
        let metadata = input.metadata.clone();

        sqlx::query_as::<_, Tenant>(
            r"INSERT INTO tenants (id, name, slug, status, metadata)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, name, slug, status, metadata, created_at, updated_at",
        )
        .bind(input.id)
        .bind(&input.name)
        .bind(&input.slug)
        .bind(input.status)
        .bind(&metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Fetch a single tenant by primary key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no tenant with the given ID exists.
    pub async fn get_by_id(&self, id: TenantId) -> Result<Tenant, StorageError> {
        sqlx::query_as::<_, Tenant>(
            "SELECT id, name, slug, status, metadata, created_at, updated_at FROM tenants WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("tenant", "id", id))
    }

    /// Fetch a single tenant by its unique slug.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no tenant with the given slug exists.
    pub async fn get_by_slug(&self, slug: &str) -> Result<Tenant, StorageError> {
        sqlx::query_as::<_, Tenant>(
            "SELECT id, name, slug, status, metadata, created_at, updated_at FROM tenants WHERE slug = $1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("tenant", "slug", slug))
    }

    /// List tenants with cursor-based pagination and optional status filter.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection / query failure.
    pub async fn list(
        &self,
        cursor: Option<&Cursor>,
        limit: i64,
        status: Option<TenantStatus>,
    ) -> Result<Page<Tenant>, StorageError> {
        let (limit, fetch_limit) = clamp_limit(limit);

        let (cursor_ts, cursor_id) = match cursor {
            Some(c) => {
                let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
                (Some(ts), Some(id))
            }
            None => (None, None),
        };

        let rows = sqlx::query_as::<_, Tenant>(
            r"SELECT id, name, slug, status, metadata, created_at, updated_at
               FROM tenants
               WHERE ($1::tenant_status IS NULL OR status = $1)
                 AND (
                     $2::TIMESTAMPTZ IS NULL
                     OR (created_at, id) < ($2, $3::UUID)
                 )
               ORDER BY created_at DESC, id DESC
               LIMIT $4",
        )
        .bind(status)
        .bind(cursor_ts)
        .bind(cursor_id)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(Page::from_rows(rows, limit, |t| (t.created_at, t.id.0)))
    }

    /// Update mutable fields on a tenant. Only non-`None` fields are applied.
    ///
    /// Uses a single `COALESCE` query to avoid TOCTOU races.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the tenant does not exist,
    /// or [`StorageError::Database`] on connection / query failure.
    pub async fn update(&self, id: TenantId, input: &UpdateTenant) -> Result<Tenant, StorageError> {
        sqlx::query_as::<_, Tenant>(
            r"UPDATE tenants
               SET name     = COALESCE($1, name),
                   metadata = COALESCE($2, metadata)
               WHERE id = $3
               RETURNING id, name, slug, status, metadata, created_at, updated_at",
        )
        .bind(input.name.as_deref())
        .bind(input.metadata.as_ref())
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if matches!(&e, sqlx::Error::RowNotFound) {
                StorageError::not_found("tenant", "id", id)
            } else {
                StorageError::from(e)
            }
        })
    }

    /// Soft-delete a tenant by setting status to `deleted`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if the tenant does not exist.
    ///
    /// Also suspends all active service accounts belonging to the tenant
    /// to prevent further data-plane authentication.
    pub async fn soft_delete(&self, id: TenantId) -> Result<Tenant, StorageError> {
        let mut tx = self.pool.begin().await?;

        // Cascade: suspend all non-terminal service accounts for this tenant.
        sqlx::query(
            "UPDATE service_accounts SET status = 'suspended' WHERE tenant_id = $1 AND status NOT IN ('suspended', 'deleted')",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        let tenant = sqlx::query_as::<_, Tenant>(
            r"UPDATE tenants
               SET status = $1
               WHERE id = $2
               RETURNING id, name, slug, status, metadata, created_at, updated_at",
        )
        .bind(TenantStatus::Deleted)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| StorageError::not_found("tenant", "id", id))?;

        tx.commit().await?;
        Ok(tenant)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_tenant(pool: &PgPool) -> Tenant {
        let repo = TenantRepo::new(pool.clone());
        repo.create(&CreateTenant {
            id: TenantId::new(),
            name: format!("Test {}", uuid::Uuid::new_v4()),
            slug: format!("test-{}", uuid::Uuid::new_v4()),
            status: TenantStatus::Active,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("create tenant")
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn create_and_get(pool: PgPool) {
        let repo = TenantRepo::new(pool.clone());
        let input = CreateTenant {
            id: TenantId::new(),
            name: "Acme Corp".to_owned(),
            slug: "acme-corp".to_owned(),
            status: TenantStatus::Active,
            metadata: serde_json::json!({"tier": "enterprise"}),
        };

        let created = repo.create(&input).await.expect("create should succeed");
        assert_eq!(created.name, "Acme Corp");
        assert_eq!(created.slug, "acme-corp");
        assert_eq!(created.status, TenantStatus::Active);
        assert_eq!(created.metadata["tier"], "enterprise");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("get_by_id should succeed");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, created.name);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_by_slug(pool: PgPool) {
        let repo = TenantRepo::new(pool.clone());
        let slug = format!("slug-{}", uuid::Uuid::new_v4());
        let created = repo
            .create(&CreateTenant {
                id: TenantId::new(),
                name: "By Slug".to_owned(),
                slug: slug.clone(),
                status: TenantStatus::Active,
                metadata: serde_json::json!({}),
            })
            .await
            .expect("create should succeed");

        let fetched = repo
            .get_by_slug(&slug)
            .await
            .expect("get_by_slug should succeed");
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn duplicate_slug_conflict(pool: PgPool) {
        let repo = TenantRepo::new(pool.clone());
        let slug = format!("dup-{}", uuid::Uuid::new_v4());
        repo.create(&CreateTenant {
            id: TenantId::new(),
            name: "First".to_owned(),
            slug: slug.clone(),
            status: TenantStatus::Active,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("first create should succeed");

        let result = repo
            .create(&CreateTenant {
                id: TenantId::new(),
                name: "Second".to_owned(),
                slug,
                status: TenantStatus::Active,
                metadata: serde_json::json!({}),
            })
            .await;

        assert!(
            matches!(result, Err(StorageError::Conflict { .. })),
            "expected Conflict, got {result:?}"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_with_pagination(pool: PgPool) {
        let repo = TenantRepo::new(pool.clone());

        // Create 5 tenants
        for i in 0..5 {
            repo.create(&CreateTenant {
                id: TenantId::new(),
                name: format!("Tenant {i}"),
                slug: format!("tenant-{}-{}", i, uuid::Uuid::new_v4()),
                status: TenantStatus::Active,
                metadata: serde_json::json!({}),
            })
            .await
            .expect("create should succeed");
        }

        // First page of 3
        let page1 = repo.list(None, 3, None).await.expect("list should succeed");
        assert_eq!(page1.items.len(), 3);
        assert!(page1.has_more);
        assert!(page1.next_cursor.is_some());

        // Second page using cursor
        let page2 = repo
            .list(page1.next_cursor.as_ref(), 3, None)
            .await
            .expect("list page 2 should succeed");
        assert_eq!(page2.items.len(), 2);
        assert!(!page2.has_more);
        assert!(page2.next_cursor.is_none());

        // No overlap between pages
        let page1_ids: Vec<_> = page1.items.iter().map(|t| t.id).collect();
        for t in &page2.items {
            assert!(
                !page1_ids.contains(&t.id),
                "page 2 should not overlap with page 1"
            );
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn update(pool: PgPool) {
        let repo = TenantRepo::new(pool.clone());
        let tenant = create_test_tenant(&pool).await;

        let updated = repo
            .update(
                tenant.id,
                &UpdateTenant {
                    name: Some("New Name".to_owned()),
                    slug: None,
                    status: None,
                    metadata: Some(serde_json::json!({"updated": true})),
                },
            )
            .await
            .expect("update should succeed");

        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.metadata["updated"], true);
        assert_eq!(updated.slug, tenant.slug); // slug unchanged
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn soft_delete(pool: PgPool) {
        let repo = TenantRepo::new(pool.clone());
        let tenant = create_test_tenant(&pool).await;
        assert_eq!(tenant.status, TenantStatus::Active);

        let deleted = repo
            .soft_delete(tenant.id)
            .await
            .expect("soft_delete should succeed");
        assert_eq!(deleted.status, TenantStatus::Deleted);

        // Verify via fresh fetch
        let fetched = repo
            .get_by_id(tenant.id)
            .await
            .expect("get_by_id should succeed");
        assert_eq!(fetched.status, TenantStatus::Deleted);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_nonexistent_not_found(pool: PgPool) {
        let repo = TenantRepo::new(pool);
        let result = repo.get_by_id(TenantId::new()).await;
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "expected NotFound, got {result:?}"
        );
    }
}
