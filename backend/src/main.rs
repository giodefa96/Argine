//! Argine backend entry point.
//!
//! Thin wrapper: init tracing, load+validate config (fail-fast), connect the DB pool,
//! run migrations, build the router (see `lib.rs`), and serve.

use argine_backend::{arpa, config::Config, open_meteo, router, AppState};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

/// Forward-poll cadence. ARPA open-data for the lowland network is published with ~18h
/// latency (DATA_SOURCES.md), so polling faster than hourly gains nothing. Open-Meteo
/// refreshes its models on a similar cadence, so it shares the interval.
const POLL_INTERVAL: Duration = Duration::from_secs(3600);

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

    let client = arpa::ArpaClient::new(arpa::DEFAULT_BASE_URL);

    // One-shot historical backfill: `argine-backend backfill` loads history, then exits.
    if std::env::args().nth(1).as_deref() == Some("backfill") {
        tracing::info!("running ARPA historical backfill");
        let stored = arpa::backfill(&pool, &client).await?;
        tracing::info!(stored, "backfill complete");
        return Ok(());
    }

    // Scheduled forward poll, in the background, alongside the HTTP server.
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(POLL_INTERVAL);
            loop {
                tick.tick().await;
                match arpa::poll_once(&pool, &client).await {
                    Ok(stored) => tracing::info!(stored, "ARPA poll complete"),
                    Err(e) => tracing::warn!(error = %e, "ARPA poll failed"),
                }
            }
        });
    }

    // Open-Meteo rain-forecast poll: one forecast run per station per tick, keyed by
    // the fetch time (run_ts) so every run is kept for backtesting.
    {
        let pool = pool.clone();
        let client = open_meteo::OpenMeteoClient::new(open_meteo::DEFAULT_BASE_URL);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(POLL_INTERVAL);
            loop {
                tick.tick().await;
                let run_ts = time::OffsetDateTime::now_utc();
                match open_meteo::poll_once(&pool, &client, run_ts).await {
                    Ok(stored) => tracing::info!(stored, "Open-Meteo poll complete"),
                    Err(e) => tracing::warn!(error = %e, "Open-Meteo poll failed"),
                }
            }
        });
    }

    let app = router(AppState { pool }, &config);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
