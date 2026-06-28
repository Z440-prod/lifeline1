use crate::config::AppConfig;
use crate::errors::AppError;
use der::{Decode, Encode};
use ring::digest::{digest, SHA256};
use x509_cert::Certificate;

/// Verification output containing the verified P-256 public key.
pub struct VerifiedAttestation {
    /// 65-byte DER-encoded uncompressed public key (0x04 || X || Y).
    pub public_key_der: Vec<u8>,
}

/// Helper to decode PEM to DER.
fn pem_to_der(pem: &str) -> Result<Vec<u8>, AppError> {
    let clean = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    engine
        .decode(clean.trim())
        .map_err(|e| AppError::CryptoError(format!("Failed to base64 decode PEM: {e}")))
}

/// Verify leaf certificate signature against parent certificate.
fn verify_cert_signature(child: &Certificate, parent: &Certificate) -> Result<(), AppError> {
    let parent_pub_key = parent
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    let tbs_bytes = child.tbs_certificate.to_der().map_err(|e| {
        AppError::CryptoError(format!("Failed to DER-serialize certificate TBS: {e}"))
    })?;

    let sig_bytes = child.signature.as_bytes().ok_or_else(|| {
        AppError::CryptoError("Invalid signature format in certificate".to_owned())
    })?;

    let peer = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_ASN1,
        parent_pub_key,
    );

    peer.verify(&tbs_bytes, sig_bytes).map_err(|e| {
        AppError::InvalidAttestation(format!("Certificate signature verification failed: {e}"))
    })?;

    Ok(())
}

/// Verify temporal validity of a certificate.
fn verify_cert_validity(cert: &Certificate) -> Result<(), AppError> {
    let now = std::time::SystemTime::now();
    let not_before = cert.tbs_certificate.validity.not_before.to_system_time();
    let not_after = cert.tbs_certificate.validity.not_after.to_system_time();

    if now < not_before {
        return Err(AppError::InvalidAttestation(
            "Certificate is not yet valid".to_owned(),
        ));
    }
    if now > not_after {
        return Err(AppError::InvalidAttestation(
            "Certificate has expired".to_owned(),
        ));
    }
    Ok(())
}

/// Parses the COSE key mapping in authData and returns the 65-byte uncompressed EC P-256 public key.
fn parse_cose_key(cose_value: &ciborium::Value) -> Result<Vec<u8>, AppError> {
    let map = match cose_value {
        ciborium::Value::Map(m) => m,
        _ => {
            return Err(AppError::InvalidAttestation(
                "COSE key is not a map".to_owned(),
            ))
        }
    };

    let mut kty = None;
    let mut alg = None;
    let mut crv = None;
    let mut x = None;
    let mut y = None;

    for (k, v) in map {
        if let ciborium::Value::Integer(i) = k {
            let key_num = i64::try_from(*i).unwrap_or(0);
            match key_num {
                1 => kty = Some(v),  // kty
                3 => alg = Some(v),  // alg
                -1 => crv = Some(v), // crv
                -2 => x = Some(v),   // x-coordinate
                -3 => y = Some(v),   // y-coordinate
                _ => {}
            }
        }
    }

    // kty must be EC2 (2)
    match kty {
        Some(ciborium::Value::Integer(i)) if i64::try_from(*i).unwrap_or(0) == 2 => {}
        _ => {
            return Err(AppError::InvalidAttestation(
                "Invalid COSE key: kty must be EC2 (2)".to_owned(),
            ))
        }
    }

    // alg must be ES256 (-7)
    match alg {
        Some(ciborium::Value::Integer(i)) if i64::try_from(*i).unwrap_or(0) == -7 => {}
        _ => {
            return Err(AppError::InvalidAttestation(
                "Invalid COSE key: alg must be ES256 (-7)".to_owned(),
            ))
        }
    }

    // crv must be P-256 (1)
    match crv {
        Some(ciborium::Value::Integer(i)) if i64::try_from(*i).unwrap_or(0) == 1 => {}
        _ => {
            return Err(AppError::InvalidAttestation(
                "Invalid COSE key: crv must be P-256 (1)".to_owned(),
            ))
        }
    }

    // X coordinate must be 32 bytes
    let x_bytes = match x {
        Some(ciborium::Value::Bytes(bytes)) if bytes.len() == 32 => bytes,
        _ => {
            return Err(AppError::InvalidAttestation(
                "Invalid COSE key: X coordinate must be 32 bytes".to_owned(),
            ))
        }
    };

    // Y coordinate must be 32 bytes
    let y_bytes = match y {
        Some(ciborium::Value::Bytes(bytes)) if bytes.len() == 32 => bytes,
        _ => {
            return Err(AppError::InvalidAttestation(
                "Invalid COSE key: Y coordinate must be 32 bytes".to_owned(),
            ))
        }
    };

    // Form uncompressed P-256 public key (65 bytes): 0x04 || X || Y
    let mut pub_key = Vec::with_capacity(65);
    pub_key.push(0x04);
    pub_key.extend_from_slice(x_bytes);
    pub_key.extend_from_slice(y_bytes);

    Ok(pub_key)
}

