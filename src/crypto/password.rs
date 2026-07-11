//! Password hashing for account sign-in.
//!
//! PBKDF2-HMAC-SHA256 with 600,000 iterations (OWASP's 2023+ floor for this
//! construction) and a random 16-byte salt per credential. `ring` performs
//! the verification in constant time. No plaintext password is ever stored
//! or logged.

use ring::rand::SecureRandom;
use ring::{pbkdf2, rand};
use std::num::NonZeroU32;

const ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 16;
const HASH_LEN: usize = 32;

/// Hash a password with a fresh random salt. Returns `(hash, salt)`.
pub fn hash_password(password: &str) -> Result<(Vec<u8>, Vec<u8>), crate::errors::AppError> {
    let rng = rand::SystemRandom::new();
    let mut salt = vec![0u8; SALT_LEN];
    rng.fill(&mut salt)?;
    let mut hash = vec![0u8; HASH_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).expect("nonzero"),
        &salt,
        password.as_bytes(),
        &mut hash,
    );
    Ok((hash, salt))
}

/// Constant-time verification of a password against a stored hash + salt.
#[must_use]
pub fn verify_password(password: &str, hash: &[u8], salt: &[u8]) -> bool {
    pbkdf2::verify(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).expect("nonzero"),
        salt,
        password.as_bytes(),
        hash,
    )
    .is_ok()
}

/// Password policy: length only, per current NIST guidance — no composition
/// rules, no truncation. 8–128 chars.
#[must_use]
pub fn is_acceptable_password(password: &str) -> bool {
    (8..=128).contains(&password.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_reject() {
        let (hash, salt) = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password(
            "correct horse battery staple",
            &hash,
            &salt
        ));
        assert!(!verify_password("wrong password", &hash, &salt));
    }

    #[test]
    fn salts_differ_per_hash() {
        let (h1, s1) = hash_password("same password").unwrap();
        let (h2, s2) = hash_password("same password").unwrap();
        assert_ne!(s1, s2, "salts must be random");
        assert_ne!(h1, h2, "hashes must differ under different salts");
    }

    #[test]
    fn policy_bounds() {
        assert!(!is_acceptable_password("short"));
        assert!(is_acceptable_password("long enough"));
        assert!(!is_acceptable_password(&"x".repeat(129)));
    }
}
