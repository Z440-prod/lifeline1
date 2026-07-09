use crate::errors::AppError;
use chrono::{Duration, Utc};
use ring::hmac;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionTokenPayload {
    pub device_id: Uuid,
    pub expires_at: i64, // Unix timestamp in seconds
}

/// Create a signed session token using HMAC-SHA256.
/// The token is formatted as: `base64url(payload_json).base64url(hmac_signature)`
pub fn create_session_token(
    key: &hmac::Key,
    device_id: Uuid,
    ttl_seconds: u64,
) -> Result<String, AppError> {
    let expires_at = Utc::now()
        .checked_add_signed(Duration::seconds(ttl_seconds as i64))
        .ok_or_else(|| AppError::Internal("Timestamp overflow".to_owned()))?
        .timestamp();

    let payload = SessionTokenPayload {
        device_id,
        expires_at,
    };

    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|e| AppError::Internal(format!("Failed to serialize token payload: {e}")))?;

    // Sign the payload
    let signature = hmac::sign(key, &payload_bytes);

    // Base64URL encode both parts
    let engine = base64::prelude::BASE64_URL_SAFE_NO_PAD;
    use base64::Engine;
    let encoded_payload = engine.encode(&payload_bytes);
    let encoded_signature = engine.encode(signature.as_ref());

    Ok(format!("{encoded_payload}.{encoded_signature}"))
}

/// Verify an HMAC-signed session token and return the authenticated `device_id`.
/// Enforces expiration checks.
pub fn verify_session_token(key: &hmac::Key, token_str: &str) -> Result<Uuid, AppError> {
    let parts: Vec<&str> = token_str.split('.').collect();
    if parts.len() != 2 {
        return Err(AppError::Unauthorized("Malformed session token".to_owned()));
    }

    let encoded_payload = parts[0];
    let encoded_signature = parts[1];

    let engine = base64::prelude::BASE64_URL_SAFE_NO_PAD;
    use base64::Engine;
    let payload_bytes = engine
        .decode(encoded_payload)
        .map_err(|_| AppError::Unauthorized("Invalid token encoding".to_owned()))?;

    let signature_bytes = engine
        .decode(encoded_signature)
        .map_err(|_| AppError::Unauthorized("Invalid signature encoding".to_owned()))?;

    // Verify HMAC signature
    hmac::verify(key, &payload_bytes, &signature_bytes)
        .map_err(|_| AppError::Unauthorized("Invalid token signature".to_owned()))?;

    // Deserialize payload
    let payload: SessionTokenPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::Unauthorized("Failed to parse token payload".to_owned()))?;

    // Check expiration
    let now = Utc::now().timestamp();
    if payload.expires_at < now {
        return Err(AppError::Unauthorized(
            "Session token has expired".to_owned(),
        ));
    }

    Ok(payload.device_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_token_lifecycle() {
        let secret = "test_secret_at_least_32_bytes_long_key_signature";
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let device_id = Uuid::new_v4();

        // Create token with 10s TTL
        let token = create_session_token(&key, device_id, 10).unwrap();
        assert!(!token.is_empty());

        // Verify token
        let verified_device = verify_session_token(&key, &token).unwrap();
        assert_eq!(verified_device, device_id);
    }

    #[test]
    fn test_expired_session_token() {
        let secret = "test_secret_at_least_32_bytes_long_key_signature";
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let device_id = Uuid::new_v4();

        // Create token with 0s TTL
        let token = create_session_token(&key, device_id, 0).unwrap();

        // Wait a second to ensure expiry is hit
        std::thread::sleep(std::time::Duration::from_secs(1));

        let res = verify_session_token(&key, &token);
        assert!(res.is_err());
    }

    #[test]
    fn test_tampered_session_token() {
        let secret = "test_secret_at_least_32_bytes_long_key_signature";
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let device_id = Uuid::new_v4();

        let token = create_session_token(&key, device_id, 10).unwrap();

        // Split token and alter the payload part slightly
        let parts: Vec<&str> = token.split('.').collect();
        let altered_payload = format!("{}a", parts[0]);
        let tampered_token = format!("{}.{}", altered_payload, parts[1]);

        let res = verify_session_token(&key, &tampered_token);
        assert!(res.is_err());
    }
}
