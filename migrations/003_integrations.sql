-- Antigravity: Third-party health data integrations + document categorization
--
-- Apple Health and Google Health Connect are on-device SDKs — the client
-- reads them locally and this table only records that a device has
-- authorized local access. Whoop is a cloud API and is the only provider for
-- which `encrypted_refresh_token` is populated (encrypted at rest with a key
-- derived from the server secret; see crypto::token_vault).
CREATE TABLE IF NOT EXISTS provider_connections (
    device_id               UUID        NOT NULL REFERENCES attested_devices (device_id)
                                                 ON DELETE CASCADE,
    provider                VARCHAR(20) NOT NULL, -- 'apple_health' | 'google_health' | 'whoop'
    status                  VARCHAR(20) NOT NULL DEFAULT 'connected',
    external_account_id     TEXT,
    encrypted_refresh_token BYTEA,
    connected_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_synced_at          TIMESTAMPTZ,

    PRIMARY KEY (device_id, provider)
);

CREATE INDEX idx_provider_connections_device ON provider_connections (device_id);

-- Client-assigned category for a sync document (e.g. "generic", "lab_result").
-- Purely a UI grouping label — the server still never sees document contents.
ALTER TABLE sync_documents ADD COLUMN document_type VARCHAR(30) NOT NULL DEFAULT 'generic';

CREATE INDEX idx_sync_documents_device_type ON sync_documents (device_id, document_type);
