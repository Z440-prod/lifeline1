-- Antigravity: Gamification (global health ranking) + Stripe billing
--
-- ── Zero-knowledge note ──────────────────────────────────────────────────────
-- The gamification layer NEVER stores raw health data. The client computes a
-- single derived integer "vitality score" (0–100) on-device from plaintext it
-- alone can read, and submits only that opaque score plus a self-chosen
-- pseudonymous handle. The server ranks these opaque scores; it still cannot
-- reconstruct any biometric.

CREATE TABLE IF NOT EXISTS game_profiles (
    device_id           UUID        PRIMARY KEY REFERENCES attested_devices (device_id)
                                                ON DELETE CASCADE,
    -- Pseudonymous, user-chosen display name for the global leaderboard.
    handle              VARCHAR(20) NOT NULL UNIQUE,
    -- Most recent submitted vitality score (0–100) — health-based league input.
    vitality_score      INTEGER     NOT NULL DEFAULT 0,
    best_vitality_score INTEGER     NOT NULL DEFAULT 0,
    -- All-time experience points (drives level).
    xp                  BIGINT      NOT NULL DEFAULT 0,
    level               INTEGER     NOT NULL DEFAULT 1,
    -- Current health league, derived from vitality_score bands.
    league              VARCHAR(16) NOT NULL DEFAULT 'bronze',
    streak_days         INTEGER     NOT NULL DEFAULT 0,
    longest_streak      INTEGER     NOT NULL DEFAULT 0,
    last_submission_date DATE,
    -- Competitive season the season_xp belongs to (ISO week, e.g. "2026-W28").
    season_id           VARCHAR(12) NOT NULL DEFAULT '',
    season_xp           BIGINT      NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Leaderboard ordering: current season first, then all-time score.
CREATE INDEX idx_game_profiles_season_rank
    ON game_profiles (season_id, season_xp DESC, best_vitality_score DESC);
CREATE INDEX idx_game_profiles_xp ON game_profiles (xp DESC);

-- ── Billing / subscriptions ──────────────────────────────────────────────────
-- One row per device. `tier` is the entitlement level the rest of the app
-- gates features on. Stripe customer/subscription ids link back to Stripe for
-- webhook reconciliation and the billing portal. No card data ever touches
-- this server — Stripe Checkout handles PCI scope.
CREATE TABLE IF NOT EXISTS subscriptions (
    device_id            UUID        PRIMARY KEY REFERENCES attested_devices (device_id)
                                                 ON DELETE CASCADE,
    tier                 VARCHAR(12) NOT NULL DEFAULT 'free',   -- 'free' | 'pro' | 'elite'
    status               VARCHAR(20) NOT NULL DEFAULT 'active', -- Stripe subscription status
    stripe_customer_id   TEXT,
    stripe_subscription_id TEXT,
    current_period_end   TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Webhooks arrive keyed by Stripe customer id — index it for reconciliation.
CREATE INDEX idx_subscriptions_customer ON subscriptions (stripe_customer_id);