/// Verify Apple App Attest attestation object.
/// Steps:
/// 1. Decode CBOR attestation object
/// 2. Extract and verify fmt (must be "apple-appattest")
/// 3. Verify rpIdHash matches SHA-256 of App ID
/// 4. Verify signCount is 0
/// 5. Verify aaguid matches production or sandbox based on environment
/// 6. Reconstruct expected nonce and verify against leaf cert extension OID 1.2.840.113635.100.8.2
/// 7. Verify certificate chain validity up to Apple App Attest Root CA
/// 8. Verify leaf public key matches credentialId and COSE key
pub fn verify_attestation(
    config: &AppConfig,
    attestation_cbor_base64: &str,
    challenge_hex: &str,
    client_key_id_base64: &str,
) -> Result<VerifiedAttestation, AppError> {
    let engine = base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    let attestation_bytes = engine
        .decode(attestation_cbor_base64)
        .map_err(|e| AppError::BadRequest(format!("Base64 decode attestation failed: {e}")))?;

    let client_key_id = engine
        .decode(client_key_id_base64)
        .map_err(|e| AppError::BadRequest(format!("Base64 decode key_id failed: {e}")))?;

    // 1. CBOR decode
    let decoded_val: ciborium::Value = ciborium::from_reader(attestation_bytes.as_slice())
        .map_err(|e| AppError::BadRequest(format!("CBOR decode attestation object failed: {e}")))?;

    let map = match decoded_val {
        ciborium::Value::Map(m) => m,
        _ => {
            return Err(AppError::InvalidAttestation(
                "Attestation object is not a CBOR map".to_owned(),
            ))
        }
    };

    let mut fmt = None;
    let mut att_stmt = None;
    let mut auth_data = None;

    for (k, v) in map {
        if let ciborium::Value::Text(s) = k {
            match s.as_str() {
                "fmt" => fmt = Some(v),
                "attStmt" => att_stmt = Some(v),
                "authData" => auth_data = Some(v),
                _ => {}
            }
        }
    }

    // 2. Verify format is apple-appattest
    match fmt {
        Some(ciborium::Value::Text(ref s)) if s == "apple-appattest" => {}
        _ => {
            return Err(AppError::InvalidAttestation(
                "Format (fmt) must be 'apple-appattest'".to_owned(),
            ))
        }
    }

    let att_stmt_map = match att_stmt {
        Some(ciborium::Value::Map(m)) => m,
        _ => return Err(AppError::InvalidAttestation("Missing attStmt".to_owned())),
    };

    let mut x5c = None;
    for (k, v) in att_stmt_map {
        if let ciborium::Value::Text(s) = k {
            if s == "x5c" {
                x5c = Some(v);
            }
        }
    }

    let x5c_array = match x5c {
        Some(ciborium::Value::Array(arr)) => arr,
        _ => {
            return Err(AppError::InvalidAttestation(
                "Missing x5c in attStmt".to_owned(),
            ))
        }
    };

    if x5c_array.len() != 2 {
        return Err(AppError::InvalidAttestation(format!(
            "Expected exactly 2 certificates in x5c chain, got {}",
            x5c_array.len()
        )));
    }

    let mut certs = Vec::new();
    for cert_val in x5c_array {
        match cert_val {
            ciborium::Value::Bytes(bytes) => certs.push(bytes),
            _ => {
                return Err(AppError::InvalidAttestation(
                    "x5c element is not a byte array".to_owned(),
                ))
            }
        }
    }

    let auth_data_bytes = match auth_data {
        Some(ciborium::Value::Bytes(bytes)) => bytes,
        _ => return Err(AppError::InvalidAttestation("Missing authData".to_owned())),
    };

    // Parse authData:
    // rpIdHash: 32 bytes
    // flags: 1 byte
    // signCount: 4 bytes
    // aaguid: 16 bytes
    // credentialIdLength: 2 bytes
    // credentialId: credentialIdLength bytes
    // coseKey: remainder
    if auth_data_bytes.len() < 55 {
        return Err(AppError::InvalidAttestation(
            "authData is too short".to_owned(),
        ));
    }

    let rp_id_hash = &auth_data_bytes[0..32];
    let flags = auth_data_bytes[32];

    let sign_count = u32::from_be_bytes([
        auth_data_bytes[33],
        auth_data_bytes[34],
        auth_data_bytes[35],
        auth_data_bytes[36],
    ]);

    let aaguid = &auth_data_bytes[37..53];

    let credential_id_len = u16::from_be_bytes([auth_data_bytes[53], auth_data_bytes[54]]) as usize;

    if auth_data_bytes.len() < 55 + credential_id_len {
        return Err(AppError::InvalidAttestation(
            "authData size mismatch for credential ID".to_owned(),
        ));
    }

    let credential_id = &auth_data_bytes[55..55 + credential_id_len];
    let cose_key_start = 55 + credential_id_len;

    // 3. Verify rpIdHash matches SHA-256 of App ID
    let expected_app_id = config.auth.app_id();
    let expected_rp_id_hash = digest(&SHA256, expected_app_id.as_bytes());
    if rp_id_hash != expected_rp_id_hash.as_ref() {
        return Err(AppError::InvalidAttestation(format!(
            "rpIdHash mismatch: expected {}, got {}",
            hex::encode(expected_rp_id_hash.as_ref()),
            hex::encode(rp_id_hash)
        )));
    }

    // Verify flags: UP (User Present) flag must be set
    if (flags & 0x01) == 0 {
        return Err(AppError::InvalidAttestation(
            "User Present flag (UP) is not set".to_owned(),
        ));
    }

    // 4. Verify signCount is 0
    if sign_count != 0 {
        return Err(AppError::InvalidAttestation(format!(
            "signCount must be 0 for initial attestation, got {sign_count}"
        )));
    }

    // 5. Verify AAGUID matches expected based on environment
    let expected_aaguid: &[u8; 16] = if config.auth.environment == "production" {
        b"appattest\x00\x00\x00\x00\x00\x00\x00"
    } else {
        b"appattestsandbox"
    };

    if aaguid != expected_aaguid {
        return Err(AppError::InvalidAttestation(format!(
            "AAGUID mismatch: expected {}, got {}",
            String::from_utf8_lossy(expected_aaguid),
            String::from_utf8_lossy(aaguid)
        )));
    }

    // 6. Reconstruct nonce: SHA-256(authData || SHA-256(challenge))
    let challenge_bytes = hex::decode(challenge_hex)
        .map_err(|e| AppError::BadRequest(format!("Invalid challenge hex encoding: {e}")))?;

    let challenge_hash = digest(&SHA256, &challenge_bytes);

    let mut nonce_buf = Vec::with_capacity(auth_data_bytes.len() + 32);
    nonce_buf.extend_from_slice(&auth_data_bytes);
    nonce_buf.extend_from_slice(challenge_hash.as_ref());
    let expected_nonce = digest(&SHA256, &nonce_buf);

    // Parse certificates
    let leaf_cert = Certificate::from_der(&certs[0]).map_err(|e| {
        AppError::InvalidAttestation(format!("Failed to parse leaf certificate: {e}"))
    })?;

    let intermediate_cert = Certificate::from_der(&certs[1]).map_err(|e| {
        AppError::InvalidAttestation(format!("Failed to parse intermediate certificate: {e}"))
    })?;

    // Embedded Apple Root CA
    let root_pem = include_str!("../../config/apple_app_attest_root_ca.pem");
    let root_der = pem_to_der(root_pem)?;
    let root_cert = Certificate::from_der(&root_der)
        .map_err(|e| AppError::CryptoError(format!("Failed to parse Apple Root CA: {e}")))?;

    // 7. Verify certificate chain (temporal validity + signature)
    verify_cert_validity(&leaf_cert)?;
    verify_cert_validity(&intermediate_cert)?;
    verify_cert_validity(&root_cert)?;

    verify_cert_signature(&leaf_cert, &intermediate_cert)?;
    verify_cert_signature(&intermediate_cert, &root_cert)?;

    // Verify leaf nonce extension: OID 1.2.840.113635.100.8.2
    let nonce_oid = der::asn1::ObjectIdentifier::new("1.2.840.113635.100.8.2")
        .map_err(|e| AppError::Internal(format!("Invalid OID: {e}")))?;

    let extensions = leaf_cert
        .tbs_certificate
        .extensions
        .as_ref()
        .ok_or_else(|| {
            AppError::InvalidAttestation("No extensions found in leaf certificate".to_owned())
        })?;

    let mut nonce_ext_bytes = None;
    for ext in extensions {
        if ext.extn_id == nonce_oid {
            nonce_ext_bytes = Some(ext.extn_value.as_bytes());
            break;
        }
    }

    let ext_bytes = nonce_ext_bytes.ok_or_else(|| {
        AppError::InvalidAttestation(
            "App Attest nonce extension OID 1.2.840.113635.100.8.2 not found".to_owned(),
        )
    })?;

    // Parse inner OctetString inside extension value
    let inner_octet = der::asn1::OctetStringRef::from_der(ext_bytes).map_err(|e| {
        AppError::InvalidAttestation(format!("Failed to parse extension inner OctetString: {e}"))
    })?;
    let cert_nonce = inner_octet.as_bytes();

    if cert_nonce != expected_nonce.as_ref() {
        return Err(AppError::InvalidAttestation(
            "Nonce in leaf certificate does not match expected nonce".to_owned(),
        ));
    }

    // 8. Verify leaf public key matches credentialId and COSE key
    let cose_val: ciborium::Value = ciborium::from_reader(&auth_data_bytes[cose_key_start..])
        .map_err(|e| {
            AppError::InvalidAttestation(format!(
                "Failed to CBOR-decode COSE key from authData: {e}"
            ))
        })?;

    let cose_pub_key_der = parse_cose_key(&cose_val)?;

    // Verify public key in COSE matches credential_id (which is SHA-256 of the uncompressed public key)
    let cose_key_hash = digest(&SHA256, &cose_pub_key_der);
    if cose_key_hash.as_ref() != credential_id {
        return Err(AppError::InvalidAttestation(
            "SHA-256 of COSE public key does not match credentialId".to_owned(),
        ));
    }

    // Also verify credentialId matches client_key_id
    if credential_id != client_key_id {
        return Err(AppError::InvalidAttestation(
            "credentialId does not match key_id from client".to_owned(),
        ));
    }

    // Check that public key in leaf certificate matches the COSE key
    let leaf_pub_key_spki = leaf_cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    if leaf_pub_key_spki != cose_pub_key_der {
        return Err(AppError::InvalidAttestation(
            "Leaf certificate public key does not match COSE public key".to_owned(),
        ));
    }

    Ok(VerifiedAttestation {
        public_key_der: cose_pub_key_der,
    })
}
