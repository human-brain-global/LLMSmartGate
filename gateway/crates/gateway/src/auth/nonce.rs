//! Timestamp validation and nonce replay protection.

use chrono::Utc;

use crate::error::GatewayError;
use crate::storage::redis::RedisClient;

/// Validate that a timestamp string is within the allowed skew window.
///
/// `timestamp_str`: Unix timestamp as a string (seconds since epoch)
/// `skew_secs`: maximum allowed clock skew in seconds (e.g. 300 = +/- 5 minutes)
///
/// # Errors
///
/// Returns [`GatewayError::Auth`] with code `invalid_timestamp` if parsing fails,
/// or `timestamp_expired` if outside the window.
pub fn validate_timestamp(timestamp_str: &str, skew_secs: u64) -> Result<(), GatewayError> {
    let timestamp: i64 = timestamp_str.parse().map_err(|_| {
        GatewayError::auth("invalid_timestamp", "X-Timestamp is not a valid integer")
    })?;

    let now = Utc::now().timestamp();
    let diff = (now - timestamp).unsigned_abs();

    if diff > skew_secs {
        return Err(GatewayError::auth(
            "timestamp_expired",
            format!("request timestamp is {diff}s from server time (max {skew_secs}s)"),
        ));
    }

    Ok(())
}

/// Validate nonce length (16-64 characters).
///
/// # Errors
///
/// Returns [`GatewayError::Auth`] with code `invalid_nonce` if out of range.
pub fn validate_nonce_length(nonce: &str) -> Result<(), GatewayError> {
    let len = nonce.len();
    if !(16..=64).contains(&len) {
        return Err(GatewayError::auth(
            "invalid_nonce",
            format!("nonce must be 16-64 characters, got {len}"),
        ));
    }
    Ok(())
}

/// Check a nonce for replay protection using Redis SET NX EX.
///
/// Stores the nonce in Redis with a TTL equal to the timestamp window.
/// If the nonce already exists, the request is a replay.
///
/// Redis key format: `nonce:{service_account_id}:{nonce}`
///
/// # Errors
///
/// Returns [`GatewayError::Auth`] with code `nonce_replay` if the nonce was already used.
/// Returns [`GatewayError::Auth`] on Redis errors (fail-closed for security).
pub async fn check_nonce(
    redis: &RedisClient,
    service_account_id: &str,
    nonce: &str,
    ttl_secs: u64,
) -> Result<(), GatewayError> {
    let key = format!("nonce:{service_account_id}:{nonce}");

    let was_set = redis.set_nx_ex(&key, "1", ttl_secs).await.map_err(|e| {
        // Fail-closed: Redis error means we can't verify nonce uniqueness
        tracing::warn!(error = %e, "Redis nonce check failed, rejecting request (fail-closed)");
        GatewayError::auth("nonce_check_failed", "unable to verify nonce uniqueness")
    })?;

    if !was_set {
        return Err(GatewayError::auth(
            "nonce_replay",
            "this nonce has already been used within the timestamp window",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Timestamp tests (pure, no I/O) ---

    #[test]
    fn valid_timestamp_within_window() {
        let now = Utc::now().timestamp().to_string();
        assert!(validate_timestamp(&now, 300).is_ok());
    }

    #[test]
    fn timestamp_slightly_in_past() {
        let past = (Utc::now().timestamp() - 100).to_string();
        assert!(validate_timestamp(&past, 300).is_ok());
    }

    #[test]
    fn timestamp_slightly_in_future() {
        let future = (Utc::now().timestamp() + 100).to_string();
        assert!(validate_timestamp(&future, 300).is_ok());
    }

    #[test]
    fn timestamp_too_old() {
        let old = (Utc::now().timestamp() - 600).to_string();
        let result = validate_timestamp(&old, 300);
        assert!(result.is_err());
    }

    #[test]
    fn timestamp_too_far_future() {
        let future = (Utc::now().timestamp() + 600).to_string();
        let result = validate_timestamp(&future, 300);
        assert!(result.is_err());
    }

    #[test]
    fn timestamp_not_a_number() {
        let result = validate_timestamp("not-a-number", 300);
        assert!(result.is_err());
    }

    #[test]
    fn timestamp_at_exact_boundary_is_ok() {
        // Exactly at the boundary should still be accepted (diff == skew_secs is not > skew_secs)
        let boundary = (Utc::now().timestamp() - 300).to_string();
        assert!(validate_timestamp(&boundary, 300).is_ok());
    }

    #[test]
    fn timestamp_one_past_boundary_is_rejected() {
        let past_boundary = (Utc::now().timestamp() - 301).to_string();
        assert!(validate_timestamp(&past_boundary, 300).is_err());
    }

    #[test]
    fn nonce_valid_length() {
        assert!(validate_nonce_length("abcdef1234567890").is_ok()); // 16 chars
        assert!(validate_nonce_length(&"a".repeat(64)).is_ok()); // 64 chars
    }

    #[test]
    fn nonce_valid_length_middle() {
        assert!(validate_nonce_length(&"b".repeat(32)).is_ok()); // 32 chars
    }

    #[test]
    fn nonce_too_short() {
        assert!(validate_nonce_length("short").is_err()); // 5 chars
        assert!(validate_nonce_length(&"a".repeat(15)).is_err()); // 15 chars
    }

    #[test]
    fn nonce_too_long() {
        assert!(validate_nonce_length(&"a".repeat(65)).is_err()); // 65 chars
    }

    #[test]
    fn nonce_empty_is_rejected() {
        assert!(validate_nonce_length("").is_err());
    }

    // --- Nonce Redis tests (need live Redis) ---

    async fn test_redis_client() -> Option<RedisClient> {
        let url =
            std::env::var("VALKEY_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let pool = crate::storage::redis::create_redis_pool_from_url(&url, 2).ok()?;
        pool.get().await.ok()?;
        Some(RedisClient::new(pool))
    }

    #[tokio::test]
    async fn nonce_first_use_succeeds() {
        let Some(redis) = test_redis_client().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let nonce = format!("test-nonce-{}", uuid::Uuid::new_v4());
        let result = check_nonce(&redis, "sa-test", &nonce, 10).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn nonce_replay_rejected() {
        let Some(redis) = test_redis_client().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let nonce = format!("test-nonce-{}", uuid::Uuid::new_v4());
        // First use succeeds
        check_nonce(&redis, "sa-test", &nonce, 10)
            .await
            .expect("first use should succeed");
        // Replay rejected
        let result = check_nonce(&redis, "sa-test", &nonce, 10).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn nonce_different_service_accounts_are_independent() {
        let Some(redis) = test_redis_client().await else {
            eprintln!("SKIP: Redis not available");
            return;
        };
        let nonce = format!("test-nonce-{}", uuid::Uuid::new_v4());
        // First service account uses nonce
        check_nonce(&redis, "sa-alice", &nonce, 10)
            .await
            .expect("first use should succeed");
        // Different service account with same nonce should also succeed (different key namespace)
        let result = check_nonce(&redis, "sa-bob", &nonce, 10).await;
        assert!(result.is_ok());
    }
}
