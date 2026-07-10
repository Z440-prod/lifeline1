use axum::{
    extract::{Path, State},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::attest_guard::VerifiedDevice;
use crate::models::sync_document::SyncDocument;
use crate::state::AppState;

const DEFAULT_DOCUMENT_TYPE: &str = "generic";
const MAX_DOCUMENT_TYPE_LEN: usize = 30;

/// Validates a client-supplied document type: lowercase ASCII letters and
/// underscores only, bounded length. This is purely a UI grouping label the
/// server stores but never interprets, so the only real requirement is that
/// it can't be used to smuggle oversized or malformed data into the column.
fn validate_document_type(document_type: &str) -> Result<(), AppError> {
    if document_type.is_empty() || document_type.len() > MAX_DOCUMENT_TYPE_LEN {
        return Err(AppError::BadRequest(format!(
            "document_type must be 1-{MAX_DOCUMENT_TYPE_LEN} characters"
        )));
    }
    if !document_type
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b == b'_')
    {
        return Err(AppError::BadRequest(
            "document_type must contain only lowercase letters and underscores".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SyncDeltaRequest {
    pub document_id: Uuid,
    pub version_sequence: i64,
    pub encrypted_blob: String,        // Base64 encoded
    pub initialization_vector: String, // Base64 encoded
    pub auth_tag: String,              // Base64 encoded
    pub client_signature: String,      // Base64 encoded (ECDSA ASN.1)
    pub device_id: Uuid,
    /// Client-assigned category, e.g. "`lab_result`". Defaults to "generic".
    #[serde(default)]
    pub document_type: Option<String>,
}

/// Handler for `POST /api/v1/sync/delta`.
/// Receives an encrypted document version, verifies the device signature,
/// and stores the version using a SERIALIZABLE transaction.
#[tracing::instrument(skip(state, payload), fields(device_id = %payload.device_id, document_id = %payload.document_id, version_sequence = payload.version_sequence))]
pub async fn sync_delta_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Json(payload): Json<SyncDeltaRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/sync/delta").increment(1);
    let start_time = std::time::Instant::now();

    // 1. Enforce that the authenticated device ID matches the payload's device ID
    if verified_device.device_id != payload.device_id {
        metrics::counter!("antigravity_api_errors_total", "endpoint" => "/sync/delta", "error" => "unauthorized").increment(1);
        return Err(AppError::Unauthorized(
            "Authenticated device ID does not match request device_id".to_owned(),
        ));
    }

    // 2. Validate the document_type category label, if supplied
    let document_type = payload
        .document_type
        .clone()
        .unwrap_or_else(|| DEFAULT_DOCUMENT_TYPE.to_owned());
    validate_document_type(&document_type)?;

    // 3. Decode the incoming base64 fields
    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;

    let encrypted_blob_bytes = engine
        .decode(&payload.encrypted_blob)
        .map_err(|e| AppError::BadRequest(format!("Invalid encrypted_blob encoding: {e}")))?;

    let iv_bytes = engine.decode(&payload.initialization_vector).map_err(|e| {
        AppError::BadRequest(format!("Invalid initialization_vector encoding: {e}"))
    })?;

    let auth_tag_bytes = engine
        .decode(&payload.auth_tag)
        .map_err(|e| AppError::BadRequest(format!("Invalid auth_tag encoding: {e}")))?;

    let signature_bytes = engine
        .decode(&payload.client_signature)
        .map_err(|e| AppError::BadRequest(format!("Invalid client_signature encoding: {e}")))?;

    // 4. Fetch device's registered public key
    let device = state
        .db
        .get_device(payload.device_id)
        .await?
        .ok_or(AppError::DeviceNotFound)?;

    // 5. Reconstruct the message over which the signature was generated
    let mut signed_data = Vec::with_capacity(
        16 + 8 + encrypted_blob_bytes.len() + iv_bytes.len() + auth_tag_bytes.len(),
    );
    signed_data.extend_from_slice(payload.document_id.as_bytes());
    signed_data.extend_from_slice(&payload.version_sequence.to_be_bytes());
    signed_data.extend_from_slice(&encrypted_blob_bytes);
    signed_data.extend_from_slice(&iv_bytes);
    signed_data.extend_from_slice(&auth_tag_bytes);

    // 6. Verify the signature using the device's public key
    let peer = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_ASN1,
        &device.public_key_der,
    );
    peer.verify(&signed_data, &signature_bytes).map_err(|_| {
        AppError::InvalidAssertion("Invalid client signature on sync payload".to_owned())
    })?;

    // 7. Assemble the model and upsert via serializable transaction with automatic retry
    let doc = SyncDocument {
        document_id: payload.document_id,
        device_id: payload.device_id,
        version_sequence: payload.version_sequence,
        encrypted_blob: encrypted_blob_bytes,
        initialization_vector: iv_bytes,
        auth_tag: auth_tag_bytes,
        client_signature: signature_bytes,
        document_type,
        created_at: chrono::Utc::now(),
    };

    let db_start = std::time::Instant::now();
    state.db.upsert_sync_document(&doc).await?;
    metrics::histogram!("antigravity_db_latency_seconds", "operation" => "upsert_sync_document")
        .record(db_start.elapsed().as_secs_f64());

    // 8. Write-through to the memory Cache
    state.doc_cache.insert(doc.document_id, doc.clone());
    metrics::counter!("antigravity_cache_updates_total", "operation" => "sync_delta").increment(1);

    // 9. Record cryptographically chained Audit Log
    let payload_hash = {
        use ring::digest::{digest, SHA256};
        digest(&SHA256, &doc.encrypted_blob).as_ref().to_vec()
    };
    state
        .db
        .insert_audit_log(
            "WRITE_DOCUMENT",
            verified_device.device_id,
            doc.document_id,
            &payload_hash,
        )
        .await?;

    metrics::histogram!("antigravity_request_duration_seconds", "endpoint" => "/sync/delta")
        .record(start_time.elapsed().as_secs_f64());

    Ok(Json(json!({
        "document_id": doc.document_id,
        "version_sequence": doc.version_sequence,
        "document_type": doc.document_type,
        "status": "synced"
    })))
}

/// Handler for `GET /api/v1/sync/document/:id`.
/// Retrieves the latest encrypted version of a document.
#[tracing::instrument(skip(state), fields(document_id = %document_id))]
pub async fn get_document_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/sync/document/{id}")
        .increment(1);
    let start_time = std::time::Instant::now();

    // 1. Cache-aside Lookup
    metrics::counter!("antigravity_cache_requests_total", "endpoint" => "get_document")
        .increment(1);
    let doc = if let Some(cached_doc) = state.doc_cache.get(&document_id) {
        metrics::counter!("antigravity_cache_hits_total", "endpoint" => "get_document")
            .increment(1);
        cached_doc
    } else {
        metrics::counter!("antigravity_cache_misses_total", "endpoint" => "get_document")
            .increment(1);

        let db_start = std::time::Instant::now();
        let db_doc = state
            .db
            .get_latest_document(document_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Document not found".to_owned()))?;
        metrics::histogram!("antigravity_db_latency_seconds", "operation" => "get_latest_document")
            .record(db_start.elapsed().as_secs_f64());

        // Pop cache with retrieved document
        state.doc_cache.insert(document_id, db_doc.clone());
        db_doc
    };

    // 2. Enforce that the requesting device owns this document.
    // Without this check any authenticated device could read another
    // device's document metadata/ciphertext and history by guessing/enumerating a UUID.
    // Return the same "not found" error as a missing document so the response
    // does not act as an oracle for document existence/ownership.
    if doc.device_id != verified_device.device_id {
        metrics::counter!("antigravity_api_errors_total", "endpoint" => "/sync/document/{id}", "error" => "unauthorized").increment(1);
        return Err(AppError::BadRequest("Document not found".to_owned()));
    }

    // 3. Log compliance Audit Trail
    let payload_hash = {
        use ring::digest::{digest, SHA256};
        digest(&SHA256, &doc.encrypted_blob).as_ref().to_vec()
    };
    state
        .db
        .insert_audit_log(
            "READ_DOCUMENT",
            verified_device.device_id,
            document_id,
            &payload_hash,
        )
        .await?;

    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;

    metrics::histogram!("antigravity_request_duration_seconds", "endpoint" => "/sync/document/{id}").record(start_time.elapsed().as_secs_f64());

    // Convert bytes back to base64 for JSON response
    Ok(Json(json!({
        "document_id": doc.document_id,
        "device_id": doc.device_id,
        "version_sequence": doc.version_sequence,
        "encrypted_blob": engine.encode(&doc.encrypted_blob),
        "initialization_vector": engine.encode(&doc.initialization_vector),
        "auth_tag": engine.encode(&doc.auth_tag),
        "client_signature": engine.encode(&doc.client_signature),
        "document_type": doc.document_type,
        "created_at": doc.created_at,
    })))
}

/// Handler for `GET /api/v1/sync/document/:id/history`.
/// Retrieves the full version history of a document (all versions, ascending).
#[tracing::instrument(skip(state), fields(document_id = %document_id))]
pub async fn get_document_history_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/sync/document/{id}/history").increment(1);
    let start_time = std::time::Instant::now();

    let db_start = std::time::Instant::now();
    let docs = state.db.get_document_history(document_id).await?;
    metrics::histogram!("antigravity_db_latency_seconds", "operation" => "get_document_history")
        .record(db_start.elapsed().as_secs_f64());

    // Enforce that the requesting device owns this document (same IDOR
    // protection as get_document_handler). An empty history is indistinguishable
    // from an unowned one to avoid leaking existence to non-owners.
    if !docs
        .iter()
        .all(|doc| doc.device_id == verified_device.device_id)
    {
        metrics::counter!("antigravity_api_errors_total", "endpoint" => "/sync/document/{id}/history", "error" => "unauthorized").increment(1);
        return Err(AppError::BadRequest("Document not found".to_owned()));
    }

    // Audit log history read action
    state
        .db
        .insert_audit_log(
            "READ_DOCUMENT_HISTORY",
            verified_device.device_id,
            document_id,
            &[],
        )
        .await?;

    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;

    let versions: Vec<serde_json::Value> = docs
        .iter()
        .map(|doc| {
            json!({
                "document_id": doc.document_id,
                "device_id": doc.device_id,
                "version_sequence": doc.version_sequence,
                "encrypted_blob": engine.encode(&doc.encrypted_blob),
                "initialization_vector": engine.encode(&doc.initialization_vector),
                "auth_tag": engine.encode(&doc.auth_tag),
                "client_signature": engine.encode(&doc.client_signature),
                "document_type": doc.document_type,
                "created_at": doc.created_at,
            })
        })
        .collect();

    metrics::histogram!("antigravity_request_duration_seconds", "endpoint" => "/sync/document/{id}/history").record(start_time.elapsed().as_secs_f64());

    Ok(Json(json!({
        "document_id": document_id,
        "total_versions": versions.len(),
        "versions": versions,
    })))
}

/// Handler for `GET /api/v1/sync/documents/:document_type`.
/// Lists the latest version of every document the authenticated device owns
/// in a given category — e.g. `lab_result` for the Lab Results screen. Since
/// documents are already scoped to `verified_device.device_id` at the query
/// level, there is no cross-device data to filter out here.
#[tracing::instrument(skip(state), fields(document_type = %document_type))]
pub async fn list_documents_by_type_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Path(document_type): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    metrics::counter!("antigravity_api_requests_total", "endpoint" => "/sync/documents/{type}")
        .increment(1);
    let start_time = std::time::Instant::now();

    validate_document_type(&document_type)?;

    let db_start = std::time::Instant::now();
    let mut docs = state
        .db
        .list_documents_by_type(verified_device.device_id, &document_type)
        .await?;
    metrics::histogram!("antigravity_db_latency_seconds", "operation" => "list_documents_by_type")
        .record(db_start.elapsed().as_secs_f64());

    // Tier history window: the free plan lists the last N days of documents
    // (the ciphertext itself is never deleted — upgrading restores the full
    // timeline instantly).
    let tier =
        crate::routes::billing::effective_tier(state.as_ref(), verified_device.device_id).await?;
    let history_days = tier.entitlements().history_days;
    if history_days >= 0 {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(history_days));
        docs.retain(|d| d.created_at >= cutoff);
    }

    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;

    let documents: Vec<serde_json::Value> = docs
        .iter()
        .map(|doc| {
            json!({
                "document_id": doc.document_id,
                "version_sequence": doc.version_sequence,
                "encrypted_blob": engine.encode(&doc.encrypted_blob),
                "initialization_vector": engine.encode(&doc.initialization_vector),
                "auth_tag": engine.encode(&doc.auth_tag),
                "client_signature": engine.encode(&doc.client_signature),
                "document_type": doc.document_type,
                "created_at": doc.created_at,
            })
        })
        .collect();

    metrics::histogram!("antigravity_request_duration_seconds", "endpoint" => "/sync/documents/{type}").record(start_time.elapsed().as_secs_f64());

    Ok(Json(json!({
        "document_type": document_type,
        "count": documents.len(),
        "documents": documents,
    })))
}
