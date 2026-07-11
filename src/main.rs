use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

use antigravity::config;
use antigravity::crypto;
use antigravity::db;
use antigravity::routes;
use antigravity::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. Load .env file (if present) for local development.
    // Silently ignored in production where env vars are set by the platform.
    dotenvy::dotenv().ok();

    // 1. Initialize Structured Logging (Tracing)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "antigravity=info,axum=info,tower_http=info".into()),
        )
        .with_target(false)
        .json()
        .init();

    tracing::info!("Initializing Antigravity Engine...");

    // 2. Load Configuration
    let config = config::load().context("Failed to load configuration")?;
    tracing::info!("Configuration loaded successfully");

    // 3. Validate sensitive configuration in non-development environments
    if config.auth.environment != "development" {
        anyhow::ensure!(
            !config.auth.server_secret.starts_with("REPLACE"),
            "FATAL: server_secret is a placeholder. Set ANTIGRAVITY__AUTH__SERVER_SECRET to a cryptographically random string (≥ 32 bytes)."
        );
        anyhow::ensure!(
            config.auth.server_secret.len() >= 32,
            "FATAL: server_secret must be at least 32 bytes for HMAC-SHA256 security."
        );
        anyhow::ensure!(
            !config.ai.anthropic_api_key.is_empty(),
            "FATAL: anthropic_api_key is empty. Set ANTIGRAVITY__AI__ANTHROPIC_API_KEY in production."
        );
        anyhow::ensure!(
            !config.auth.apple_team_id.starts_with("REPLACE"),
            "FATAL: apple_team_id is a placeholder. Set ANTIGRAVITY__AUTH__APPLE_TEAM_ID."
        );
    }

    // 4. Establish Database Connection Pool (with local MockDatabase fallback for offline development)
    tracing::info!("Connecting to PostgreSQL database...");
    let db: Arc<dyn db::Database> = match PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .acquire_timeout(std::time::Duration::from_secs(2)) // Fail fast if Postgres is down
        .connect(&config.database.url)
        .await
    {
        Ok(pool) => {
            tracing::info!("PostgreSQL connection pool established");

            // 5. Run SQL Migrations Automatically
            tracing::info!("Executing database migrations...");
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .context("Failed to run database migrations")?;
            tracing::info!("Database migrations applied successfully");

            Arc::new(db::PostgresDatabase::new(pool, &config.auth.server_secret))
        }
        Err(e) => {
            if config.auth.environment == "development" {
                tracing::warn!(
                    error = %e,
                    "PostgreSQL connection failed. Falling back to In-Memory MockDatabase for development mode..."
                );
                Arc::new(db::MockDatabase::new(&config.auth.server_secret))
            } else {
                return Err(e).context("Database connection failed");
            }
        }
    };

    // 6. Initialize In-Process TTL Nonce Cache
    let nonce_cache = crypto::nonce::NonceCache::new(config.auth.nonce_ttl_seconds);
    tracing::info!(
        ttl_seconds = config.auth.nonce_ttl_seconds,
        "Challenge Nonce TTL cache initialized"
    );

    // 7. Derive HMAC-SHA256 Key for Session Tokens
    let hmac_key = ring::hmac::Key::new(
        ring::hmac::HMAC_SHA256,
        config.auth.server_secret.as_bytes(),
    );
    let oauth_state_key = crypto::oauth_state::derive_oauth_state_key(&config.auth.server_secret);
    let token_vault_key = crypto::token_vault::derive_token_vault_key(&config.auth.server_secret);
    tracing::info!("HMAC server secret key derived");

    // 8. Instantiate Outbound HTTP Client for external services (AI proxy)
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")?;

    // 8.5 Initialize In-Process E2EE Document Cache (cache-aside)
    let doc_cache = moka::sync::Cache::builder()
        .max_capacity(10_000)
        .time_to_idle(std::time::Duration::from_secs(600))
        .build();

    // 8.6 AI-coach usage meter — enforces daily/monthly/global token budgets.
    //     Keys are date-scoped (`…:d:<day>`, `…:m:<month>`, `ai:global:<day>`),
    //     so the TTL only needs to outlive the longest window (monthly). 40 days
    //     keeps a month's counter alive through its entire span, after which the
    //     rolled-over key is irrelevant and expires on its own.
    let ai_usage = moka::sync::Cache::builder()
        .max_capacity(100_000)
        .time_to_live(std::time::Duration::from_secs(40 * 24 * 3600))
        .build();

    // 9. Assemble Shared Application State
    let app_state = Arc::new(AppState {
        db,
        nonce_cache,
        config: config.clone(),
        http_client,
        hmac_key,
        oauth_state_key,
        token_vault_key,
        doc_cache,
        ai_usage,
    });

    // 10. Build router with all submodules nested under `/api/v1`
    let app = routes::create_router(app_state);

    // 11. Bind TCP Listener and start server
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Failed to bind to {bind_addr}"))?;

    // NOTE: This process speaks plain HTTP. TLS must be terminated in front of
    // it (e.g. a load balancer, reverse proxy, or platform ingress) — the
    // engine does not perform TLS termination itself.
    tracing::info!("Antigravity server listening on http://{}", bind_addr);

    // Run Axum server with graceful shutdown.
    // `into_make_service_with_connect_info` is required so that tower_governor's
    // PeerIpKeyExtractor can read the client's socket address — without it every
    // request fails at the rate-limiting layer ("Unable To Extract Key!").
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("Server execution error")?;

    Ok(())
}

/// Graceful shutdown signal receiver for SIGINT/SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("Shutdown signal received. Starting graceful shutdown...");
}
