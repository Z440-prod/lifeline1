use chrono::{DateTime, Utc};
use ring::hmac;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub event_time: DateTime<Utc>,
    pub action: String,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub payload_hash: Vec<u8>,
    pub prev_signature: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Derive the HMAC key used to sign the audit log hash-chain.
///
/// Domain-separated from the session-token HMAC key (`AppState::hmac_key`) so
/// that a signature computed for one purpose can never be replayed as valid
/// for the other, even though both are derived from the same server secret.
#[must_use]
pub fn derive_audit_key(server_secret: &str) -> hmac::Key {
    hmac::Key::new(
        hmac::HMAC_SHA256,
        format!("antigravity-audit-log-v1:{server_secret}").as_bytes(),
    )
}

/// Fields of a single audit log record that feed into its chained signature.
pub struct AuditRecordFields<'a> {
    pub id: Uuid,
    pub event_time: DateTime<Utc>,
    pub action: &'a str,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub payload_hash: &'a [u8],
    pub prev_signature: &'a [u8],
}

/// Compute the HMAC-SHA256 signature for a specific audit log record, chained
/// to the previous record's signature.
///
/// Unlike a plain hash, this is keyed: forging a consistent chain (e.g. to
/// cover up a tampered or deleted row) requires the server secret, not just
/// database write access.
#[must_use]
pub fn compute_signature(key: &hmac::Key, fields: &AuditRecordFields<'_>) -> Vec<u8> {
    let mut data = Vec::with_capacity(
        16 + 8
            + fields.action.len()
            + 16
            + 16
            + fields.payload_hash.len()
            + fields.prev_signature.len(),
    );

    // Concatenate fields
    data.extend_from_slice(fields.id.as_bytes());
    data.extend_from_slice(&fields.event_time.timestamp().to_be_bytes());
    data.extend_from_slice(fields.action.as_bytes());
    data.extend_from_slice(fields.actor_id.as_bytes());
    data.extend_from_slice(fields.target_id.as_bytes());
    data.extend_from_slice(fields.payload_hash);
    data.extend_from_slice(fields.prev_signature);

    hmac::sign(key, &data).as_ref().to_vec()
}
