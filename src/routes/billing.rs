use axum::{body::Bytes, extract::State, http::HeaderMap, Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::attest_guard::VerifiedDevice;
use crate::models::subscription::{status_grants_access, Subscription, Tier};
use crate::state::AppState;

/// Human-facing catalog entry for a tier, used by `/billing/config`.
struct TierCard {
    tier: Tier,
    name: &'static str,
    tagline: &'static str,
    price_monthly_usd: f64,
    features: &'static [&'static str],
}

const TIER_CARDS: [TierCard; 3] = [
    TierCard {
        tier: Tier::Free,
        name: "Lifeline",
        tagline: "Your daily health portrait, free forever.",
        price_monthly_usd: 0.0,
        features: &[
            "Daily Vital Constellation & Lifeline Age",
            "Basic readiness from one source",
            "7 days of history",
            "Global leaderboard — view only",
        ],
    },
    TierCard {
        tier: Tier::Pro,
        name: "Lifeline Pro",
        tagline: "Every source, every insight, full competition.",
        price_monthly_usd: 7.99,
        features: &[
            "Everything in Lifeline",
            "Apple + Google + Whoop, fused",
            "Biomarker tracking vs reference ranges",
            "Unlimited history & AI coach",
            "Compete in weekly seasons",
            "Ad-free",
        ],
    },
    TierCard {
        tier: Tier::Elite,
        name: "Lifeline Elite",
        tagline: "Everything, plus the future first.",
        price_monthly_usd: 14.99,
        features: &[
            "Everything in Pro",
            "Beta access to new releases",
            "Early features before anyone else",
            "Priority support",
        ],
    },
];

/// Resolve the effective tier for a device: the stored tier if its Stripe
/// status still grants access, otherwise the free tier.
///
/// Public because every tier-gated endpoint (arena scoring, AI coach limits,
/// source fusion, history windows) resolves entitlements through this one
/// function, so billing state is enforced identically everywhere.
pub async fn effective_tier(state: &AppState, device_id: Uuid) -> Result<Tier, AppError> {
    match state.db.get_subscription(device_id).await? {
        Some(sub) if status_grants_access(&sub.status) => {
            Ok(sub.tier.parse().unwrap_or(Tier::Free))
        }
        _ => Ok(Tier::Free),
    }
}

fn subscription_json(tier: Tier, status: &str, period_end: Option<String>) -> Value {
    json!({
        "tier": tier.as_str(),
        "status": status,
        "current_period_end": period_end,
        "entitlements": tier.entitlements(),
    })
}

/// Handler for `GET /api/v1/billing/config`.
/// Public, rules-only: the tier catalog, prices, and entitlements so the client
/// can render the paywall. No user data.
#[tracing::instrument(skip(state))]
pub async fn billing_config_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/billing/config")
        .increment(1);

    let tiers: Vec<Value> = TIER_CARDS
        .iter()
        .map(|c| {
            json!({
                "tier": c.tier.as_str(),
                "name": c.name,
                "tagline": c.tagline,
                "price_monthly_usd": c.price_monthly_usd,
                "features": c.features,
                "entitlements": c.tier.entitlements(),
            })
        })
        .collect();

    Json(json!({
        "version": "1.0.0",
        "currency": "usd",
        "provider": "stripe",
        // Lets the client show a "test mode" ribbon when no live keys are set.
        "live": state.config.billing.stripe_configured(),
        "tiers": tiers,
    }))
}

/// Handler for `GET /api/v1/billing/subscription`.
/// The caller's current entitlement. Absence of a row = free tier.
#[tracing::instrument(skip(state), fields(device_id = %verified_device.device_id))]
pub async fn subscription_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/billing/subscription")
        .increment(1);

    let sub = state.db.get_subscription(verified_device.device_id).await?;
    let (tier, status, period_end) = match sub {
        Some(s) => {
            let tier = if status_grants_access(&s.status) {
                s.tier.parse().unwrap_or(Tier::Free)
            } else {
                Tier::Free
            };
            (tier, s.status, s.current_period_end.map(|d| d.to_rfc3339()))
        }
        None => (Tier::Free, "active".to_owned(), None),
    };
    Ok(Json(subscription_json(tier, &status, period_end)))
}

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    /// Target paid tier: "pro" or "elite".
    pub tier: String,
}

