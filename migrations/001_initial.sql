-- Antigravity: Initial schema for Lifeline backend
-- Requires PostgreSQL 14+ for SERIALIZABLE advisory lock compatibility.

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ────────────────────────────────────────────────────────────────────────────
-- Attested Devices
-- Stores the P-256 public key extracted during Apple App Attest verification.
-- Each row represents a single verified device/app installation.
-- ────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS attested_devices (
    device_id       UUID        PRIMARY KEY,
    public_key_der  BYTEA       NOT NULL,           -- Uncompressed EC P-256 point (65 bytes)
    sign_counter    BIGINT      NOT NULL DEFAULT 0,  -- Monotonic counter for replay protection
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attested_devices_registered ON attested_devices (registered_at);

-- ────────────────────────────────────────────────────────────────────────────
-- Sync Documents (Zero-Knowledge E2EE)
-- All payloads are opaque ciphertext encrypted on-device.
-- The backend never possesses decryption keys.
-- ────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS sync_documents (
    document_id           UUID        NOT NULL,
    device_id             UUID        NOT NULL REFERENCES attested_devices (device_id)
                                              ON DELETE CASCADE,
    version_sequence      BIGINT      NOT NULL,      -- Client-managed monotonic version counter
    encrypted_blob        BYTEA       NOT NULL,      -- AES-256-GCM or XChaCha20-Poly1305 ciphertext
    initialization_vector BYTEA       NOT NULL,      -- 12-byte (GCM) or 24-byte (XChaCha) IV
    auth_tag              BYTEA       NOT NULL,      -- AEAD authentication tag
    client_signature      BYTEA       NOT NULL,      -- ECDSA P-256 signature over the payload
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (document_id, version_sequence)
);

CREATE INDEX idx_sync_documents_device ON sync_documents (device_id);
CREATE INDEX idx_sync_documents_latest ON sync_documents (document_id, version_sequence DESC);
