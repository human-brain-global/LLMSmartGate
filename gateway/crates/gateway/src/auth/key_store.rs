//! Key store with 3-tier caching: moka L1 (in-process) → Redis L2 → PostgreSQL L3.
//!
//! Provides fast key lookups for the auth hot path. Cache invalidation uses
//! TTL expiry as a baseline. Active invalidation via [`KeyStore::invalidate`]
//! is called on key revocation to prevent compromised or revoked keys from
//! authenticating during the remaining TTL window.

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use sqlx::PgPool;

use crate::error::GatewayError;
use crate::models::ServiceAccountKey;
use crate::storage::redis::RedisClient;
use crate::storage::repositories::keys::KeyRepo;

/// TTL for the in-process moka L1 cache (seconds).
const L1_TTL_SECS: u64 = 60;
/// Maximum entries in the moka L1 cache.
const L1_MAX_CAPACITY: u64 = 10_000;
/// TTL for the Redis L2 cache (seconds).
const L2_TTL_SECS: u64 = 300;
/// Redis key prefix for cached keys.
const L2_KEY_PREFIX: &str = "keycache:";

/// Three-tier key cache for fast auth lookups.
///
/// L1: in-process moka cache (~1us lookup, 60s TTL, 10k capacity)
/// L2: Redis cache (~0.5ms lookup, 300s TTL)
/// L3: PostgreSQL (source of truth, ~2-5ms lookup)
#[derive(Clone)]
pub struct KeyStore {
    l1: Cache<String, Arc<ServiceAccountKey>>,
    redis: Option<RedisClient>,
    repo: KeyRepo,
}

impl KeyStore {
    /// Create a new `KeyStore` backed by the given database pool and optional Redis client.
    pub fn new(db: PgPool, redis: Option<RedisClient>) -> Self {
        let l1 = Cache::builder()
            .max_capacity(L1_MAX_CAPACITY)
            .time_to_live(Duration::from_secs(L1_TTL_SECS))
            .build();

        Self {
            l1,
            redis,
            repo: KeyRepo::new(db),
        }
    }

    /// Look up a key by its globally-unique `key_id` string.
    ///
    /// Checks L1 (moka) → L2 (Redis) → L3 (PostgreSQL) in order.
    /// On cache miss, populates the lower caches for subsequent lookups.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::Auth` with code `unknown_key` if the key does not exist.
    pub async fn get_by_key_id(
        &self,
        key_id: &str,
    ) -> Result<Arc<ServiceAccountKey>, GatewayError> {
        // L1: in-process cache
        if let Some(key) = self.l1.get(key_id).await {
            return Ok(key);
        }

        // L2: Redis cache
        if let Some(redis) = &self.redis {
            if let Some(key) = self.get_from_redis(redis, key_id).await {
                self.l1.insert(key_id.to_owned(), Arc::clone(&key)).await;
                return Ok(key);
            }
        }

        // L3: PostgreSQL (source of truth)
        let key = self.repo.get_by_key_id(key_id).await.map_err(|e| {
            tracing::warn!(key_id = %key_id, error = %e, "auth: key lookup failed");
            GatewayError::auth("unknown_key", format!("key '{key_id}' not found"))
        })?;

        let key = Arc::new(key);

        // Populate L1
        self.l1.insert(key_id.to_owned(), Arc::clone(&key)).await;

        // Populate L2 (best-effort, don't fail the request)
        if let Some(redis) = &self.redis {
            self.set_in_redis(redis, key_id, &key).await;
        }

        Ok(key)
    }

    /// Invalidate a key from both L1 (moka) and L2 (Redis) caches.
    ///
    /// Called after key revocation to ensure the revoked key stops
    /// authenticating immediately rather than waiting for TTL expiry.
    pub async fn invalidate(&self, key_id: &str) {
        self.l1.invalidate(key_id).await;

        if let Some(redis) = &self.redis {
            let redis_key = format!("{L2_KEY_PREFIX}{key_id}");
            if let Err(e) = redis.del(&redis_key).await {
                tracing::warn!(key_id = %key_id, error = %e, "failed to invalidate key from Redis cache");
            }
        }
    }

    /// Try to read a cached key from Redis L2.
    async fn get_from_redis(
        &self,
        redis: &RedisClient,
        key_id: &str,
    ) -> Option<Arc<ServiceAccountKey>> {
        let redis_key = format!("{L2_KEY_PREFIX}{key_id}");
        let json = redis.get(&redis_key).await.ok()??;
        let key: ServiceAccountKey = serde_json::from_str(&json).ok()?;
        Some(Arc::new(key))
    }

    /// Store a key in Redis L2 (best-effort, errors are logged and swallowed).
    async fn set_in_redis(&self, redis: &RedisClient, key_id: &str, key: &ServiceAccountKey) {
        let redis_key = format!("{L2_KEY_PREFIX}{key_id}");
        let json = match serde_json::to_string(key) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize key for Redis cache");
                return;
            }
        };
        if let Err(e) = redis.set_ex(&redis_key, &json, L2_TTL_SECS).await {
            tracing::warn!(error = %e, "failed to set key in Redis cache");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // L1 cache is tested with an in-memory setup (no DB/Redis needed)

    #[tokio::test]
    async fn l1_cache_returns_none_on_miss() {
        let l1: Cache<String, Arc<ServiceAccountKey>> = Cache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_secs(60))
            .build();

        let result = l1.get("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn l1_cache_hit_after_insert() {
        let l1: Cache<String, Arc<ServiceAccountKey>> = Cache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_secs(60))
            .build();

        let key = Arc::new(ServiceAccountKey {
            id: crate::types::KeyId::new(),
            service_account_id: crate::types::ServiceAccountId::new(),
            key_id: "test-key".to_owned(),
            algorithm: "ed25519".to_owned(),
            public_key_pem: "pem-data".to_owned(),
            fingerprint: "fp".to_owned(),
            status: crate::types::KeyStatus::Active,
            expires_at: None,
            created_at: chrono::Utc::now(),
            revoked_at: None,
            last_used_at: None,
        });

        l1.insert("test-key".to_owned(), Arc::clone(&key)).await;

        let result = l1.get("test-key").await;
        assert!(result.is_some());
        assert_eq!(result.expect("should be some").key_id, "test-key");
    }

    #[test]
    fn service_account_key_serializes_for_redis() {
        let key = ServiceAccountKey {
            id: crate::types::KeyId::new(),
            service_account_id: crate::types::ServiceAccountId::new(),
            key_id: "test-key".to_owned(),
            algorithm: "ed25519".to_owned(),
            public_key_pem: "pem-data".to_owned(),
            fingerprint: "fp".to_owned(),
            status: crate::types::KeyStatus::Active,
            expires_at: None,
            created_at: chrono::Utc::now(),
            revoked_at: None,
            last_used_at: None,
        };

        let json = serde_json::to_string(&key).expect("serialize");
        let deserialized: ServiceAccountKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.key_id, "test-key");
    }
}
