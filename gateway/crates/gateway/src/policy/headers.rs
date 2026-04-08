//! Rate limit response header injection.
//!
//! Adds `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset`
//! to every response. `Retry-After` on 429 is handled by `GatewayError`.

use axum::http::HeaderValue;
use axum::response::Response;

use super::rate_limit::RateLimitOutcome;

/// Convert a numeric value to a `HeaderValue`.
///
/// Decimal digit strings are always valid header values, so this is infallible.
fn header_from_u64(n: u64) -> HeaderValue {
    HeaderValue::from_str(&n.to_string())
        .expect("decimal digit string is always a valid header value")
}

/// Inject rate limit headers into a response.
pub fn inject_rate_limit_headers(mut response: Response, outcome: &RateLimitOutcome) -> Response {
    let headers = response.headers_mut();

    headers.insert(
        "x-ratelimit-limit",
        header_from_u64(u64::from(outcome.limit)),
    );
    headers.insert(
        "x-ratelimit-remaining",
        header_from_u64(u64::from(outcome.remaining)),
    );
    headers.insert("x-ratelimit-reset", header_from_u64(outcome.reset_at));

    response
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn make_response() -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .body(axum::body::Body::empty())
            .expect("build response")
    }

    #[test]
    fn injects_all_rate_limit_headers() {
        let outcome = RateLimitOutcome {
            limit: 1000,
            remaining: 999,
            reset_at: 1_700_000_000,
        };

        let response = inject_rate_limit_headers(make_response(), &outcome);
        let headers = response.headers();

        assert_eq!(headers.get("x-ratelimit-limit").unwrap(), "1000");
        assert_eq!(headers.get("x-ratelimit-remaining").unwrap(), "999");
        assert_eq!(headers.get("x-ratelimit-reset").unwrap(), "1700000000"); // raw header value
    }

    #[test]
    fn headers_with_zero_remaining() {
        let outcome = RateLimitOutcome {
            limit: 100,
            remaining: 0,
            reset_at: 1_700_000_060,
        };

        let response = inject_rate_limit_headers(make_response(), &outcome);
        assert_eq!(
            response.headers().get("x-ratelimit-remaining").unwrap(),
            "0"
        );
    }
}
