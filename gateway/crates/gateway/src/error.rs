//! Gateway error types and OpenAI-compatible HTTP error responses.
//!
//! Every [`GatewayError`] variant maps to an HTTP status code and serialises
//! into the OpenAI-compatible JSON envelope:
//!
//! ```json
//! { "error": { "code": "...", "message": "...", "type": "...", "request_id": "..." } }
//! ```

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Response structs
// ---------------------------------------------------------------------------

/// Top-level JSON envelope returned to clients on error.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

/// Inner detail object -- mirrors the OpenAI error schema.
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

// ---------------------------------------------------------------------------
// GatewayError
// ---------------------------------------------------------------------------

/// Unified error enum for the gateway data-plane and admin API.
///
/// Each variant carries a machine-readable `code` and a human-readable
/// `message`.  The `IntoResponse` impl converts them into the correct HTTP
/// status + JSON body.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("authentication failed: {message}")]
    Auth { code: String, message: String },

    #[error("policy denied: {message}")]
    Policy { code: String, message: String },

    #[error("rate limit exceeded: {message}")]
    RateLimit {
        code: String,
        message: String,
        retry_after: Option<u64>,
    },

    #[error("budget exhausted: {message}")]
    Budget { code: String, message: String },

    #[error("provider error: {message}")]
    Provider {
        code: String,
        message: String,
        status: StatusCode,
    },

    #[error("routing error: {message}")]
    Routing { code: String, message: String },

    #[error("storage error: {message}")]
    Storage { code: String, message: String },

    #[error("configuration error: {message}")]
    Config { code: String, message: String },

    #[error("validation error: {message}")]
    Validation { code: String, message: String },

    #[error("{message}")]
    NotFound { code: String, message: String },

    #[error("{message}")]
    Conflict { code: String, message: String },
}

// ---------------------------------------------------------------------------
// Status-code mapping
// ---------------------------------------------------------------------------

