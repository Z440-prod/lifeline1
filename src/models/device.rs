use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An iOS device that has passed Apple App Attest verification.
/// Stores the EC P-256 public key extracted from the attestation certificate
/// and the monotonic sign counter used for replay protection.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AttestedDevice {
    /// Unique device identifier (`UUIDv4`), provided by the client.
    pub device_id: Uuid,
    /// DER-encoded uncompressed EC P-256 public key (65 bytes: 0x04 || x || y).
    pub public_key_der: Vec<u8>,
    /// Last seen assertion sign counter. Assertions with counter ≤ this value
    /// are rejected as replays.
    pub sign_counter: i64,
    /// Timestamp of initial attestation verification.
    pub registered_at: DateTime<Utc>,
}
