//! Key repository -- CRUD operations for the `service_account_keys` table.

use sqlx::PgPool;

use crate::models::{Cursor, Page, RegisterKey, ServiceAccountKey, clamp_limit};
use crate::storage::StorageError;
use crate::types::{KeyId, KeyStatus, ServiceAccountId};

/// Repository for service account key persistence.
#[derive(Clone)]
pub struct KeyRepo {
    pool: PgPool,
}

impl KeyRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register a new key for a service account.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Conflict`] if the `key_id` already exists,
    /// [`StorageError::ReferenceError`] if the service account does not exist,
    /// or [`StorageError::Database`] on connection / query failure.
    pub async fn register_key(
        &self,
        input: &RegisterKey,
    ) -> Result<ServiceAccountKey, StorageError> {
        sqlx::query_as::<_, ServiceAccountKey>(
            r"INSERT INTO service_account_keys
                   (service_account_id, key_id, algorithm, public_key_pem, fingerprint, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, service_account_id, key_id, algorithm, public_key_pem,
                         fingerprint, status, expires_at, created_at, revoked_at, last_used_at",
        )
        .bind(input.service_account_id)
        .bind(&input.key_id)
        .bind(&input.algorithm)
        .bind(&input.public_key_pem)
        .bind(&input.fingerprint)
        .bind(input.expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(StorageError::from)
    }

    /// Look up a key by its globally-unique `key_id` string.
    ///
    /// This does NOT include `tenant_id` because `key_id` is globally unique
    /// and this is the auth hot path.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no key with the given `key_id` exists.
    pub async fn get_by_key_id(&self, key_id: &str) -> Result<ServiceAccountKey, StorageError> {
        sqlx::query_as::<_, ServiceAccountKey>(
            r"SELECT id, service_account_id, key_id, algorithm, public_key_pem,
                      fingerprint, status, expires_at, created_at, revoked_at, last_used_at
               FROM service_account_keys
               WHERE key_id = $1",
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("key", "key_id", key_id))
    }

    /// List keys belonging to a service account with cursor-based pagination,
    /// ordered descending (newest first).
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection / query failure.
    pub async fn list_by_service_account(
        &self,
        service_account_id: ServiceAccountId,
        cursor: Option<&Cursor>,
        limit: i64,
    ) -> Result<Page<ServiceAccountKey>, StorageError> {
        let (limit, fetch_limit) = clamp_limit(limit);

        let rows = if let Some(c) = cursor {
            let (ts, id) = c.decode().map_err(StorageError::InvalidCursor)?;
            sqlx::query_as::<_, ServiceAccountKey>(
                r"SELECT id, service_account_id, key_id, algorithm, public_key_pem,
                          fingerprint, status, expires_at, created_at, revoked_at, last_used_at
                   FROM service_account_keys
                   WHERE service_account_id = $1 AND (created_at, id) < ($2, $3)
                   ORDER BY created_at DESC, id DESC
                   LIMIT $4",
            )
            .bind(service_account_id)
            .bind(ts)
            .bind(id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ServiceAccountKey>(
                r"SELECT id, service_account_id, key_id, algorithm, public_key_pem,
                          fingerprint, status, expires_at, created_at, revoked_at, last_used_at
                   FROM service_account_keys
                   WHERE service_account_id = $1
                   ORDER BY created_at DESC, id DESC
                   LIMIT $2",
            )
            .bind(service_account_id)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(Page::from_rows(rows, limit, |k| (k.created_at, k.id.0)))
    }

    /// Revoke a key by setting its status to `revoked` and recording the timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no key with the given ID exists.
    pub async fn revoke(&self, id: KeyId) -> Result<ServiceAccountKey, StorageError> {
        sqlx::query_as::<_, ServiceAccountKey>(
            r"UPDATE service_account_keys
               SET status = $1, revoked_at = NOW()
               WHERE id = $2
               RETURNING id, service_account_id, key_id, algorithm, public_key_pem,
                         fingerprint, status, expires_at, created_at, revoked_at, last_used_at",
        )
        .bind(KeyStatus::Revoked)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("key", "id", id))
    }

    /// Revoke a key scoped to a specific service account, preventing IDOR.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no key with the given ID and
    /// service account exists.
    pub async fn revoke_scoped(
        &self,
        id: KeyId,
        service_account_id: ServiceAccountId,
    ) -> Result<ServiceAccountKey, StorageError> {
        sqlx::query_as::<_, ServiceAccountKey>(
            r"UPDATE service_account_keys
               SET status = $1, revoked_at = NOW()
               WHERE id = $2 AND service_account_id = $3
               RETURNING id, service_account_id, key_id, algorithm, public_key_pem,
                         fingerprint, status, expires_at, created_at, revoked_at, last_used_at",
        )
        .bind(KeyStatus::Revoked)
        .bind(id)
        .bind(service_account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::not_found("key", "id", id))
    }

    /// Update the `last_used_at` timestamp (fire-and-forget in the hot path).
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] on connection / query failure.
    pub async fn update_last_used(&self, id: KeyId) -> Result<(), StorageError> {
        sqlx::query("UPDATE service_account_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateServiceAccount, CreateTenant};
    use crate::storage::repositories::service_accounts::ServiceAccountRepo;
    use crate::storage::repositories::tenants::TenantRepo;
    use crate::types::{Environment, ServiceAccountStatus, TenantId, TenantStatus};

    async fn create_test_tenant(pool: &PgPool) -> crate::models::Tenant {
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

    async fn create_test_service_account(
        pool: &PgPool,
        tenant_id: crate::types::TenantId,
    ) -> crate::models::ServiceAccount {
        let repo = ServiceAccountRepo::new(pool.clone());
        repo.create(&CreateServiceAccount {
            id: ServiceAccountId::new(),
            tenant_id,
            name: format!("SA {}", uuid::Uuid::new_v4()),
            slug: format!("sa-{}", uuid::Uuid::new_v4()),
            environment: Environment::Dev,
            description: None,
            status: ServiceAccountStatus::Active,
            default_policy_id: None,
        })
        .await
        .expect("create service account")
    }

    fn make_register_key(service_account_id: ServiceAccountId) -> RegisterKey {
        RegisterKey {
            service_account_id,
            key_id: format!("key-{}", uuid::Uuid::new_v4()),
            algorithm: "ed25519".to_owned(),
            public_key_pem:
                "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEA...\n-----END PUBLIC KEY-----"
                    .to_owned(),
            fingerprint: format!("fp-{}", uuid::Uuid::new_v4()),
            expires_at: None,
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn register_and_get(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_service_account(&pool, tenant.id).await;

        let repo = KeyRepo::new(pool.clone());
        let input = make_register_key(sa.id);
        let key_id_str = input.key_id.clone();

        let created = repo
            .register_key(&input)
            .await
            .expect("register_key should succeed");
        assert_eq!(created.key_id, key_id_str);
        assert_eq!(created.service_account_id, sa.id);
        assert_eq!(created.status, KeyStatus::Active);
        assert!(created.revoked_at.is_none());
        assert!(created.last_used_at.is_none());

        // Fetch by key_id (global lookup)
        let fetched = repo
            .get_by_key_id(&key_id_str)
            .await
            .expect("get_by_key_id should succeed");
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_by_service_account(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_service_account(&pool, tenant.id).await;

        let repo = KeyRepo::new(pool.clone());

        // Register 3 keys
        for _ in 0..3 {
            repo.register_key(&make_register_key(sa.id))
                .await
                .expect("register_key should succeed");
        }

        let page = repo
            .list_by_service_account(sa.id, None, 50)
            .await
            .expect("list should succeed");
        assert_eq!(page.items.len(), 3);
        assert!(!page.has_more);

        // All keys belong to this service account
        for key in &page.items {
            assert_eq!(key.service_account_id, sa.id);
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn revoke_key(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_service_account(&pool, tenant.id).await;

        let repo = KeyRepo::new(pool.clone());
        let created = repo
            .register_key(&make_register_key(sa.id))
            .await
            .expect("register_key should succeed");
        assert_eq!(created.status, KeyStatus::Active);

        let revoked = repo
            .revoke(created.id)
            .await
            .expect("revoke should succeed");
        assert_eq!(revoked.status, KeyStatus::Revoked);
        assert!(revoked.revoked_at.is_some());

        // Verify via fresh fetch
        let fetched = repo
            .get_by_key_id(&created.key_id)
            .await
            .expect("get_by_key_id should succeed");
        assert_eq!(fetched.status, KeyStatus::Revoked);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn update_last_used(pool: PgPool) {
        let tenant = create_test_tenant(&pool).await;
        let sa = create_test_service_account(&pool, tenant.id).await;

        let repo = KeyRepo::new(pool.clone());
        let created = repo
            .register_key(&make_register_key(sa.id))
            .await
            .expect("register_key should succeed");
        assert!(created.last_used_at.is_none());

        repo.update_last_used(created.id)
            .await
            .expect("update_last_used should succeed");

        let fetched = repo
            .get_by_key_id(&created.key_id)
            .await
            .expect("get_by_key_id should succeed");
        assert!(
            fetched.last_used_at.is_some(),
            "last_used_at should be set after update"
        );
    }
}
