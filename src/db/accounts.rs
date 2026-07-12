use crate::errors::AppError;
use crate::models::account::Account;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_account_by_email(pool: &PgPool, email: &str) -> Result<Option<Account>, AppError> {
    let row = sqlx::query_as::<_, Account>(
        "SELECT id, email, password_hash, password_salt, oauth_provider, oauth_subject, created_at \
         FROM accounts WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_account_by_oauth(
    pool: &PgPool,
    provider: &str,
    subject: &str,
) -> Result<Option<Account>, AppError> {
    let row = sqlx::query_as::<_, Account>(
        "SELECT id, email, password_hash, password_salt, oauth_provider, oauth_subject, created_at \
         FROM accounts WHERE oauth_provider = $1 AND oauth_subject = $2",
    )
    .bind(provider)
    .bind(subject)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Insert a new account. The unique constraints on `email` and
/// `(oauth_provider, oauth_subject)` surface as `AppError::Conflict`.
pub async fn insert_account(pool: &PgPool, a: &Account) -> Result<(), AppError> {
    let result = sqlx::query(
        "INSERT INTO accounts \
            (id, email, password_hash, password_salt, oauth_provider, oauth_subject) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(a.id)
    .bind(a.email.as_deref())
    .bind(a.password_hash.as_deref())
    .bind(a.password_salt.as_deref())
    .bind(a.oauth_provider.as_deref())
    .bind(a.oauth_subject.as_deref())
    .execute(pool)
    .await;
    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(AppError::Conflict(
            "An account with those details already exists.".to_owned(),
        )),
        Err(e) => Err(e.into()),
    }
}

/// Permanently delete the account that owns `device_id` **and all of its data**,
/// in a single transaction. Satisfies App Store 5.1.1(v) (in-app account
/// deletion) and GDPR/CCPA erasure.
///
/// Because every device-scoped table (`sync_documents`, `provider_connections`,
/// `game_profiles`, `subscriptions`, `account_devices`) is declared
/// `ON DELETE CASCADE` from `attested_devices`, deleting the device rows erases
/// the encrypted vault, connections, game profile, and subscription with them.
/// Audit logs carry no FK, so they're cleared explicitly. If the device is
/// linked to an account, every device under that account is purged too (so
/// deletion from any one signed-in device removes the whole account).
///
/// Returns `true` if a formal account existed, `false` if only device-scoped
/// data was purged (a device that was never linked to an account).
pub async fn delete_account_and_data(pool: &PgPool, device_id: Uuid) -> Result<bool, AppError> {
    let mut tx = pool.begin().await?;

    let account_id: Option<Uuid> =
        sqlx::query_scalar("SELECT account_id FROM account_devices WHERE device_id = $1")
            .bind(device_id)
            .fetch_optional(&mut *tx)
            .await?;

    // Every device to erase: all devices under the account, or just this one.
    let device_ids: Vec<Uuid> = match account_id {
        Some(aid) => {
            sqlx::query_scalar("SELECT device_id FROM account_devices WHERE account_id = $1")
                .bind(aid)
                .fetch_all(&mut *tx)
                .await?
        }
        None => vec![device_id],
    };

    // Audit logs have no FK to cascade, so clear them for these devices.
    sqlx::query("DELETE FROM audit_logs WHERE actor_id = ANY($1) OR target_id = ANY($1)")
        .bind(&device_ids)
        .execute(&mut *tx)
        .await?;

    // Deleting the device rows cascades to sync_documents, provider_connections,
    // game_profiles, subscriptions, and account_devices.
    sqlx::query("DELETE FROM attested_devices WHERE device_id = ANY($1)")
        .bind(&device_ids)
        .execute(&mut *tx)
        .await?;

    if let Some(aid) = account_id {
        sqlx::query("DELETE FROM accounts WHERE id = $1")
            .bind(aid)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(account_id.is_some())
}

/// Link a device to an account (idempotent; a device belongs to one account).
pub async fn link_device(pool: &PgPool, account_id: Uuid, device_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO account_devices (account_id, device_id) VALUES ($1, $2) \
         ON CONFLICT (device_id) DO UPDATE SET account_id = EXCLUDED.account_id, linked_at = NOW()",
    )
    .bind(account_id)
    .bind(device_id)
    .execute(pool)
    .await?;
    Ok(())
}
