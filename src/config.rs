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
    pub integrations: IntegrationsConfig,
    #[serde(default)]
    pub billing: BillingConfig,
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
    #[must_use]
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

/// Third-party health data provider integration settings.
///
/// Apple Health and Google Health Connect are on-device SDKs: the client
/// reads them locally and the server only records connection status. Whoop
/// is a cloud API and needs a real `OAuth2` client registered at
/// <https://developer.whoop.com>.
#[derive(Debug, Clone, Deserialize)]
pub struct IntegrationsConfig {
    pub whoop_client_id: String,
    pub whoop_client_secret: String,
    pub whoop_authorize_url: String,
    pub whoop_token_url: String,
    pub whoop_api_base: String,
    pub whoop_redirect_uri: String,
}

/// Stripe billing settings. All fields default to empty so the server boots
/// (and tests run) without any Stripe credentials — in that state the billing
/// endpoints fall back to a mocked checkout flow, mirroring the AI proxy and
/// Whoop dev-mode behavior. Set the secrets via `ANTIGRAVITY__BILLING__*` env
/// vars in production; never commit live keys.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct BillingConfig {
    /// Stripe secret key (`sk_live_…` / `sk_test_…`).
    pub stripe_secret_key: String,
    /// Stripe webhook signing secret (`whsec_…`) for verifying event callbacks.
    pub stripe_webhook_secret: String,
    /// Stripe Price id for the Pro subscription.
    pub price_pro: String,
    /// Stripe Price id for the Elite subscription.
    pub price_elite: String,
    /// Where Stripe returns the user after a successful checkout.
    pub success_url: String,
    /// Where Stripe returns the user if they cancel checkout.
    pub cancel_url: String,
    /// Where the Stripe billing portal returns the user afterward.
    pub portal_return_url: String,
    /// Stripe API base. Overridable for tests; defaults to the live host.
    pub api_base: String,
    /// Optional pre-created Stripe Payment Link for donations. When set, the
    /// client's donate button opens it directly; when empty, POST
    /// /billing/donate creates a one-time Checkout Session instead.
    pub donate_url: String,

    /// App Store shared secret for receipt verification (App Store Connect →
    /// App Information → App-Specific Shared Secret). Store builds purchase
    /// via StoreKit and redeem through POST /billing/store-receipt.
    pub apple_shared_secret: String,
    /// Apple verifyReceipt endpoint. Defaults to the production host;
    /// overridable for tests and sandbox runs.
    pub apple_verify_url: String,
    /// StoreKit product identifiers mapped to tiers.
    pub apple_product_pro: String,
    pub apple_product_elite: String,
}

impl BillingConfig {
    /// Stripe endpoints are only live once a secret key is configured. Without
    /// it, checkout returns a simulated URL and the client can preview the
    /// upgrade flow without real charges.
    #[must_use]
    pub fn stripe_configured(&self) -> bool {
        !self.stripe_secret_key.is_empty()
    }

    /// The configured Stripe API base, defaulting to the live host.
    #[must_use]
    pub fn api_base_url(&self) -> &str {
        if self.api_base.is_empty() {
            "https://api.stripe.com"
        } else {
            &self.api_base
        }
    }

    /// The Stripe Price id for a given paid tier, if configured.
    #[must_use]
    pub fn price_for(&self, tier: &str) -> Option<&str> {
        let id = match tier {
            "pro" => &self.price_pro,
            "elite" => &self.price_elite,
            _ => return None,
        };
        (!id.is_empty()).then_some(id.as_str())
    }

    /// Apple receipt verification endpoint (production default).
    #[must_use]
    pub fn apple_verify_url_or_default(&self) -> &str {
        if self.apple_verify_url.is_empty() {
            "https://buy.itunes.apple.com/verifyReceipt"
        } else {
            &self.apple_verify_url
        }
    }

    /// Map a StoreKit product id to a tier string, honoring config overrides
    /// with sensible defaults matching the shells' bundle id.
    #[must_use]
    pub fn tier_for_apple_product(&self, product_id: &str) -> Option<&'static str> {
        let pro = if self.apple_product_pro.is_empty() {
            "health.lifeline.app.pro_monthly"
        } else {
            self.apple_product_pro.as_str()
        };
        let elite = if self.apple_product_elite.is_empty() {
            "health.lifeline.app.elite_monthly"
        } else {
            self.apple_product_elite.as_str()
        };
        if product_id == pro {
            Some("pro")
        } else if product_id == elite {
            Some("elite")
        } else {
            None
        }
    }
}

impl IntegrationsConfig {
    /// Whoop OAuth is only usable once a real client id/secret is configured.
    /// Without them, Whoop endpoints fall back to a mocked flow in
    /// development, mirroring the AI proxy's dev-mode behavior.
    #[must_use]
    pub fn whoop_configured(&self) -> bool {
        !self.whoop_client_id.is_empty() && !self.whoop_client_secret.is_empty()
    }
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
