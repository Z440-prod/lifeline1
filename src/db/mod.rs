use crate::errors::AppError;
use crate::models::device::AttestedDevice;
use crate::models::sync_document::SyncDocument;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

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
}

/// Postgres implementation of the Database trait.
pub struct PostgresDatabase {
    pub pool: sqlx::PgPool,
}

impl PostgresDatabase {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
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
}

/// In-memory thread-safe Mock database for offline development and local debugging.
pub struct MockDatabase {
    devices: Mutex<HashMap<Uuid, AttestedDevice>>,
    documents: Mutex<HashMap<Uuid, Vec<SyncDocument>>>,
}

impl MockDatabase {
    pub fn new() -> Self {
        Self {
            devices: Mutex::new(HashMap::new()),
            documents: Mutex::new(HashMap::new()),
        }
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
}
