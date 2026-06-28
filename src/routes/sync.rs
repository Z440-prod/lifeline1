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

#[derive(Debug, Deserialize)]
pub struct SyncDeltaRequest {
    pub document_id: Uuid,
    pub version_sequence: i64,
    pub encrypted_blob: String,        // Base64 encoded
    pub initialization_vector: String, // Base64 encoded
    pub auth_tag: String,              // Base64 encoded
    pub client_signature: String,      // Base64 encoded (ECDSA ASN.1)
    pub device_id: Uuid,
}

/// Handler for `POST /api/v1/sync/delta`.
/// Receives an encrypted document version, verifies the device signature,
/// and stores the version using a SERIALIZABLE transaction.
pub async fn sync_delta_handler(
    State(state): State<Arc<AppState>>,
    Extension(verified_device): Extension<VerifiedDevice>,
    Json(payload): Json<SyncDeltaRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. Enforce that the authenticated device ID matches the payload's device ID
    if verified_device.device_id != payload.device_id {
        return Err(AppError::Unauthorized(
            "Authenticated device ID does not match request device_id".to_owned(),
        ));
    }

    // 2. Decode the incoming base64 fields
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

    // 3. Fetch device's registered public key
    let device = state
        .db
        .get_device(payload.device_id)
        .await?
        .ok_or(AppError::DeviceNotFound)?;

    // 4. Reconstruct the message over which the signature was generated
    // Message = document_id (16 bytes) || version_sequence (8 bytes BE) || encrypted_blob || IV || auth_tag
    let mut signed_data = Vec::with_capacity(
        16 + 8 + encrypted_blob_bytes.len() + iv_bytes.len() + auth_tag_bytes.len(),
    );
    signed_data.extend_from_slice(payload.document_id.as_bytes());
    signed_data.extend_from_slice(&payload.version_sequence.to_be_bytes());
    signed_data.extend_from_slice(&encrypted_blob_bytes);
    signed_data.extend_from_slice(&iv_bytes);
    signed_data.extend_from_slice(&auth_tag_bytes);

    // 5. Verify the signature using the device's public key
    let peer = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_ASN1,
        &device.public_key_der,
    );
    peer.verify(&signed_data, &signature_bytes).map_err(|_| {
        AppError::InvalidAssertion("Invalid client signature on sync payload".to_owned())
    })?;

    // 6. Assemble the model and upsert via serializable transaction with automatic retry
    let doc = SyncDocument {
        document_id: payload.document_id,
        device_id: payload.device_id,
        version_sequence: payload.version_sequence,
        encrypted_blob: encrypted_blob_bytes,
        initialization_vector: iv_bytes,
        auth_tag: auth_tag_bytes,
        client_signature: signature_bytes,
        created_at: chrono::Utc::now(),
    };

    state.db.upsert_sync_document(&doc).await?;

    Ok(Json(json!({
        "document_id": doc.document_id,
        "version_sequence": doc.version_sequence,
        "status": "synced"
    })))
}

/// Handler for `GET /api/v1/sync/document/:id`.
/// Retrieves the latest encrypted version of a document.
pub async fn get_document_handler(
    State(state): State<Arc<AppState>>,
    Extension(_verified_device): Extension<VerifiedDevice>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let doc = state
        .db
        .get_latest_document(document_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("Document not found".to_owned()))?;

    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;

    // Convert bytes back to base64 for JSON response
    Ok(Json(json!({
        "document_id": doc.document_id,
        "device_id": doc.device_id,
        "version_sequence": doc.version_sequence,
        "encrypted_blob": engine.encode(&doc.encrypted_blob),
        "initialization_vector": engine.encode(&doc.initialization_vector),
        "auth_tag": engine.encode(&doc.auth_tag),
        "client_signature": engine.encode(&doc.client_signature),
        "created_at": doc.created_at,
    })))
}

/// Handler for `GET /api/v1/sync/document/:id/history`.
/// Retrieves the full version history of a document (all versions, ascending).
pub async fn get_document_history_handler(
    State(state): State<Arc<AppState>>,
    Extension(_verified_device): Extension<VerifiedDevice>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let docs = state.db.get_document_history(document_id).await?;

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
                "created_at": doc.created_at,
            })
        })
        .collect();

    Ok(Json(json!({
        "document_id": document_id,
        "total_versions": versions.len(),
        "versions": versions,
    })))
}
