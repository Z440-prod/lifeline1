use crate::errors::AppError;
use crate::state::AppState;
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::sync::Arc;
use uuid::Uuid;

/// Struct containing verified authentication details injected into request extensions.
#[derive(Debug, Clone)]
pub struct VerifiedDevice {
    pub device_id: Uuid,
}

/// Tower/Axum middleware that validates session tokens and ensures the requesting device is registered.
/// Validates either the `Authorization: Bearer <token>` or `X-Assertion-Token: <token>` header.
/// Optionally verifies that the client-supplied `X-Device-Id` matches the session token.
pub async fn attest_guard(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // 1. Extract session token
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("X-Assertion-Token")
                .and_then(|h| h.to_str().ok())
        })
        .ok_or_else(|| AppError::Unauthorized("Missing session token".to_owned()))?;

    // 2. Cryptographically verify the session token
    let device_id = crate::crypto::session::verify_session_token(&state.hmac_key, token)?;

    // 3. Optional consistency check with X-Device-Id header
    if let Some(x_device_id_str) = req
        .headers()
        .get("X-Device-Id")
        .and_then(|h| h.to_str().ok())
    {
        let x_device_id = Uuid::parse_str(x_device_id_str)
            .map_err(|_| AppError::BadRequest("Malformed X-Device-Id header".to_owned()))?;
        if x_device_id != device_id {
            return Err(AppError::Unauthorized("Device ID mismatch".to_owned()));
        }
    }

    // 4. Inject VerifiedDevice into the request extensions for route handlers
    req.extensions_mut().insert(VerifiedDevice { device_id });

    // 5. Execute the next handler
    Ok(next.run(req).await)
}