/// Handler for `POST /api/v1/billing/checkout`.
///
/// Creates a Stripe Checkout Session for the requested tier and returns the
/// hosted-checkout URL. When Stripe isn't configured (local/dev), it upgrades
/// the device immediately and returns a simulated URL so the flow is testable
/// end-to-end without real charges.
#[tracing::instrument(skip(state, payload), fields(device_id = %verified_device.device_id))]
pub async fn checkout_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Json(payload): Json<CheckoutRequest>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/billing/checkout")
        .increment(1);

    let device_id = verified_device.device_id;
    let tier: Tier = payload.tier.parse()?;
    if tier == Tier::Free {
        return Err(AppError::BadRequest(
            "Checkout is only for paid tiers (pro, elite).".to_owned(),
        ));
    }
    let billing = &state.config.billing;

    // ── Dev / unconfigured fallback: simulate a successful upgrade ───────────
    if !billing.stripe_configured() {
        let now = chrono::Utc::now();
        let sub = Subscription {
            device_id,
            tier: tier.as_str().to_owned(),
            status: "active".to_owned(),
            stripe_customer_id: None,
            stripe_subscription_id: None,
            current_period_end: Some(now + chrono::Duration::days(30)),
            created_at: now,
            updated_at: now,
        };
        state.db.upsert_subscription(&sub).await?;
        state
            .db
            .insert_audit_log(
                "BILLING_SIMULATED_UPGRADE",
                device_id,
                device_id,
                tier.as_str().as_bytes(),
            )
            .await?;
        return Ok(Json(json!({
            "simulated": true,
            "tier": tier.as_str(),
            "checkout_url": format!("{}?simulated=1&tier={}", billing.success_url, tier.as_str()),
            "message": "Stripe is not configured; upgraded in simulation mode.",
        })));
    }

    let price_id = billing.price_for(tier.as_str()).ok_or_else(|| {
        AppError::Internal(format!(
            "No Stripe price configured for tier '{}'.",
            tier.as_str()
        ))
    })?;

    // Reuse an existing Stripe customer id if we have one, else create one.
    let existing = state.db.get_subscription(device_id).await?;
    let customer_id = match existing.as_ref().and_then(|s| s.stripe_customer_id.clone()) {
        Some(c) => c,
        None => create_stripe_customer(state.as_ref(), device_id).await?,
    };

    // Persist the customer id up front so webhooks can reconcile even if the
    // user completes checkout on another device.
    let now = chrono::Utc::now();
    let base = existing.unwrap_or(Subscription {
        device_id,
        tier: "free".to_owned(),
        status: "incomplete".to_owned(),
        stripe_customer_id: None,
        stripe_subscription_id: None,
        current_period_end: None,
        created_at: now,
        updated_at: now,
    });
    state
        .db
        .upsert_subscription(&Subscription {
            stripe_customer_id: Some(customer_id.clone()),
            ..base
        })
        .await?;

    let checkout_url =
        create_checkout_session(state.as_ref(), device_id, &customer_id, price_id, tier).await?;
    Ok(Json(json!({
        "simulated": false,
        "tier": tier.as_str(),
        "checkout_url": checkout_url,
    })))
}

/// Handler for `POST /api/v1/billing/portal`.
/// Returns a Stripe billing-portal URL so the user can manage or cancel.
#[tracing::instrument(skip(state), fields(device_id = %verified_device.device_id))]
pub async fn portal_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/billing/portal")
        .increment(1);

    let billing = &state.config.billing;
    let sub = state.db.get_subscription(verified_device.device_id).await?;
    let customer_id = sub
        .and_then(|s| s.stripe_customer_id)
        .ok_or_else(|| AppError::BadRequest("No active subscription to manage.".to_owned()))?;

    if !billing.stripe_configured() {
        return Ok(Json(json!({
            "simulated": true,
            "portal_url": format!("{}?simulated=1", billing.portal_return_url),
        })));
    }

    let params = [
        ("customer", customer_id.as_str()),
        ("return_url", billing.portal_return_url.as_str()),
    ];
    let resp: Value = stripe_post(state.as_ref(), "/v1/billing_portal/sessions", &params).await?;
    let url = resp["url"].as_str().ok_or_else(|| {
        AppError::ExternalServiceError("Stripe portal session had no url".to_owned())
    })?;
    Ok(Json(json!({ "simulated": false, "portal_url": url })))
}

