//! Health check endpoints -- liveness, readiness, and metrics.

use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Liveness probe: returns 200 "ok" if the process is running.
/// No dependency checks -- if this handler runs, the server is alive.
pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Readiness probe: returns 200 "ok" when the server can accept traffic.
/// TODO: Add database and Redis connectivity checks in a later task.
pub async fn readyz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Metrics endpoint: placeholder for Prometheus-format metrics.
/// TODO: Wire up `metrics-exporter-prometheus` in a later task.
pub async fn metrics_handler() -> impl IntoResponse {
    StatusCode::OK
}
