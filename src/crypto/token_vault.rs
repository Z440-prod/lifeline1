use crate::errors::AppError;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};

/// Derive the AEAD key used to encrypt third-party OAuth refresh tokens at rest.
///
/// Domain-separated from the session and audit-log HMAC keys (see
/// `crypto::session` and `db::audit`) so a key computed for one purpose can
/// never be reused for another, even though all three are derived from the
/// same server secret.
#[must_use]
pub fn derive_token_vault_key(server_secret: &str) -> LessSafeKey {
    let key_bytes = digest(
        &SHA256,
        format!("antigravity-token-vault-v1:{server_secret}").as_bytes(),
    );
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key_bytes.as_ref())
        .expect("SHA-256 digest is exactly 32 bytes, the required ChaCha20-Poly1305 key length");
    LessSafeKey::new(unbound)
}

/// Encrypt a refresh token for storage. Output is `nonce (12 bytes) || ciphertext || tag`.
pub fn encrypt_token(key: &LessSafeKey, plaintext: &str) -> Result<Vec<u8>, AppError> {
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| AppError::CryptoError("Entropy source failure".to_owned()))?;

    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce_bytes),
        Aad::empty(),
        &mut in_out,
    )
    .map_err(|_| AppError::CryptoError("Failed to encrypt token".to_owned()))?;

    let mut out = Vec::with_capacity(NONCE_LEN + in_out.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&in_out);
    Ok(out)
}

/// Decrypt a token blob previously produced by [`encrypt_token`].
pub fn decrypt_token(key: &LessSafeKey, blob: &[u8]) -> Result<String, AppError> {
    if blob.len() < NONCE_LEN {
        return Err(AppError::CryptoError("Token blob is too short".to_owned()));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce_bytes)
        .map_err(|_| AppError::CryptoError("Invalid nonce".to_owned()))?;

    let mut in_out = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| AppError::CryptoError("Failed to decrypt token".to_owned()))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::CryptoError("Decrypted token is not valid UTF-8".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_token_vault_key("test_secret_at_least_32_bytes_long_key_signature");
        let token = "whoop_refresh_token_abc123";

        let encrypted = encrypt_token(&key, token).unwrap();
        assert_ne!(encrypted, token.as_bytes());

        let decrypted = decrypt_token(&key, &encrypted).unwrap();
        assert_eq!(decrypted, token);
    }

    #[test]
    fn test_decrypt_tampered_blob_fails() {
        let key = derive_token_vault_key("test_secret_at_least_32_bytes_long_key_signature");
        let mut encrypted = encrypt_token(&key, "some_token").unwrap();
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xFF;

        assert!(decrypt_token(&key, &encrypted).is_err());
    }

    #[test]
    fn test_different_keys_produce_different_ciphertext_cannot_cross_decrypt() {
        let key_a = derive_token_vault_key("secret_a_at_least_32_bytes_long_for_test");
        let key_b = derive_token_vault_key("secret_b_at_least_32_bytes_long_for_test");
        let encrypted = encrypt_token(&key_a, "token").unwrap();
        assert!(decrypt_token(&key_b, &encrypted).is_err());
    }
}
