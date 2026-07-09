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

/// Creates a mock `AppState` for testing.
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
            anthropic_api_key: String::new(),
            policy_matrix_version: "1.0.0".to_string(),
        },
        rate_limit: antigravity::config::RateLimitConfig {
            requests_per_second: 100,
            burst_size: 100,
        },
        integrations: antigravity::config::IntegrationsConfig {
            whoop_client_id: String::new(),
            whoop_client_secret: String::new(),
            whoop_authorize_url: "https://api.prod.whoop.com/oauth/oauth2/auth".to_string(),
            whoop_token_url: "https://api.prod.whoop.com/oauth/oauth2/token".to_string(),
            whoop_api_base: "https://api.prod.whoop.com/developer".to_string(),
            whoop_redirect_uri: "http://localhost:8443/api/v1/integrations/whoop/callback"
                .to_string(),
        },
    };

    let db = Arc::new(MockDatabase::new(&config.auth.server_secret));
    let nonce_cache = antigravity::crypto::nonce::NonceCache::new(config.auth.nonce_ttl_seconds);
    let hmac_key = ring::hmac::Key::new(
        ring::hmac::HMAC_SHA256,
        config.auth.server_secret.as_bytes(),
    );
    let oauth_state_key =
        antigravity::crypto::oauth_state::derive_oauth_state_key(&config.auth.server_secret);
    let token_vault_key =
        antigravity::crypto::token_vault::derive_token_vault_key(&config.auth.server_secret);
    let http_client = reqwest::Client::new();
    let doc_cache = moka::sync::Cache::new(100);

    let state = Arc::new(AppState {
        db: db.clone(),
        nonce_cache,
        config,
        http_client,
        hmac_key,
        oauth_state_key,
        token_vault_key,
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
    assert!(
        status == StatusCode::OK,
        "Request failed. Status: {status}. Body: {:?}",
        String::from_utf8_lossy(&body)
    );
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
    assert!(
        status == StatusCode::OK,
        "Sync failed. Status: {status}. Body: {:?}",
        String::from_utf8_lossy(&body)
    );

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
    let audit_key = antigravity::db::audit::derive_audit_key(&state.config.auth.server_secret);
    let recomputed_write_sig = antigravity::db::audit::compute_signature(
        &audit_key,
        &antigravity::db::audit::AuditRecordFields {
            id: write_log.id,
            event_time: write_log.event_time,
            action: &write_log.action,
            actor_id: write_log.actor_id,
            target_id: write_log.target_id,
            payload_hash: &write_log.payload_hash,
            prev_signature: &write_log.prev_signature,
        },
    );
    assert_eq!(write_log.signature, recomputed_write_sig);

    let recomputed_read_sig = antigravity::db::audit::compute_signature(
        &audit_key,
        &antigravity::db::audit::AuditRecordFields {
            id: read_log.id,
            event_time: read_log.event_time,
            action: &read_log.action,
            actor_id: read_log.actor_id,
            target_id: read_log.target_id,
            payload_hash: &read_log.payload_hash,
            prev_signature: &read_log.prev_signature,
        },
    );
    assert_eq!(read_log.signature, recomputed_read_sig);

    // ── 6. IDOR regression: a different authenticated device must not be able
    // to read this device's document or history ──────────────────────────────
    let other_device_id = Uuid::new_v4();
    let other_device = AttestedDevice {
        device_id: other_device_id,
        public_key_der: key_pair.public_key().as_ref().to_vec(),
        sign_counter: 0,
        registered_at: chrono::Utc::now(),
    };
    db.insert_device(&other_device).await.unwrap();
    let other_session_token =
        antigravity::crypto::session::create_session_token(&state.hmac_key, other_device_id, 3600)
            .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sync/document/{document_id}"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {other_session_token}"),
                )
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "a device must not be able to read another device's document"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sync/document/{document_id}/history"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {other_session_token}"),
                )
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "a device must not be able to read another device's document history"
    );
}

