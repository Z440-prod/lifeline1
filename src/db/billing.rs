use crate::errors::AppError;
use crate::models::subscription::Subscription;
use sqlx::PgPool;
use uuid::Uuid;

/// Fetch a device's subscription row, if one exists. Absence means the device
/// is on the implicit free tier.
pub async fn get_subscription(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<Subscription>, AppError> {
    let row = sqlx::query_as::<_, Subscription>(
        "SELECT device_id, tier, status, stripe_customer_id, stripe_subscription_id, \
                current_period_end, created_at, updated_at \
         FROM subscriptions WHERE device_id = $1",
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Reconcile a subscription by its Stripe customer id — the key webhooks carry.
pub async fn get_subscription_by_customer(
    pool: &PgPool,
    stripe_customer_id: &str,
) -> Result<Option<Subscription>, AppError> {
    let row = sqlx::query_as::<_, Subscription>(
        "SELECT device_id, tier, status, stripe_customer_id, stripe_subscription_id, \
                current_period_end, created_at, updated_at \
         FROM subscriptions WHERE stripe_customer_id = $1 LIMIT 1",
    )
    .bind(stripe_customer_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Insert or update a device's subscription. Called both at checkout time (to
/// stash the Stripe customer id) and from webhooks (to flip tier/status).
pub async fn upsert_subscription(pool: &PgPool, s: &Subscription) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO subscriptions \
            (device_id, tier, status, stripe_customer_id, stripe_subscription_id, \
             current_period_end, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6, NOW()) \
         ON CONFLICT (device_id) DO UPDATE SET \
             tier = EXCLUDED.tier, \
             status = EXCLUDED.status, \
             stripe_customer_id = COALESCE(EXCLUDED.stripe_customer_id, subscriptions.stripe_customer_id), \
             stripe_subscription_id = COALESCE(EXCLUDED.stripe_subscription_id, subscriptions.stripe_subscription_id), \
             current_period_end = EXCLUDED.current_period_end, \
             updated_at = NOW()",
    )
    .bind(s.device_id)
    .bind(&s.tier)
    .bind(&s.status)
    .bind(s.stripe_customer_id.as_deref())
    .bind(s.stripe_subscription_id.as_deref())
    .bind(s.current_period_end)
    .execute(pool)
    .await?;
    Ok(())
}
