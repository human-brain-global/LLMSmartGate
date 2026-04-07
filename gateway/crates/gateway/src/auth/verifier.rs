//! Ed25519 signature verification for request authentication.
//!
//! Provides two core functions:
//! - [`parse_public_key`]: Parse an Ed25519 public key from PEM-encoded string
//! - [`verify_signature`]: Verify an Ed25519 signature against a canonical signing string

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::error::GatewayError;

/// Parse an Ed25519 public key from PEM-encoded string.
///
/// Handles two formats:
/// 1. PKCS#8 SubjectPublicKeyInfo (44 bytes DER = 12-byte header + 32-byte key)
/// 2. Raw 32-byte key (base64-encoded between PEM markers)
///
/// # Errors
///
/// Returns `GatewayError::Auth` with code `invalid_key_format` if parsing fails.
pub fn parse_public_key(pem: &str) -> Result<VerifyingKey, GatewayError> {
    // Strip PEM headers and whitespace
    let b64: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");

    let der = STANDARD.decode(&b64).map_err(|e| {
        GatewayError::auth("invalid_key_format", format!("base64 decode failed: {e}"))
    })?;

    // Try PKCS#8 SubjectPublicKeyInfo (44 bytes: 12-byte header + 32-byte key)
    let key_bytes = if der.len() == 44 {
        &der[12..]
    } else if der.len() == 32 {
        &der[..]
    } else {
        return Err(GatewayError::auth(
            "invalid_key_format",
            format!(
                "unexpected key length: {} bytes (expected 32 or 44)",
                der.len()
            ),
        ));
    };

    let key_array: &[u8; 32] = key_bytes
        .try_into()
        .map_err(|_| GatewayError::auth("invalid_key_format", "key bytes conversion failed"))?;

    VerifyingKey::from_bytes(key_array).map_err(|e| {
        GatewayError::auth(
            "invalid_key_format",
            format!("invalid Ed25519 public key: {e}"),
        )
    })
}

