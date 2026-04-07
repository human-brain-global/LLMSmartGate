//! Feature flag enforcement -- streaming, tool use, and file input controls.
//!
//! Each flag defaults to `true` (permissive). When a flag is `false`, requests
//! that use the corresponding feature are rejected with a 403.

use crate::error::GatewayError;

/// Feature flags from a merged policy evaluation.
#[derive(Debug, Clone)]
pub struct FeatureFlags {
    /// Whether streaming responses are allowed.
    pub allow_streaming: bool,
    /// Whether tool/function calling is allowed.
    pub allow_tools: bool,
    /// Whether file/image content parts are allowed.
    pub allow_files: bool,
}

impl Default for FeatureFlags {
    /// Permissive defaults: all features enabled.
    ///
    /// # Security note
    ///
    /// `EvaluatedPolicy::default()` overrides `allow_tools` and `allow_files`
    /// to `false` for unbound service accounts (fail-safe per LLD). Do **not**
    /// use `FeatureFlags::default()` directly for policy enforcement — always
    /// go through `EvaluatedPolicy`.
    fn default() -> Self {
        Self {
            allow_streaming: true,
            allow_tools: true,
            allow_files: true,
        }
    }
}

/// Features detected in the incoming request.
#[derive(Debug, Clone, Default)]
pub struct RequestFeatures {
    /// `true` if `stream: true` is set in the request body.
    pub stream: bool,
    /// `true` if `tools` or `tool_choice` is present in the request body.
    pub has_tools: bool,
    /// `true` if any message content part contains file/image data.
    pub has_files: bool,
}

/// Check feature flags against the request's detected features.
///
/// Returns the first violation found (checked in order: streaming, tools, files).
///
/// # Errors
///
/// Returns `GatewayError::Policy` with one of:
/// - `streaming_not_allowed`
/// - `tools_not_allowed`
/// - `files_not_allowed`
pub fn check_features(flags: &FeatureFlags, request: &RequestFeatures) -> Result<(), GatewayError> {
    if !flags.allow_streaming && request.stream {
        return Err(GatewayError::policy(
            "streaming_not_allowed",
            "streaming is not allowed by policy",
        ));
    }

    if !flags.allow_tools && request.has_tools {
        return Err(GatewayError::policy(
            "tools_not_allowed",
            "tool use is not allowed by policy",
        ));
    }

    if !flags.allow_files && request.has_files {
        return Err(GatewayError::policy(
            "files_not_allowed",
            "file input is not allowed by policy",
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

    #[test]
    fn default_flags_allow_everything() {
        let flags = FeatureFlags::default();
        let request = RequestFeatures {
            stream: true,
            has_tools: true,
            has_files: true,
        };
        assert!(check_features(&flags, &request).is_ok());
    }

    #[test]
    fn no_features_always_passes() {
        let flags = FeatureFlags {
            allow_streaming: false,
            allow_tools: false,
            allow_files: false,
        };
        let request = RequestFeatures::default();
        assert!(check_features(&flags, &request).is_ok());
    }

    #[test]
    fn streaming_denied() {
        let flags = FeatureFlags {
            allow_streaming: false,
            ..Default::default()
        };
        let request = RequestFeatures {
            stream: true,
            ..Default::default()
        };
        let err = check_features(&flags, &request).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "streaming_not_allowed"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn streaming_flag_false_but_not_streaming_passes() {
        let flags = FeatureFlags {
            allow_streaming: false,
            ..Default::default()
        };
        let request = RequestFeatures {
            stream: false,
            ..Default::default()
        };
        assert!(check_features(&flags, &request).is_ok());
    }

    #[test]
    fn tools_denied() {
        let flags = FeatureFlags {
            allow_tools: false,
            ..Default::default()
        };
        let request = RequestFeatures {
            has_tools: true,
            ..Default::default()
        };
        let err = check_features(&flags, &request).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "tools_not_allowed"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn tools_flag_false_but_no_tools_passes() {
        let flags = FeatureFlags {
            allow_tools: false,
            ..Default::default()
        };
        let request = RequestFeatures::default();
        assert!(check_features(&flags, &request).is_ok());
    }

    #[test]
    fn files_denied() {
        let flags = FeatureFlags {
            allow_files: false,
            ..Default::default()
        };
        let request = RequestFeatures {
            has_files: true,
            ..Default::default()
        };
        let err = check_features(&flags, &request).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "files_not_allowed"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }

    #[test]
    fn files_flag_false_but_no_files_passes() {
        let flags = FeatureFlags {
            allow_files: false,
            ..Default::default()
        };
        let request = RequestFeatures::default();
        assert!(check_features(&flags, &request).is_ok());
    }

    #[test]
    fn first_violation_wins_streaming_before_tools() {
        let flags = FeatureFlags {
            allow_streaming: false,
            allow_tools: false,
            allow_files: false,
        };
        let request = RequestFeatures {
            stream: true,
            has_tools: true,
            has_files: true,
        };
        let err = check_features(&flags, &request).unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "streaming_not_allowed"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }
}
