use crate::errors::AppError;
use crate::models::device::AttestedDevice;
use crate::models::provider_connection::ProviderConnection;
use crate::models::sync_document::SyncDocument;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

pub mod audit;
pub mod devices;
pub mod integrations;
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
    async fn list_documents_by_type(
        &self,
        device_id: Uuid,
        document_type: &str,
    ) -> Result<Vec<SyncDocument>, AppError>;
    async fn insert_audit_log(
        &self,
        action: &str,
        actor_id: Uuid,
        target_id: Uuid,
        payload_hash: &[u8],
    ) -> Result<(), AppError>;

    async fn list_provider_connections(
        &self,
        device_id: Uuid,
    ) -> Result<Vec<ProviderConnection>, AppError>;
    async fn upsert_provider_connection(
        &self,
        device_id: Uuid,
        provider: &str,
        status: &str,
        external_account_id: Option<&str>,
        encrypted_refresh_token: Option<&[u8]>,
    ) -> Result<(), AppError>;
    async fn delete_provider_connection(
        &self,
        device_id: Uuid,
        provider: &str,
    ) -> Result<(), AppError>;
    async fn get_encrypted_refresh_token(
        &self,
        device_id: Uuid,
        provider: &str,
    ) -> Result<Option<Vec<u8>>, AppError>;
    async fn touch_last_synced(&self, device_id: Uuid, provider: &str) -> Result<(), AppError>;
}

/// Postgres implementation of the Database trait.
pub struct PostgresDatabase {
    pub pool: sqlx::PgPool,
    audit_key: ring::hmac::Key,
}

