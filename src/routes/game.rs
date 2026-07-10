use axum::{
    extract::{Query, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::errors::AppError;
use crate::middleware::attest_guard::VerifiedDevice;
use crate::models::game::{self, GameProfile, LEAGUES};
use crate::state::AppState;

/// Body for `POST /api/v1/game/score`.
#[derive(Debug, Deserialize)]
pub struct SubmitScoreRequest {
    /// On-device-derived vitality score (0–100). This is the ONLY health-derived
    /// value that ever reaches the server, and it is opaque — no biometric can be
    /// reconstructed from it.
    pub vitality_score: i32,
    /// Pseudonymous display handle. Required on the first submission; optional
    /// afterwards (a change renames the profile).
    #[serde(default)]
    pub handle: Option<String>,
}

/// Serialize a profile plus its live rank/percentile into the API shape.
fn profile_json(profile: &GameProfile, rank: i64, total: i64) -> Value {
    let next_level_xp = game::xp_for_level(profile.level + 1);
    let this_level_xp = game::xp_for_level(profile.level);
    #[allow(clippy::cast_precision_loss)]
    let percentile = if total > 1 {
        // Top percentile: rank 1 → ~99th. Higher is better.
        (100.0 * (1.0 - (rank as f64 - 1.0) / total as f64)).round()
    } else {
        100.0
    };
    json!({
        "handle": profile.handle,
        "vitality_score": profile.vitality_score,
        "best_vitality_score": profile.best_vitality_score,
        "xp": profile.xp,
        "level": profile.level,
        "xp_into_level": profile.xp - this_level_xp,
        "xp_for_next_level": next_level_xp - this_level_xp,
        "league": profile.league,
        "streak_days": profile.streak_days,
        "longest_streak": profile.longest_streak,
        "season_id": profile.season_id,
        "season_xp": profile.season_xp,
        "rank": rank,
        "population": total,
        "percentile": percentile,
    })
}

/// Handler for `POST /api/v1/game/score`.
///
/// Submits today's derived vitality score, advancing XP / streak / league and
/// returning the updated standing. Idempotent within a calendar day: a second
/// submission the same day refreshes the score band but never double-counts XP
/// or streak (a simple anti-farming guard).
#[tracing::instrument(skip(state, payload), fields(device_id = %verified_device.device_id))]
pub async fn submit_score_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Json(payload): Json<SubmitScoreRequest>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/game/score").increment(1);

    let device_id = verified_device.device_id;
    let vitality = payload.vitality_score.clamp(0, 100);
    let today = chrono::Utc::now().date_naive();
    let season = game::season_id_for(today);

    let existing = state.db.get_game_profile(device_id).await?;

    // Resolve the handle: reuse the stored one, or validate a supplied one.
    let handle = match (&payload.handle, &existing) {
        (Some(h), _) => {
            let h = h.trim().to_owned();
            if !game::is_valid_handle(&h) {
                return Err(AppError::BadRequest(
                    "Handle must be 3–20 characters of letters, numbers, or underscore.".to_owned(),
                ));
            }
            if state.db.is_handle_taken(&h, device_id).await? {
                return Err(AppError::Conflict(
                    "That handle is already taken — choose another.".to_owned(),
                ));
            }
            h
        }
        (None, Some(p)) => p.handle.clone(),
        (None, None) => {
            return Err(AppError::BadRequest(
                "A handle is required for your first leaderboard entry.".to_owned(),
            ));
        }
    };

    let now = chrono::Utc::now();
    let already_today = existing
        .as_ref()
        .and_then(|p| p.last_submission_date)
        .is_some_and(|d| d == today);

    let profile = if let Some(prev) = existing {
        if already_today {
            // Same-day resubmit: update the score band only, no XP/streak change.
            let best = prev.best_vitality_score.max(vitality);
            GameProfile {
                handle,
                vitality_score: vitality,
                best_vitality_score: best,
                league: game::league_for(vitality).id.to_owned(),
                updated_at: now,
                ..prev
            }
        } else {
            let streak = game::next_streak(prev.last_submission_date, today, prev.streak_days);
            let gained = game::xp_for_submission(vitality, streak);
            let xp = prev.xp + gained;
            // Reset season XP when the season rolls over.
            let season_xp = if prev.season_id == season {
                prev.season_xp + gained
            } else {
                gained
            };
            GameProfile {
                handle,
                vitality_score: vitality,
                best_vitality_score: prev.best_vitality_score.max(vitality),
                xp,
                level: game::level_for(xp),
                league: game::league_for(vitality).id.to_owned(),
                streak_days: streak,
                longest_streak: prev.longest_streak.max(streak),
                last_submission_date: Some(today),
                season_id: season,
                season_xp,
                updated_at: now,
                ..prev
            }
        }
    } else {
        // First-ever submission.
        let streak = 1;
        let gained = game::xp_for_submission(vitality, streak);
        GameProfile {
            device_id,
            handle,
            vitality_score: vitality,
            best_vitality_score: vitality,
            xp: gained,
            level: game::level_for(gained),
            league: game::league_for(vitality).id.to_owned(),
            streak_days: streak,
            longest_streak: streak,
            last_submission_date: Some(today),
            season_id: season,
            season_xp: gained,
            created_at: now,
            updated_at: now,
        }
    };

    state.db.upsert_game_profile(&profile).await?;

    let (rank, total) = state
        .db
        .season_rank(
            &profile.season_id,
            profile.season_xp,
            profile.best_vitality_score,
        )
        .await?;

    Ok(Json(profile_json(&profile, rank, total)))
}

/// Handler for `GET /api/v1/game/profile`.
/// Returns the caller's own standing, or a 404 if they've never competed.
#[tracing::instrument(skip(state), fields(device_id = %verified_device.device_id))]
pub async fn get_profile_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/game/profile").increment(1);

    let profile = state
        .db
        .get_game_profile(verified_device.device_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("No profile yet — submit a vitality score to join.".to_owned())
        })?;
    let (rank, total) = state
        .db
        .season_rank(
            &profile.season_id,
            profile.season_xp,
            profile.best_vitality_score,
        )
        .await?;
    Ok(Json(profile_json(&profile, rank, total)))
}

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    #[serde(default)]
    pub season_id: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// Handler for `GET /api/v1/game/leaderboard`.
