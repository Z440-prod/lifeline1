use axum::{response::IntoResponse, Json};
use serde_json::json;

/// Handler for `GET /health`.
/// Lightweight liveness probe returning build metadata.
/// Suitable for Kubernetes readiness/liveness probes and load balancer health checks.
pub async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
