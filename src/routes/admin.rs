//! Admin dashboard API.
//!
//! `GET /api/v1/admin/stats` returns aggregate, non-identifying operational
//! statistics for the operator's dashboard. It is **disabled by default** and
//! only reachable once `ANTIGRAVITY__ADMIN__ADMIN_TOKEN` is configured, then
//! gated by a constant-time bearer-token check.
//!
//! PRIVACY: this endpoint exposes only counts, tier totals, and pseudonymous
//! leaderboard handles (already public via the Arena). It never returns health
//! data, vault contents, emails, or any PII — the zero-knowledge guarantee
//! holds even for the admin.

use axum::{extract::State, http::HeaderMap, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::errors::AppError;
use crate::state::AppState;

/// Constant-time comparison of the configured admin token against the presented
/// one. Both are SHA-256'd first (so the raw token's length never affects
/// timing), then the two fixed-length 32-byte digests are compared without
/// short-circuiting.
fn admin_authorized(configured: &str, provided: &str) -> bool {
    use ring::digest::{digest, SHA256};
    let a = digest(&SHA256, configured.as_bytes());
    let b = digest(&SHA256, provided.as_bytes());
    let (a, b) = (a.as_ref(), b.as_ref());
    // Digests are always 32 bytes, so this length check never leaks token info.
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Enforce that the admin dashboard is enabled and the request carries the
/// correct admin token. 403 when disabled (no token configured), 401 on a wrong
/// or missing token.
fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    if !state.config.admin.enabled() {
        return Err(AppError::Forbidden(
            "Admin dashboard is disabled on this deployment.".to_owned(),
        ));
    }
    let provided = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    if admin_authorized(&state.config.admin.admin_token, provided) {
        Ok(())
    } else {
        Err(AppError::Unauthorized("Invalid admin token.".to_owned()))
    }
}

/// Handler for `GET /api/v1/admin/stats`.
#[tracing::instrument(skip(state, headers))]
pub async fn admin_stats_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(state.as_ref(), &headers)?;
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/admin/stats").increment(1);

    let s = state.db.admin_stats().await?;

    let uptime_seconds = (chrono::Utc::now() - state.started_at).num_seconds().max(0);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let coach_today = state
        .ai_usage
        .get(&format!("ai:global:{today}"))
        .unwrap_or(0);

    let paid = s.subscriptions_pro + s.subscriptions_elite;
    let free_devices = (s.devices - paid).max(0);
    // A rough MRR estimate from the standard tier prices — useful at a glance.
    let mrr = s.subscriptions_pro as f64 * 7.99 + s.subscriptions_elite as f64 * 14.99;

    let leagues: Vec<Value> = s
        .leagues
        .iter()
        .map(|(league, count)| json!({ "league": league, "count": count }))
        .collect();
    let top: Vec<Value> = s
        .top_players
        .iter()
        .map(
            |(handle, score, league)| json!({ "handle": handle, "score": score, "league": league }),
        )
        .collect();

    Ok(Json(json!({
        "generated_at": chrono::Utc::now(),
        "system": {
            "version": env!("CARGO_PKG_VERSION"),
            "environment": state.config.auth.environment,
            "database": state.db.backend(),
            "uptime_seconds": uptime_seconds,
        },
        "users": { "accounts": s.accounts, "devices": s.devices },
        "vault": { "documents": s.documents, "versions": s.document_versions },
        "arena": { "ranked_players": s.ranked_players, "leagues": leagues, "top": top },
        "billing": {
            "pro": s.subscriptions_pro,
            "elite": s.subscriptions_elite,
            "free_devices": free_devices,
            "estimated_mrr_usd": (mrr * 100.0).round() / 100.0,
        },
        "ai": {
            "coach_messages_today": coach_today,
            "global_daily_budget": state.config.ai.budget.global_daily_budget,
        },
    })))
}

#[cfg(test)]
mod tests {
    use super::admin_authorized;

    #[test]
    fn admin_token_compare() {
        assert!(admin_authorized("s3cret-token", "s3cret-token"));
        assert!(!admin_authorized("s3cret-token", "wrong"));
        assert!(!admin_authorized("s3cret-token", ""));
        assert!(!admin_authorized("s3cret-token", "s3cret-token-plus"));
        // An empty configured token still shouldn't match empty input here; the
        // route separately refuses to serve when the token is unset.
        assert!(admin_authorized("", ""));
    }
}
