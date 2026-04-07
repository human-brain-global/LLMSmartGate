//! Canonical signing string construction and body integrity verification.
//!
//! These are pure functions used by the Ed25519 signature verification pipeline.
//! The canonical string defines a deterministic format for signing HTTP requests,
//! and the body hash functions ensure request body integrity.

use crate::error::GatewayError;
use sha2::{Digest, Sha256};

/// Build the canonical signing string for Ed25519 signature verification.
///
/// Format: `METHOD\nPATH\nTIMESTAMP\nNONCE\nBODY_SHA256`
///
/// - `method`: HTTP method (uppercase: GET, POST, etc.)
/// - `path`: request path including query string (e.g. `/v1/chat/completions?stream=true`)
/// - `timestamp`: Unix timestamp as string
/// - `nonce`: unique request nonce
/// - `body_sha256`: hex-encoded SHA-256 hash of the request body
pub fn build_canonical_string(
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body_sha256: &str,
) -> String {
    format!("{method}\n{path}\n{timestamp}\n{nonce}\n{body_sha256}")
}

/// Compute the SHA-256 hash of a byte slice, returned as a lowercase hex string.
pub fn compute_body_hash(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let result = hasher.finalize();
    // Pre-allocate exact capacity; encode directly without fmt::Write overhead.
    let mut hex = String::with_capacity(64);
    for byte in result {
        hex.push(HEX_TABLE[(byte >> 4) as usize]);
        hex.push(HEX_TABLE[(byte & 0x0f) as usize]);
    }
    hex
}

/// Lookup table for hex encoding (avoids per-byte formatting overhead).
const HEX_TABLE: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7',
    '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

/// Verify that the provided hash matches the SHA-256 hash of the body.
///
/// # Errors
///
/// Returns `GatewayError::Auth` with code `body_hash_mismatch` if the hashes don't match.
pub fn verify_body_hash(body: &[u8], expected_hash: &str) -> Result<(), GatewayError> {
    let actual = compute_body_hash(body);
    if actual != expected_hash {
        return Err(GatewayError::auth(
            "body_hash_mismatch",
            "request body SHA-256 hash does not match X-Body-SHA256 header",
        ));
    }
    Ok(())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Canonical string tests ---

    #[test]
    fn canonical_string_format() {
        let result = build_canonical_string(
            "POST",
            "/v1/chat/completions",
            "1700000000",
            "abc123def456",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        assert_eq!(
            result,
            "POST\n/v1/chat/completions\n1700000000\nabc123def456\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn canonical_string_with_query_params() {
        let result =
            build_canonical_string("GET", "/v1/models?limit=10", "123", "nonce1", "hash1");
        assert!(result.contains("/v1/models?limit=10"));
    }

    #[test]
    fn canonical_string_get_request() {
        let result = build_canonical_string("GET", "/v1/models", "123", "nonce1", "hash1");
        assert!(result.starts_with("GET\n"));
    }

    #[test]
    fn canonical_string_has_five_lines() {
        let result = build_canonical_string("DELETE", "/v1/keys/k1", "999", "n1", "h1");
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "DELETE");
        assert_eq!(lines[1], "/v1/keys/k1");
        assert_eq!(lines[2], "999");
        assert_eq!(lines[3], "n1");
        assert_eq!(lines[4], "h1");
    }

    // --- Body hash tests ---

    #[test]
    fn compute_hash_of_empty_body() {
        let hash = compute_body_hash(b"");
        // SHA-256 of empty string is a well-known constant
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn compute_hash_of_json_body() {
        let body = br#"{"model":"gpt-4","messages":[{"role":"user","content":"hello"}]}"#;
        let hash = compute_body_hash(body);
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars
        // Hash should be deterministic
        assert_eq!(hash, compute_body_hash(body));
    }

    #[test]
    fn compute_hash_is_lowercase_hex() {
        let hash = compute_body_hash(b"test");
        // All chars should be lowercase hex digits
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }

    #[test]
    fn verify_body_hash_matching() {
        let body = b"test body";
        let hash = compute_body_hash(body);
        assert!(verify_body_hash(body, &hash).is_ok());
    }

    #[test]
    fn verify_body_hash_mismatching() {
        let body = b"test body";
        let result = verify_body_hash(
            body,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(result.is_err());
    }

    #[test]
    fn verify_body_hash_mismatch_error_code() {
        let body = b"test body";
        let err = verify_body_hash(body, "wrong_hash").unwrap_err();
        match err {
            GatewayError::Auth { code, .. } => assert_eq!(code, "body_hash_mismatch"),
            other => panic!("expected Auth error, got: {other:?}"),
        }
    }

    #[test]
    fn verify_body_hash_empty_body() {
        let empty_hash =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(verify_body_hash(b"", empty_hash).is_ok());
    }

    #[test]
    fn different_bodies_produce_different_hashes() {
        let hash1 = compute_body_hash(b"body one");
        let hash2 = compute_body_hash(b"body two");
        assert_ne!(hash1, hash2);
    }
}
