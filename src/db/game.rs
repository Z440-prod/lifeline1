use crate::errors::AppError;
use crate::models::game::GameProfile;
use sqlx::PgPool;
use uuid::Uuid;

/// Fetch a device's gamification profile, if it has ever submitted a score.
pub async fn get_game_profile(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<GameProfile>, AppError> {
    let row = sqlx::query_as::<_, GameProfile>(
        "SELECT device_id, handle, vitality_score, best_vitality_score, xp, level, league, \
                streak_days, longest_streak, last_submission_date, season_id, season_xp, \
                created_at, updated_at \
         FROM game_profiles WHERE device_id = $1",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Whether a handle is already claimed by a *different* device.
pub async fn is_handle_taken(
    pool: &PgPool,
    handle: &str,
    exclude_device: Uuid,
) -> Result<bool, AppError> {
    let taken: Option<Uuid> = sqlx::query_scalar(
        "SELECT device_id FROM game_profiles WHERE handle = $1 AND device_id <> $2 LIMIT 1",
    )
    .bind(handle)
    .bind(exclude_device)
    .fetch_optional(pool)
    .await?;
    Ok(taken.is_some())
}

/// Insert or replace a device's full profile. The handler computes all derived
/// fields (xp, level, league, streak) so persistence stays a dumb write. The
/// unique constraint on `handle` surfaces as an `AppError::Conflict`.
pub async fn upsert_game_profile(pool: &PgPool, p: &GameProfile) -> Result<(), AppError> {
    let result = sqlx::query(
        "INSERT INTO game_profiles \
            (device_id, handle, vitality_score, best_vitality_score, xp, level, league, \
             streak_days, longest_streak, last_submission_date, season_id, season_xp, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12, NOW()) \
         ON CONFLICT (device_id) DO UPDATE SET \
             handle = EXCLUDED.handle, \
             vitality_score = EXCLUDED.vitality_score, \
             best_vitality_score = EXCLUDED.best_vitality_score, \
             xp = EXCLUDED.xp, \
             level = EXCLUDED.level, \
             league = EXCLUDED.league, \
             streak_days = EXCLUDED.streak_days, \
             longest_streak = EXCLUDED.longest_streak, \
             last_submission_date = EXCLUDED.last_submission_date, \
             season_id = EXCLUDED.season_id, \
             season_xp = EXCLUDED.season_xp, \
             updated_at = NOW()",
    )
    .bind(p.device_id)
    .bind(&p.handle)
    .bind(p.vitality_score)
    .bind(p.best_vitality_score)
    .bind(p.xp)
    .bind(p.level)
    .bind(&p.league)
    .bind(p.streak_days)
    .bind(p.longest_streak)
    .bind(p.last_submission_date)
    .bind(&p.season_id)
    .bind(p.season_xp)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(AppError::Conflict(
            "That handle is already taken — choose another.".to_owned(),
        )),
        Err(e) => Err(e.into()),
    }
}

/// Top `limit` profiles for a season, ranked by season XP then best score.
pub async fn leaderboard(
    pool: &PgPool,
    season_id: &str,
    limit: i64,
) -> Result<Vec<GameProfile>, AppError> {
    let rows = sqlx::query_as::<_, GameProfile>(
        "SELECT device_id, handle, vitality_score, best_vitality_score, xp, level, league, \
                streak_days, longest_streak, last_submission_date, season_id, season_xp, \
                created_at, updated_at \
         FROM game_profiles WHERE season_id = $1 \
         ORDER BY season_xp DESC, best_vitality_score DESC, updated_at ASC \
         LIMIT $2",
    )
    .bind(season_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// A device's 1-based rank within a season and the season's total population.
/// Rank counts everyone strictly ahead on `(season_xp, best_vitality_score)`.
pub async fn season_rank(
    pool: &PgPool,
    season_id: &str,
    season_xp: i64,
    best_vitality_score: i32,
) -> Result<(i64, i64), AppError> {
    let ahead: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM game_profiles \
         WHERE season_id = $1 \
           AND (season_xp > $2 OR (season_xp = $2 AND best_vitality_score > $3))",
    )
    .bind(season_id)
    .bind(season_xp)
    .bind(best_vitality_score)
    .fetch_one(pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_profiles WHERE season_id = $1")
        .bind(season_id)
        .fetch_one(pool)
        .await?;
    Ok((ahead + 1, total))
}
