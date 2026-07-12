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
    #[serde(default)]
    pub admin: AdminConfig,
}

/// Admin dashboard settings. The stats endpoint (`GET /api/v1/admin/stats`) and
/// the `/admin` page are **disabled by default** — they only work once an
/// `admin_token` is configured, so a fresh/misconfigured deployment never
/// exposes them. Set it via `ANTIGRAVITY__ADMIN__ADMIN_TOKEN` (a long random
/// string); never commit a real token.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AdminConfig {
    /// Bearer token required to read admin stats. Empty ⇒ admin disabled.
    pub admin_token: String,
}

impl AdminConfig {
    /// Admin surfaces are only reachable once a token is set.
    #[must_use]
    pub fn enabled(&self) -> bool {
        !self.admin_token.is_empty()
    }
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
    /// AI-coach usage budgets. Hard caps so the token bill can never run away,
    /// enforced server-side regardless of tier marketing. All default to sane
    /// values so existing configs keep working.
    #[serde(default)]
    pub budget: AiBudget,
    /// Cloud-coach provider selection for devices that can't run Gemma locally.
    /// "auto" (default) uses the open-source model when `openai_api_key` is set,
    /// otherwise Anthropic; "openai" forces the open-source path; "anthropic"
    /// forces Claude. The on-device Gemma path is chosen by the client and never
    /// touches this — this only governs the cloud fallback.
    #[serde(default)]
    pub provider: String,
    /// OpenAI-compatible endpoint for a cheaper open-source model (Llama, Qwen,
    /// DeepSeek, …) served by Together / Groq / OpenRouter / DeepInfra / vLLM.
    /// Base URL only — the handler appends `/chat/completions`.
    #[serde(default)]
    pub openai_base_url: String,
    /// API key for the OpenAI-compatible endpoint above. When set (and provider
    /// is "auto" or "openai"), the cloud coach uses this instead of Anthropic.
    #[serde(default)]
    pub openai_api_key: String,
    /// Model id for the open-source endpoint,
    /// e.g. "meta-llama/Llama-3.3-70B-Instruct-Turbo".
    #[serde(default)]
    pub openai_model: String,
}

impl AiConfig {
    /// Which cloud provider the proxy should use, resolved from `provider` +
    /// which keys are set. Returns "openai", "anthropic", or "none".
    #[must_use]
    pub fn cloud_provider(&self) -> &'static str {
        let has_openai = !self.openai_api_key.is_empty();
        let has_anthropic = !self.anthropic_api_key.is_empty();
        match self.provider.as_str() {
            "openai" if has_openai => "openai",
            "anthropic" if has_anthropic => "anthropic",
            // "auto" / empty / anything else: prefer the cheaper open-source
            // model when configured, else Claude.
            _ if has_openai => "openai",
            _ if has_anthropic => "anthropic",
            _ => "none",
        }
    }
}

/// Coach usage limits. Per-device daily and monthly caps bound each user's
/// spend; the process-wide daily budget is a circuit breaker that protects the
/// whole token bill if usage spikes.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiBudget {
    pub coach_daily_free: u32,
    pub coach_daily_pro: u32,
    pub coach_daily_elite: u32,
    pub coach_monthly_pro: u32,
    pub coach_monthly_elite: u32,
    /// Total coach messages allowed across ALL users per day. When reached,
    /// the coach pauses for everyone until the next day.
    pub global_daily_budget: u32,
}

impl Default for AiBudget {
    fn default() -> Self {
        Self {
            coach_daily_free: 3,
            coach_daily_pro: 50,
            coach_daily_elite: 120,
            coach_monthly_pro: 800,
            coach_monthly_elite: 2000,
            global_daily_budget: 5000,
        }
    }
}

impl AiBudget {
    /// The per-device daily message cap for a tier.
    #[must_use]
    pub fn daily_for(&self, tier: &str) -> u32 {
        match tier {
            "pro" => self.coach_daily_pro,
            "elite" => self.coach_daily_elite,
            _ => self.coach_daily_free,
        }
    }

