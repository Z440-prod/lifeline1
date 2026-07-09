use crate::config::AppConfig;
use crate::errors::AppError;
use ring::digest::{digest, SHA256};

/// Output of verified assertion containing the updated monotonic counter.
pub struct VerifiedAssertion {
    pub new_counter: i64,
}

/// Verify Apple App Attest assertion.
/// Steps:
/// 1. Decode CBOR assertion object to extract authenticatorData and signature
/// 2. Parse authenticatorData to verify rpIdHash and flags
/// 3. Verify that the assertion's signCount is strictly greater than the `stored_counter`
/// 4. Reconstruct clientDataHash = SHA-256(challenge || `request_body_hash`)
/// 5. Verify ECDSA signature over (authenticatorData || clientDataHash)
pub fn verify_assertion(
    config: &AppConfig,
    assertion_cbor_base64: &str,
    challenge_hex: &str,
    request_body_hash_hex: &str,
    public_key_der: &[u8],
    stored_counter: i64,
) -> Result<VerifiedAssertion, AppError> {
    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    let assertion_bytes = engine
        .decode(assertion_cbor_base64)
        .map_err(|e| AppError::BadRequest(format!("Base64 decode assertion failed: {e}")))?;

    // 1. CBOR decode
    let decoded_val: ciborium::Value = ciborium::from_reader(assertion_bytes.as_slice())
        .map_err(|e| AppError::BadRequest(format!("CBOR decode assertion object failed: {e}")))?;

    let map = match decoded_val {
        ciborium::Value::Map(m) => m,
        _ => {
            return Err(AppError::InvalidAssertion(
                "Assertion object is not a CBOR map".to_owned(),
            ))
        }
    };

    let mut signature = None;
    let mut authenticator_data = None;

    for (k, v) in map {
        if let ciborium::Value::Text(s) = k {
            match s.as_str() {
                "signature" => signature = Some(v),
                "authenticatorData" => authenticator_data = Some(v),
                _ => {}
            }
        }
    }

    let signature_bytes = match signature {
        Some(ciborium::Value::Bytes(bytes)) => bytes,
        _ => {
            return Err(AppError::InvalidAssertion(
                "Missing or invalid signature".to_owned(),
            ))
        }
    };

    let authenticator_data_bytes = match authenticator_data {
        Some(ciborium::Value::Bytes(bytes)) => bytes,
        _ => {
            return Err(AppError::InvalidAssertion(
                "Missing or invalid authenticatorData".to_owned(),
            ))
        }
    };

    // 2. Parse authenticatorData:
    // rpIdHash: 32 bytes
    // flags: 1 byte
    // signCount: 4 bytes
    if authenticator_data_bytes.len() < 37 {
        return Err(AppError::InvalidAssertion(
            "authenticatorData is too short".to_owned(),
        ));
    }

    let rp_id_hash = &authenticator_data_bytes[0..32];
    let flags = authenticator_data_bytes[32];

    let sign_count = i64::from(u32::from_be_bytes([
        authenticator_data_bytes[33],
        authenticator_data_bytes[34],
        authenticator_data_bytes[35],
        authenticator_data_bytes[36],
    ]));

    // Verify rpIdHash matches SHA-256 of App ID
    let expected_app_id = config.auth.app_id();
    let expected_rp_id_hash = digest(&SHA256, expected_app_id.as_bytes());
    if rp_id_hash != expected_rp_id_hash.as_ref() {
        return Err(AppError::InvalidAssertion(
            "rpIdHash does not match expected App ID".to_owned(),
        ));
    }

    // Verify User Present (UP) flag is set
    if (flags & 0x01) == 0 {
        return Err(AppError::InvalidAssertion(
            "User Present flag (UP) is not set".to_owned(),
        ));
    }

    // 3. Verify signCount is strictly greater than stored_counter
    if sign_count <= stored_counter {
        return Err(AppError::ReplayDetected);
    }

    // 4. Reconstruct clientDataHash = SHA-256(challenge || request_body_hash)
    let challenge_bytes = hex::decode(challenge_hex)
        .map_err(|e| AppError::BadRequest(format!("Invalid challenge hex encoding: {e}")))?;

    let request_body_hash_bytes = hex::decode(request_body_hash_hex).map_err(|e| {
        AppError::BadRequest(format!("Invalid request body hash hex encoding: {e}"))
    })?;

    let mut client_data_buf =
        Vec::with_capacity(challenge_bytes.len() + request_body_hash_bytes.len());
    client_data_buf.extend_from_slice(&challenge_bytes);
    client_data_buf.extend_from_slice(&request_body_hash_bytes);
    let client_data_hash = digest(&SHA256, &client_data_buf);

    // 5. Reconstruct verification_data = authenticatorData || clientDataHash
    let mut verification_data = Vec::with_capacity(authenticator_data_bytes.len() + 32);
    verification_data.extend_from_slice(&authenticator_data_bytes);
    verification_data.extend_from_slice(client_data_hash.as_ref());

    // Verify ECDSA signature
    let peer = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_ASN1,
        public_key_der,
    );
    peer.verify(&verification_data, &signature_bytes)
        .map_err(|e| {
            AppError::InvalidAssertion(format!("Assertion signature verification failed: {e}"))
        })?;

    Ok(VerifiedAssertion {
        new_counter: sign_count,
    })
}
