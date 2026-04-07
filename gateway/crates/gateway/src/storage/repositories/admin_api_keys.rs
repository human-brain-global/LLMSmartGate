//! Repository for admin API key operations.

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::AdminApiKey;
use crate::storage::StorageError;

/// Repository for admin API key CRUD operations.
pub struct AdminApiKeyRepo;

impl AdminApiKeyRepo {
    /// List all active admin API keys.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError::Database` if the query fails.
    pub async fn list_active(pool: &PgPool) -> Result<Vec<AdminApiKey>, StorageError> {
        let keys = sqlx::query_as::<_, AdminApiKey>(
            "SELECT id, name, key_hash, role, is_active, created_at, last_used_at
             FROM admin_api_keys
             WHERE is_active = TRUE
             ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;
        Ok(keys)
    }

    /// Update the `last_used_at` timestamp for a given admin API key.
    ///
    /// This is fire-and-forget -- callers should not block on the result.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError::Database` if the query fails.
    pub async fn touch_last_used(pool: &PgPool, id: Uuid) -> Result<(), StorageError> {
        sqlx::query("UPDATE admin_api_keys SET last_used_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
