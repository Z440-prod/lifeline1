use crate::errors::AppError;
use crate::models::sync_document::SyncDocument;
use sqlx::PgPool;
use uuid::Uuid;

/// Inserts a new sync document version. Uses PostgreSQL SERIALIZABLE isolation level
/// and retries the transaction if a serialization failure (SQLSTATE 40001) occurs.
pub async fn upsert_sync_document(pool: &PgPool, doc: &SyncDocument) -> Result<(), AppError> {
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 5;

    loop {
        attempts += 1;
        match try_upsert_sync_document(pool, doc).await {
            Ok(()) => return Ok(()),
            Err(AppError::SerializationConflict) if attempts < MAX_ATTEMPTS => {
                tracing::warn!(
                    document_id = %doc.document_id,
                    version = doc.version_sequence,
                    attempt = attempts,
                    "Postgres serialization conflict (40001). Retrying transaction..."
                );
                tokio::time::sleep(std::time::Duration::from_millis(5 * attempts as u64)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

async fn try_upsert_sync_document(pool: &PgPool, doc: &SyncDocument) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    // Set transaction isolation level to SERIALIZABLE to ensure full consistency and prevent race conditions.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;

    // Get the latest version sequence of the document, if it exists
    let latest: Option<i64> = sqlx::query_scalar(
        "SELECT version_sequence FROM sync_documents WHERE document_id = $1 ORDER BY version_sequence DESC LIMIT 1"
    )
    .bind(doc.document_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(latest_ver) = latest {
        if doc.version_sequence <= latest_ver {
            return Err(AppError::Conflict(format!(
                "Version sequence {} must be strictly greater than current latest version sequence {}",
                doc.version_sequence, latest_ver
            )));
        }
    }

    // Insert the new document version
    sqlx::query(
        "INSERT INTO sync_documents (document_id, device_id, version_sequence, encrypted_blob, initialization_vector, auth_tag, client_signature, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    )
    .bind(doc.document_id)
    .bind(doc.device_id)
    .bind(doc.version_sequence)
    .bind(&doc.encrypted_blob)
    .bind(&doc.initialization_vector)
    .bind(&doc.auth_tag)
    .bind(&doc.client_signature)
    .bind(doc.created_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Retrieve the latest version of a sync document.
pub async fn get_latest_document(
    pool: &PgPool,
    document_id: Uuid,
) -> Result<Option<SyncDocument>, AppError> {
    let doc = sqlx::query_as::<_, SyncDocument>(
        "SELECT document_id, device_id, version_sequence, encrypted_blob, initialization_vector, auth_tag, client_signature, created_at \
         FROM sync_documents \
         WHERE document_id = $1 \
         ORDER BY version_sequence DESC \
         LIMIT 1"
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?;
    Ok(doc)
}

/// Retrieve the full version history for a sync document.
pub async fn get_document_history(
    pool: &PgPool,
    document_id: Uuid,
) -> Result<Vec<SyncDocument>, AppError> {
    let docs = sqlx::query_as::<_, SyncDocument>(
        "SELECT document_id, device_id, version_sequence, encrypted_blob, initialization_vector, auth_tag, client_signature, created_at \
         FROM sync_documents \
         WHERE document_id = $1 \
         ORDER BY version_sequence ASC"
    )
    .bind(document_id)
    .fetch_all(pool)
    .await?;
    Ok(docs)
}
