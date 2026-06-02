//! Argine backend entry point.
//!
//! Minimal Axum service: a `/health` endpoint, config-driven CORS allowlist,
//! request tracing, and fail-fast secret validation at startup.

mod config;

use axum::{routing::get, Json, Router};
use config::Config;
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
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

    let app = Router::new()
        .route("/health", get(health))
        .layer(cors_layer(&config))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// Build the CORS layer from the explicit origin allowlist (SECURITY.md §4).
fn cors_layer(config: &Config) -> CorsLayer {
    let mut origins = Vec::new();
    for raw in &config.cors_origins {
        match raw.parse() {
            Ok(origin) => origins.push(origin),
            // Don't drop a misconfigured origin silently — surface it in the logs.
            Err(_) => tracing::warn!(origin = %raw, "ignoring invalid CORS origin"),
        }
    }
    CorsLayer::new().allow_origin(AllowOrigin::list(origins))
}
