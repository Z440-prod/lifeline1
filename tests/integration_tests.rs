use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use base64::Engine;
use ring::signature::KeyPair;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use antigravity::config::AppConfig;
use antigravity::db::{Database, MockDatabase};
use antigravity::models::device::AttestedDevice;
use antigravity::routes::create_router;
use antigravity::state::AppState;

/// Creates a mock AppState for testing.
fn create_test_state() -> (Arc<AppState>, Arc<MockDatabase>) {
    let config = AppConfig {
        server: antigravity::config::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8443,
        },
        database: antigravity::config::DatabaseConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 5,
        },
        auth: antigravity::config::AuthConfig {
            apple_team_id: "TESTTEAM".to_string(),
            apple_bundle_id: "com.test.app".to_string(),
            nonce_ttl_seconds: 60,
            session_token_ttl_seconds: 3600,
            server_secret: "super_secret_signing_key_at_least_32_bytes".to_string(),
            environment: "development".to_string(),
        },
        ai: antigravity::config::AiConfig {
            anthropic_api_url: "http://localhost:1234/v1/messages".to_string(),
            anthropic_api_key: "".to_string(),
            policy_matrix_version: "1.0.0".to_string(),
        },
        rate_limit: antigravity::config::RateLimitConfig {
            requests_per_second: 100,
            burst_size: 100,
        },
    };

    let db = Arc::new(MockDatabase::new());
    let nonce_cache = antigravity::crypto::nonce::NonceCache::new(config.auth.nonce_ttl_seconds);
    let hmac_key = ring::hmac::Key::new(
        ring::hmac::HMAC_SHA256,
        config.auth.server_secret.as_bytes(),
    );
    let http_client = reqwest::Client::new();
    let doc_cache = moka::sync::Cache::new(100);

    let state = Arc::new(AppState {
        db: db.clone(),
        nonce_cache,
        config,
        http_client,
        hmac_key,
        doc_cache,
    });

    (state, db)
}

#[tokio::test]
async fn test_end_to_end_flow() {
    let (state, db) = create_test_state();
    let app = create_router(state.clone());
    let mock_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12345));

    // ── 1. GET /api/v1/auth/challenge ─────────────────────────────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/challenge")
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 10000)
        .await
        .unwrap();
    if status != StatusCode::OK {
        panic!(
            "Request failed. Status: {status}. Body: {:?}",
            String::from_utf8_lossy(&body)
        );
    }
    let res_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let challenge = res_json["challenge"].as_str().expect("challenge missing");
    assert!(!challenge.is_empty());

    // ── 2. Setup Registered Device and Session Token ──────────────────────────
    let rng = ring::rand::SystemRandom::new();
    let pkcs8_bytes = ring::signature::EcdsaKeyPair::generate_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
        &rng,
    )
    .unwrap();
    let key_pair = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
        pkcs8_bytes.as_ref(),
        &rng,
    )
    .unwrap();
    let public_key_der = key_pair.public_key().as_ref().to_vec();

    let device_id = Uuid::new_v4();
    let device = AttestedDevice {
        device_id,
        public_key_der,
        sign_counter: 0,
        registered_at: chrono::Utc::now(),
    };
    db.insert_device(&device).await.unwrap();

    // Create session token signed with local hmac_key
    let session_token =
        antigravity::crypto::session::create_session_token(&state.hmac_key, device_id, 3600)
            .unwrap();

    // ── 3. POST /api/v1/sync/delta ────────────────────────────────────────────
    let document_id = Uuid::new_v4();
    let version_sequence: i64 = 1;
    let raw_payload = b"encrypted biometric health indicators";

    let iv = b"12byte_iv___";
    let auth_tag = b"16byte_authtag__";

    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(document_id.as_bytes());
    signed_data.extend_from_slice(&version_sequence.to_be_bytes());
    signed_data.extend_from_slice(raw_payload);
    signed_data.extend_from_slice(iv);
    signed_data.extend_from_slice(auth_tag);

    // Sign using ring KeyPair
    let signature = key_pair.sign(&rng, &signed_data).unwrap();
    let signature_base64 = base64::prelude::BASE64_STANDARD.encode(signature.as_ref());

    let sync_payload = json!({
        "document_id": document_id,
        "version_sequence": version_sequence,
        "encrypted_blob": base64::prelude::BASE64_STANDARD.encode(raw_payload),
        "initialization_vector": base64::prelude::BASE64_STANDARD.encode(iv),
        "auth_tag": base64::prelude::BASE64_STANDARD.encode(auth_tag),
        "client_signature": signature_base64,
        "device_id": device_id,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/delta")
                .header(header::AUTHORIZATION, format!("Bearer {session_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::from(serde_json::to_vec(&sync_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 10000)
        .await
        .unwrap();
    if status != StatusCode::OK {
        panic!(
            "Sync failed. Status: {status}. Body: {:?}",
            String::from_utf8_lossy(&body)
        );
    }

    // ── 4. GET /api/v1/sync/document/{id} (Assert Cache Hit) ──────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sync/document/{document_id}"))
                .header(header::AUTHORIZATION, format!("Bearer {session_token}"))
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 10000)
        .await
        .unwrap();
    let doc_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(doc_json["version_sequence"], version_sequence);

    // Verify document cache is populated
    assert!(state.doc_cache.contains_key(&document_id));

    // ── 5. Verify Cryptographic Audit Chain Linearity ─────────────────────────
    let logs = db.get_audit_logs();
    assert!(logs.len() >= 2);

    let write_log = &logs[0];
    assert_eq!(write_log.action, "WRITE_DOCUMENT");
    assert_eq!(write_log.actor_id, device_id);
    assert_eq!(write_log.target_id, document_id);

    let read_log = &logs[1];
    assert_eq!(read_log.action, "READ_DOCUMENT");
    assert_eq!(read_log.actor_id, device_id);
    assert_eq!(read_log.target_id, document_id);

    // Verify hash chain link
    assert_eq!(read_log.prev_signature, write_log.signature);

    // Recompute signatures to verify tamper-resistance integrity
    let recomputed_write_sig = antigravity::db::audit::compute_signature(
        write_log.id,
        write_log.event_time,
        &write_log.action,
        write_log.actor_id,
        write_log.target_id,
        &write_log.payload_hash,
        &write_log.prev_signature,
    );
    assert_eq!(write_log.signature, recomputed_write_sig);

    let recomputed_read_sig = antigravity::db::audit::compute_signature(
        read_log.id,
        read_log.event_time,
        &read_log.action,
        read_log.actor_id,
        read_log.target_id,
        &read_log.payload_hash,
        &read_log.prev_signature,
    );
    assert_eq!(read_log.signature, recomputed_read_sig);
}
