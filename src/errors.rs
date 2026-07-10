use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Unified application error type.
/// Every variant maps to a specific HTTP status code and machine-readable error code.
///
/// Uses `thiserror` derive macros for cleaner, maintainable `Display` and `Error` impls.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 401 — Missing or invalid authentication credentials.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    /// 403 — Authenticated, but the subscription tier doesn't permit this.
    #[error("Forbidden: {0}")]
    Forbidden(String),
    /// 400 — The attestation object failed cryptographic verification.
    #[error("InvalidAttestation: {0}")]
    InvalidAttestation(String),
    /// 400 — The assertion signature or counter check failed.
    #[error("InvalidAssertion: {0}")]
    InvalidAssertion(String),
    /// 410 — The challenge nonce has expired beyond its TTL window.
    #[allow(dead_code)]
    #[error("Challenge nonce has expired")]
    NonceExpired,
    /// 404 — No nonce found for the supplied challenge value.
    #[error("Challenge nonce not found")]
    NonceNotFound,
    /// 409 — Assertion counter is not strictly greater than the stored value.
    #[error("Replay attack detected: counter not advancing")]
    ReplayDetected,
    /// 404 — The requested `device_id` has no attestation record.
    #[error("Device not found in attestation registry")]
    DeviceNotFound,
    /// 400 — Generic malformed request body.
    #[error("BadRequest: {0}")]
    BadRequest(String),
    /// 409 — Optimistic concurrency conflict on sync `version_sequence`.
    #[error("Conflict: {0}")]
    Conflict(String),
    /// 409 — `PostgreSQL` SERIALIZABLE transaction serialization failure (code 40001).
    /// Caller should retry.
    #[error("Serialization conflict — retry transaction")]
    SerializationConflict,
    /// 500 — Database query failed.
    #[error("DatabaseError: {0}")]
    DatabaseError(String),
    /// 500 — A cryptographic operation failed unexpectedly.
    #[error("CryptoError: {0}")]
    CryptoError(String),
    /// 502 — An outbound service call (e.g. Claude API) failed.
    #[error("ExternalServiceError: {0}")]
    ExternalServiceError(String),
    /// 500 — Catch-all for unexpected internal errors.
    #[error("Internal: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            Self::InvalidAttestation(msg) => {
                (StatusCode::BAD_REQUEST, "INVALID_ATTESTATION", msg.clone())
            }
            Self::InvalidAssertion(msg) => {
                (StatusCode::BAD_REQUEST, "INVALID_ASSERTION", msg.clone())
            }
            Self::NonceExpired => (
                StatusCode::GONE,
                "NONCE_EXPIRED",
                "Challenge nonce has expired".to_owned(),
            ),
            Self::NonceNotFound => (
                StatusCode::NOT_FOUND,
                "NONCE_NOT_FOUND",
                "Challenge nonce not found or already consumed".to_owned(),
            ),
            Self::ReplayDetected => (
                StatusCode::CONFLICT,
                "REPLAY_DETECTED",
                "Assertion counter is stale — possible replay attack".to_owned(),
            ),
            Self::DeviceNotFound => (
                StatusCode::NOT_FOUND,
                "DEVICE_NOT_FOUND",
                "No attestation record for this device".to_owned(),
            ),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            Self::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            Self::SerializationConflict => (
                StatusCode::CONFLICT,
                "SERIALIZATION_CONFLICT",
                "Transaction serialization failure — retry".to_owned(),
            ),
            Self::DatabaseError(msg) => {
                tracing::error!(error = %msg, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR",
                    "An internal database error occurred".to_owned(),
                )
            }
            Self::CryptoError(msg) => {
                tracing::error!(error = %msg, "Cryptographic error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CRYPTO_ERROR",
                    "An internal cryptographic error occurred".to_owned(),
                )
            }
            Self::ExternalServiceError(msg) => {
                tracing::error!(error = %msg, "External service error");
                (
                    StatusCode::BAD_GATEWAY,
                    "EXTERNAL_SERVICE_ERROR",
                    "Upstream service unavailable".to_owned(),
                )
            }
            Self::Internal(msg) => {
                tracing::error!(error = %msg, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An unexpected internal error occurred".to_owned(),
                )
            }
        };

        let body = json!({
            "error": {
                "code": code,
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}

// ── Conversion impls for ergonomic `?` usage ─────────────────────────────────

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        // Detect PostgreSQL serialization failure (SQLSTATE 40001)
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.code().as_deref() == Some("40001") {
                return Self::SerializationConflict;
            }
        }
        Self::DatabaseError(e.to_string())
    }
}

impl From<ring::error::Unspecified> for AppError {
    fn from(_: ring::error::Unspecified) -> Self {
        Self::CryptoError("Unspecified cryptographic error".to_owned())
    }
}

impl From<base64::DecodeError> for AppError {
    fn from(e: base64::DecodeError) -> Self {
        Self::BadRequest(format!("Base64 decode error: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::BadRequest(format!("JSON error: {e}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::ExternalServiceError(format!("HTTP client error: {e}"))
    }
}
