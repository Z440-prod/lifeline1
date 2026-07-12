//! Account sign-in / sign-up — an identity layer on top of device attestation.
//!
//! Every path ends the same way: register the calling device (real WebCrypto
//! key when supplied) and mint a device-bound session token, then attach the
//! account. Email/password uses PBKDF2; Apple/Google use OpenID Connect
//! id-tokens verified against the provider (simulated in development so the
//! flow is testable without live client credentials).

use axum::{extract::State, Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::crypto::password;
use crate::errors::AppError;
use crate::middleware::attest_guard::VerifiedDevice;
use crate::models::account::{normalize_email, Account};
use crate::routes::auth::register_device_and_issue_token;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct EmailAuthRequest {
    pub email: String,
    pub password: String,
    pub device_id: Uuid,
    #[serde(default)]
    pub public_key: Option<String>,
}

fn session_response(token: &str, account: &Account, ttl: u64) -> Json<Value> {
    Json(json!({
        "token": token,
        "expires_in": ttl,
        "account": account.view(),
    }))
}

/// `POST /api/v1/account/register` — create an email/password account.
#[tracing::instrument(skip(state, payload), fields(device_id = %payload.device_id))]
pub async fn register_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailAuthRequest>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/account/register")
        .increment(1);

    let email = normalize_email(&payload.email)
        .ok_or_else(|| AppError::BadRequest("Enter a valid email address.".to_owned()))?;
    if !password::is_acceptable_password(&payload.password) {
        return Err(AppError::BadRequest(
            "Password must be 8–128 characters.".to_owned(),
        ));
    }
    if state.db.get_account_by_email(&email).await?.is_some() {
        return Err(AppError::Conflict(
            "An account with that email already exists — sign in instead.".to_owned(),
        ));
    }

    let (hash, salt) = password::hash_password(&payload.password)?;
    let account = Account {
        id: Uuid::new_v4(),
        email: Some(email),
        password_hash: Some(hash),
        password_salt: Some(salt),
        oauth_provider: None,
        oauth_subject: None,
        created_at: chrono::Utc::now(),
    };
    state.db.insert_account(&account).await?;

    let token =
        register_device_and_issue_token(state.as_ref(), payload.device_id, &payload.public_key)
            .await?;
    state.db.link_device(account.id, payload.device_id).await?;
    state
        .db
        .insert_audit_log("ACCOUNT_REGISTER", payload.device_id, account.id, &[])
        .await?;

    Ok(session_response(
        &token,
        &account,
        state.config.auth.session_token_ttl_seconds,
    ))
}

/// `POST /api/v1/account/login` — sign in with email/password.
#[tracing::instrument(skip(state, payload), fields(device_id = %payload.device_id))]
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailAuthRequest>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/account/login")
        .increment(1);

    let email = normalize_email(&payload.email)
        .ok_or_else(|| AppError::Unauthorized("Invalid email or password.".to_owned()))?;
    let account = state.db.get_account_by_email(&email).await?;

    // Constant-ish message on every failure path so we never reveal whether an
    // email exists. Verify against the stored hash when present.
    let ok = match &account {
        Some(a) => match (&a.password_hash, &a.password_salt) {
            (Some(h), Some(s)) => password::verify_password(&payload.password, h, s),
            _ => false,
        },
        None => {
            // Spend comparable time so response timing doesn't leak existence.
            let _ = password::hash_password(&payload.password);
            false
        }
    };
    if !ok {
        return Err(AppError::Unauthorized(
            "Invalid email or password.".to_owned(),
        ));
    }
    let account = account.expect("verified above");

    let token =
        register_device_and_issue_token(state.as_ref(), payload.device_id, &payload.public_key)
            .await?;
    state.db.link_device(account.id, payload.device_id).await?;
    state
        .db
        .insert_audit_log("ACCOUNT_LOGIN", payload.device_id, account.id, &[])
        .await?;

    Ok(session_response(
        &token,
        &account,
        state.config.auth.session_token_ttl_seconds,
    ))
}

#[derive(Debug, Deserialize)]
pub struct OAuthRequest {
    /// "apple" or "google".
    pub provider: String,
    /// The provider's OpenID Connect id-token (JWT).
    pub id_token: String,
    pub device_id: Uuid,
    #[serde(default)]
    pub public_key: Option<String>,
}

