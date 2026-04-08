//! Sliding-window rate limiting using Redis Lua scripts.
//!
//! Two levels are evaluated in order: global → service account.
//! The first exceeded limit triggers a 429 rejection. Headers reflect
//! the most restrictive applicable limit.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use deadpool_redis::Pool as RedisPool;
use redis::Script;

use crate::config::RateLimitConfig;
use crate::error::GatewayError;
use crate::storage::StorageError;
use crate::types::ServiceAccountId;

use super::engine::EvaluatedPolicy;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which level caused a rate limit rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitLevel {
    Global,
    ServiceAccount,
}

/// Result of a single sliding-window check at one level.
#[derive(Debug, Clone)]
pub struct WindowCheckResult {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: u64,
}

/// Aggregated outcome across all levels (returned to middleware).
#[derive(Debug, Clone)]
pub struct RateLimitOutcome {
    /// The most restrictive limit value.
    pub limit: u32,
    /// The fewest remaining requests across all checked levels.
    pub remaining: u32,
    /// The earliest window reset (Unix timestamp, seconds).
    pub reset_at: u64,
    /// Which level caused rejection, if any.
    pub rejected_by: Option<RateLimitLevel>,
}

// ---------------------------------------------------------------------------
// Pure calculation (extracted for unit testing)
// ---------------------------------------------------------------------------

/// Compute the weighted request count for the sliding window approximation.
///
/// Uses two fixed windows: the current window's count and the previous window's
/// count, weighted by how much of the previous window still overlaps.
#[allow(
    clippy::cast_precision_loss,
    reason = "rate limit counters are small enough that f64 precision is sufficient"
)]
fn weighted_count(
    current_count: u64,
    previous_count: u64,
    elapsed_secs: u64,
    window_secs: u64,
) -> f64 {
    if window_secs == 0 {
        return current_count as f64;
    }
    let overlap = 1.0 - (elapsed_secs as f64 / window_secs as f64);
    (previous_count as f64 * overlap) + current_count as f64
}

/// Build a `WindowCheckResult` from raw Redis counters.
fn build_check_result(
    current_count: u64,
    previous_count: u64,
    now_secs: u64,
    window_secs: u64,
    limit: u32,
) -> WindowCheckResult {
    let elapsed = now_secs % window_secs;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rate limit counts are always non-negative and fit in u32"
    )]
    let count = weighted_count(current_count, previous_count, elapsed, window_secs).ceil() as u32;
    let remaining = limit.saturating_sub(count);
    let current_window = now_secs / window_secs;
    let reset_at = (current_window + 1) * window_secs;

    WindowCheckResult {
        allowed: count <= limit,
        limit,
        remaining,
        reset_at,
    }
}

// ---------------------------------------------------------------------------
// SlidingWindowLimiter
// ---------------------------------------------------------------------------

/// Lua script that atomically increments the current window counter and
/// reads the previous window counter in a single Redis round-trip.
///
/// KEYS[1] = current window key
/// KEYS[2] = previous window key
/// ARGV[1] = TTL in seconds (2 × window_secs)
///
/// Returns: {current_count, previous_count}
const LUA_SLIDING_WINDOW: &str = r"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
local previous = tonumber(redis.call('GET', KEYS[2]) or '0')
return {current, previous}
";

/// Redis-backed sliding window rate limiter.
#[derive(Clone)]
pub struct SlidingWindowLimiter {
    script: Arc<Script>,
    window_secs: u64,
}

impl SlidingWindowLimiter {
    pub fn new(window_secs: u64) -> Self {
        Self {
            script: Arc::new(Script::new(LUA_SLIDING_WINDOW)),
            window_secs,
        }
    }

    /// Check and increment the rate limit counter for the given key prefix.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::Redis` on any Redis communication failure.
    pub async fn check_and_increment(
        &self,
        pool: &RedisPool,
        key_prefix: &str,
        limit: u32,
    ) -> Result<WindowCheckResult, StorageError> {
        let now_secs = now_unix_secs();
        let current_window = now_secs / self.window_secs;
        let prev_window = current_window.saturating_sub(1);

        let current_key = format!("{key_prefix}:{current_window}");
        let prev_key = format!("{key_prefix}:{prev_window}");
        let ttl = self.window_secs * 2;

        let mut conn = pool
            .get()
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))?;

        let (current_count, previous_count): (u64, u64) = self
            .script
            .key(&current_key)
            .key(&prev_key)
            .arg(ttl)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| StorageError::Redis(e.to_string()))?;

        Ok(build_check_result(
            current_count,
            previous_count,
            now_secs,
            self.window_secs,
            limit,
        ))
    }
}

// ---------------------------------------------------------------------------
// RateLimitEvaluator
// ---------------------------------------------------------------------------

/// Orchestrates multi-level rate limit checks (global → SA).
#[derive(Clone)]
pub struct RateLimitEvaluator {
    limiter: SlidingWindowLimiter,
    global_rpm: u32,
}

