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

/// Register an attested device.
///
/// # Security
/// `device_id` is a client-chosen UUID and is **not** cryptographically bound
/// to the attested key. If registration blindly upserted the public key, any
/// party holding a valid App Attest attestation (their own genuine device)
/// could claim a `device_id` already owned by someone else and overwrite the
/// key on the row that all of that victim's documents and integrations hang
/// off of — an account-takeover. So:
///
/// * First registration of a `device_id` inserts normally.
/// * Re-registration with the **same** public key is idempotent and must NOT
///   reset `sign_counter` (that would reopen the assertion-replay window).
/// * Re-registration of an existing `device_id` with a **different** key is
///   rejected with a conflict.
///
/// The conflict decision is made in a single statement so it is race-safe: the
/// `WHERE` guard on the `DO UPDATE` branch only matches when the stored key is
/// identical, so a differing key updates zero rows.
pub async fn insert_device(pool: &PgPool, device: &AttestedDevice) -> Result<(), AppError> {
    let result = sqlx::query(
        "INSERT INTO attested_devices (device_id, public_key_der, sign_counter, registered_at) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (device_id) DO UPDATE SET registered_at = EXCLUDED.registered_at \
         WHERE attested_devices.public_key_der = EXCLUDED.public_key_der",
    )
    .bind(device.device_id)
    .bind(&device.public_key_der)
    .bind(device.sign_counter)
    .bind(device.registered_at)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        // Row exists with a different public key — reject rather than hijack it.
        return Err(AppError::Conflict(
            "device_id is already registered to a different key".to_owned(),
        ));
    }
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
