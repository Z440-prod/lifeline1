use axum::http::{HeaderName, Method};
use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use axum_prometheus::{
    metrics_exporter_prometheus::PrometheusHandle, PrometheusMetricLayerBuilder,
};
use std::sync::{Arc, OnceLock};
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

pub mod ai;
pub mod auth;
pub mod health;
pub mod integrations;
pub mod stream;
pub mod sync;

/// Process-wide Prometheus recorder handle. `axum_prometheus`'s default
/// handle installs a *global* `metrics` recorder on creation, which panics
/// if attempted twice in the same process — so `create_router` must only
/// ever trigger that installation once, even though it may legitimately be
/// called more than once per process (multiple `#[tokio::test]`s in one
/// binary each build their own router).
static METRIC_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Assemble the application router.
/// Defines all endpoints under the `/api/v1` namespace and applies `attest_guard` middleware
/// to protected resources (sync, AI proxy).
///
/// Infrastructure endpoints (`/health`, `/metrics`) are mounted at the root level,
/// exempt from authentication.
pub fn create_router(state: Arc<AppState>) -> Router {
    // ── CORS ──────────────────────────────────────────────────────────────────
    // The primary client is a native iOS app, which never sends an `Origin`
    // header and is therefore unaffected by this policy. A wildcard origin
    // combined with wildcard headers only matters for browser-based callers
    // (e.g. the local demo page) and would otherwise let any website make
    // authenticated cross-origin requests against a leaked bearer token.
    // Wide open only in development; locked down to just what the API needs
    // in every other environment.
    let cors = if state.config.auth.environment == "development" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([
                HeaderName::from_static("authorization"),
                HeaderName::from_static("content-type"),
                HeaderName::from_static("x-device-id"),
                HeaderName::from_static("x-assertion-token"),
            ])
    };

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
    // The exporter handle (used to render `/metrics`) is installed at most
    // once per process via `METRIC_HANDLE`; the recording layer itself is
    // cheap and stateless, so it's safe to build fresh on every call.
    let metric_handle = METRIC_HANDLE
        .get_or_init(|| axum_prometheus::Handle::default().0)
        .clone();
    let (prometheus_layer, _) = PrometheusMetricLayerBuilder::new()
        .with_metrics_from_fn(|| metric_handle.clone())
        .build_pair();

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
        .route(
            "/sync/documents/{document_type}",
            get(sync::list_documents_by_type_handler),
        )
        .route("/ai/proxy", post(ai::ai_proxy_handler))
        .route(
            "/integrations",
            get(integrations::list_integrations_handler),
        )
        .route(
            "/integrations/{provider}/connect",
            post(integrations::connect_on_device_handler),
        )
        .route(
            "/integrations/{provider}",
            delete(integrations::disconnect_handler),
        )
        .route(
            "/integrations/whoop/authorize",
            get(integrations::whoop_authorize_handler),
        )
        .route(
            "/integrations/whoop/metrics",
            get(integrations::whoop_metrics_handler),
        )
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
        .route("/stream", get(stream::ws_upgrade_handler))
        // Whoop redirects the user's own browser here after consent — no
        // Authorization header is present, so this cannot sit behind
        // attest_guard. The signed `state` param (see crypto::oauth_state)
        // is what authenticates the callback instead.
        .route(
            "/integrations/whoop/callback",
            get(integrations::whoop_callback_handler),
        );

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
