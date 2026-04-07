//! Token limit enforcement -- clamp output tokens, reject on input excess.
//!
//! Output tokens are *clamped* (silently reduced to the policy limit), while
//! input tokens that exceed the limit cause the request to be **rejected**.

use crate::error::GatewayError;

/// Token limits from a merged policy evaluation.
#[derive(Debug, Clone, Default)]
pub struct TokenLimits {
    /// Maximum allowed input tokens. `None` means no limit.
    pub max_input_tokens: Option<u32>,
    /// Maximum allowed output tokens. `None` means no limit.
    pub max_output_tokens: Option<u32>,
}

/// Result of a token-limit check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenCheckResult {
    /// The effective `max_output_tokens` to send to the provider.
    /// `None` if the request didn't specify one and no policy limit applies.
    pub effective_max_output_tokens: Option<u32>,
    /// Whether the output tokens were clamped by policy.
    /// When `true`, the gateway should add `X-Policy-Max-Tokens-Applied: true`.
    pub output_tokens_clamped: bool,
}

/// Check token limits against a request.
///
/// - **Input tokens**: if `estimated_input_tokens` exceeds `limits.max_input_tokens`,
///   the request is rejected with `input_tokens_exceeded`.
/// - **Output tokens**: if `requested_max_output_tokens` exceeds `limits.max_output_tokens`,
///   it is clamped (not rejected) and `output_tokens_clamped` is set to `true`.
///
/// # Errors
///
/// Returns `GatewayError::Policy` with code `input_tokens_exceeded` when input
/// tokens exceed the policy limit.
pub fn check_token_limits(
    limits: &TokenLimits,
    estimated_input_tokens: Option<u32>,
    requested_max_output_tokens: Option<u32>,
) -> Result<TokenCheckResult, GatewayError> {
    // Check input tokens
    if let (Some(estimate), Some(limit)) = (estimated_input_tokens, limits.max_input_tokens) {
        if estimate > limit {
            return Err(GatewayError::policy(
                "input_tokens_exceeded",
                format!("estimated input tokens ({estimate}) exceeds policy limit ({limit})"),
            ));
        }
    }

    // Clamp output tokens
    let (effective_max_output_tokens, output_tokens_clamped) =
        match (requested_max_output_tokens, limits.max_output_tokens) {
            (Some(requested), Some(limit)) if requested > limit => (Some(limit), true),
            (Some(requested), Some(_limit)) => (Some(requested), false),
            (None, Some(limit)) => (Some(limit), false),
            (requested, None) => (requested, false),
        };

    Ok(TokenCheckResult {
        effective_max_output_tokens,
        output_tokens_clamped,
    })
}

