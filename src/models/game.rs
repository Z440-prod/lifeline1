use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A device's pseudonymous gamification profile — its standing in Lifeline's
/// global health competition.
///
/// # Zero-knowledge
/// This record holds **no raw health data**. `vitality_score` is a single
/// opaque 0–100 integer the client derives on-device from plaintext only it
/// can read. The server ranks these opaque scores; it cannot reconstruct any
/// underlying biometric.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameProfile {
    #[serde(skip_serializing)]
    pub device_id: Uuid,
    pub handle: String,
    pub vitality_score: i32,
    pub best_vitality_score: i32,
    pub xp: i64,
    pub level: i32,
    pub league: String,
    pub streak_days: i32,
    pub longest_streak: i32,
    pub last_submission_date: Option<NaiveDate>,
    pub season_id: String,
    pub season_xp: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A health league. Assigned purely from the current vitality score, so the
/// ladder rewards *health*, not merely time spent in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct League {
    pub id: &'static str,
    pub name: &'static str,
    /// Inclusive lower bound of the vitality-score band.
    pub min_score: i32,
}

/// League ladder, ascending. The last band whose `min_score` the vitality
/// score meets or exceeds wins.
pub const LEAGUES: [League; 6] = [
    League {
        id: "bronze",
        name: "Bronze",
        min_score: 0,
    },
    League {
        id: "silver",
        name: "Silver",
        min_score: 40,
    },
    League {
        id: "gold",
        name: "Gold",
        min_score: 55,
    },
    League {
        id: "platinum",
        name: "Platinum",
        min_score: 70,
    },
    League {
        id: "diamond",
        name: "Diamond",
        min_score: 82,
    },
    League {
        id: "apex",
        name: "Apex",
        min_score: 92,
    },
];

/// XP awarded for a fresh daily submission, before streak bonus.
const XP_DAILY_BASE: i64 = 40;
/// Per-point multiplier on the vitality score.
const XP_PER_SCORE_POINT: i64 = 2;
/// XP per consecutive-day streak, capped so streaks can't run away.
const XP_PER_STREAK_DAY: i64 = 5;
const XP_STREAK_CAP_DAYS: i64 = 30;
/// Level curve tuning: `level = floor(sqrt(xp / XP_PER_LEVEL_UNIT)) + 1`.
const XP_PER_LEVEL_UNIT: f64 = 90.0;

/// The health league for a given vitality score.
#[must_use]
pub fn league_for(vitality_score: i32) -> League {
    LEAGUES
        .iter()
        .rev()
        .find(|l| vitality_score >= l.min_score)
        .copied()
        .unwrap_or(LEAGUES[0])
}

/// Level derived from all-time XP. A gentle square-root curve so early levels
/// come quickly and later ones ask for real consistency.
#[must_use]
pub fn level_for(xp: i64) -> i32 {
    if xp <= 0 {
        return 1;
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let lvl = (xp as f64 / XP_PER_LEVEL_UNIT).sqrt() as i32 + 1;
    lvl
}

/// The XP threshold required to *reach* a given level — the inverse of
/// [`level_for`]. Used to render progress toward the next level.
#[must_use]
pub fn xp_for_level(level: i32) -> i64 {
    if level <= 1 {
        return 0;
    }
    let base = f64::from(level - 1);
    #[allow(clippy::cast_possible_truncation)]
    let xp = (base * base * XP_PER_LEVEL_UNIT).ceil() as i64;
    xp
}

/// XP earned by a single daily submission, given the score and the resulting
/// streak length.
#[must_use]
pub fn xp_for_submission(vitality_score: i32, streak_days: i32) -> i64 {
    let score = i64::from(vitality_score.clamp(0, 100));
    let streak = i64::from(streak_days.max(0)).min(XP_STREAK_CAP_DAYS);
    XP_DAILY_BASE + score * XP_PER_SCORE_POINT + streak * XP_PER_STREAK_DAY
}

/// ISO-week competitive season identifier, e.g. `"2026-W28"`. Seasons roll
/// over weekly so the leaderboard stays lively and newcomers can climb.
#[must_use]
pub fn season_id_for(date: NaiveDate) -> String {
    let iso = date.iso_week();
    format!("{}-W{:02}", iso.year(), iso.week())
}

/// The next streak length given the previous submission date and today.
///
/// * same day → unchanged (idempotent re-submit)
/// * consecutive day → +1
/// * any gap → reset to 1
#[must_use]
pub fn next_streak(last: Option<NaiveDate>, today: NaiveDate, current_streak: i32) -> i32 {
    match last {
        Some(d) if d == today => current_streak.max(1),
        Some(d) if d.succ_opt() == Some(today) => current_streak + 1,
        _ => 1,
    }
}

/// Whether a handle is a valid pseudonymous display name: 3–20 chars of
/// `[A-Za-z0-9_]`. Deliberately excludes anything that could carry PII.
#[must_use]
pub fn is_valid_handle(handle: &str) -> bool {
    let len = handle.chars().count();
    (3..=20).contains(&len)
        && handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn league_bands_are_monotonic_and_correct() {
        assert_eq!(league_for(0).id, "bronze");
        assert_eq!(league_for(39).id, "bronze");
        assert_eq!(league_for(40).id, "silver");
        assert_eq!(league_for(69).id, "gold");
        assert_eq!(league_for(70).id, "platinum");
        assert_eq!(league_for(91).id, "diamond");
        assert_eq!(league_for(92).id, "apex");
        assert_eq!(league_for(100).id, "apex");
    }

    #[test]
    fn level_curve_is_increasing_and_inverts() {
        assert_eq!(level_for(0), 1);
        assert!(level_for(1000) > level_for(100));
        // xp_for_level is the inverse threshold: reaching it yields that level.
        for lvl in 2..20 {
            assert!(
                level_for(xp_for_level(lvl)) >= lvl,
                "level {lvl} threshold {} yielded {}",
                xp_for_level(lvl),
                level_for(xp_for_level(lvl))
            );
        }
    }

    #[test]
    fn submission_xp_scales_with_score_and_streak() {
        assert!(xp_for_submission(90, 10) > xp_for_submission(40, 0));
        // Streak bonus is capped.
        assert_eq!(
            xp_for_submission(50, 30),
            xp_for_submission(50, 999),
            "streak XP must be capped"
        );
    }

    #[test]
    fn streak_logic_handles_same_consecutive_and_gap() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 7, 8).unwrap();
        let last_week = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        assert_eq!(next_streak(None, today, 0), 1);
        assert_eq!(next_streak(Some(yesterday), today, 4), 5);
        assert_eq!(next_streak(Some(today), today, 4), 4);
        assert_eq!(next_streak(Some(last_week), today, 4), 1);
    }

    #[test]
    fn handle_validation() {
        assert!(is_valid_handle("iron_lung"));
        assert!(is_valid_handle("VO2max99"));
        assert!(!is_valid_handle("ab"));
        assert!(!is_valid_handle("has spaces"));
        assert!(!is_valid_handle("emoji💪here"));
        assert!(!is_valid_handle(&"x".repeat(21)));
    }

    #[test]
    fn season_id_is_iso_week() {
        let d = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();
        assert_eq!(season_id_for(d), "2026-W28");
    }
}
