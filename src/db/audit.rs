use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
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

/// Compute the SHA-256 signature for a specific audit log record.
/// Concatenates all record fields cryptographically to prevent tamper attacks.
pub fn compute_signature(
    id: Uuid,
    event_time: DateTime<Utc>,
    action: &str,
    actor_id: Uuid,
    target_id: Uuid,
    payload_hash: &[u8],
    prev_signature: &[u8],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(
        16 + 8 + action.len() + 16 + 16 + payload_hash.len() + prev_signature.len(),
    );

    // Concatenate fields
    data.extend_from_slice(id.as_bytes());
    data.extend_from_slice(&event_time.timestamp().to_be_bytes());
    data.extend_from_slice(action.as_bytes());
    data.extend_from_slice(actor_id.as_bytes());
    data.extend_from_slice(target_id.as_bytes());
    data.extend_from_slice(payload_hash);
    data.extend_from_slice(prev_signature);

    digest(&SHA256, &data).as_ref().to_vec()
}
