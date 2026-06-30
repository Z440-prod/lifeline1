use crate::errors::AppError;
use crate::models::device::AttestedDevice;
use crate::models::sync_document::SyncDocument;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

pub mod audit;
pub mod devices;
pub mod sync;

/// Abstract database interface for all persistence operations.
///
/// # Why `#[async_trait]` is used here
/// This trait is consumed via `Arc<dyn Database>` for dynamic dispatch (allowing
/// runtime swapping between `PostgresDatabase` and `MockDatabase`). Native
/// `async fn` in traits (stabilized in Rust 1.75) produces opaque return types
/// that are **not object-safe**, so `dyn Database` would not compile without
/// `#[async_trait]`. If we later switch to generic `AppState<DB: Database>`
/// (static dispatch), this macro can be removed entirely.
#[async_trait]
pub trait Database: Send + Sync {
    async fn get_device(&self, device_id: Uuid) -> Result<Option<AttestedDevice>, AppError>;
    async fn insert_device(&self, device: &AttestedDevice) -> Result<(), AppError>;
    async fn update_counter(&self, device_id: Uuid, sign_counter: i64) -> Result<(), AppError>;
    async fn upsert_sync_document(&self, doc: &SyncDocument) -> Result<(), AppError>;
    async fn get_latest_document(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SyncDocument>, AppError>;
    async fn get_document_history(&self, document_id: Uuid) -> Result<Vec<SyncDocument>, AppError>;
    async fn insert_audit_log(
        &self,
        action: &str,
        actor_id: Uuid,
        target_id: Uuid,
        payload_hash: &[u8],
    ) -> Result<(), AppError>;
}

/// Postgres implementation of the Database trait.
pub struct PostgresDatabase {
    pub pool: sqlx::PgPool,
}

impl PostgresDatabase {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    async fn try_insert_audit_log(
        &self,
        action: &str,
        actor_id: Uuid,
        target_id: Uuid,
        payload_hash: &[u8],
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;

        // Use SERIALIZABLE isolation to guarantee signature chain linearity.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *tx)
            .await?;

        let prev_signature: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT signature FROM audit_logs ORDER BY event_time DESC, id DESC LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;

        let prev_sig = prev_signature.unwrap_or_else(|| vec![0u8; 32]);
        let id = Uuid::new_v4();
        let event_time = chrono::Utc::now();
        let signature = audit::compute_signature(
            id,
            event_time,
            action,
            actor_id,
            target_id,
            payload_hash,
            &prev_sig,
        );

        sqlx::query(
            "INSERT INTO audit_logs (id, event_time, action, actor_id, target_id, payload_hash, prev_signature, signature) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(id)
        .bind(event_time)
        .bind(action)
        .bind(actor_id)
        .bind(target_id)
        .bind(payload_hash)
        .bind(prev_sig)
        .bind(signature)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[async_trait]
impl Database for PostgresDatabase {
    async fn get_device(&self, device_id: Uuid) -> Result<Option<AttestedDevice>, AppError> {
        devices::get_device(&self.pool, device_id).await
    }

    async fn insert_device(&self, device: &AttestedDevice) -> Result<(), AppError> {
        devices::insert_device(&self.pool, device).await
    }

    async fn update_counter(&self, device_id: Uuid, sign_counter: i64) -> Result<(), AppError> {
        devices::update_counter(&self.pool, device_id, sign_counter).await
    }

    async fn upsert_sync_document(&self, doc: &SyncDocument) -> Result<(), AppError> {
        sync::upsert_sync_document(&self.pool, doc).await
    }

    async fn get_latest_document(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SyncDocument>, AppError> {
        sync::get_latest_document(&self.pool, document_id).await
    }

    async fn get_document_history(&self, document_id: Uuid) -> Result<Vec<SyncDocument>, AppError> {
        sync::get_document_history(&self.pool, document_id).await
    }

    async fn insert_audit_log(
        &self,
        action: &str,
        actor_id: Uuid,
        target_id: Uuid,
        payload_hash: &[u8],
    ) -> Result<(), AppError> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 5;

        loop {
            attempts += 1;
            match self
                .try_insert_audit_log(action, actor_id, target_id, payload_hash)
                .await
            {
                Ok(()) => return Ok(()),
                Err(AppError::SerializationConflict) if attempts < MAX_ATTEMPTS => {
                    tracing::warn!(
                        action = action,
                        actor_id = %actor_id,
                        attempt = attempts,
                        "Postgres serialization conflict on audit log. Retrying..."
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(5 * attempts as u64)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// In-memory thread-safe Mock database for offline development and local debugging.
pub struct MockDatabase {
    devices: Mutex<HashMap<Uuid, AttestedDevice>>,
    documents: Mutex<HashMap<Uuid, Vec<SyncDocument>>>,
    audit_logs: Mutex<Vec<audit::AuditLogEntry>>,
}

impl MockDatabase {
    pub fn new() -> Self {
        Self {
            devices: Mutex::new(HashMap::new()),
            documents: Mutex::new(HashMap::new()),
            audit_logs: Mutex::new(Vec::new()),
        }
    }

    pub fn get_audit_logs(&self) -> Vec<audit::AuditLogEntry> {
        let guard = self.audit_logs.lock().unwrap();
        guard.clone()
    }
}

impl Default for MockDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Database for MockDatabase {
    async fn get_device(&self, device_id: Uuid) -> Result<Option<AttestedDevice>, AppError> {
        let guard = self.devices.lock().unwrap();
        Ok(guard.get(&device_id).cloned())
    }

    async fn insert_device(&self, device: &AttestedDevice) -> Result<(), AppError> {
        let mut guard = self.devices.lock().unwrap();
        guard.insert(device.device_id, device.clone());
        Ok(())
    }

    async fn update_counter(&self, device_id: Uuid, sign_counter: i64) -> Result<(), AppError> {
        let mut guard = self.devices.lock().unwrap();
        if let Some(device) = guard.get_mut(&device_id) {
            device.sign_counter = sign_counter;
            Ok(())
        } else {
            Err(AppError::DeviceNotFound)
        }
    }

    async fn upsert_sync_document(&self, doc: &SyncDocument) -> Result<(), AppError> {
        let mut guard = self.documents.lock().unwrap();
        let history = guard.entry(doc.document_id).or_default();
        if let Some(latest) = history.last() {
            if doc.version_sequence <= latest.version_sequence {
                return Err(AppError::Conflict(
                    "Optimistic concurrency conflict".to_owned(),
                ));
            }
        }
        history.push(doc.clone());
        Ok(())
    }

    async fn get_latest_document(
        &self,
        document_id: Uuid,
    ) -> Result<Option<SyncDocument>, AppError> {
        let guard = self.documents.lock().unwrap();
        Ok(guard.get(&document_id).and_then(|h| h.last().cloned()))
    }

    async fn get_document_history(&self, document_id: Uuid) -> Result<Vec<SyncDocument>, AppError> {
        let guard = self.documents.lock().unwrap();
        Ok(guard.get(&document_id).cloned().unwrap_or_default())
    }

    async fn insert_audit_log(
        &self,
        action: &str,
        actor_id: Uuid,
        target_id: Uuid,
        payload_hash: &[u8],
    ) -> Result<(), AppError> {
        let mut guard = self.audit_logs.lock().unwrap();
        let prev_sig = guard
            .last()
            .map(|entry| entry.signature.clone())
            .unwrap_or_else(|| vec![0u8; 32]);

        let id = Uuid::new_v4();
        let event_time = chrono::Utc::now();
        let signature = audit::compute_signature(
            id,
            event_time,
            action,
            actor_id,
            target_id,
            payload_hash,
            &prev_sig,
        );

        guard.push(audit::AuditLogEntry {
            id,
            event_time,
            action: action.to_owned(),
            actor_id,
            target_id,
            payload_hash: payload_hash.to_vec(),
            prev_signature: prev_sig,
            signature,
        });
        Ok(())
    }
}
