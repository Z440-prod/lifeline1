-- Antigravity: Audit Logs schema for health record access compliance
-- Provides a tamper-resistant hash-chained ledger.

CREATE TABLE IF NOT EXISTS audit_logs (
    id             UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_time     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    action         VARCHAR(50) NOT NULL, -- e.g., 'READ_DOCUMENT', 'WRITE_DOCUMENT', 'REGISTER_DEVICE', 'VERIFY_ASSERTION'
    actor_id       UUID        NOT NULL, -- device_id or system actor
    target_id      UUID        NOT NULL, -- target resource ID (e.g., document_id, device_id)
    payload_hash   BYTEA       NOT NULL, -- SHA-256 hash of request payload metadata
    prev_signature BYTEA       NOT NULL, -- SHA-256 signature of the previous row (hash-chain link)
    signature      BYTEA       NOT NULL  -- Cryptographic signature of this record: SHA-256(id || event_time || action || actor_id || target_id || payload_hash || prev_signature)
);

CREATE INDEX idx_audit_logs_event_time ON audit_logs (event_time DESC);
CREATE INDEX idx_audit_logs_actor ON audit_logs (actor_id);
CREATE INDEX idx_audit_logs_target ON audit_logs (target_id);
