//! Argine backend entry point.
//!
//! Thin wrapper: init tracing, load+validate config (fail-fast), build the router
//! (see `lib.rs`), and serve.

use argine_backend::{config::Config, router};
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

    let app = router(&config);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