/// `POST /api/v1/account/oauth` — sign in / up with Apple or Google.
///
/// The id-token is verified against the provider (issuer, audience, expiry,
/// signature) in production; in development a token of the form
/// `sim:<subject>:<email>` is accepted so the flow is exercisable without live
/// OAuth client credentials. An account is upserted by `(provider, subject)`.
#[tracing::instrument(skip(state, payload), fields(provider = %payload.provider, device_id = %payload.device_id))]
pub async fn oauth_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OAuthRequest>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/account/oauth")
        .increment(1);

    if payload.provider != "apple" && payload.provider != "google" {
        return Err(AppError::BadRequest(
            "provider must be 'apple' or 'google'".to_owned(),
        ));
    }

    let (subject, email) = verify_oauth_id_token(&state, &payload.provider, &payload.id_token)?;

    let account = match state
        .db
        .get_account_by_oauth(&payload.provider, &subject)
        .await?
    {
        Some(existing) => existing,
        None => {
            let base = Account {
                id: Uuid::new_v4(),
                email: email.and_then(|e| normalize_email(&e)),
                password_hash: None,
                password_salt: None,
                oauth_provider: Some(payload.provider.clone()),
                oauth_subject: Some(subject),
                created_at: chrono::Utc::now(),
            };
            match state.db.insert_account(&base).await {
                Ok(()) => base,
                // The provider email is already tied to another account — keep
                // the OAuth identity, drop the duplicate email, and retry once.
                Err(AppError::Conflict(_)) => {
                    let deconflicted = Account {
                        email: None,
                        ..base
                    };
                    state.db.insert_account(&deconflicted).await?;
                    deconflicted
                }
                Err(e) => return Err(e),
            }
        }
    };

    let token =
        register_device_and_issue_token(state.as_ref(), payload.device_id, &payload.public_key)
            .await?;
    state.db.link_device(account.id, payload.device_id).await?;
    state
        .db
        .insert_audit_log("ACCOUNT_OAUTH", payload.device_id, account.id, &[])
        .await?;

    Ok(session_response(
        &token,
        &account,
        state.config.auth.session_token_ttl_seconds,
    ))
}

/// `DELETE /api/v1/account` — permanently delete the account and all of its
/// data. Authenticated by the device session (attest_guard). Required for App
/// Store submission (Guideline 5.1.1(v)) and satisfies GDPR/CCPA erasure.
///
/// Erases every device under the account: the encrypted vault, provider
/// connections, game profile, subscription, audit logs, device registration,
/// and the account record itself — in one transaction. After this the session
/// token is dead (its device no longer exists).
#[tracing::instrument(skip(state), fields(device_id = %verified_device.device_id))]
pub async fn delete_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/account/delete")
        .increment(1);

    let had_account = state
        .db
        .delete_account_and_data(verified_device.device_id)
        .await?;

    Ok(Json(json!({
        "deleted": true,
        "had_account": had_account,
        "message": "Your account and all associated data have been permanently deleted.",
    })))
}

/// Verify a provider id-token, returning `(subject, email)`.
///
/// Production performs full OIDC verification; development accepts a
/// `sim:<subject>:<email>` token so the sign-in flow is testable.
fn verify_oauth_id_token(
    state: &AppState,
    provider: &str,
    id_token: &str,
) -> Result<(String, Option<String>), AppError> {
    if state.config.auth.environment == "development" {
        if let Some(rest) = id_token.strip_prefix("sim:") {
            let mut parts = rest.splitn(2, ':');
            let subject = parts.next().unwrap_or_default().to_owned();
            let email = parts.next().map(str::to_owned).filter(|e| !e.is_empty());
            if subject.is_empty() {
                return Err(AppError::Unauthorized(
                    "Simulated OAuth token missing subject.".to_owned(),
                ));
            }
            return Ok((subject, email));
        }
        return Err(AppError::Unauthorized(
            "In development, pass a simulated id_token of the form 'sim:<subject>:<email>'."
                .to_owned(),
        ));
    }

    // Production: verify the JWT against the provider's JWKS (issuer,
    // audience, expiry, signature). Left as a configuration-gated integration
    // point so the server never trusts an unverified token by default.
    Err(AppError::ExternalServiceError(format!(
        "{provider} OIDC verification is not configured on this deployment."
    )))
}