/// Handler for `GET /api/v1/billing/beta-features`.
/// Tier-gated example: only Elite subscribers may enroll in beta releases.
/// Everyone else gets a 403 pointing them at the upgrade.
#[tracing::instrument(skip(state), fields(device_id = %verified_device.device_id))]
pub async fn beta_features_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/billing/beta-features")
        .increment(1);

    let tier = effective_tier(state.as_ref(), verified_device.device_id).await?;
    if tier < Tier::Elite {
        return Err(AppError::Forbidden(
            "Beta access is an Elite feature. Upgrade to enroll.".to_owned(),
        ));
    }

    Ok(Json(json!({
        "channel": "beta",
        "builds": [
            { "version": "2.1.0-beta.3", "notes": "Sleep-stage constellation overlay" },
            { "version": "2.1.0-beta.2", "notes": "Live season leaderboard deltas" }
        ]
    })))
}

/// Handler for `POST /api/v1/billing/webhook`.
///
/// Stripe calls this after payment events. It is public (no session) but is
/// authenticated by verifying the `Stripe-Signature` HMAC against the raw body.
#[tracing::instrument(skip(state, headers, body))]
pub async fn webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/billing/webhook")
        .increment(1);

    let secret = &state.config.billing.stripe_webhook_secret;
    if secret.is_empty() {
        // Without a webhook secret we cannot authenticate Stripe — refuse
        // rather than trust an unsigned caller.
        return Err(AppError::BadRequest(
            "Webhooks are not enabled (no signing secret configured).".to_owned(),
        ));
    }
    let sig = headers
        .get("Stripe-Signature")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Stripe-Signature".to_owned()))?;

    verify_stripe_signature(secret, &body, sig)?;

    let event: Value = serde_json::from_slice(&body)?;
    let event_type = event["type"].as_str().unwrap_or_default();
    let object = &event["data"]["object"];

    match event_type {
        "checkout.session.completed" => {
            let device_id = object["client_reference_id"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok());
            let customer = object["customer"].as_str().map(str::to_owned);
            let subscription_id = object["subscription"].as_str().map(str::to_owned);
            let tier = object["metadata"]["tier"]
                .as_str()
                .unwrap_or("pro")
                .to_owned();
            if let Some(device_id) = device_id {
                let now = chrono::Utc::now();
                state
                    .db
                    .upsert_subscription(&Subscription {
                        device_id,
                        tier,
                        status: "active".to_owned(),
                        stripe_customer_id: customer,
                        stripe_subscription_id: subscription_id,
                        current_period_end: None,
                        created_at: now,
                        updated_at: now,
                    })
                    .await?;
            }
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            if let Some(customer) = object["customer"].as_str() {
                if let Some(mut sub) = state.db.get_subscription_by_customer(customer).await? {
                    let status = object["status"].as_str().unwrap_or("canceled").to_owned();
                    // Map the price back to a tier when present.
                    let price_id = object["items"]["data"][0]["price"]["id"].as_str();
                    if let Some(pid) = price_id {
                        if let Some(t) = tier_for_price(&state.config.billing, pid) {
                            sub.tier = t.as_str().to_owned();
                        }
                    }
                    if event_type == "customer.subscription.deleted"
                        || !status_grants_access(&status)
                    {
                        sub.tier = "free".to_owned();
                    }
                    sub.status = status;
                    sub.current_period_end = object["current_period_end"]
                        .as_i64()
                        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));
                    state.db.upsert_subscription(&sub).await?;
                }
            }
        }
        _ => {
            tracing::debug!(event_type, "Ignoring unhandled Stripe event");
        }
    }

    Ok(Json(json!({ "received": true })))
}

// ── Stripe REST helpers ──────────────────────────────────────────────────────

