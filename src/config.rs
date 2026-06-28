use serde::Deserialize;

/// Top-level application configuration.
/// Loaded from `config/default.toml` with environment variable overrides
/// prefixed with `ANTIGRAVITY__` (double-underscore separator for nesting).
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub ai: AiConfig,
    pub rate_limit: RateLimitConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    /// Apple Developer Team ID (e.g. "ABCDE12345").
    pub apple_team_id: String,
    /// App bundle identifier (e.g. "com.yourcompany.lifeline").
    pub apple_bundle_id: String,
    /// How many seconds a challenge nonce remains valid.
    pub nonce_ttl_seconds: u64,
    /// How many seconds a server-issued session token remains valid.
    pub session_token_ttl_seconds: u64,
    /// HMAC-SHA256 secret for signing session tokens. Must be ≥ 32 bytes.
    pub server_secret: String,
    /// "production" or "development" — selects expected AAGUID for App Attest.
    pub environment: String,
}

impl AuthConfig {
    /// Returns the full Apple App ID used for RP-ID hashing.
    /// Format: `{TeamID}.{BundleID}`
    pub fn app_id(&self) -> String {
        format!("{}.{}", self.apple_team_id, self.apple_bundle_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiConfig {
    pub anthropic_api_url: String,
    pub anthropic_api_key: String,
    pub policy_matrix_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum sustained requests per second (per source IP).
    pub requests_per_second: u64,
    /// Maximum burst size — the number of requests allowed in a quick burst.
    pub burst_size: u32,
}

/// Load configuration from `config/default.toml` with environment overrides.
pub fn load() -> Result<AppConfig, config::ConfigError> {
    let settings = config::Config::builder()
        .add_source(config::File::with_name("config/default"))
        .add_source(
            config::Environment::with_prefix("ANTIGRAVITY")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

    settings.try_deserialize()
}
