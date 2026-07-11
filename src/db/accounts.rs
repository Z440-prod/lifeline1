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