impl PostgresDatabase {
    #[must_use]
    pub fn new(pool: sqlx::PgPool, server_secret: &str) -> Self {
        Self {
            pool,
            audit_key: audit::derive_audit_key(server_secret),
        }
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
            &self.audit_key,
            &audit::AuditRecordFields {
                id,
                event_time,
                action,
                actor_id,
                target_id,
                payload_hash,
                prev_signature: &prev_sig,
            },
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

    async fn list_documents_by_type(
        &self,
        device_id: Uuid,
        document_type: &str,
    ) -> Result<Vec<SyncDocument>, AppError> {
        sync::list_latest_documents_by_type(&self.pool, device_id, document_type).await
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
                    tokio::time::sleep(std::time::Duration::from_millis(5 * u64::from(attempts)))
                        .await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn list_provider_connections(
        &self,
        device_id: Uuid,
    ) -> Result<Vec<ProviderConnection>, AppError> {
        integrations::list_provider_connections(&self.pool, device_id).await
    }

    async fn upsert_provider_connection(
        &self,
        device_id: Uuid,
        provider: &str,
        status: &str,
        external_account_id: Option<&str>,
        encrypted_refresh_token: Option<&[u8]>,
    ) -> Result<(), AppError> {
        integrations::upsert_provider_connection(
            &self.pool,
            device_id,
            provider,
            status,
            external_account_id,
            encrypted_refresh_token,
        )
        .await
    }

    async fn delete_provider_connection(
        &self,
        device_id: Uuid,
        provider: &str,
    ) -> Result<(), AppError> {
        integrations::delete_provider_connection(&self.pool, device_id, provider).await
    }

    async fn get_encrypted_refresh_token(
        &self,
        device_id: Uuid,
        provider: &str,
    ) -> Result<Option<Vec<u8>>, AppError> {
        integrations::get_encrypted_refresh_token(&self.pool, device_id, provider).await
    }

    async fn touch_last_synced(&self, device_id: Uuid, provider: &str) -> Result<(), AppError> {
        integrations::touch_last_synced(&self.pool, device_id, provider).await
    }
}

/// A stored provider connection paired with its encrypted refresh token (if
/// any). Kept separate from the public `ProviderConnection` type, mirroring
/// how the Postgres schema never returns the token over the API.
type ProviderConnectionRecord = (ProviderConnection, Option<Vec<u8>>);

/// In-memory thread-safe Mock database for offline development and local debugging.
pub struct MockDatabase {
    devices: Mutex<HashMap<Uuid, AttestedDevice>>,
    documents: Mutex<HashMap<Uuid, Vec<SyncDocument>>>,
    audit_logs: Mutex<Vec<audit::AuditLogEntry>>,
    audit_key: ring::hmac::Key,
    // Keyed by (device_id, provider).
    provider_connections: Mutex<HashMap<(Uuid, String), ProviderConnectionRecord>>,
}

impl MockDatabase {
    #[must_use]
    pub fn new(server_secret: &str) -> Self {
        Self {
            devices: Mutex::new(HashMap::new()),
            documents: Mutex::new(HashMap::new()),
            audit_logs: Mutex::new(Vec::new()),
            audit_key: audit::derive_audit_key(server_secret),
            provider_connections: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_audit_logs(&self) -> Vec<audit::AuditLogEntry> {
        let guard = self.audit_logs.lock().unwrap();
        guard.clone()
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

    async fn list_documents_by_type(
        &self,
        device_id: Uuid,
        document_type: &str,
    ) -> Result<Vec<SyncDocument>, AppError> {
        let guard = self.documents.lock().unwrap();
        let docs = guard
            .values()
            .filter_map(|history| {
                history
                    .iter()
                    .filter(|d| d.device_id == device_id && d.document_type == document_type)
                    .max_by_key(|d| d.version_sequence)
                    .cloned()
            })
            .collect();
        Ok(docs)
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
            .map_or_else(|| vec![0u8; 32], |entry| entry.signature.clone());

        let id = Uuid::new_v4();
        let event_time = chrono::Utc::now();
        let signature = audit::compute_signature(
            &self.audit_key,
            &audit::AuditRecordFields {
                id,
                event_time,
                action,
                actor_id,
                target_id,
                payload_hash,
                prev_signature: &prev_sig,
            },
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

    async fn list_provider_connections(
        &self,
        device_id: Uuid,
    ) -> Result<Vec<ProviderConnection>, AppError> {
        let guard = self.provider_connections.lock().unwrap();
        let mut conns: Vec<ProviderConnection> = guard
            .values()
            .filter(|(conn, _)| conn.device_id == device_id)
            .map(|(conn, _)| conn.clone())
            .collect();
        conns.sort_by(|a, b| a.provider.cmp(&b.provider));
        Ok(conns)
    }

    async fn upsert_provider_connection(
        &self,
        device_id: Uuid,
        provider: &str,
        status: &str,
        external_account_id: Option<&str>,
        encrypted_refresh_token: Option<&[u8]>,
    ) -> Result<(), AppError> {
        let mut guard = self.provider_connections.lock().unwrap();
        let key = (device_id, provider.to_owned());
        let existing = guard.get(&key);

        let connected_at = existing.map_or_else(chrono::Utc::now, |(conn, _)| conn.connected_at);
        let external_account_id = external_account_id
            .map(str::to_owned)
            .or_else(|| existing.and_then(|(conn, _)| conn.external_account_id.clone()));
        let token = encrypted_refresh_token
            .map(<[u8]>::to_vec)
            .or_else(|| existing.and_then(|(_, tok)| tok.clone()));
        let last_synced_at = existing.and_then(|(conn, _)| conn.last_synced_at);

        guard.insert(
            key,
            (
                ProviderConnection {
                    device_id,
                    provider: provider.to_owned(),
                    status: status.to_owned(),
                    external_account_id,
                    connected_at,
                    last_synced_at,
                },
                token,
            ),
        );
        Ok(())
    }

    async fn delete_provider_connection(
        &self,
        device_id: Uuid,
        provider: &str,
    ) -> Result<(), AppError> {
        let mut guard = self.provider_connections.lock().unwrap();
        guard.remove(&(device_id, provider.to_owned()));
        Ok(())
    }

    async fn get_encrypted_refresh_token(
        &self,
        device_id: Uuid,
        provider: &str,
    ) -> Result<Option<Vec<u8>>, AppError> {
        let guard = self.provider_connections.lock().unwrap();
        Ok(guard
            .get(&(device_id, provider.to_owned()))
            .and_then(|(_, tok)| tok.clone()))
    }

    async fn touch_last_synced(&self, device_id: Uuid, provider: &str) -> Result<(), AppError> {
        let mut guard = self.provider_connections.lock().unwrap();
        if let Some((conn, _)) = guard.get_mut(&(device_id, provider.to_owned())) {
            conn.last_synced_at = Some(chrono::Utc::now());
            Ok(())
        } else {
            Err(AppError::BadRequest(
                "No provider connection to update".to_owned(),
            ))
        }
    }
}