async fn create_stripe_customer(state: &AppState, device_id: Uuid) -> Result<String, AppError> {
    let device_str = device_id.to_string();
    let params = [
        ("metadata[device_id]", device_str.as_str()),
        // Zero-knowledge: we intentionally send no name/email — the device id
        // is the only linkage, and it isn't personally identifying.
    ];
    let resp = stripe_post(state, "/v1/customers", &params).await?;
    resp["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::ExternalServiceError("Stripe customer had no id".to_owned()))
}

async fn create_checkout_session(
    state: &AppState,
    device_id: Uuid,
    customer_id: &str,
    price_id: &str,
    tier: Tier,
) -> Result<String, AppError> {
    let billing = &state.config.billing;
    let device_str = device_id.to_string();
    let params = [
        ("mode", "subscription"),
        ("customer", customer_id),
        ("client_reference_id", device_str.as_str()),
        ("line_items[0][price]", price_id),
        ("line_items[0][quantity]", "1"),
        ("success_url", billing.success_url.as_str()),
        ("cancel_url", billing.cancel_url.as_str()),
        ("metadata[tier]", tier.as_str()),
        ("metadata[device_id]", device_str.as_str()),
        (
            "subscription_data[metadata][device_id]",
            device_str.as_str(),
        ),
        ("subscription_data[metadata][tier]", tier.as_str()),
    ];
    let resp = stripe_post(state, "/v1/checkout/sessions", &params).await?;
    resp["url"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::ExternalServiceError("Stripe checkout had no url".to_owned()))
}

/// POST form-encoded params to the Stripe API with the secret key as bearer.
async fn stripe_post(
    state: &AppState,
    path: &str,
    params: &[(&str, &str)],
) -> Result<Value, AppError> {
    let billing = &state.config.billing;
    let url = format!("{}{}", billing.api_base_url(), path);
    let resp = state
        .http_client
        .post(&url)
        .bearer_auth(&billing.stripe_secret_key)
        .form(params)
        .send()
        .await?;

    let status = resp.status();
    let value: Value = resp.json().await?;
    if !status.is_success() {
        let msg = value["error"]["message"]
            .as_str()
            .unwrap_or("Stripe request failed")
            .to_owned();
        return Err(AppError::ExternalServiceError(format!("Stripe: {msg}")));
    }
    Ok(value)
}

/// Reverse-map a Stripe price id back to the tier it represents.
fn tier_for_price(billing: &crate::config::BillingConfig, price_id: &str) -> Option<Tier> {
    if billing.price_pro == price_id {
        Some(Tier::Pro)
    } else if billing.price_elite == price_id {
        Some(Tier::Elite)
    } else {
        None
    }
}

/// Verify a `Stripe-Signature` header against the raw request body.
///
/// The header looks like `t=1699999999,v1=<hex>`. Stripe signs
/// `"{t}.{payload}"` with HMAC-SHA256 keyed by the webhook secret. We recompute
/// and compare in constant time.
fn verify_stripe_signature(secret: &str, payload: &[u8], sig_header: &str) -> Result<(), AppError> {
    let mut timestamp: Option<&str> = None;
    let mut signatures: Vec<&str> = Vec::new();
    for part in sig_header.split(',') {
        if let Some((k, v)) = part.split_once('=') {
            match k {
                "t" => timestamp = Some(v),
                "v1" => signatures.push(v),
                _ => {}
            }
        }
    }
    let t =
        timestamp.ok_or_else(|| AppError::Unauthorized("Malformed Stripe-Signature".to_owned()))?;
    if signatures.is_empty() {
        return Err(AppError::Unauthorized(
            "No v1 signature in Stripe-Signature".to_owned(),
        ));
    }

    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
    let mut signed = Vec::with_capacity(t.len() + 1 + payload.len());
    signed.extend_from_slice(t.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(payload);

    // Any provided v1 whose bytes match the HMAC authenticates the event.
    // `hmac::verify` performs the comparison in constant time.
    let ok = signatures.iter().any(|candidate| {
        hex::decode(candidate)
            .ok()
            .is_some_and(|bytes| ring::hmac::verify(&key, &signed, &bytes).is_ok())
    });
    if ok {
        Ok(())
    } else {
        Err(AppError::Unauthorized(
            "Stripe signature verification failed".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_roundtrip_verifies() {
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let t = "1700000000";
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
        let mut signed = Vec::new();
        signed.extend_from_slice(t.as_bytes());
        signed.push(b'.');
        signed.extend_from_slice(payload);
        let sig = hex::encode(ring::hmac::sign(&key, &signed).as_ref());
        let header = format!("t={t},v1={sig}");
        assert!(verify_stripe_signature(secret, payload, &header).is_ok());
    }

    #[test]
    fn tampered_signature_rejected() {
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"x"}"#;
        let header = "t=1700000000,v1=deadbeef";
        assert!(verify_stripe_signature(secret, payload, header).is_err());
    }

    #[test]
    fn wrong_secret_rejected() {
        let payload = br#"{"a":1}"#;
        let t = "1700000000";
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, b"real_secret");
        let mut signed = Vec::new();
        signed.extend_from_slice(t.as_bytes());
        signed.push(b'.');
        signed.extend_from_slice(payload);
        let sig = hex::encode(ring::hmac::sign(&key, &signed).as_ref());
        let header = format!("t={t},v1={sig}");
        assert!(verify_stripe_signature("attacker_secret", payload, &header).is_err());
    }
}
