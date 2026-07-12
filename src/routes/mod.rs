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

pub mod account;
pub mod ai;
pub mod auth;
pub mod billing;
pub mod game;
pub mod health;
pub mod insights;
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

/// Response hardening + edge-cache policy, one pass over every response.
///
/// * Security headers on everything: `nosniff`, `DENY` framing, referrer
///   suppression, a conservative CSP (the app is fully self-contained — no
///   external scripts, styles, or fonts), and HSTS in production (the engine
///   sits behind TLS termination there).
/// * `Cache-Control` on the public, user-independent surfaces so browsers and
///   any CDN (e.g. Cloudflare in front) serve repeats without touching the
///   engine: rulebook configs for 5 minutes, static assets for an hour —
///   faster for users, cheaper on compute and egress.
async fn harden_and_cache(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_owned();
    let is_get = req.method() == Method::GET;
    let mut res = next.run(req).await;
    let ok = res.status().is_success();
    let h = res.headers_mut();

    let hv = axum::http::HeaderValue::from_static;
    h.insert("x-content-type-options", hv("nosniff"));
    h.insert("x-frame-options", hv("DENY"));
    h.insert("referrer-policy", hv("no-referrer"));
    h.insert(
        "permissions-policy",
        hv("camera=(), microphone=(), geolocation=()"),
    );
    h.insert(
        "content-security-policy",
        hv(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
            img-src 'self' data:; connect-src 'self' ws: wss:; manifest-src 'self'; \
            frame-ancestors 'none'; base-uri 'self'",
        ),
    );
    if state.config.auth.environment == "production" {
        h.insert(
            "strict-transport-security",
            hv("max-age=63072000; includeSubDomains"),
        );
    }

    if is_get && ok && !h.contains_key(axum::http::header::CACHE_CONTROL) {
        let policy = if path.starts_with("/assets/") {
            Some("public, max-age=3600, stale-while-revalidate=86400")
        } else if matches!(
            path.as_str(),
            "/api/v1/insights/config"
                | "/api/v1/game/config"
                | "/api/v1/billing/config"
                | "/api/v1/ai/policy-matrix"
                | "/api/v1/ai/local-models"
        ) {
            Some("public, max-age=300, stale-while-revalidate=3600")
        } else {
            None
        };
        if let Some(p) = policy {
            h.insert(axum::http::header::CACHE_CONTROL, hv(p));
        }
    }
    res
}

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
        // ── Gamification: global health ranking (client submits only an opaque
        //    derived vitality score — never raw health data) ──────────────────
        .route("/game/score", post(game::submit_score_handler))
        .route("/game/profile", get(game::get_profile_handler))
        .route("/game/leaderboard", get(game::leaderboard_handler))
        // ── Billing: subscription state + Stripe checkout/portal ─────────────
        .route("/billing/subscription", get(billing::subscription_handler))
        .route("/billing/checkout", post(billing::checkout_handler))
        .route("/billing/portal", post(billing::portal_handler))
        .route("/billing/donate", post(billing::donate_handler))
        .route(
            "/billing/store-receipt",
            post(billing::store_receipt_handler),
        )
        .route(
            "/billing/beta-features",
            get(billing::beta_features_handler),
        )
        // Permanent account + data deletion (App Store 5.1.1(v) / GDPR erasure).
        // Authenticated by the device session like every other protected route.
        .route("/account", delete(account::delete_handler))
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
        // Development-only session mint for clients without App Attest (the
        // browser app). The handler hard-rejects outside development.
        .route("/auth/dev-session", post(auth::dev_session_handler))
        // Account sign-in layer (email/password + Apple/Google). Public: these
        // establish the session. Each mints a device-bound token internally.
        .route("/account/register", post(account::register_handler))
        .route("/account/login", post(account::login_handler))
        .route("/account/oauth", post(account::oauth_handler))
        .route("/ai/policy-matrix", get(ai::policy_matrix_handler))
        // Catalog of on-device AI models (Gemma sizes, hardware floors) so
        // premium phones can run the coach offline. Rules-only, cacheable.
        .route("/ai/local-models", get(ai::local_models_handler))
        // Rules-only insights config for the on-device longevity engine; ships
        // no user data, so it's public + cacheable like the policy matrix.
        .route("/insights/config", get(insights::insights_config_handler))
        // Rules-only game + billing catalogs (league ladder, tier prices) —
        // no user data, public like the policy matrix.
        .route("/game/config", get(game::game_config_handler))
        .route("/billing/config", get(billing::billing_config_handler))
        // Stripe posts payment events here. Public (no session) but
        // authenticated by verifying the Stripe-Signature HMAC over the raw
        // body — see billing::verify_stripe_signature.
        .route("/billing/webhook", post(billing::webhook_handler))
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

    // ── Web app (static SPA) ─────────────────────────────────────────────────
    // The Lifeline web app lives in `web/` and is served by this same binary,
    // so `cargo run` gives the complete product at http://host:port/. Assets
    // resolve exactly; every other non-API path serves index.html with 200 so
    // client-side routes deep-link correctly.
    let assets = tower_http::services::ServeDir::new("web/assets");
    let manifest = tower_http::services::ServeFile::new("web/manifest.webmanifest");
    // Service worker — must be served from the root so it can control the whole
    // origin scope (a worker's scope is capped at its own URL path).
    let service_worker = tower_http::services::ServeFile::new("web/sw.js");
    // Store listings (App Store / Google Play) require a public privacy
    // policy URL — served by the same binary at /privacy.
    let privacy = tower_http::services::ServeFile::new("web/privacy.html");
    let shell = tower_http::services::ServeFile::new("web/index.html");

    // Combine routes with middleware layers.
    // Layer application order (outermost first): CORS → rate limiting → metrics.
    Router::new()
        .merge(infra_routes)
        .nest("/api/v1", public_routes)
        .nest("/api/v1", protected_routes)
        .nest_service("/assets", assets)
        .route_service("/manifest.webmanifest", manifest)
        .route_service("/sw.js", service_worker)
        .route_service("/privacy", privacy)
        .fallback_service(shell)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            harden_and_cache,
        ))
        // Brotli/gzip on every compressible response (HTML/CSS/JS/JSON):
        // ~70% less egress — faster loads, cheaper hosting.
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(prometheus_layer)
        .layer(governor_layer)
        .layer(cors)
        .with_state(state)
}
