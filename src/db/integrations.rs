use crate::errors::AppError;
use crate::models::provider_connection::ProviderConnection;
use sqlx::PgPool;
use uuid::Uuid;

/// List every provider connection a device has, regardless of status.
pub async fn list_provider_connections(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Vec<ProviderConnection>, AppError> {
    let rows = sqlx::query_as::<_, ProviderConnection>(
        "SELECT device_id, provider, status, external_account_id, connected_at, last_synced_at \
         FROM provider_connections \
         WHERE device_id = $1 \
         ORDER BY provider",
    )
    .bind(device_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Insert or update a provider connection. `encrypted_refresh_token` is only
/// meaningful for cloud-OAuth providers (Whoop) — pass `None` to leave an
/// existing token untouched on a status-only update.
pub async fn upsert_provider_connection(
    pool: &PgPool,
    device_id: Uuid,
    provider: &str,
    status: &str,
    external_account_id: Option<&str>,
    encrypted_refresh_token: Option<&[u8]>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO provider_connections (device_id, provider, status, external_account_id, encrypted_refresh_token, connected_at) \
         VALUES ($1, $2, $3, $4, $5, NOW()) \
         ON CONFLICT (device_id, provider) DO UPDATE SET \
         status = EXCLUDED.status, \
         external_account_id = COALESCE(EXCLUDED.external_account_id, provider_connections.external_account_id), \
         encrypted_refresh_token = COALESCE(EXCLUDED.encrypted_refresh_token, provider_connections.encrypted_refresh_token)"
    )
    .bind(device_id)
    .bind(provider)
    .bind(status)
    .bind(external_account_id)
    .bind(encrypted_refresh_token)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_provider_connection(
    pool: &PgPool,
    device_id: Uuid,
    provider: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM provider_connections WHERE device_id = $1 AND provider = $2")
        .bind(device_id)
        .bind(provider)
        .execute(pool)
        .await?;
    Ok(())
}

/// Fetch the encrypted refresh token for a provider connection, if any.
pub async fn get_encrypted_refresh_token(
    pool: &PgPool,
    device_id: Uuid,
    provider: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    let token: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT encrypted_refresh_token FROM provider_connections WHERE device_id = $1 AND provider = $2",
    )
    .bind(device_id)
    .bind(provider)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(token)
}

pub async fn touch_last_synced(
    pool: &PgPool,
    device_id: Uuid,
    provider: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE provider_connections SET last_synced_at = NOW() WHERE device_id = $1 AND provider = $2",
    )
    .bind(device_id)
    .bind(provider)
    .execute(pool)
    .await?;
    Ok(())
}
