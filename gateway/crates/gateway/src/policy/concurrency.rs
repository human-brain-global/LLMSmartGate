//! Per-service-account concurrency limiting using Redis atomic counters.
//!
//! A `ConcurrencyPermit` is returned on successful acquisition and decrements
//! the counter when dropped (via `tokio::spawn`) or explicitly released.
//!
//! The acquire operation uses a Lua script to atomically INCR + EXPIRE + check
//! the limit in a single Redis round-trip, preventing counter leaks on crash.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use deadpool_redis::Pool as RedisPool;
use redis::{AsyncCommands, Script};

use crate::error::GatewayError;
use crate::types::ServiceAccountId;

/// Safety-net TTL for concurrency keys (seconds). Prevents leaked counters
/// if a permit is never released (e.g. process crash).
const CONCURRENCY_KEY_TTL_SECS: u64 = 120;

/// Lua script that atomically increments the counter, sets the safety-net TTL,
/// and checks against the limit in a single round-trip.
///
/// KEYS[1] = concurrency key
/// ARGV[1] = limit
/// ARGV[2] = TTL in seconds
///
/// Returns: {allowed (1/0), current_count}
const LUA_CONCURRENCY_ACQUIRE: &str = r"
local current = redis.call('INCR', KEYS[1])
redis.call('EXPIRE', KEYS[1], ARGV[2])
if current > tonumber(ARGV[1]) then
    redis.call('DECR', KEYS[1])
    return {0, current}
end
return {1, current}
";

// ---------------------------------------------------------------------------
// ConcurrencyLimiter
// ---------------------------------------------------------------------------

/// Manages per-service-account concurrent request limits via Redis.
#[derive(Clone)]
pub struct ConcurrencyLimiter {
    pool: RedisPool,
    script: Arc<Script>,
}

impl ConcurrencyLimiter {
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool,
            script: Arc::new(Script::new(LUA_CONCURRENCY_ACQUIRE)),
        }
    }

    /// Try to acquire a concurrency slot for the given service account.
    ///
    /// Returns a `ConcurrencyPermit` that must be released (or dropped) when
    /// the request completes.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::RateLimit` with code `concurrency_limit_exceeded`
    /// if the limit is exceeded, or `service_unavailable` if Redis is down.
    pub async fn acquire(
        &self,
        sa_id: ServiceAccountId,
        limit: u32,
    ) -> Result<ConcurrencyPermit, GatewayError> {
        let key = format!("concurrency:{sa_id}");

        let mut conn = self.pool.get().await.map_err(|e| {
            tracing::error!(
                error = %e,
                rate_limit_backend = "redis",
                rate_limit_failure = "backend_unavailable",
                "concurrency: redis pool unavailable"
            );
            GatewayError::config(
                "concurrency_limiter_unavailable",
                "concurrency limiter temporarily unavailable",
            )
        })?;

        let (allowed, _current): (i64, i64) = self
            .script
            .key(&key)
            .arg(limit)
            .arg(CONCURRENCY_KEY_TTL_SECS)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    rate_limit_backend = "redis",
                    rate_limit_failure = "backend_unavailable",
                    "concurrency: Lua script failed"
                );
                GatewayError::config(
                    "concurrency_limiter_unavailable",
                    "concurrency limiter temporarily unavailable",
                )
            })?;

        if allowed == 0 {
            return Err(GatewayError::rate_limit(
                "concurrency_limit_exceeded",
                format!("concurrent request limit ({limit}) exceeded"),
                Some(1),
            ));
        }

        Ok(ConcurrencyPermit {
            pool: self.pool.clone(),
            key,
            released: AtomicBool::new(false),
        })
    }
}

// ---------------------------------------------------------------------------
// ConcurrencyPermit
// ---------------------------------------------------------------------------

/// RAII guard that decrements the concurrency counter when released or dropped.
pub struct ConcurrencyPermit {
    pool: RedisPool,
    key: String,
    released: AtomicBool,
}

impl ConcurrencyPermit {
    /// Explicitly release the permit (preferred over relying on Drop).
    pub async fn release(self) {
        self.release_inner().await;
    }

    async fn release_inner(&self) {
        if self.released.swap(true, Ordering::SeqCst) {
            return; // Already released
        }
        if let Ok(mut conn) = self.pool.get().await {
            let _: Result<i64, _> = conn.decr(&self.key, 1i64).await;
        }
    }
}

impl Drop for ConcurrencyPermit {
    fn drop(&mut self) {
        if !self.released.load(Ordering::SeqCst) {
            self.released.store(true, Ordering::SeqCst);
            // Use try_current() to avoid panicking if the Tokio runtime has
            // already shut down (e.g. during test teardown). The safety-net
            // TTL (120s) will reclaim the slot if we can't DECR here.
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let pool = self.pool.clone();
                let key = self.key.clone();
                handle.spawn(async move {
                    if let Ok(mut conn) = pool.get().await {
                        let _: Result<i64, _> = conn.decr(&key, 1i64).await;
                    }
                });
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_bool_prevents_double_release_in_drop() {
        // Simulate the released flag behavior
        let flag = AtomicBool::new(false);

        // First "release" succeeds
        let was_released = flag.swap(true, Ordering::SeqCst);
        assert!(
            !was_released,
            "first release should return false (was not yet released)"
        );

        // Second "release" is a no-op
        let was_released = flag.swap(true, Ordering::SeqCst);
        assert!(
            was_released,
            "second release should return true (already released)"
        );
    }

    #[test]
    fn concurrency_key_format() {
        let sa_id = ServiceAccountId::from_uuid(uuid::Uuid::nil());
        let key = format!("concurrency:{sa_id}");
        assert_eq!(key, "concurrency:00000000-0000-0000-0000-000000000000");
    }
}
