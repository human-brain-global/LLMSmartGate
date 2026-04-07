//! Redis/Valkey connection pool, health check, and typed client helpers.

use deadpool_redis::{Config, Connection, Pool, Runtime};
use redis::AsyncCommands;

use crate::config::RedisConfig;
use crate::storage::StorageError;

/// Create a Redis/Valkey connection pool from configuration.
///
/// # Errors
///
/// Returns [`StorageError::Redis`] if the pool cannot be created.
pub fn create_redis_pool(config: &RedisConfig) -> Result<Pool, StorageError> {
    create_redis_pool_from_url(config.url.expose(), config.pool_size as usize)
}

/// Create a Redis pool from a URL string and pool size.
///
/// # Errors
///
/// Returns [`StorageError::Redis`] if the URL is invalid or the pool cannot be built.
pub fn create_redis_pool_from_url(url: &str, pool_size: usize) -> Result<Pool, StorageError> {
    let cfg = Config::from_url(url);
    let pool = cfg
        .builder()
        .map_err(|e| StorageError::Redis(e.to_string()))?
        .max_size(pool_size)
        .runtime(Runtime::Tokio1)
        .build()
        .map_err(|e| StorageError::Redis(e.to_string()))?;

    tracing::info!(pool_size, "Redis connection pool created");
    Ok(pool)
}

/// Check Redis connectivity (used by /readyz endpoint).
///
/// # Errors
///
/// Returns [`StorageError::Redis`] if the PING command fails.
pub async fn redis_health_check(pool: &Pool) -> Result<(), StorageError> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| StorageError::Redis(e.to_string()))?;
    redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .map_err(|e| StorageError::Redis(e.to_string()))?;
    Ok(())
}

/// Typed Redis client wrapping a connection pool.
#[derive(Clone)]
pub struct RedisClient {
    pool: Pool,
}

impl RedisClient {
    /// Create a new `RedisClient` backed by the given pool.
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Acquire a connection from the pool.
    async fn conn(&self) -> Result<Connection, StorageError> {
        self.pool
            .get()
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Get a string value by key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut conn = self.conn().await?;
        conn.get(key)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Set a string value.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let mut conn = self.conn().await?;
        conn.set(key, value)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Set a value with expiry in seconds.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn set_ex(&self, key: &str, value: &str, seconds: u64) -> Result<(), StorageError> {
        let mut conn = self.conn().await?;
        conn.set_ex(key, value, seconds)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Increment a key by 1, returning the new value.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn incr(&self, key: &str) -> Result<i64, StorageError> {
        let mut conn = self.conn().await?;
        conn.incr(key, 1i64)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Decrement a key by 1, returning the new value.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn decr(&self, key: &str) -> Result<i64, StorageError> {
        let mut conn = self.conn().await?;
        conn.decr(key, 1i64)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Check if a key exists.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let mut conn = self.conn().await?;
        conn.exists(key)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }

    /// Delete a key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Redis`] if the command fails.
    pub async fn del(&self, key: &str) -> Result<(), StorageError> {
        let mut conn = self.conn().await?;
        conn.del(key)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Try to create a pool from the `VALKEY_URL` env var (or localhost default).
    /// Returns `None` if Redis is not available, allowing tests to skip gracefully.
    async fn test_pool() -> Option<Pool> {
        let url =
            std::env::var("VALKEY_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let pool = create_redis_pool_from_url(&url, 2).ok()?;
        // Verify the pool can actually connect before returning it.
        pool.get().await.ok()?;
        Some(pool)
    }

    /// Generate a unique key for each test to avoid collisions.
    fn test_key(suffix: &str) -> String {
        format!("llmsmartgate:test:{}:{}", suffix, uuid::Uuid::new_v4())
    }

    #[tokio::test]
    async fn pool_creation_succeeds() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        // Pool was created, verify we can get a connection.
        let conn = pool.get().await;
        assert!(conn.is_ok(), "should obtain a connection from the pool");
    }

    #[tokio::test]
    async fn health_check_ping_succeeds() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let result = redis_health_check(&pool).await;
        assert!(result.is_ok(), "PING health check should succeed");
    }

    #[tokio::test]
    async fn set_and_get_value() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let client = RedisClient::new(pool);
        let key = test_key("set_get");

        client.set(&key, "hello").await.expect("SET should succeed");
        let value = client.get(&key).await.expect("GET should succeed");
        assert_eq!(value.as_deref(), Some("hello"));

        // Cleanup
        client.del(&key).await.expect("DEL should succeed");
    }

    #[tokio::test]
    async fn set_ex_with_ttl() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let client = RedisClient::new(pool.clone());
        let key = test_key("set_ex");

        client
            .set_ex(&key, "expires-soon", 10)
            .await
            .expect("SET EX should succeed");

        let value = client.get(&key).await.expect("GET should succeed");
        assert_eq!(value.as_deref(), Some("expires-soon"));

        // Verify TTL was set (should be > 0 and <= 10).
        let mut conn = pool.get().await.expect("should get connection");
        let ttl: i64 = redis::cmd("TTL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .expect("TTL query should succeed");
        assert!(ttl > 0, "TTL should be positive, got {ttl}");
        assert!(ttl <= 10, "TTL should be <= 10, got {ttl}");

        // Cleanup
        client.del(&key).await.expect("DEL should succeed");
    }

    #[tokio::test]
    async fn increment_and_decrement() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let client = RedisClient::new(pool);
        let key = test_key("incr_decr");

        let v1 = client.incr(&key).await.expect("INCR should succeed");
        assert_eq!(v1, 1, "first INCR on missing key should return 1");

        let v2 = client.incr(&key).await.expect("INCR should succeed");
        assert_eq!(v2, 2, "second INCR should return 2");

        let v3 = client.decr(&key).await.expect("DECR should succeed");
        assert_eq!(v3, 1, "DECR should return 1");

        // Cleanup
        client.del(&key).await.expect("DEL should succeed");
    }

    #[tokio::test]
    async fn delete_key() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let client = RedisClient::new(pool);
        let key = test_key("del");

        client
            .set(&key, "to-delete")
            .await
            .expect("SET should succeed");
        client.del(&key).await.expect("DEL should succeed");

        let value = client.get(&key).await.expect("GET should succeed");
        assert_eq!(value, None, "key should be gone after DEL");
    }

    #[tokio::test]
    async fn check_existence() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let client = RedisClient::new(pool);
        let key = test_key("exists");

        let exists_before = client.exists(&key).await.expect("EXISTS should succeed");
        assert!(!exists_before, "key should not exist before SET");

        client
            .set(&key, "present")
            .await
            .expect("SET should succeed");
        let exists_after = client.exists(&key).await.expect("EXISTS should succeed");
        assert!(exists_after, "key should exist after SET");

        // Cleanup
        client.del(&key).await.expect("DEL should succeed");
    }

    #[tokio::test]
    async fn get_missing_key_returns_none() {
        let Some(pool) = test_pool().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let client = RedisClient::new(pool);
        let key = test_key("missing");

        let value = client.get(&key).await.expect("GET should succeed");
        assert_eq!(value, None, "missing key should return None");
    }

    #[test]
    fn create_pool_from_invalid_url_fails() {
        let result = create_redis_pool_from_url("not-a-valid-url", 2);
        assert!(result.is_err(), "invalid URL should return an error");
    }
}
