-- Antigravity: Accounts — an identity layer *on top of* device attestation.
--
-- The zero-knowledge model is unchanged: an account is only an email and a
-- password hash (or an OAuth subject). It holds no keys and no health data.
-- Devices remain the cryptographic identity; an account simply groups the
-- devices a person signs in from (continuity + recovery).

CREATE TABLE IF NOT EXISTS accounts (
    id             UUID         PRIMARY KEY,
    -- Lowercased at the API boundary; unique per account.
    email          VARCHAR(254) UNIQUE,
    -- PBKDF2-HMAC-SHA256 (600k iterations); NULL for OAuth-only accounts.
    password_hash  BYTEA,
    password_salt  BYTEA,
    -- 'apple' | 'google' when the account was created via OAuth.
    oauth_provider VARCHAR(10),
    oauth_subject  TEXT,
    created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    UNIQUE (oauth_provider, oauth_subject)
);

CREATE TABLE IF NOT EXISTS account_devices (
    account_id UUID        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    device_id  UUID        PRIMARY KEY REFERENCES attested_devices (device_id)
                                        ON DELETE CASCADE,
    linked_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_account_devices_account ON account_devices (account_id);

ALTER TABLE accounts        ENABLE ROW LEVEL SECURITY;
ALTER TABLE account_devices ENABLE ROW LEVEL SECURITY;
