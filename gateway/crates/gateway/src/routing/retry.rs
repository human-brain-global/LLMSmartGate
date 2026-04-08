//! Retry logic with exponential backoff and full jitter.
//!
//! Provides [`compute_backoff`] for calculating retry delays and
//! [`retry_with_backoff`] for executing a single route with retries.

use std::future::Future;
use std::time::Duration;

use rand::Rng;

use crate::error::GatewayError;
use crate::models::ProviderRoute;
use crate::routing::router::RouteAttempt;

/// Maximum bit-shift exponent for exponential backoff.
///
/// `2^32` = ~4 billion ms (~49 days), which exceeds any reasonable backoff cap.
/// Clamping here prevents intermediate overflow before `saturating_mul` and
/// `min(cap)` bring the value back to a sane range.
const MAX_BACKOFF_SHIFT: u32 = 32;

// ---------------------------------------------------------------------------
// AttemptOutcome
// ---------------------------------------------------------------------------

/// The outcome of a single provider call attempt.
#[derive(Debug)]
pub enum AttemptOutcome<T> {
    /// The call succeeded.
    Success(T),
    /// The call failed with a retryable error (429, 5xx, connection timeout).
    Retryable(GatewayError),
    /// The call failed with a non-retryable error (4xx client errors).
    /// Do NOT retry on this route, but may fallback to the next route.
    Fatal(GatewayError),
}

// ---------------------------------------------------------------------------
// Backoff computation
// ---------------------------------------------------------------------------

/// Compute the backoff duration for a given attempt using exponential
/// backoff with full jitter.
///
/// Formula: `random(0, min(cap, base * 2^attempt))`
///
/// This implements the "Full Jitter" approach recommended by the AWS
/// Architecture Blog to prevent thundering herd effects.
///
/// # Arguments
///
/// * `base` - Base backoff duration (e.g., 250ms)
/// * `cap` - Maximum backoff duration (e.g., 10s)
/// * `attempt` - Zero-indexed retry attempt number (0 = first retry)
/// * `rng` - Random number generator for jitter
pub fn compute_backoff(
    base: Duration,
    cap: Duration,
    attempt: u32,
    rng: &mut impl Rng,
) -> Duration {
    // Durations used here are always well under u64::MAX milliseconds.
    #[allow(clippy::cast_possible_truncation)]
    let base_ms = base.as_millis() as u64;
    #[allow(clippy::cast_possible_truncation)]
    let cap_ms = cap.as_millis() as u64;

    if base_ms == 0 || cap_ms == 0 {
        return Duration::ZERO;
    }

    // Exponential: base * 2^attempt. The shift is capped at MAX_BACKOFF_SHIFT
    // because 2^32 (~4 billion) already exceeds any reasonable cap_ms and
    // higher shifts risk u64 overflow even with saturating_mul.
    let exponential_ms = base_ms.saturating_mul(1u64 << attempt.min(MAX_BACKOFF_SHIFT));
    let bounded_ms = exponential_ms.min(cap_ms);

    // Full jitter: random in [0, bounded]
    let jitter_ms = rng.gen_range(0..=bounded_ms);
    Duration::from_millis(jitter_ms)
}

// ---------------------------------------------------------------------------
// Retry executor
// ---------------------------------------------------------------------------

/// Execute a single route with retries and exponential backoff.
///
/// Calls `call_fn` up to `1 + max_retries` times. On retryable failure,
/// sleeps for the computed backoff then retries. On fatal failure, returns
/// immediately (the caller may fallback to the next route).
///
/// # Returns
///
/// `Ok((result, attempts_used))` on success, where `attempts_used` is the
/// total number of calls made (1 = no retries). `Err((last_error, attempts_used))`
/// if all attempts fail or a fatal error is encountered.
///
/// # Errors
///
/// Returns the last `GatewayError` if all attempts fail (retryable errors
/// exhausted) or the first fatal error encountered.
///
/// # Type Parameters
///
/// * `T` - The success type (e.g., provider response)
/// * `F` - Async closure that performs a single provider call
pub async fn retry_with_backoff<T, F, Fut>(
    route_attempt: &RouteAttempt,
    call_fn: F,
) -> Result<(T, u32), (GatewayError, u32)>
where
    F: Fn(&ProviderRoute) -> Fut,
    Fut: Future<Output = AttemptOutcome<T>>,
{
    retry_with_backoff_rng(route_attempt, call_fn, &mut rand::thread_rng()).await
}

