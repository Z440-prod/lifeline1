use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A device's subscription entitlement. No card data is ever stored here —
/// Stripe Checkout keeps all PCI scope. This row only records *which tier* a
/// device is entitled to and the Stripe ids needed to reconcile webhooks and
/// open the billing portal.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Subscription {
    #[serde(skip_serializing)]
    pub device_id: Uuid,
    pub tier: String,
    pub status: String,
    #[serde(skip_serializing)]
    pub stripe_customer_id: Option<String>,
    #[serde(skip_serializing)]
    pub stripe_subscription_id: Option<String>,
    pub current_period_end: Option<DateTime<Utc>>,
    #[serde(skip_serializing)]
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The three subscription tiers. Ordered by capability so `>=` comparisons
/// express "at least this tier".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Free forever — the core longevity portrait, limited history, ads later.
    Free,
    /// Everything the serious user needs: all sources, biomarkers, seasons.
    Pro,
    /// Pro plus beta access and early features.
    Elite,
}

impl Tier {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Elite => "elite",
        }
    }

    /// Entitlement flags the client uses to gate UI, and the server uses to
    /// gate tier-restricted endpoints. Higher tiers are supersets of lower.
    #[must_use]
    pub fn entitlements(self) -> Entitlements {
        match self {
            Self::Free => Entitlements {
                history_days: 7,
                ai_coach_daily_limit: 3,
                all_integrations: false,
                biomarker_tracking: false,
                competitive_seasons: false,
                ad_free: false,
                beta_access: false,
                early_features: false,
            },
            Self::Pro => Entitlements {
                history_days: -1, // unlimited
                ai_coach_daily_limit: -1,
                all_integrations: true,
                biomarker_tracking: true,
                competitive_seasons: true,
                ad_free: true,
                beta_access: false,
                early_features: false,
            },
            Self::Elite => Entitlements {
                history_days: -1,
                ai_coach_daily_limit: -1,
                all_integrations: true,
                biomarker_tracking: true,
                competitive_seasons: true,
                ad_free: true,
                beta_access: true,
                early_features: true,
            },
        }
    }
}

impl std::str::FromStr for Tier {
    type Err = crate::errors::AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "free" => Ok(Self::Free),
            "pro" => Ok(Self::Pro),
            "elite" => Ok(Self::Elite),
            other => Err(crate::errors::AppError::BadRequest(format!(
                "Unknown tier '{other}'. Expected one of: free, pro, elite"
            ))),
        }
    }
}

/// Feature entitlements for a tier. `-1` means "unlimited". Serialized into the
/// subscription/billing responses so the client can gate UI without hardcoding
/// business rules.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Entitlements {
    /// How many days of history are visible. `-1` = unlimited.
    pub history_days: i32,
    /// AI coach messages per day. `-1` = unlimited.
    pub ai_coach_daily_limit: i32,
    pub all_integrations: bool,
    pub biomarker_tracking: bool,
    pub competitive_seasons: bool,
    pub ad_free: bool,
    pub beta_access: bool,
    pub early_features: bool,
}

/// Map a Stripe subscription status to whether it currently grants access.
/// `active` and `trialing` keep the paid tier; anything else falls back to free.
#[must_use]
pub fn status_grants_access(status: &str) -> bool {
    matches!(status, "active" | "trialing")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_are_ordered() {
        assert!(Tier::Free < Tier::Pro);
        assert!(Tier::Pro < Tier::Elite);
    }

    #[test]
    fn only_elite_gets_beta() {
        assert!(!Tier::Free.entitlements().beta_access);
        assert!(!Tier::Pro.entitlements().beta_access);
        assert!(Tier::Elite.entitlements().beta_access);
    }

    #[test]
    fn pro_unlocks_integrations_and_seasons() {
        let e = Tier::Pro.entitlements();
        assert!(e.all_integrations && e.competitive_seasons && e.ad_free);
        assert_eq!(e.history_days, -1);
    }

    #[test]
    fn parse_roundtrip() {
        for t in [Tier::Free, Tier::Pro, Tier::Elite] {
            assert_eq!(t.as_str().parse::<Tier>().unwrap(), t);
        }
        assert!("platinum".parse::<Tier>().is_err());
    }

    #[test]
    fn access_statuses() {
        assert!(status_grants_access("active"));
        assert!(status_grants_access("trialing"));
        assert!(!status_grants_access("canceled"));
        assert!(!status_grants_access("past_due"));
    }
}
