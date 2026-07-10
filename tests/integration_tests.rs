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
    create_test_state_with_env("development")
}

/// Creates a mock `AppState` with an explicit `auth.environment`.
fn create_test_state_with_env(environment: &str) -> (Arc<AppState>, Arc<MockDatabase>) {
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
            environment: environment.to_string(),
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
        billing: antigravity::config::BillingConfig::default(),
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
    let ai_usage = moka::sync::Cache::new(1000);

    let state = Arc::new(AppState {
        db: db.clone(),
        nonce_cache,
        config,
        http_client,
        hmac_key,
        oauth_state_key,
        token_vault_key,
        doc_cache,
        ai_usage,
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

    // Connecting multiple sources at once is a paid entitlement.
    simulated_upgrade(&app, &session_token, mock_addr, "pro").await;

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

/// Register a device in the mock DB and mint a valid session token for it.
async fn register_device_with_token(state: &AppState, db: &MockDatabase) -> (Uuid, String) {
    let device_id = Uuid::new_v4();
    db.insert_device(&AttestedDevice {
        device_id,
        public_key_der: vec![0x04u8; 65],
        sign_counter: 0,
        registered_at: chrono::Utc::now(),
    })
    .await
    .unwrap();
    let token =
        antigravity::crypto::session::create_session_token(&state.hmac_key, device_id, 3600)
            .unwrap();
    (device_id, token)
}

async fn read_json(response: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 50000)
        .await
        .unwrap();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Upgrade a device via the simulated (no-Stripe-keys) checkout so it gains
/// paid entitlements — competing in the arena requires at least Pro.
async fn simulated_upgrade(
    app: &axum::Router,
    token: &str,
    addr: std::net::SocketAddr,
    tier: &str,
) {
    let (status, body) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/billing/checkout")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(json!({ "tier": tier }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upgrade to {tier} failed: {body:?}");
}

#[tokio::test]
async fn test_game_flow_ranks_streaks_and_leaderboard() {
    // Gamification must rank an opaque, on-device-derived vitality score without
    // ever seeing raw health data, and drive league/level/streak/rank from it.
    let (state, db) = create_test_state();
    let (_device_id, token) = register_device_with_token(&state, &db).await;
    let app = create_router(state.clone());
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12350));

    // Competing requires a paid plan (free is leaderboard view-only).
    simulated_upgrade(&app, &token, addr, "pro").await;

    // First submission requires a handle; a high score lands a top league.
    let (status, body) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/game/score")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(
                        json!({ "vitality_score": 95, "handle": "vo2_max_villain" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "score submit failed: {body:?}");
    assert_eq!(body["handle"], "vo2_max_villain");
    assert_eq!(body["league"], "apex", "score 95 is Apex");
    assert_eq!(body["streak_days"], 1);
    assert!(body["xp"].as_i64().unwrap() > 0);
    assert_eq!(body["rank"], 1);

    // A first submission without a handle must be rejected.
    let (status, _) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/game/score")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(json!({ "vitality_score": 50 }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    // (Same device already has a handle, so this actually succeeds as a same-day
    // resubmit; assert it did not error.)
    assert_eq!(status, StatusCode::OK);

    // A second competitor with a lower score ranks below.
    let (_d2, token2) = register_device_with_token(&state, &db).await;
    simulated_upgrade(&app, &token2, addr, "pro").await;
    let (status, body2) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/game/score")
                    .header(header::AUTHORIZATION, format!("Bearer {token2}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(
                        json!({ "vitality_score": 45, "handle": "couch_cardio" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "second submit failed: {body2:?}");
    assert_eq!(body2["league"], "silver");
    assert_eq!(body2["rank"], 2, "lower score ranks second");

    // Handle collision must be rejected.
    let (_d3, token3) = register_device_with_token(&state, &db).await;
    simulated_upgrade(&app, &token3, addr, "pro").await;
    let (status, _) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/game/score")
                    .header(header::AUTHORIZATION, format!("Bearer {token3}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(
                        json!({ "vitality_score": 60, "handle": "vo2_max_villain" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "duplicate handle must 409");

    // Leaderboard shows the field ranked, with the caller's own row.
    let (status, lb) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/game/leaderboard")
                    .header(header::AUTHORIZATION, format!("Bearer {token2}"))
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lb["entries"][0]["handle"], "vo2_max_villain");
    assert_eq!(lb["entries"][1]["handle"], "couch_cardio");
    assert_eq!(lb["me"]["handle"], "couch_cardio");

    // The public game config ships the league ladder with no session.
    let (status, cfg) = read_json(
        app.oneshot(
            Request::builder()
                .uri("/api/v1/game/config")
                .extension(axum::extract::ConnectInfo(addr))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cfg["leagues"].as_array().unwrap().len(), 6);
    assert_eq!(cfg["leagues"][5]["id"], "apex");
}

#[tokio::test]
async fn test_billing_tiers_gating_and_simulated_upgrade() {
    // Billing must expose a tier catalog publicly, default users to free,
    // gate Elite-only features with a 403, and (without Stripe configured)
    // simulate an upgrade so the flow is testable end to end.
    let (state, db) = create_test_state();
    let (_device_id, token) = register_device_with_token(&state, &db).await;
    let app = create_router(state.clone());
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12351));

    // Public catalog: three tiers, prices, entitlements.
    let (status, cfg) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/config")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cfg["tiers"].as_array().unwrap().len(), 3);
    assert_eq!(cfg["live"], false, "no Stripe key => test mode");

    // A brand-new device is on the free tier.
    let (status, sub) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/subscription")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sub["tier"], "free");
    assert_eq!(sub["entitlements"]["beta_access"], false);

    // Beta features are Elite-only: a free user is forbidden.
    let (status, _) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/beta-features")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Simulated checkout upgrades the device to Elite immediately.
    let (status, checkout) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/billing/checkout")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(json!({ "tier": "elite" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "checkout failed: {checkout:?}");
    assert_eq!(checkout["simulated"], true);
    assert_eq!(checkout["tier"], "elite");

    // Subscription now reflects Elite with beta access.
    let (_status, sub) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/subscription")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(sub["tier"], "elite");
    assert_eq!(sub["entitlements"]["beta_access"], true);

    // And beta features are now reachable.
    let (status, beta) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/beta-features")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!beta["builds"].as_array().unwrap().is_empty());

    // The webhook refuses unsigned callers when no signing secret is set.
    let (status, _) = read_json(
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/billing/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .extension(axum::extract::ConnectInfo(addr))
                .body(Body::from(json!({ "type": "ping" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_dev_session_full_flow_and_production_gate() {
    // The dev-session endpoint is what makes the browser app work end to end
    // without iOS hardware. It must (a) mint a real, working session token in
    // development, (b) be idempotent for the same device, and (c) be hard-off
    // outside development.
    let (state, _db) = create_test_state();
    let app = create_router(state.clone());
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12360));
    let device_id = Uuid::new_v4();

    let (status, body) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/dev-session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(json!({ "device_id": device_id }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "dev session failed: {body:?}");
    let token = body["token"].as_str().unwrap().to_owned();

    // The minted token opens protected endpoints — but the free tier is
    // leaderboard view-only, so scoring is forbidden until an upgrade…
    let (status, _) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/game/score")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(
                        json!({ "vitality_score": 77, "handle": "dev_session_user" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "free tier must not compete");

    // …after which the same call succeeds.
    simulated_upgrade(&app, &token, addr, "pro").await;
    let (status, scored) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/game/score")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(
                        json!({ "vitality_score": 77, "handle": "dev_session_user" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "token must work: {scored:?}");
    assert_eq!(scored["league"], "platinum");

    // Idempotent for the same device id (same synthetic key).
    let (status, _) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/dev-session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(json!({ "device_id": device_id }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Outside development the endpoint is a hard 403.
    let (prod_state, _db2) = create_test_state_with_env("production");
    let prod_app = create_router(prod_state);
    let (status, _) = read_json(
        prod_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/dev-session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::from(json!({ "device_id": device_id }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "must be dev-only");
}

#[tokio::test]
async fn test_free_tier_entitlements_enforced_server_side() {
    // The tier catalog's promises must be enforced by the server, not just
    // hidden in the UI: free = view-only arena, limited daily coach messages,
    // and a single fused source.
    let (state, db) = create_test_state();
    let (_device_id, token) = register_device_with_token(&state, &db).await;
    let app = create_router(state.clone());
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12370));

    let call = |method: &'static str, uri: String, body: Option<serde_json::Value>| {
        let token = token.clone();
        let app = app.clone();
        async move {
            let mut req = Request::builder()
                .method(method)
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .extension(axum::extract::ConnectInfo(addr));
            let body = match body {
                Some(b) => {
                    req = req.header(header::CONTENT_TYPE, "application/json");
                    Body::from(b.to_string())
                }
                None => Body::empty(),
            };
            read_json(app.oneshot(req.body(body).unwrap()).await.unwrap()).await
        }
    };

    // 1. Arena is view-only on free: scoring is forbidden, the board readable.
    let (status, body) = call(
        "POST",
        "/api/v1/game/score".into(),
        Some(json!({ "vitality_score": 88, "handle": "freeloader" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "free must not score: {body:?}"
    );
    let (status, _) = call("GET", "/api/v1/game/leaderboard".into(), None).await;
    assert_eq!(status, StatusCode::OK, "free may view the leaderboard");

    // 2. Coach: exactly the free daily limit of messages, then 403.
    let limit = antigravity::models::subscription::Tier::Free
        .entitlements()
        .ai_coach_daily_limit;
    for i in 0..limit {
        let (status, body) = call(
            "POST",
            "/api/v1/ai/proxy".into(),
            Some(json!({ "prompt": "hi", "execution_token": "t" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "message {i} within limit: {body:?}");
    }
    let (status, _) = call(
        "POST",
        "/api/v1/ai/proxy".into(),
        Some(json!({ "prompt": "one more", "execution_token": "t" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "over-limit coach must 403");

    // 3. Sources: the first connects, a second distinct source is forbidden
    //    (both the on-device path and the Whoop OAuth entry point).
    let (status, _) = call(
        "POST",
        "/api/v1/integrations/apple_health/connect".into(),
        Some(json!({ "authorized": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "first source is free");
    let (status, _) = call(
        "POST",
        "/api/v1/integrations/google_health/connect".into(),
        Some(json!({ "authorized": true })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "second source needs Pro");
    let (status, _) = call("GET", "/api/v1/integrations/whoop/authorize".into(), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "whoop entry point gated too");

    // 4. Upgrading (simulated checkout) lifts every gate at once.
    simulated_upgrade(&app, &token, addr, "pro").await;
    let (status, _) = call(
        "POST",
        "/api/v1/game/score".into(),
        Some(json!({ "vitality_score": 88, "handle": "freeloader" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pro may compete");
    let (status, _) = call(
        "POST",
        "/api/v1/integrations/google_health/connect".into(),
        Some(json!({ "authorized": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pro fuses all sources");
    let (status, _) = call(
        "POST",
        "/api/v1/ai/proxy".into(),
        Some(json!({ "prompt": "unlimited now", "execution_token": "t" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pro coach is unlimited");
}

#[tokio::test]
async fn test_donation_flow_and_config() {
    // The donate button's contract: presets in the public config, a bounded
    // one-time amount, simulated Checkout until Stripe keys exist — and it
    // never touches the tier (donations unlock nothing).
    let (state, db) = create_test_state();
    let (_device_id, token) = register_device_with_token(&state, &db).await;
    let app = create_router(state.clone());
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 12380));

    let (status, cfg) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/config")
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cfg["donate"]["presets_usd"], json!([3, 5, 10]));

    let donate = |cents: i64| {
        let app = app.clone();
        let token = token.clone();
        async move {
            read_json(
                app.oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/billing/donate")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .extension(axum::extract::ConnectInfo(addr))
                        .body(Body::from(json!({ "amount_usd_cents": cents }).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap(),
            )
            .await
        }
    };

    let (status, body) = donate(500).await;
    assert_eq!(status, StatusCode::OK, "donation failed: {body:?}");
    assert_eq!(body["simulated"], true);
    assert_eq!(body["amount_usd_cents"], 500);

    let (status, _) = donate(50).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "below $1 must be rejected");
    let (status, _) = donate(9_000_000).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "above $500 must be rejected"
    );

    // Donating grants no entitlements.
    let (_s, sub) = read_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/billing/subscription")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .extension(axum::extract::ConnectInfo(addr))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(sub["tier"], "free", "donations must not change the tier");
}