/// Global top-N for a season (defaults to the current one) plus the caller's
/// own row and rank so the client can render "you're #N".
#[tracing::instrument(skip(state), fields(device_id = %verified_device.device_id))]
pub async fn leaderboard_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<Json<Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/game/leaderboard")
        .increment(1);

    let season = q
        .season_id
        .unwrap_or_else(|| game::season_id_for(chrono::Utc::now().date_naive()));
    let limit = q.limit.unwrap_or(50).clamp(1, 100);

    let rows = state.db.leaderboard(&season, limit).await?;
    let entries: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, p)| {
            json!({
                "rank": i + 1,
                "handle": p.handle,
                "vitality_score": p.vitality_score,
                "level": p.level,
                "league": p.league,
                "season_xp": p.season_xp,
                "streak_days": p.streak_days,
            })
        })
        .collect();

    // The caller's own standing, even if they're outside the visible top-N.
    let me = match state.db.get_game_profile(verified_device.device_id).await? {
        Some(p) if p.season_id == season => {
            let (rank, total) = state
                .db
                .season_rank(&season, p.season_xp, p.best_vitality_score)
                .await?;
            profile_json(&p, rank, total)
        }
        _ => Value::Null,
    };

    Ok(Json(json!({
        "season_id": season,
        "entries": entries,
        "me": me,
    })))
}

/// Handler for `GET /api/v1/game/config`.
/// Public, rules-only: ships the league ladder, level curve, and XP rules so
/// the client can render and preview the game without any user data. Mirrors
/// `/insights/config` and `/ai/policy-matrix`.
#[tracing::instrument(skip(_state))]
pub async fn game_config_handler(State(_state): State<Arc<AppState>>) -> Json<Value> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/game/config").increment(1);

    let leagues: Vec<Value> = LEAGUES
        .iter()
        .map(|l| json!({ "id": l.id, "name": l.name, "min_score": l.min_score }))
        .collect();

    Json(json!({
        "version": "1.0.0",
        "leagues": leagues,
        "level_curve": {
            "formula": "level = floor(sqrt(xp / 90)) + 1",
            "examples": [
                { "level": 1, "xp": game::xp_for_level(1) },
                { "level": 5, "xp": game::xp_for_level(5) },
                { "level": 10, "xp": game::xp_for_level(10) },
                { "level": 20, "xp": game::xp_for_level(20) }
            ]
        },
        "xp_rules": {
            "daily_base": 40,
            "per_score_point": 2,
            "per_streak_day": 5,
            "streak_cap_days": 30,
            "note": "One scoring submission counts per calendar day."
        },
        "season": {
            "cadence": "weekly",
            "id_format": "ISO-8601 week, e.g. 2026-W28",
            "current": game::season_id_for(chrono::Utc::now().date_naive())
        }
    }))
}
