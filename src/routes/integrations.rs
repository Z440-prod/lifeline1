use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::crypto::{oauth_state, token_vault};
use crate::errors::AppError;
use crate::middleware::attest_guard::VerifiedDevice;
use crate::models::provider_connection::Provider;
use crate::state::AppState;

const WHOOP_STATE_TTL_SECONDS: u64 = 600;

/// Handler for `GET /api/v1/integrations`.
/// Lists every provider connection the authenticated device has.
#[tracing::instrument(skip(state))]
pub async fn list_integrations_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/integrations").increment(1);
    let connections = state
        .db
        .list_provider_connections(verified_device.device_id)
        .await?;
    Ok(Json(json!({ "connections": connections })))
}

#[derive(Debug, Deserialize)]
pub struct ConnectOnDeviceRequest {
    /// Whether the user granted `HealthKit` / Health Connect authorization
    /// on-device. The server never sees the underlying health data — only
    /// that permission was granted, so it can tailor recommendations.
    pub authorized: bool,
    #[serde(default)]
    pub external_account_id: Option<String>,
}

/// Handler for `POST /api/v1/integrations/{provider}/connect`.
/// Records that a device authorized an **on-device** health SDK
/// (Apple `HealthKit` or Google Health Connect). Rejects `whoop`, which is a
/// cloud API and must go through the OAuth authorize/callback flow instead.
#[tracing::instrument(skip(state, payload), fields(provider = %provider_str))]
pub async fn connect_on_device_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Path(provider_str): Path<String>,
    Json(payload): Json<ConnectOnDeviceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/integrations/connect")
        .increment(1);

    let provider: Provider = provider_str.parse()?;
    if provider.is_cloud_oauth() {
        return Err(AppError::BadRequest(format!(
            "{provider_str} requires the OAuth authorize flow — use GET /integrations/{provider_str}/authorize"
        )));
    }
    if !payload.authorized {
        return Err(AppError::BadRequest(
            "authorized must be true to record a connection".to_owned(),
        ));
    }

    state
        .db
        .upsert_provider_connection(
            verified_device.device_id,
            provider.as_str(),
            "connected",
            payload.external_account_id.as_deref(),
            None,
        )
        .await?;

    state
        .db
        .insert_audit_log(
            "CONNECT_PROVIDER",
            verified_device.device_id,
            verified_device.device_id,
            provider.as_str().as_bytes(),
        )
        .await?;

    Ok(Json(
        json!({ "provider": provider.as_str(), "status": "connected" }),
    ))
}

/// Handler for `DELETE /api/v1/integrations/{provider}`.
#[tracing::instrument(skip(state), fields(provider = %provider_str))]
pub async fn disconnect_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Path(provider_str): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/integrations/disconnect")
        .increment(1);

    let provider: Provider = provider_str.parse()?;
    state
        .db
        .delete_provider_connection(verified_device.device_id, provider.as_str())
        .await?;

    state
        .db
        .insert_audit_log(
            "DISCONNECT_PROVIDER",
            verified_device.device_id,
            verified_device.device_id,
            provider.as_str().as_bytes(),
        )
        .await?;

    Ok(Json(
        json!({ "provider": provider.as_str(), "status": "disconnected" }),
    ))
}

/// Handler for `GET /api/v1/integrations/whoop/authorize`.
/// Returns the Whoop `OAuth2` authorize URL with a signed `state` parameter
/// binding the flow to the requesting device. Falls back to a mock URL that
/// completes against our own callback when Whoop isn't configured and the
/// environment is `development` — mirroring the AI proxy's dev-mode pattern.
#[tracing::instrument(skip(state))]
pub async fn whoop_authorize_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/integrations/whoop/authorize").increment(1);

    let state_token = oauth_state::create_state_token(
        &state.oauth_state_key,
        verified_device.device_id,
        "whoop",
        WHOOP_STATE_TTL_SECONDS,
    )?;

    if state.config.integrations.whoop_configured() {
        let cfg = &state.config.integrations;
        let authorize_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope=read:recovery+read:cycles+read:sleep+read:profile&state={}",
            cfg.whoop_authorize_url, cfg.whoop_client_id, cfg.whoop_redirect_uri, state_token
        );
        return Ok(Json(
            json!({ "authorize_url": authorize_url, "mock": false }),
        ));
    }

    if state.config.auth.environment != "development" {
        return Err(AppError::ExternalServiceError(
            "Whoop integration is not configured".to_owned(),
        ));
    }

    tracing::info!("Whoop client not configured; returning mock authorize URL in development");
    let mock_url = format!(
        "{}?code=mock_dev_code&state={}",
        state.config.integrations.whoop_redirect_uri, state_token
    );
    Ok(Json(json!({ "authorize_url": mock_url, "mock": true })))
}

#[derive(Debug, Deserialize)]
pub struct WhoopCallbackQuery {
    pub code: String,
    pub state: String,
}