/// Estimate the number of input tokens from a JSON `messages` array.
///
/// Uses a simple heuristic: total character count / 4.  This is a reasonable
/// approximation for English text across most tokenisers.  Provider-specific
/// tokenisers (tiktoken for OpenAI) are handled at the provider adapter layer.
pub fn estimate_input_tokens(messages_json: &serde_json::Value) -> Option<u32> {
    let arr = messages_json.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let total_chars: usize = arr
        .iter()
        .filter_map(|msg| {
            // Count characters in "content" field (string or array of parts)
            let content = msg.get("content")?;
            if let Some(s) = content.as_str() {
                Some(s.len())
            } else {
                content.as_array().map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| part.get("text")?.as_str().map(str::len))
                        .sum()
                })
            }
        })
        .sum();

    // chars / 4 is a common approximation for English text
    #[allow(clippy::cast_possible_truncation)] // token counts won't exceed u32::MAX
    Some((total_chars / 4).max(1) as u32)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // check_token_limits
    // -----------------------------------------------------------------------

    #[test]
    fn no_limits_passes_through() {
        let limits = TokenLimits::default();
        let result = check_token_limits(&limits, Some(1000), Some(500)).unwrap();
        assert_eq!(result.effective_max_output_tokens, Some(500));
        assert!(!result.output_tokens_clamped);
    }

    #[test]
    fn output_clamped_to_policy_limit() {
        let limits = TokenLimits {
            max_input_tokens: None,
            max_output_tokens: Some(100),
        };
        let result = check_token_limits(&limits, None, Some(500)).unwrap();
        assert_eq!(result.effective_max_output_tokens, Some(100));
        assert!(result.output_tokens_clamped);
    }

    #[test]
    fn output_within_limit_not_clamped() {
        let limits = TokenLimits {
            max_input_tokens: None,
            max_output_tokens: Some(500),
        };
        let result = check_token_limits(&limits, None, Some(100)).unwrap();
        assert_eq!(result.effective_max_output_tokens, Some(100));
        assert!(!result.output_tokens_clamped);
    }

    #[test]
    fn output_equal_to_limit_not_clamped() {
        let limits = TokenLimits {
            max_input_tokens: None,
            max_output_tokens: Some(100),
        };
        let result = check_token_limits(&limits, None, Some(100)).unwrap();
        assert_eq!(result.effective_max_output_tokens, Some(100));
        assert!(!result.output_tokens_clamped);
    }

    #[test]
    fn no_requested_output_gets_policy_limit() {
        let limits = TokenLimits {
            max_input_tokens: None,
            max_output_tokens: Some(200),
        };
        let result = check_token_limits(&limits, None, None).unwrap();
        assert_eq!(result.effective_max_output_tokens, Some(200));
        assert!(!result.output_tokens_clamped);
    }

    #[test]
    fn input_exceeds_limit_rejected() {
        let limits = TokenLimits {
            max_input_tokens: Some(1000),
            max_output_tokens: None,
        };
        let err = check_token_limits(&limits, Some(1500), None).unwrap_err();
        match err {
            GatewayError::Policy { code, message } => {
                assert_eq!(code, "input_tokens_exceeded");
                assert!(message.contains("1500"));
                assert!(message.contains("1000"));
            }
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn input_within_limit_passes() {
        let limits = TokenLimits {
            max_input_tokens: Some(1000),
            max_output_tokens: None,
        };
        let result = check_token_limits(&limits, Some(500), None).unwrap();
        assert_eq!(result.effective_max_output_tokens, None);
        assert!(!result.output_tokens_clamped);
    }

    #[test]
    fn input_equal_to_limit_passes() {
        let limits = TokenLimits {
            max_input_tokens: Some(1000),
            max_output_tokens: None,
        };
        assert!(check_token_limits(&limits, Some(1000), None).is_ok());
    }

    #[test]
    fn no_estimated_input_with_limit_passes() {
        let limits = TokenLimits {
            max_input_tokens: Some(1000),
            max_output_tokens: None,
        };
        // Can't estimate → don't reject (fail-open for estimation, not policy)
        assert!(check_token_limits(&limits, None, None).is_ok());
    }

    // -----------------------------------------------------------------------
    // estimate_input_tokens
    // -----------------------------------------------------------------------

    #[test]
    fn estimate_from_string_content() {
        let messages = json!([
            {"role": "user", "content": "Hello, world!"}
        ]);
        // "Hello, world!" = 13 chars → 13/4 = 3 (integer division)
        let estimate = estimate_input_tokens(&messages).unwrap();
        assert_eq!(estimate, 3);
    }

    #[test]
    fn estimate_from_multiple_messages() {
        let messages = json!([
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "Tell me a joke."}
        ]);
        // 16 + 15 = 31 chars → 31/4 = 7
        let estimate = estimate_input_tokens(&messages).unwrap();
        assert_eq!(estimate, 7);
    }

    #[test]
    fn estimate_from_content_parts() {
        let messages = json!([
            {"role": "user", "content": [
                {"type": "text", "text": "What is in this image?"},
                {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
            ]}
        ]);
        // "What is in this image?" = 22 chars → 22/4 = 5
        let estimate = estimate_input_tokens(&messages).unwrap();
        assert_eq!(estimate, 5);
    }

    #[test]
    fn estimate_minimum_is_one() {
        let messages = json!([
            {"role": "user", "content": "Hi"}
        ]);
        // "Hi" = 2 chars → 2/4 = 0 → clamped to 1
        let estimate = estimate_input_tokens(&messages).unwrap();
        assert_eq!(estimate, 1);
    }

    #[test]
    fn estimate_returns_none_for_non_array() {
        let messages = json!("not an array");
        assert!(estimate_input_tokens(&messages).is_none());
    }

    #[test]
    fn estimate_returns_none_for_empty_array() {
        let messages = json!([]);
        assert!(estimate_input_tokens(&messages).is_none());
    }
}