    /// The per-device monthly message cap for a tier (0 = not enforced).
    #[must_use]
    pub fn monthly_for(&self, tier: &str) -> u32 {
        match tier {
            "pro" => self.coach_monthly_pro,
            "elite" => self.coach_monthly_elite,
            _ => 0,
        }
    }
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
    /// Optional pre-created Stripe **Payment Links** for the paid tiers
    /// (`https://buy.stripe.com/…`). When set, checkout returns the link
    /// directly — real payment with no secret key or Checkout Session API call.
    /// The device id (and tier) is appended as `client_reference_id` so the
    /// webhook can grant the tier once a signing secret is configured.
    pub payment_link_pro: String,
    pub payment_link_elite: String,

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

    /// A pre-created Stripe Payment Link for a paid tier, if configured.
    #[must_use]
    pub fn payment_link_for(&self, tier: &str) -> Option<&str> {
        let url = match tier {
            "pro" => &self.payment_link_pro,
            "elite" => &self.payment_link_elite,
            _ => return None,
        };
        (!url.is_empty()).then_some(url.as_str())
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

#[cfg(test)]
mod tests {
    use super::{AiBudget, AiConfig};

    fn ai(provider: &str, openai_key: &str, anthropic_key: &str) -> AiConfig {
        AiConfig {
            anthropic_api_url: String::new(),
            anthropic_api_key: anthropic_key.to_owned(),
            policy_matrix_version: "1.0.0".to_owned(),
            budget: AiBudget::default(),
            provider: provider.to_owned(),
            openai_base_url: String::new(),
            openai_api_key: openai_key.to_owned(),
            openai_model: String::new(),
        }
    }

    #[test]
    fn cloud_provider_selection() {
        // auto: prefer the cheaper open-source model when its key is set.
        assert_eq!(ai("auto", "osk", "").cloud_provider(), "openai");
        assert_eq!(ai("auto", "osk", "ak").cloud_provider(), "openai");
        // auto: fall back to Anthropic when only that key is set.
        assert_eq!(ai("auto", "", "ak").cloud_provider(), "anthropic");
        // auto / empty provider with no keys → none (dev mock or prod error).
        assert_eq!(ai("auto", "", "").cloud_provider(), "none");
        assert_eq!(ai("", "", "").cloud_provider(), "none");
        // Forced providers honor the choice when the matching key exists…
        assert_eq!(ai("anthropic", "osk", "ak").cloud_provider(), "anthropic");
        assert_eq!(ai("openai", "osk", "ak").cloud_provider(), "openai");
        // …but a forced provider with no key falls through to whatever is set.
        assert_eq!(ai("anthropic", "osk", "").cloud_provider(), "openai");
        assert_eq!(ai("openai", "", "ak").cloud_provider(), "anthropic");
    }

    #[test]
    fn payment_link_selection() {
        let mut b = super::BillingConfig::default();
        assert_eq!(b.payment_link_for("pro"), None);
        assert_eq!(b.payment_link_for("elite"), None);
        b.payment_link_pro = "https://buy.stripe.com/pro".to_owned();
        b.payment_link_elite = "https://buy.stripe.com/elite".to_owned();
        assert_eq!(
            b.payment_link_for("pro"),
            Some("https://buy.stripe.com/pro")
        );
        assert_eq!(
            b.payment_link_for("elite"),
            Some("https://buy.stripe.com/elite")
        );
        assert_eq!(b.payment_link_for("free"), None);
    }

    #[test]
    fn budget_caps_by_tier() {
        let b = AiBudget::default();
        // Daily caps rise with tier; unknown/free share the free cap.
        assert_eq!(b.daily_for("free"), b.coach_daily_free);
        assert_eq!(b.daily_for("pro"), b.coach_daily_pro);
        assert_eq!(b.daily_for("elite"), b.coach_daily_elite);
        assert_eq!(b.daily_for("mystery"), b.coach_daily_free);
        assert!(b.daily_for("pro") > b.daily_for("free"));
        assert!(b.daily_for("elite") > b.daily_for("pro"));
        // Monthly caps: free rides on the daily cap alone (0 = not enforced).
        assert_eq!(b.monthly_for("free"), 0);
        assert_eq!(b.monthly_for("pro"), b.coach_monthly_pro);
        assert_eq!(b.monthly_for("elite"), b.coach_monthly_elite);
    }
}
