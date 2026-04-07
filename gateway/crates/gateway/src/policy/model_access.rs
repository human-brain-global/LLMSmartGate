//! Model access control -- allowed/denied model lists with glob-pattern matching.
//!
//! Deny takes precedence over allow. Empty `allowed_models` means all models
//! (not explicitly denied) are permitted.

use crate::error::GatewayError;

/// Merged model access rules from one or more policies.
#[derive(Debug, Clone, Default)]
pub struct ModelAccessRules {
    /// Allowlist of model name glob patterns (e.g. `["gpt-4*", "claude-3-sonnet"]`).
    /// Empty means "allow all models not in denied list".
    pub allowed_models: Vec<String>,
    /// Denylist of model name glob patterns. Deny always takes precedence over allow.
    pub denied_models: Vec<String>,
}

/// Check whether `model` is permitted by the given access rules.
///
/// Logic:
/// 1. If any `denied_models` pattern matches → **deny**
/// 2. If `allowed_models` is non-empty and no pattern matches → **deny**
/// 3. Otherwise → **allow**
///
/// # Errors
///
/// Returns `GatewayError::Policy` with code `model_not_allowed` on denial.
pub fn check_model_access(rules: &ModelAccessRules, model: &str) -> Result<(), GatewayError> {
    // 1. Check denylist first — deny takes precedence
    for pattern in &rules.denied_models {
        if matches_glob(pattern, model) {
            return Err(GatewayError::policy(
                "model_not_allowed",
                format!("model '{model}' is denied by policy"),
            ));
        }
    }

    // 2. Check allowlist (empty = allow all)
    if !rules.allowed_models.is_empty() {
        let allowed = rules.allowed_models.iter().any(|p| matches_glob(p, model));
        if !allowed {
            return Err(GatewayError::policy(
                "model_not_allowed",
                format!("model '{model}' is not allowed by policy"),
            ));
        }
    }

    Ok(())
}

/// Simple glob-pattern matching for model names.
///
/// Supports:
/// - Exact match: `"gpt-4"` matches only `"gpt-4"`
/// - Trailing wildcard: `"gpt-4*"` matches `"gpt-4"`, `"gpt-4-turbo"`, etc.
/// - Lone wildcard: `"*"` matches everything
fn matches_glob(pattern: &str, value: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // matches_glob
    // -----------------------------------------------------------------------

    #[test]
    fn glob_exact_match() {
        assert!(matches_glob("gpt-4", "gpt-4"));
        assert!(!matches_glob("gpt-4", "gpt-4-turbo"));
    }

    #[test]
    fn glob_trailing_wildcard() {
        assert!(matches_glob("gpt-4*", "gpt-4"));
        assert!(matches_glob("gpt-4*", "gpt-4-turbo"));
        assert!(matches_glob("gpt-4*", "gpt-4o"));
        assert!(!matches_glob("gpt-4*", "gpt-3.5-turbo"));
    }

    #[test]
    fn glob_lone_wildcard_matches_everything() {
        assert!(matches_glob("*", "gpt-4"));
        assert!(matches_glob("*", "claude-3-sonnet"));
        assert!(matches_glob("*", ""));
    }

    #[test]
    fn glob_empty_pattern_matches_only_empty() {
        assert!(matches_glob("", ""));
        assert!(!matches_glob("", "gpt-4"));
    }

    // -----------------------------------------------------------------------
    // check_model_access — allow/deny logic
    // -----------------------------------------------------------------------

    #[test]
    fn empty_rules_allow_everything() {
        let rules = ModelAccessRules::default();
        assert!(check_model_access(&rules, "gpt-4").is_ok());
        assert!(check_model_access(&rules, "claude-3-opus").is_ok());
    }

    #[test]
    fn allowlist_permits_matching_model() {
        let rules = ModelAccessRules {
            allowed_models: vec!["gpt-4*".to_owned(), "claude-3-sonnet".to_owned()],
            denied_models: vec![],
        };
        assert!(check_model_access(&rules, "gpt-4").is_ok());
        assert!(check_model_access(&rules, "gpt-4-turbo").is_ok());
        assert!(check_model_access(&rules, "claude-3-sonnet").is_ok());
    }

    #[test]
    fn allowlist_denies_non_matching_model() {
        let rules = ModelAccessRules {
            allowed_models: vec!["gpt-4*".to_owned()],
            denied_models: vec![],
        };
        let err = check_model_access(&rules, "claude-3-opus").unwrap_err();
        assert!(err.to_string().contains("not allowed"));
    }

    #[test]
    fn denylist_blocks_matching_model() {
        let rules = ModelAccessRules {
            allowed_models: vec![],
            denied_models: vec!["gpt-4*".to_owned()],
        };
        let err = check_model_access(&rules, "gpt-4-turbo").unwrap_err();
        assert!(err.to_string().contains("denied"));
    }

    #[test]
    fn denylist_allows_non_matching_model() {
        let rules = ModelAccessRules {
            allowed_models: vec![],
            denied_models: vec!["gpt-4*".to_owned()],
        };
        assert!(check_model_access(&rules, "claude-3-opus").is_ok());
    }

    #[test]
    fn deny_takes_precedence_over_allow() {
        let rules = ModelAccessRules {
            allowed_models: vec!["gpt-4*".to_owned()],
            denied_models: vec!["gpt-4-turbo".to_owned()],
        };
        // gpt-4 is allowed and not denied → ok
        assert!(check_model_access(&rules, "gpt-4").is_ok());
        // gpt-4-turbo matches both allow and deny → denied (deny wins)
        let err = check_model_access(&rules, "gpt-4-turbo").unwrap_err();
        assert!(err.to_string().contains("denied"));
    }

    #[test]
    fn deny_glob_overrides_allow_glob() {
        let rules = ModelAccessRules {
            allowed_models: vec!["*".to_owned()],
            denied_models: vec!["gpt-4*".to_owned()],
        };
        assert!(check_model_access(&rules, "claude-3-opus").is_ok());
        assert!(check_model_access(&rules, "gpt-4-turbo").is_err());
    }

    #[test]
    fn error_code_is_model_not_allowed() {
        let rules = ModelAccessRules {
            allowed_models: vec!["gpt-4".to_owned()],
            denied_models: vec![],
        };
        let err = check_model_access(&rules, "claude-3-opus").unwrap_err();
        match err {
            GatewayError::Policy { code, .. } => assert_eq!(code, "model_not_allowed"),
            other => panic!("expected Policy error, got: {other:?}"),
        }
    }
}
