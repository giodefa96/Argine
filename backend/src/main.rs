//! Argine backend entry point.
//!
//! Thin wrapper: init tracing, load+validate config (fail-fast), connect the DB pool,
//! run migrations, build the router (see `lib.rs`), and serve.

use argine_backend::{config::Config, router, AppState};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env()?;
    tracing::info!(environment = ?config.environment, "starting argine-backend");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    tracing::info!("database connected and migrations applied");

    let app = router(AppState { pool }, &config);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
