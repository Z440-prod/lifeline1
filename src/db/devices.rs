use crate::errors::AppError;
use crate::models::device::AttestedDevice;
use sqlx::PgPool;
use uuid::Uuid;

/// Retrieve an attested device by its ID.
pub async fn get_device(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<AttestedDevice>, AppError> {
    let device = sqlx::query_as::<_, AttestedDevice>(
        "SELECT device_id, public_key_der, sign_counter, registered_at FROM attested_devices WHERE device_id = $1"
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;
    Ok(device)
}

/// Insert or update an attested device.
pub async fn insert_device(pool: &PgPool, device: &AttestedDevice) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO attested_devices (device_id, public_key_der, sign_counter, registered_at) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (device_id) DO UPDATE SET \
         public_key_der = EXCLUDED.public_key_der, \
         sign_counter = EXCLUDED.sign_counter, \
         registered_at = EXCLUDED.registered_at",
    )
    .bind(device.device_id)
    .bind(&device.public_key_der)
    .bind(device.sign_counter)
    .bind(device.registered_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update the assertion counter for a specific device.
/// Returns `DeviceNotFound` if the device does not exist.
pub async fn update_counter(
    pool: &PgPool,
    device_id: Uuid,
    sign_counter: i64,
) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE attested_devices SET sign_counter = $1 WHERE device_id = $2")
        .bind(sign_counter)
        .bind(device_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::DeviceNotFound);
    }
    Ok(())
}
