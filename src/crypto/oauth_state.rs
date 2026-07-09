use crate::errors::AppError;
use chrono::{Duration, Utc};
use ring::hmac;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Derive the HMAC key used to sign OAuth `state` parameters.
///
/// Domain-separated from the session-token and audit-log keys (see
/// `crypto::session` and `db::audit`), all derived from the same server
/// secret.
#[must_use]
pub fn derive_oauth_state_key(server_secret: &str) -> hmac::Key {
    hmac::Key::new(
        hmac::HMAC_SHA256,
        format!("antigravity-oauth-state-v1:{server_secret}").as_bytes(),
    )
}

#[derive(Debug, Serialize, Deserialize)]
struct OAuthStatePayload {
    device_id: Uuid,
    provider: String,
    expires_at: i64,
}

/// Create a signed, short-lived `state` parameter binding an OAuth authorize
/// request to the device that initiated it. The external provider echoes
/// this value back verbatim on the callback redirect, which has no other
/// authentication (the user's browser, not our API client, makes that
/// request), so the signature is what prevents a forged callback from
/// attaching someone else's provider account to an attacker's device.
pub fn create_state_token(
    key: &hmac::Key,
    device_id: Uuid,
    provider: &str,
    ttl_seconds: u64,
) -> Result<String, AppError> {
    let expires_at = Utc::now()
        .checked_add_signed(Duration::seconds(
            i64::try_from(ttl_seconds).unwrap_or(i64::MAX),
        ))
        .ok_or_else(|| AppError::Internal("Timestamp overflow".to_owned()))?
        .timestamp();

    let payload = OAuthStatePayload {
        device_id,
        provider: provider.to_owned(),
        expires_at,
    };

    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|e| AppError::Internal(format!("Failed to serialize OAuth state: {e}")))?;

    let signature = hmac::sign(key, &payload_bytes);

    let engine = base64::prelude::BASE64_URL_SAFE_NO_PAD;
    use base64::Engine;
    let encoded_payload = engine.encode(&payload_bytes);
    let encoded_signature = engine.encode(signature.as_ref());

    Ok(format!("{encoded_payload}.{encoded_signature}"))
}

/// Verify a `state` token and return the `device_id` that initiated the
/// OAuth flow, provided it matches the expected provider and has not expired.
pub fn verify_state_token(
    key: &hmac::Key,
    state_str: &str,
    expected_provider: &str,
) -> Result<Uuid, AppError> {
    let parts: Vec<&str> = state_str.split('.').collect();
    if parts.len() != 2 {
        return Err(AppError::Unauthorized("Malformed OAuth state".to_owned()));
    }

    let engine = base64::prelude::BASE64_URL_SAFE_NO_PAD;
    use base64::Engine;
    let payload_bytes = engine
        .decode(parts[0])
        .map_err(|_| AppError::Unauthorized("Invalid OAuth state encoding".to_owned()))?;
    let signature_bytes = engine
        .decode(parts[1])
        .map_err(|_| AppError::Unauthorized("Invalid OAuth state signature encoding".to_owned()))?;

    hmac::verify(key, &payload_bytes, &signature_bytes)
        .map_err(|_| AppError::Unauthorized("Invalid OAuth state signature".to_owned()))?;

    let payload: OAuthStatePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::Unauthorized("Failed to parse OAuth state payload".to_owned()))?;

    if payload.provider != expected_provider {
        return Err(AppError::Unauthorized(
            "OAuth state was issued for a different provider".to_owned(),
        ));
    }

    if payload.expires_at < Utc::now().timestamp() {
        return Err(AppError::Unauthorized("OAuth state has expired".to_owned()));
    }

    Ok(payload.device_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_token_roundtrip() {
        let key = derive_oauth_state_key("test_secret_at_least_32_bytes_long_key_signature");
        let device_id = Uuid::new_v4();
        let token = create_state_token(&key, device_id, "whoop", 600).unwrap();
        let verified = verify_state_token(&key, &token, "whoop").unwrap();
        assert_eq!(verified, device_id);
    }

    #[test]
    fn test_state_token_wrong_provider_rejected() {
        let key = derive_oauth_state_key("test_secret_at_least_32_bytes_long_key_signature");
        let token = create_state_token(&key, Uuid::new_v4(), "whoop", 600).unwrap();
        assert!(verify_state_token(&key, &token, "google_health").is_err());
    }

    #[test]
    fn test_state_token_expired_rejected() {
        let key = derive_oauth_state_key("test_secret_at_least_32_bytes_long_key_signature");
        let token = create_state_token(&key, Uuid::new_v4(), "whoop", 0).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert!(verify_state_token(&key, &token, "whoop").is_err());
    }

    #[test]
    fn test_state_token_tampered_rejected() {
        let key = derive_oauth_state_key("test_secret_at_least_32_bytes_long_key_signature");
        let token = create_state_token(&key, Uuid::new_v4(), "whoop", 600).unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        let tampered = format!("{}a.{}", parts[0], parts[1]);
        assert!(verify_state_token(&key, &tampered, "whoop").is_err());
    }
}
