use crate::errors::AppError;
use moka::sync::Cache;
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use std::time::Duration;

/// Memory-efficient, TTL-based cache for single-use challenge nonces.
#[derive(Clone)]
pub struct NonceCache {
    cache: Cache<String, ()>,
}

impl NonceCache {
    /// Create a new NonceCache with the specified time-to-live in seconds.
    pub fn new(ttl_seconds: u64) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(ttl_seconds))
            .build();
        Self { cache }
    }

    /// Generate a cryptographically secure 32-byte challenge, hex-encode it,
    /// insert it into the cache, and return it.
    pub fn generate_nonce(&self) -> Result<String, AppError> {
        let mut bytes = [0u8; 32];
        let rng = SystemRandom::new();
        rng.fill(&mut bytes)
            .map_err(|_| AppError::CryptoError("Entropy source failure".to_owned()))?;

        // SHA-256 hash the random bytes to ensure stable 32-byte format
        let hash = digest(&SHA256, &bytes);
        let hex_nonce = hex::encode(hash.as_ref());

        // Insert into cache (value is just unit)
        self.cache.insert(hex_nonce.clone(), ());
        Ok(hex_nonce)
    }

    /// Verify if a nonce exists in the cache, and consume it (remove it) to prevent replays.
    /// Returns `NonceNotFound` if the nonce does not exist or has expired.
    pub fn verify_and_consume(&self, nonce: &str) -> Result<(), AppError> {
        if self.cache.remove(nonce).is_some() {
            Ok(())
        } else {
            Err(AppError::NonceNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonce_cache() {
        let cache = NonceCache::new(5);
        let nonce = cache.generate_nonce().unwrap();
        assert_eq!(nonce.len(), 64); // 32 bytes hex-encoded is 64 chars

        // First verification succeeds
        assert!(cache.verify_and_consume(&nonce).is_ok());

        // Second verification fails because it was consumed
        assert!(cache.verify_and_consume(&nonce).is_err());
    }

    #[test]
    fn test_invalid_nonce() {
        let cache = NonceCache::new(5);
        assert!(cache.verify_and_consume("nonexistent_nonce").is_err());
    }
}
