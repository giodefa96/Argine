//! Argine backend library: configuration and HTTP router.
//! The binary (`main.rs`) is a thin wrapper around this so the router can be tested.

pub mod config;

use axum::{routing::get, Json, Router};
use config::Config;
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

/// Build the application router from configuration.
pub fn router(config: &Config) -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(cors_layer(config))
        .layer(TraceLayer::new_for_http())
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