/// Handler for `GET /api/v1/integrations/whoop/callback`.
///
/// **Public route** — Whoop redirects the user's own browser here after
/// consent, so this request carries no `Authorization` header. The signed
/// `state` parameter (verified below) is what proves the callback belongs to
/// a device that legitimately started the flow, not an attacker replaying an
/// arbitrary authorization code against someone else's session.
#[tracing::instrument(skip(state, query))]
pub async fn whoop_callback_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WhoopCallbackQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/integrations/whoop/callback").increment(1);

    let device_id = oauth_state::verify_state_token(&state.oauth_state_key, &query.state, "whoop")?;

    let (external_account_id, refresh_token) = if state.config.integrations.whoop_configured() {
        let cfg = &state.config.integrations;
        let resp = state
            .http_client
            .post(&cfg.whoop_token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", query.code.as_str()),
                ("client_id", cfg.whoop_client_id.as_str()),
                ("client_secret", cfg.whoop_client_secret.as_str()),
                ("redirect_uri", cfg.whoop_redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalServiceError(format!("Whoop token exchange failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!(
                "Whoop token exchange returned {status}: {body}"
            )));
        }

        let token_json: serde_json::Value = resp.json().await.map_err(|e| {
            AppError::ExternalServiceError(format!("Failed to parse Whoop token response: {e}"))
        })?;
        let refresh_token = token_json["refresh_token"]
            .as_str()
            .ok_or_else(|| {
                AppError::ExternalServiceError(
                    "Whoop token response missing refresh_token".to_owned(),
                )
            })?
            .to_owned();
        (None, refresh_token)
    } else if state.config.auth.environment == "development" {
        tracing::info!("Whoop client not configured; using mock token exchange in development");
        (
            Some("mock_whoop_account".to_owned()),
            format!("mock_refresh_token_{}", Uuid::new_v4()),
        )
    } else {
        return Err(AppError::ExternalServiceError(
            "Whoop integration is not configured".to_owned(),
        ));
    };

    let encrypted = token_vault::encrypt_token(&state.token_vault_key, &refresh_token)?;
    state
        .db
        .upsert_provider_connection(
            device_id,
            Provider::Whoop.as_str(),
            "connected",
            external_account_id.as_deref(),
            Some(&encrypted),
        )
        .await?;

    state
        .db
        .insert_audit_log("CONNECT_PROVIDER", device_id, device_id, b"whoop")
        .await?;

    Ok(Json(json!({ "provider": "whoop", "status": "connected" })))
}

/// Handler for `GET /api/v1/integrations/whoop/metrics`.
/// Fetches the latest recovery/strain/sleep summary from Whoop using the
/// stored (encrypted-at-rest) refresh token. Returns a mock summary when
/// Whoop isn't configured, matching the connect/authorize dev-mode fallback.
#[tracing::instrument(skip(state))]
pub async fn whoop_metrics_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/integrations/whoop/metrics").increment(1);

    let encrypted = state
        .db
        .get_encrypted_refresh_token(verified_device.device_id, Provider::Whoop.as_str())
        .await?
        .ok_or_else(|| AppError::BadRequest("Whoop is not connected".to_owned()))?;

    if !state.config.integrations.whoop_configured() {
        state
            .db
            .touch_last_synced(verified_device.device_id, Provider::Whoop.as_str())
            .await?;
        return Ok(Json(json!({
            "provider": "whoop",
            "recovery_score": 78,
            "hrv_ms": 62,
            "resting_heart_rate": 48,
            "sleep_performance_pct": 91,
            "strain": 11.4,
            "mock": true
        })));
    }

    let refresh_token = token_vault::decrypt_token(&state.token_vault_key, &encrypted)?;
    let cfg = &state.config.integrations;

    let token_resp = state
        .http_client
        .post(&cfg.whoop_token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", cfg.whoop_client_id.as_str()),
            ("client_secret", cfg.whoop_client_secret.as_str()),
        ])
        .send()
        .await
        .map_err(|e| AppError::ExternalServiceError(format!("Whoop token refresh failed: {e}")))?;

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        return Err(AppError::ExternalServiceError(format!(
            "Whoop token refresh returned {status}"
        )));
    }

    let token_json: serde_json::Value = token_resp.json().await.map_err(|e| {
        AppError::ExternalServiceError(format!("Failed to parse Whoop refresh response: {e}"))
    })?;
    let access_token = token_json["access_token"].as_str().ok_or_else(|| {
        AppError::ExternalServiceError("Whoop refresh response missing access_token".to_owned())
    })?;

    // Whoop rotates refresh tokens on every use — persist the new one.
    if let Some(new_refresh) = token_json["refresh_token"].as_str() {
        let encrypted_new = token_vault::encrypt_token(&state.token_vault_key, new_refresh)?;
        state
            .db
            .upsert_provider_connection(
                verified_device.device_id,
                Provider::Whoop.as_str(),
                "connected",
                None,
                Some(&encrypted_new),
            )
            .await?;
    }

    let recovery_resp = state
        .http_client
        .get(format!("{}/v1/recovery", cfg.whoop_api_base))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::ExternalServiceError(format!("Whoop recovery fetch failed: {e}")))?;

    let recovery_json: serde_json::Value = recovery_resp.json().await.map_err(|e| {
        AppError::ExternalServiceError(format!("Failed to parse Whoop recovery response: {e}"))
    })?;

    state
        .db
        .touch_last_synced(verified_device.device_id, Provider::Whoop.as_str())
        .await?;

    Ok(Json(recovery_json))
}
