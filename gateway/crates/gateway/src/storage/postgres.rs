//! PostgreSQL connection pool, health check, and migration runner.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::DatabaseConfig;
use crate::storage::StorageError;

/// Create a PostgreSQL connection pool from configuration.
///
/// # Errors
///
/// Returns [`StorageError::Database`] if the connection cannot be established.
pub async fn create_pg_pool(config: &DatabaseConfig) -> Result<PgPool, StorageError> {
    let pool = PgPoolOptions::new()
        .max_connections(config.pool_size)
        .min_connections(config.pool_size / 4)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .connect(config.url.expose())
        .await
        .map_err(StorageError::from)?;

    tracing::info!(
        max_connections = config.pool_size,
        "PostgreSQL connection pool created"
    );

    Ok(pool)
}

/// Run pending database migrations.
///
/// # Errors
///
/// Returns [`StorageError::Database`] if a migration fails to apply.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::migrate!("../../migrations")
        .run(pool)
        .await
        .map_err(|e| StorageError::Database(sqlx::Error::Migrate(Box::new(e))))?;

    tracing::info!("Database migrations applied");
    Ok(())
}

/// Check PostgreSQL connectivity (used by /readyz endpoint).
///
/// # Errors
///
/// Returns [`StorageError::Database`] if the database is unreachable.
pub async fn pg_health_check(pool: &PgPool) -> Result<(), StorageError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
        .map_err(StorageError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that pg_health_check succeeds on a valid pool
    #[sqlx::test(migrations = "../../migrations")]
    async fn health_check_succeeds(pool: PgPool) {
        let result = pg_health_check(&pool).await;
        assert!(result.is_ok());
    }

    // Test that migrations are applied (tables exist)
    #[sqlx::test(migrations = "../../migrations")]
    async fn migrations_create_tables(pool: PgPool) {
        // Verify core tables exist by querying information_schema
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'tenants')",
        )
        .fetch_one(&pool)
        .await
        .expect("query should succeed");
        assert!(exists, "tenants table should exist after migrations");
    }
}