/// Verify an Ed25519 signature against a canonical signing string.
///
/// `public_key`: the parsed verifying key
/// `canonical`: the canonical signing string to verify against
/// `signature_b64`: base64-encoded 64-byte Ed25519 signature
///
/// # Errors
///
/// Returns `GatewayError::Auth` with code `invalid_signature` if verification fails.
pub fn verify_signature(
    public_key: &VerifyingKey,
    canonical: &str,
    signature_b64: &str,
) -> Result<(), GatewayError> {
    let sig_bytes = STANDARD.decode(signature_b64).map_err(|e| {
        GatewayError::auth(
            "invalid_signature",
            format!("signature base64 decode failed: {e}"),
        )
    })?;

    let signature = Signature::from_slice(&sig_bytes).map_err(|e| {
        GatewayError::auth(
            "invalid_signature",
            format!("invalid signature format: {e}"),
        )
    })?;

    public_key
        .verify(canonical.as_bytes(), &signature)
        .map_err(|_| {
            GatewayError::auth("invalid_signature", "Ed25519 signature verification failed")
        })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn generate_test_keypair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    fn to_pem_pkcs8(verifying_key: &VerifyingKey) -> String {
        // PKCS#8 SubjectPublicKeyInfo header for Ed25519
        let header: [u8; 12] = [
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        let mut der = Vec::with_capacity(44);
        der.extend_from_slice(&header);
        der.extend_from_slice(verifying_key.as_bytes());
        let b64 = STANDARD.encode(&der);
        format!("-----BEGIN PUBLIC KEY-----\n{b64}\n-----END PUBLIC KEY-----")
    }

    fn to_pem_raw(verifying_key: &VerifyingKey) -> String {
        let b64 = STANDARD.encode(verifying_key.as_bytes());
        format!("-----BEGIN PUBLIC KEY-----\n{b64}\n-----END PUBLIC KEY-----")
    }

    // -----------------------------------------------------------------------
    // parse_public_key
    // -----------------------------------------------------------------------

    #[test]
    fn parse_pkcs8_pem() {
        let (_, vk) = generate_test_keypair();
        let pem = to_pem_pkcs8(&vk);
        let parsed = parse_public_key(&pem).expect("should parse PKCS#8 PEM");
        assert_eq!(parsed.as_bytes(), vk.as_bytes());
    }

    #[test]
    fn parse_raw_pem() {
        let (_, vk) = generate_test_keypair();
        let pem = to_pem_raw(&vk);
        let parsed = parse_public_key(&pem).expect("should parse raw PEM");
        assert_eq!(parsed.as_bytes(), vk.as_bytes());
    }

    #[test]
    fn parse_invalid_pem_returns_error() {
        let result = parse_public_key("not a pem");
        assert!(result.is_err());
    }

    #[test]
    fn parse_wrong_length_pem_returns_error() {
        let b64 = STANDARD.encode([0u8; 48]); // wrong length
        let pem = format!("-----BEGIN PUBLIC KEY-----\n{b64}\n-----END PUBLIC KEY-----");
        let result = parse_public_key(&pem);
        assert!(result.is_err());
    }

    #[test]
    fn parse_pem_with_multiline_base64() {
        let (_, vk) = generate_test_keypair();
        // PKCS#8 with line-wrapped base64 (as some PEM encoders produce)
        let header: [u8; 12] = [
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        let mut der = Vec::with_capacity(44);
        der.extend_from_slice(&header);
        der.extend_from_slice(vk.as_bytes());
        let b64 = STANDARD.encode(&der);
        // Split the base64 across multiple lines
        let mid = b64.len() / 2;
        let pem = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n{}\n-----END PUBLIC KEY-----",
            &b64[..mid],
            &b64[mid..]
        );
        let parsed = parse_public_key(&pem).expect("should parse multi-line PEM");
        assert_eq!(parsed.as_bytes(), vk.as_bytes());
    }

    // -----------------------------------------------------------------------
    // verify_signature
    // -----------------------------------------------------------------------

    #[test]
    fn verify_valid_signature() {
        let (sk, vk) = generate_test_keypair();
        let canonical = "POST\n/v1/chat/completions\n1700000000\nnonce123\nhash123";
        let signature = sk.sign(canonical.as_bytes());
        let sig_b64 = STANDARD.encode(signature.to_bytes());

        assert!(verify_signature(&vk, canonical, &sig_b64).is_ok());
    }

    #[test]
    fn verify_invalid_signature_returns_error() {
        let (_, vk) = generate_test_keypair();
        let canonical = "POST\n/v1/chat/completions\n1700000000\nnonce123\nhash123";
        // Random signature that doesn't match
        let fake_sig = STANDARD.encode([0u8; 64]);

        let result = verify_signature(&vk, canonical, &fake_sig);
        assert!(result.is_err());
    }

    #[test]
    fn verify_tampered_canonical_returns_error() {
        let (sk, vk) = generate_test_keypair();
        let canonical = "POST\n/v1/chat/completions\n1700000000\nnonce123\nhash123";
        let signature = sk.sign(canonical.as_bytes());
        let sig_b64 = STANDARD.encode(signature.to_bytes());

        // Tamper with the canonical string (change timestamp by 1 second)
        let tampered = "POST\n/v1/chat/completions\n1700000001\nnonce123\nhash123";
        let result = verify_signature(&vk, tampered, &sig_b64);
        assert!(result.is_err());
    }

    #[test]
    fn verify_bad_base64_signature_returns_error() {
        let (_, vk) = generate_test_keypair();
        let result = verify_signature(&vk, "canonical", "!!!not-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn verify_wrong_length_signature_returns_error() {
        let (_, vk) = generate_test_keypair();
        // 32 bytes instead of 64
        let short_sig = STANDARD.encode([0u8; 32]);
        let result = verify_signature(&vk, "canonical", &short_sig);
        assert!(result.is_err());
    }

    #[test]
    fn verify_with_wrong_key_returns_error() {
        let (sk, _vk) = generate_test_keypair();
        let (_, other_vk) = generate_test_keypair();
        let canonical = "POST\n/v1/chat/completions\n1700000000\nnonce123\nhash123";
        let signature = sk.sign(canonical.as_bytes());
        let sig_b64 = STANDARD.encode(signature.to_bytes());

        // Verify with the wrong key
        let result = verify_signature(&other_vk, canonical, &sig_b64);
        assert!(result.is_err());
    }
}
