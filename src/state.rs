use ring::hmac;

use crate::config::AppConfig;
use crate::crypto::nonce::NonceCache;

/// Shared application state carried through every Axum handler via `State<Arc<AppState>>`.
pub struct AppState {
    /// Thread-safe database interface (Postgres or Mock fallback).
    pub db: std::sync::Arc<dyn crate::db::Database>,
    /// In-memory TTL cache for single-use challenge nonces.
    pub nonce_cache: NonceCache,
    /// Loaded application configuration.
    pub config: AppConfig,
    /// Reusable HTTP client for outbound calls (AI proxy, etc.).
    pub http_client: reqwest::Client,
    /// HMAC-SHA256 key derived from `config.auth.server_secret`.
    /// Used to sign and verify server-issued session tokens.
    pub hmac_key: hmac::Key,
    /// In-memory TTL/size cache for E2EE Sync Documents (cache-aside read optimizer)
    pub doc_cache: moka::sync::Cache<uuid::Uuid, crate::models::sync_document::SyncDocument>,
}
