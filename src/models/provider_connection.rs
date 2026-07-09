use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Third-party health data providers Lifeline can connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// On-device `HealthKit` access (iOS). No server-held credentials.
    AppleHealth,
    /// On-device Health Connect access (Android). No server-held credentials.
    GoogleHealth,
    /// Cloud API — the only provider requiring server-side `OAuth2`.
    Whoop,
}

impl Provider {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppleHealth => "apple_health",
            Self::GoogleHealth => "google_health",
            Self::Whoop => "whoop",
        }
    }

    /// Whether this provider is a cloud API needing a real `OAuth2` exchange,
    /// as opposed to an on-device SDK the client reads locally.
    #[must_use]
    pub fn is_cloud_oauth(self) -> bool {
        matches!(self, Self::Whoop)
    }
}

impl std::str::FromStr for Provider {
    type Err = crate::errors::AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "apple_health" => Ok(Self::AppleHealth),
            "google_health" => Ok(Self::GoogleHealth),
            "whoop" => Ok(Self::Whoop),
            other => Err(crate::errors::AppError::BadRequest(format!(
                "Unknown provider '{other}'. Expected one of: apple_health, google_health, whoop"
            ))),
        }
    }
}

/// A device's connection status to a third-party health data provider.
///
/// The encrypted refresh token (for cloud-OAuth providers) is intentionally
/// not part of this struct — it is never returned over the API, only
/// decrypted server-side at sync time.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderConnection {
    pub device_id: Uuid,
    pub provider: String,
    pub status: String,
    pub external_account_id: Option<String>,
    pub connected_at: DateTime<Utc>,
    pub last_synced_at: Option<DateTime<Utc>>,
}
