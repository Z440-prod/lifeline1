use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// A sign-in account. An identity layer *on top of* device attestation — it
/// groups the devices one person signs in from and enables recovery.
///
/// # Zero-knowledge, still
/// An account holds only an email + password hash, or an OAuth subject. It
/// carries **no encryption keys and no health data**. Devices remain the
/// cryptographic identity; the account is metadata that links them.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub email: Option<String>,
    pub password_hash: Option<Vec<u8>>,
    pub password_salt: Option<Vec<u8>>,
    pub oauth_provider: Option<String>,
    pub oauth_subject: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// The public projection of an account returned to clients — never the hash.
#[derive(Debug, Clone, Serialize)]
pub struct AccountView {
    pub id: Uuid,
    pub email: Option<String>,
    pub auth_method: &'static str,
}

impl Account {
    #[must_use]
    pub fn view(&self) -> AccountView {
        AccountView {
            id: self.id,
            email: self.email.clone(),
            auth_method: match self.oauth_provider.as_deref() {
                Some("apple") => "apple",
                Some("google") => "google",
                _ => "password",
            },
        }
    }
}

/// Normalize an email for storage/lookup: trim + lowercase. Returns `None`
/// if it doesn't look like an address (one `@`, non-empty local + domain).
#[must_use]
pub fn normalize_email(raw: &str) -> Option<String> {
    let e = raw.trim().to_lowercase();
    let bytes = e.as_bytes();
    if e.len() > 254 || bytes.iter().filter(|&&c| c == b'@').count() != 1 {
        return None;
    }
    let (local, domain) = e.split_once('@')?;
    if local.is_empty() || domain.len() < 3 || !domain.contains('.') {
        return None;
    }
    Some(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalization() {
        assert_eq!(
            normalize_email("  Foo@Bar.COM "),
            Some("foo@bar.com".into())
        );
        assert_eq!(normalize_email("no-at-sign"), None);
        assert_eq!(normalize_email("a@b@c.com"), None);
        assert_eq!(normalize_email("@nolocal.com"), None);
        assert_eq!(normalize_email("x@nodot"), None);
    }
}