#[tokio::test]
async fn test_integrations_and_lab_results_flow() {
    let (state, db) = create_test_state();
    let app = create_router(state.clone());
    let mock_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12346));

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
    db.insert_device(&AttestedDevice {
        device_id,
        public_key_der,
        sign_counter: 0,
        registered_at: chrono::Utc::now(),
    })
    .await
    .unwrap();

    let session_token =
        antigravity::crypto::session::create_session_token(&state.hmac_key, device_id, 3600)
            .unwrap();

    let auth_header = format!("Bearer {session_token}");

    // ── 1. Connect Apple Health (on-device provider, no OAuth) ────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/apple_health/connect")
                .header(header::AUTHORIZATION, &auth_header)
                .header(header::CONTENT_TYPE, "application/json")
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::from(
                    serde_json::to_vec(&json!({ "authorized": true })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Whoop must not be connectable through the on-device /connect route.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/whoop/connect")
                .header(header::AUTHORIZATION, &auth_header)
                .header(header::CONTENT_TYPE, "application/json")
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::from(
                    serde_json::to_vec(&json!({ "authorized": true })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // ── 2. Whoop OAuth authorize (mocked — no client secret configured) ───────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/integrations/whoop/authorize")
                .header(header::AUTHORIZATION, &auth_header)
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
    let authorize_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(authorize_json["mock"], true);
    let authorize_url = authorize_json["authorize_url"].as_str().unwrap();
    let state_param = authorize_url.split("state=").nth(1).unwrap();

    // ── 3. Whoop callback (public route — no auth header, state proves identity) ──
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/integrations/whoop/callback?code=mock_dev_code&state={state_param}"
                ))
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // A forged callback with a tampered state must be rejected.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/integrations/whoop/callback?code=mock_dev_code&state={state_param}x"
                ))
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // ── 4. List connections — both providers now present ──────────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/integrations")
                .header(header::AUTHORIZATION, &auth_header)
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
    let list_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let providers: Vec<&str> = list_json["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["provider"].as_str().unwrap())
        .collect();
    assert!(providers.contains(&"apple_health"));
    assert!(providers.contains(&"whoop"));

    // ── 5. Whoop metrics (mock summary) ────────────────────────────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/integrations/whoop/metrics")
                .header(header::AUTHORIZATION, &auth_header)
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // ── 6. Disconnect Apple Health ──────────────────────────────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/integrations/apple_health")
                .header(header::AUTHORIZATION, &auth_header)
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // ── 7. Upload a doctor lab result as a categorized encrypted document ──────
    let document_id = Uuid::new_v4();
    let version_sequence: i64 = 1;
    let raw_payload = b"encrypted lab panel: lipid results";
    let iv = b"12byte_iv___";
    let auth_tag = b"16byte_authtag__";

    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(document_id.as_bytes());
    signed_data.extend_from_slice(&version_sequence.to_be_bytes());
    signed_data.extend_from_slice(raw_payload);
    signed_data.extend_from_slice(iv);
    signed_data.extend_from_slice(auth_tag);
    let signature = key_pair.sign(&rng, &signed_data).unwrap();

    let sync_payload = json!({
        "document_id": document_id,
        "version_sequence": version_sequence,
        "encrypted_blob": base64::prelude::BASE64_STANDARD.encode(raw_payload),
        "initialization_vector": base64::prelude::BASE64_STANDARD.encode(iv),
        "auth_tag": base64::prelude::BASE64_STANDARD.encode(auth_tag),
        "client_signature": base64::prelude::BASE64_STANDARD.encode(signature.as_ref()),
        "device_id": device_id,
        "document_type": "lab_result",
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/delta")
                .header(header::AUTHORIZATION, &auth_header)
                .header(header::CONTENT_TYPE, "application/json")
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::from(serde_json::to_vec(&sync_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // ── 8. List lab results — the categorized document shows up ────────────────
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sync/documents/lab_result")
                .header(header::AUTHORIZATION, &auth_header)
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
    let labs_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(labs_json["count"], 1);
    assert_eq!(
        labs_json["documents"][0]["document_id"],
        document_id.to_string()
    );

    // A document_type never uploaded returns an empty (not error) list.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sync/documents/medication_log")
                .header(header::AUTHORIZATION, &auth_header)
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
    let empty_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(empty_json["count"], 0);
}

#[tokio::test]
async fn test_device_registration_cannot_be_hijacked() {
    // Regression test: device_id is a client-chosen UUID that is NOT bound to
    // the attested key. Registration must not let a second party overwrite the
    // public key of an already-registered device_id (account takeover), while
    // still allowing idempotent same-key re-registration without resetting the
    // monotonic sign counter.
    let (_state, db) = create_test_state();

    let device_id = Uuid::new_v4();
    let key_a = vec![0x04u8; 65];
    let key_b = vec![0x05u8; 65];

    // First registration succeeds.
    db.insert_device(&AttestedDevice {
        device_id,
        public_key_der: key_a.clone(),
        sign_counter: 0,
        registered_at: chrono::Utc::now(),
    })
    .await
    .unwrap();

    // Advance the counter as a legitimate assertion would.
    db.update_counter(device_id, 7).await.unwrap();

    // Re-registration with the SAME key is idempotent and must preserve the
    // advanced counter (resetting it would reopen the assertion-replay window).
    db.insert_device(&AttestedDevice {
        device_id,
        public_key_der: key_a.clone(),
        sign_counter: 0,
        registered_at: chrono::Utc::now(),
    })
    .await
    .unwrap();
    let device = db.get_device(device_id).await.unwrap().unwrap();
    assert_eq!(
        device.sign_counter, 7,
        "same-key re-registration must not reset the sign counter"
    );
    assert_eq!(device.public_key_der, key_a);

    // Re-registration with a DIFFERENT key must be rejected, not silently
    // overwrite the victim's key.
    let hijack = db
        .insert_device(&AttestedDevice {
            device_id,
            public_key_der: key_b,
            sign_counter: 0,
            registered_at: chrono::Utc::now(),
        })
        .await;
    assert!(
        hijack.is_err(),
        "must not allow overwriting an existing device's key"
    );

    // The original key is intact.
    let device = db.get_device(device_id).await.unwrap().unwrap();
    assert_eq!(device.public_key_der, key_a);
    assert_eq!(device.sign_counter, 7);
}

#[tokio::test]
async fn test_insights_config_ships_rules_only() {
    // The insights config is what makes the on-device longevity features work
    // without breaking zero-knowledge: it must be reachable without a session
    // and must carry only rules (coefficients, reference ranges), never user
    // data.
    let (state, _db) = create_test_state();
    let app = create_router(state);
    let mock_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12347));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/insights/config")
                .extension(axum::extract::ConnectInfo(mock_addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 20000)
        .await
        .unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // The five rule blocks the client needs are all present.
    assert!(cfg["biological_age"]["signals"]["resting_heart_rate"].is_object());
    assert!(cfg["readiness"]["components"]["hrv"].is_object());
    assert!(cfg["biomarkers"]["ldl_cholesterol"].is_object());
    assert!(cfg["correlation"]["habits"]["winddown_routine"].is_object());
    assert!(cfg["circadian"]["chronotypes"]["neutral"].is_object());
    assert_eq!(cfg["version"], "1.0.0");
}
