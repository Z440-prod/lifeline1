use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single version of an encrypted sync document.
/// The backend is completely blind to contents — all fields except metadata
/// are opaque ciphertext encrypted on-device in the Secure Enclave.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncDocument {
    /// Client-assigned document identifier (`UUIDv4`).
    pub document_id: Uuid,
    /// The device that owns and authored this version.
    pub device_id: Uuid,
    /// Monotonically increasing version counter managed by the client.
    pub version_sequence: i64,
    /// Opaque ciphertext (AES-256-GCM or XChaCha20-Poly1305).
    pub encrypted_blob: Vec<u8>,
    /// Initialization vector / nonce used for encryption.
    pub initialization_vector: Vec<u8>,
    /// AEAD authentication tag.
    pub auth_tag: Vec<u8>,
    /// ECDSA P-256 signature computed by the device over the payload,
    /// verifiable using the device's attested public key.
    pub client_signature: Vec<u8>,
    /// Client-assigned category (e.g. "generic", "`lab_result`") used only for
    /// UI grouping — the server cannot see and does not need to see document
    /// contents to know what kind of record it is.
    pub document_type: String,
    /// Timestamp when this version was persisted on the backend.
    pub created_at: DateTime<Utc>,
}
