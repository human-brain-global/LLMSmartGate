//! Per-service-account concurrency limiting using Redis atomic counters.
//!
//! A `ConcurrencyPermit` is returned on successful acquisition and decrements
//! the counter when dropped (via `tokio::spawn`) or explicitly released.

use std::sync::atomic::{AtomicBool, Ordering};

use deadpool_redis::Pool as RedisPool;
use redis::AsyncCommands;

use crate::error::GatewayError;
use crate::storage::StorageError;
use crate::types::ServiceAccountId;

/// Safety-net TTL for concurrency keys (seconds). Prevents leaked counters
/// if a permit is never released (e.g. process crash).
const CONCURRENCY_KEY_TTL_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// ConcurrencyLimiter
// ---------------------------------------------------------------------------

/// Manages per-service-account concurrent request limits via Redis.
#[derive(Clone)]
pub struct ConcurrencyLimiter {
    pool: RedisPool,
}

impl ConcurrencyLimiter {
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
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
            tracing::error!(error = %e, "concurrency: redis pool unavailable");
            GatewayError::rate_limit(
                "service_unavailable",
                "concurrency limiter unavailable",
                Some(5),
            )
        })?;

        // Atomically increment and set safety-net TTL
        let current: i64 = conn.incr(&key, 1i64).await.map_err(|e| {
            tracing::error!(error = %e, "concurrency: INCR failed");
            GatewayError::rate_limit(
                "service_unavailable",
                "concurrency limiter unavailable",
                Some(5),
            )
        })?;

        #[allow(
            clippy::cast_possible_wrap,
            reason = "CONCURRENCY_KEY_TTL_SECS is a small constant (120)"
        )]
        let _: () = conn
            .expire(&key, CONCURRENCY_KEY_TTL_SECS as i64)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "concurrency: EXPIRE failed");
                StorageError::Redis(e.to_string())
            })
            .unwrap_or(());

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "concurrency count is always non-negative and fits in u32"
        )]
        let current_u32 = current as u32;
        if current_u32 > limit {
            // Over limit — roll back immediately
            let _: Result<i64, _> = conn.decr(&key, 1i64).await;
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
            let pool = self.pool.clone();
            let key = self.key.clone();
            tokio::spawn(async move {
                if let Ok(mut conn) = pool.get().await {
                    let _: Result<i64, _> = conn.decr(&key, 1i64).await;
                }
            });
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