impl RateLimitEvaluator {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            limiter: SlidingWindowLimiter::new(config.window_secs),
            global_rpm: config.global_rpm,
        }
    }

    /// Evaluate rate limits at all applicable levels.
    ///
    /// Checks global first (cheapest — shared key), then per-SA if configured.
    /// Short-circuits on the first rejected level.
    ///
    /// # Errors
    ///
    /// Returns `GatewayError::RateLimit` if any level is exceeded or Redis is unavailable.
    pub async fn evaluate(
        &self,
        pool: &RedisPool,
        sa_id: ServiceAccountId,
        evaluated: &EvaluatedPolicy,
    ) -> Result<RateLimitOutcome, GatewayError> {
        // 1. Global check
        let global = self
            .limiter
            .check_and_increment(pool, "rl:global", self.global_rpm)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "rate limit: redis unavailable for global check");
                GatewayError::rate_limit("service_unavailable", "rate limiter unavailable", Some(5))
            })?;

        if !global.allowed {
            let retry_after = global.reset_at.saturating_sub(now_unix_secs());
            return Err(GatewayError::rate_limit(
                "rate_limit_exceeded",
                "global rate limit exceeded",
                Some(retry_after),
            ));
        }

        // Start with global as the most restrictive result
        let mut outcome = RateLimitOutcome {
            limit: global.limit,
            remaining: global.remaining,
            reset_at: global.reset_at,
            rejected_by: None,
        };

        // 2. Per-SA check (if policy specifies rpm_limit)
        if let Some(sa_rpm) = evaluated.rpm_limit {
            let sa_key = format!("rl:sa:{sa_id}");
            let sa_result = self
                .limiter
                .check_and_increment(pool, &sa_key, sa_rpm)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "rate limit: redis unavailable for SA check");
                    GatewayError::rate_limit(
                        "service_unavailable",
                        "rate limiter unavailable",
                        Some(5),
                    )
                })?;

            if !sa_result.allowed {
                let retry_after = sa_result.reset_at.saturating_sub(now_unix_secs());
                return Err(GatewayError::rate_limit(
                    "rate_limit_exceeded",
                    "service account rate limit exceeded",
                    Some(retry_after),
                ));
            }

            // Track the most restrictive result for headers
            outcome = most_restrictive(outcome, &sa_result);
        }

        Ok(outcome)
    }
}

/// Pick the most restrictive outcome for response headers.
fn most_restrictive(current: RateLimitOutcome, check: &WindowCheckResult) -> RateLimitOutcome {
    if check.remaining < current.remaining {
        RateLimitOutcome {
            limit: check.limit,
            remaining: check.remaining,
            reset_at: check.reset_at,
            rejected_by: current.rejected_by,
        }
    } else {
        current
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // weighted_count (pure function)
    // -----------------------------------------------------------------------

    #[test]
    fn weighted_count_start_of_window() {
        // At the start of a new window (elapsed = 0), previous window fully overlaps.
        let count = weighted_count(1, 10, 0, 60);
        // expected: 10 * 1.0 + 1 = 11.0
        assert!((count - 11.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_count_mid_window() {
        // Halfway through the window (elapsed = 30), previous overlaps 50%.
        let count = weighted_count(5, 10, 30, 60);
        // expected: 10 * 0.5 + 5 = 10.0
        assert!((count - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_count_end_of_window() {
        // At the end of the window (elapsed = 59), previous barely overlaps.
        let count = weighted_count(5, 10, 59, 60);
        // expected: 10 * (1/60) + 5 ≈ 5.167
        let expected = 10.0 * (1.0 / 60.0) + 5.0;
        assert!((count - expected).abs() < 0.001);
    }

    #[test]
    fn weighted_count_no_previous() {
        let count = weighted_count(5, 0, 30, 60);
        assert!((count - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_count_zero_window_returns_current() {
        let count = weighted_count(5, 10, 0, 0);
        assert!((count - 5.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // build_check_result
    // -----------------------------------------------------------------------

    #[test]
    fn check_result_allowed_when_under_limit() {
        let result = build_check_result(5, 0, 1000, 60, 100);
        assert!(result.allowed);
        assert_eq!(result.limit, 100);
        assert_eq!(result.remaining, 95);
    }

    #[test]
    fn check_result_rejected_when_over_limit() {
        let result = build_check_result(101, 0, 1000, 60, 100);
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);
    }

    #[test]
    fn check_result_reset_at_is_next_window_boundary() {
        // now_secs = 1000, window = 60 → current_window = 16, reset_at = 17 * 60 = 1020
        let result = build_check_result(1, 0, 1000, 60, 100);
        assert_eq!(result.reset_at, 1020);
    }

    #[test]
    fn check_result_weighted_count_causes_rejection() {
        // Previous window had 90 requests, current has 20.
        // Elapsed = 10s into 60s window → overlap = 50/60 ≈ 0.833
        // Weighted = 90 * 0.833 + 20 = 75 + 20 = 95.0 → ceil = 95 (under 100, allowed)
        let now = 60 * 100 + 10; // 10 seconds into window 100
        let result = build_check_result(20, 90, now, 60, 100);
        assert!(result.allowed);
        assert_eq!(result.remaining, 5); // 100 - 95
    }

    // -----------------------------------------------------------------------
    // most_restrictive
    // -----------------------------------------------------------------------

    #[test]
    fn most_restrictive_picks_lower_remaining() {
        let outcome = RateLimitOutcome {
            limit: 1000,
            remaining: 500,
            reset_at: 2000,
            rejected_by: None,
        };
        let check = WindowCheckResult {
            allowed: true,
            limit: 100,
            remaining: 50,
            reset_at: 1500,
        };
        let result = most_restrictive(outcome, &check);
        assert_eq!(result.limit, 100);
        assert_eq!(result.remaining, 50);
        assert_eq!(result.reset_at, 1500);
    }

    #[test]
    fn most_restrictive_keeps_current_when_less_restrictive() {
        let outcome = RateLimitOutcome {
            limit: 100,
            remaining: 10,
            reset_at: 1500,
            rejected_by: None,
        };
        let check = WindowCheckResult {
            allowed: true,
            limit: 1000,
            remaining: 500,
            reset_at: 2000,
        };
        let result = most_restrictive(outcome, &check);
        assert_eq!(result.limit, 100);
        assert_eq!(result.remaining, 10);
    }
}
