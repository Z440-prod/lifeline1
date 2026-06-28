use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use std::sync::Arc;
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

pub mod ai;
pub mod auth;
pub mod health;
pub mod stream;
pub mod sync;

/// Assemble the application router.
/// Defines all endpoints under the `/api/v1` namespace and applies `attest_guard` middleware
/// to protected resources (sync, AI proxy).
///
/// Infrastructure endpoints (`/health`, `/metrics`) are mounted at the root level,
/// exempt from authentication.
pub fn create_router(state: Arc<AppState>) -> Router {
    // CORS layer for development — allows the demo web app to connect
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // ── Rate Limiting (per-IP, token bucket) ─────────────────────────────────
    // Uses tower_governor with the default PeerIpKeyExtractor.
    // Config: `requests_per_second` controls replenish interval, `burst_size`
    // controls the maximum number of requests that can be made in a burst.
    let governor_config = GovernorConfigBuilder::default()
        .per_second(state.config.rate_limit.requests_per_second)
        .burst_size(state.config.rate_limit.burst_size)
        .finish()
        .expect("Invalid rate limit configuration: burst_size and per_second must be non-zero");

    let governor_layer = tower_governor::GovernorLayer {
        config: Arc::new(governor_config),
    };

    // ── Prometheus Metrics ───────────────────────────────────────────────────
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    // Define the core API v1 routes.
    // We separate them into public routes (challenges, attestations, stream)
    // and protected routes (sync, AI proxy).
    let protected_routes = Router::new()
        .route("/sync/delta", post(sync::sync_delta_handler))
        .route("/sync/document/{id}", get(sync::get_document_handler))
        .route(
            "/sync/document/{id}/history",
            get(sync::get_document_history_handler),
        )
        .route("/ai/proxy", post(ai::ai_proxy_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::attest_guard::attest_guard,
        ));

    let public_routes = Router::new()
        .route("/auth/challenge", get(auth::challenge_handler))
        .route(
            "/auth/verify-attestation",
            post(auth::verify_attestation_handler),
        )
        .route("/auth/assert", post(auth::assert_handler))
        .route("/ai/policy-matrix", get(ai::policy_matrix_handler))
        .route("/stream", get(stream::ws_upgrade_handler));

    // ── Infrastructure endpoints (no auth) ───────────────────────────────────
    let infra_routes = Router::new()
        .route("/health", get(health::health_handler))
        .route("/metrics", get(|| async move { metric_handle.render() }));

    // Combine routes with middleware layers.
    // Layer application order (outermost first): CORS → rate limiting → metrics.
    Router::new()
        .merge(infra_routes)
        .nest("/api/v1", public_routes)
        .nest("/api/v1", protected_routes)
        .layer(prometheus_layer)
        .layer(governor_layer)
        .layer(cors)
        .with_state(state)
}