/// Retry executor with an explicit RNG for deterministic testing.
async fn retry_with_backoff_rng<T, F, Fut>(
    route_attempt: &RouteAttempt,
    call_fn: F,
    rng: &mut impl Rng,
) -> Result<(T, u32), (GatewayError, u32)>
where
    F: Fn(&ProviderRoute) -> Fut,
    Fut: Future<Output = AttemptOutcome<T>>,
{
    let total_calls = 1 + route_attempt.max_retries;
    let mut last_error = None;

    for attempt in 0..total_calls {
        match call_fn(&route_attempt.route).await {
            AttemptOutcome::Success(result) => {
                return Ok((result, attempt + 1));
            }
            AttemptOutcome::Fatal(e) => {
                tracing::warn!(
                    route_id = %route_attempt.route.id,
                    provider = ?route_attempt.route.provider,
                    attempt,
                    error = %e,
                    "fatal error — not retrying"
                );
                // Fatal after exactly 1 call on this attempt
                return Err((e, attempt + 1));
            }
            AttemptOutcome::Retryable(e) => {
                let remaining = total_calls - attempt - 1;
                if remaining > 0 {
                    let backoff = compute_backoff(
                        route_attempt.backoff_base,
                        route_attempt.backoff_cap,
                        attempt,
                        rng,
                    );
                    tracing::warn!(
                        route_id = %route_attempt.route.id,
                        provider = ?route_attempt.route.provider,
                        attempt,
                        remaining_retries = remaining,
                        backoff_ms = backoff.as_millis(),
                        error = %e,
                        "retryable error — backing off"
                    );
                    tokio::time::sleep(backoff).await;
                } else {
                    tracing::warn!(
                        route_id = %route_attempt.route.id,
                        provider = ?route_attempt.route.provider,
                        attempt,
                        error = %e,
                        "retryable error — retries exhausted"
                    );
                }
                last_error = Some(e);
            }
        }
    }

    Err((
        last_error.unwrap_or_else(|| {
            GatewayError::routing("retry_exhausted", "all retry attempts exhausted")
        }),
        total_calls,
    ))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProviderRoute;
    use crate::routing::router::RouteAttempt;
    use crate::types::{Provider, RouteId};
    use rand::{SeedableRng, rngs::StdRng};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_attempt(max_retries: u32) -> RouteAttempt {
        RouteAttempt {
            route: ProviderRoute {
                id: RouteId::new(),
                tenant_id: None,
                model_alias: "test-model".to_owned(),
                provider: Provider::Openai,
                provider_model_name: "gpt-4".to_owned(),
                priority: 100,
                enabled: true,
                timeout_ms: 30000,
                #[allow(clippy::cast_possible_wrap)]
                max_retries: max_retries as i32,
                retry_backoff_ms: 250,
                config: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            max_retries,
            timeout: Duration::from_millis(30000),
            backoff_base: Duration::from_millis(250),
            backoff_cap: Duration::from_secs(10),
        }
    }

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    // -----------------------------------------------------------------------
    // compute_backoff
    // -----------------------------------------------------------------------

    #[test]
    fn backoff_attempt_zero() {
        let mut rng = seeded_rng();
        let backoff = compute_backoff(
            Duration::from_millis(250),
            Duration::from_secs(10),
            0,
            &mut rng,
        );
        // Should be in [0, 250ms]
        assert!(backoff <= Duration::from_millis(250));
    }

    #[test]
    fn backoff_grows_exponentially() {
        let mut rng = seeded_rng();

        // Run many samples to verify the upper bounds grow
        let base = Duration::from_millis(250);
        let cap = Duration::from_secs(10);

        // attempt 0: max 250ms, attempt 1: max 500ms, attempt 2: max 1000ms
        for attempt in 0..3 {
            let max_expected_ms = 250u64 * (1 << attempt);
            let backoff = compute_backoff(base, cap, attempt, &mut rng);
            assert!(
                backoff.as_millis() <= u128::from(max_expected_ms),
                "attempt {attempt}: backoff {}ms > max {max_expected_ms}ms",
                backoff.as_millis()
            );
        }
    }

    #[test]
    fn backoff_is_not_always_zero() {
        // Verify that compute_backoff produces non-zero values across seeds,
        // catching a broken implementation that always returns Duration::ZERO.
        let base = Duration::from_millis(250);
        let cap = Duration::from_secs(10);
        let mut any_nonzero = false;

        for seed in 0..20 {
            let mut rng = StdRng::seed_from_u64(seed);
            let backoff = compute_backoff(base, cap, 2, &mut rng);
            if backoff > Duration::ZERO {
                any_nonzero = true;
                break;
            }
        }

        assert!(any_nonzero, "backoff should not be zero for all seeds");
    }

    #[test]
    fn backoff_respects_cap() {
        let mut rng = seeded_rng();
        let cap = Duration::from_secs(10);
        // Very high attempt — without cap this would be enormous
        let backoff = compute_backoff(Duration::from_millis(250), cap, 30, &mut rng);
        assert!(backoff <= cap);
    }

    #[test]
    fn backoff_zero_base_returns_zero() {
        let mut rng = seeded_rng();
        let backoff = compute_backoff(Duration::ZERO, Duration::from_secs(10), 5, &mut rng);
        assert_eq!(backoff, Duration::ZERO);
    }

    #[test]
    fn backoff_zero_cap_returns_zero() {
        let mut rng = seeded_rng();
        let backoff = compute_backoff(Duration::from_millis(250), Duration::ZERO, 0, &mut rng);
        assert_eq!(backoff, Duration::ZERO);
    }

    // -----------------------------------------------------------------------
    // retry_with_backoff
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let attempt = make_attempt(2);
        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let (result, attempts_used) = retry_with_backoff(&attempt, |_route| {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                AttemptOutcome::Success("ok")
            }
        })
        .await
        .expect("should succeed");

        assert_eq!(result, "ok");
        assert_eq!(attempts_used, 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_succeeds_after_retryable_failure() {
        tokio::time::pause();

        let attempt = make_attempt(2);
        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let (result, attempts_used) = retry_with_backoff(&attempt, |_route| {
            let count = Arc::clone(&count);
            async move {
                let n = count.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    AttemptOutcome::Retryable(GatewayError::routing("timeout", "timed out"))
                } else {
                    AttemptOutcome::Success("recovered")
                }
            }
        })
        .await
        .expect("should succeed on retry");

        assert_eq!(result, "recovered");
        assert_eq!(attempts_used, 2);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retry_stops_on_fatal_error() {
        let attempt = make_attempt(3);
        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let result = retry_with_backoff::<(), _, _>(&attempt, |_route| {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                AttemptOutcome::Fatal(GatewayError::routing("bad_request", "400"))
            }
        })
        .await;

        let (_, attempts_used) = result.unwrap_err();
        // Fatal error stops immediately — only 1 call despite max_retries=3
        assert_eq!(attempts_used, 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_exhausts_all_attempts() {
        tokio::time::pause();

        let attempt = make_attempt(2); // 1 initial + 2 retries = 3 calls
        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let result = retry_with_backoff::<(), _, _>(&attempt, |_route| {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                AttemptOutcome::Retryable(GatewayError::routing("server_error", "500"))
            }
        })
        .await;

        let (_, attempts_used) = result.unwrap_err();
        assert_eq!(attempts_used, 3); // 1 + 2 retries
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_zero_retries_calls_once() {
        let attempt = make_attempt(0);
        let call_count = Arc::new(AtomicU32::new(0));

        let count = Arc::clone(&call_count);
        let result = retry_with_backoff::<(), _, _>(&attempt, |_route| {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                AttemptOutcome::Retryable(GatewayError::routing("err", "fail"))
            }
        })
        .await;

        let (_, attempts_used) = result.unwrap_err();
        assert_eq!(attempts_used, 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