impl GatewayError {
    /// Returns the HTTP status code for this error variant.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Auth { .. } => StatusCode::UNAUTHORIZED,
            Self::Policy { .. } | Self::Budget { .. } => StatusCode::FORBIDDEN,
            Self::RateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Provider { status, .. } => *status,
            Self::Routing { .. } => StatusCode::BAD_GATEWAY,
            Self::Storage { .. } | Self::Config { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Conflict { .. } => StatusCode::CONFLICT,
        }
    }

    /// Returns the `type` string used in the JSON error response.
    fn error_type(&self) -> &'static str {
        match self {
            Self::Auth { .. } => "authentication_error",
            Self::Policy { .. } => "policy_error",
            Self::RateLimit { .. } => "rate_limit_error",
            Self::Budget { .. } => "budget_error",
            Self::Provider { .. } => "provider_error",
            Self::Routing { .. } => "routing_error",
            Self::Storage { .. } => "storage_error",
            Self::Config { .. } => "configuration_error",
            Self::Validation { .. } => "validation_error",
            Self::NotFound { .. } => "not_found_error",
            Self::Conflict { .. } => "conflict_error",
        }
    }

    /// Builds the [`ErrorResponse`] payload (without `request_id` -- that is
    /// injected by middleware).
    fn to_error_response(&self) -> ErrorResponse {
        let (code, message) = match self {
            Self::Auth { code, message }
            | Self::Policy { code, message }
            | Self::Budget { code, message }
            | Self::Routing { code, message }
            | Self::Storage { code, message }
            | Self::Config { code, message }
            | Self::Validation { code, message }
            | Self::NotFound { code, message }
            | Self::Conflict { code, message }
            | Self::RateLimit { code, message, .. }
            | Self::Provider { code, message, .. } => (code.as_str(), message.as_str()),
        };

        ErrorResponse {
            error: ErrorDetail {
                code: code.to_owned(),
                message: message.to_owned(),
                error_type: self.error_type().to_owned(),
                request_id: None,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// IntoResponse
// ---------------------------------------------------------------------------

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let retry_after = match &self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        };
        let body = self.to_error_response();

        // `serde_json::to_vec` only fails on non-string map keys or
        // unsupported types, neither of which apply here.  Fall back to a
        // plain-text 500 if serialisation somehow fails.
        match serde_json::to_vec(&body) {
            Ok(bytes) => {
                let mut response = (
                    status,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        axum::http::HeaderValue::from_static("application/json"),
                    )],
                    bytes,
                )
                    .into_response();
                if let Some(secs) = retry_after {
                    if let Ok(val) = axum::http::HeaderValue::from_str(&secs.to_string()) {
                        response.headers_mut().insert("retry-after", val);
                    }
                }
                response
            }
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error: failed to serialize error response",
            )
                .into_response(),
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl GatewayError {
    pub fn auth(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Auth {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn policy(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Policy {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn rate_limit(
        code: impl Into<String>,
        message: impl Into<String>,
        retry_after: Option<u64>,
    ) -> Self {
        Self::RateLimit {
            code: code.into(),
            message: message.into(),
            retry_after,
        }
    }

    pub fn budget(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Budget {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn provider(
        code: impl Into<String>,
        message: impl Into<String>,
        status: StatusCode,
    ) -> Self {
        Self::Provider {
            code: code.into(),
            message: message.into(),
            status,
        }
    }

    pub fn routing(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Routing {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn storage(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Storage {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn config(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn validation(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::NotFound {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Conflict {
            code: code.into(),
            message: message.into(),
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    // -----------------------------------------------------------------------
    // Status-code mapping
    // -----------------------------------------------------------------------

    #[test]
    fn auth_maps_to_401() {
        let err = GatewayError::auth("invalid_key", "bad key");
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn policy_maps_to_403() {
        let err = GatewayError::policy("denied", "nope");
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn rate_limit_maps_to_429() {
        let err = GatewayError::rate_limit("rate_limited", "slow down", Some(30));
        assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn budget_maps_to_403() {
        let err = GatewayError::budget("budget_exceeded", "no budget");
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn provider_uses_given_status() {
        let err = GatewayError::provider("upstream", "bad gateway", StatusCode::BAD_GATEWAY);
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);

        let err2 =
            GatewayError::provider("upstream_timeout", "timed out", StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(err2.status_code(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[test]
    fn routing_maps_to_502() {
        let err = GatewayError::routing("no_route", "cannot route");
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn storage_maps_to_500() {
        let err = GatewayError::storage("db_error", "connection lost");
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn config_maps_to_500() {
        let err = GatewayError::config("bad_config", "missing field");
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn validation_maps_to_422() {
        let err = GatewayError::validation("invalid_model", "unknown model");
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    // -----------------------------------------------------------------------
    // JSON serialisation (OpenAI-compatible envelope)
    // -----------------------------------------------------------------------

    #[test]
    fn error_response_serialises_to_openai_format() {
        let err = GatewayError::auth("invalid_api_key", "The API key is invalid");
        let resp = err.to_error_response();
        let json = serde_json::to_value(&resp).expect("serialisation should not fail");

        assert_eq!(json["error"]["code"], "invalid_api_key");
        assert_eq!(json["error"]["message"], "The API key is invalid");
        assert_eq!(json["error"]["type"], "authentication_error");
        // request_id is None so the field must be absent (skip_serializing_if)
        assert!(json["error"].get("request_id").is_none());
    }

    #[test]
    fn error_response_includes_request_id_when_present() {
        let resp = ErrorResponse {
            error: ErrorDetail {
                code: "x".to_owned(),
                message: "y".to_owned(),
                error_type: "test".to_owned(),
                request_id: Some("req-123".to_owned()),
            },
        };
        let json = serde_json::to_value(&resp).expect("serialisation should not fail");
        assert_eq!(json["error"]["request_id"], "req-123");
    }

    // -----------------------------------------------------------------------
    // IntoResponse integration
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn into_response_returns_correct_status_and_json_body() {
        let err = GatewayError::validation("missing_field", "field `model` is required");
        let response = err.into_response();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .expect("Content-Type header must be present");
        assert_eq!(content_type, "application/json");

        let body_bytes = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body read should succeed");
        let body: serde_json::Value =
            serde_json::from_slice(&body_bytes).expect("body must be valid JSON");

        assert_eq!(body["error"]["code"], "missing_field");
        assert_eq!(body["error"]["message"], "field `model` is required");
        assert_eq!(body["error"]["type"], "validation_error");
    }

    #[tokio::test]
    async fn into_response_rate_limit_returns_429() {
        let err = GatewayError::rate_limit("rate_limited", "too many requests", Some(60));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn into_response_rate_limit_includes_retry_after_header() {
        let err = GatewayError::rate_limit("rate_limited", "slow down", Some(30));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry = response
            .headers()
            .get("retry-after")
            .expect("retry-after header must be present");
        assert_eq!(retry.to_str().expect("valid utf-8"), "30");
    }

    #[tokio::test]
    async fn into_response_rate_limit_no_retry_after_when_none() {
        let err = GatewayError::rate_limit("rate_limited", "slow down", None);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            response.headers().get("retry-after").is_none(),
            "retry-after should not be present when None"
        );
    }

    // -----------------------------------------------------------------------
    // Display / Error trait
    // -----------------------------------------------------------------------

    #[test]
    fn display_formats_correctly() {
        let err = GatewayError::auth("x", "bad token");
        assert_eq!(err.to_string(), "authentication failed: bad token");

        let err = GatewayError::routing("y", "no backends");
        assert_eq!(err.to_string(), "routing error: no backends");
    }

    // -----------------------------------------------------------------------
    // Convenience constructors
    // -----------------------------------------------------------------------

    #[test]
    fn convenience_constructors_accept_str_and_string() {
        // &str
        let _ = GatewayError::auth("code", "msg");
        // String
        let _ = GatewayError::auth("code".to_owned(), "msg".to_owned());
    }
}
