//! Admin dashboard aggregates.
//!
//! PRIVACY: everything here is aggregate counts and pseudonymous leaderboard
//! handles (already public via the Arena). It returns **no health data and no
//! PII** — vault contents are only ever counted, never read; emails are never
//! selected. This preserves the zero-knowledge guarantee even for the operator.

use crate::errors::AppError;
use sqlx::PgPool;

/// Aggregate, non-identifying statistics for the admin dashboard.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct AdminStats {
    pub accounts: i64,
    pub devices: i64,
    /// Distinct encrypted documents (never their contents).
    pub documents: i64,
    /// Total stored versions across all documents.
    pub document_versions: i64,
    pub ranked_players: i64,
    pub subscriptions_pro: i64,
    pub subscriptions_elite: i64,
    /// League id → player count.
    pub leagues: Vec<(String, i64)>,
    /// Top players: (handle, best_vitality_score, league) — all already public.
    pub top_players: Vec<(String, i32, String)>,
}

pub async fn admin_stats(pool: &PgPool) -> Result<AdminStats, AppError> {
    let count =
        |sql: &'static str| async move { sqlx::query_scalar::<_, i64>(sql).fetch_one(pool).await };

    let accounts = count("SELECT COUNT(*) FROM accounts").await?;
    let devices = count("SELECT COUNT(*) FROM attested_devices").await?;
    let documents = count("SELECT COUNT(DISTINCT document_id) FROM sync_documents").await?;
    let document_versions = count("SELECT COUNT(*) FROM sync_documents").await?;
    let ranked_players = count("SELECT COUNT(*) FROM game_profiles").await?;
    let subscriptions_pro =
        count("SELECT COUNT(*) FROM subscriptions WHERE tier = 'pro' AND status = 'active'")
            .await?;
    let subscriptions_elite =
        count("SELECT COUNT(*) FROM subscriptions WHERE tier = 'elite' AND status = 'active'")
            .await?;

    let leagues: Vec<(String, i64)> = sqlx::query_as(
        "SELECT league, COUNT(*)::bigint FROM game_profiles GROUP BY league ORDER BY 2 DESC",
    )
    .fetch_all(pool)
    .await?;

    let top_players: Vec<(String, i32, String)> = sqlx::query_as(
        "SELECT handle, best_vitality_score, league FROM game_profiles \
         ORDER BY best_vitality_score DESC, updated_at ASC LIMIT 8",
    )
    .fetch_all(pool)
    .await?;

    Ok(AdminStats {
        accounts,
        devices,
        documents,
        document_versions,
        ranked_players,
        subscriptions_pro,
        subscriptions_elite,
        leagues,
        top_players,
    })
}
