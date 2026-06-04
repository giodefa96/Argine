//! Argine backend library: configuration and HTTP router.
//! The binary (`main.rs`) is a thin wrapper around this so the router can be tested.

pub mod api;
pub mod arpa;
pub mod config;
pub mod domain;
pub mod open_meteo;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{routing::get, Json, Router};
use config::Config;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

/// Shared application state passed to handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

/// Build the application router from state and configuration.
pub fn router(state: AppState, config: &Config) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/stations", get(api::list_stations))
        .route("/stations/{id}", get(api::get_station))
        .route(
            "/stations/{id}/observations",
            get(api::station_observations),
        )
        .layer(cors_layer(config))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Liveness: the process is up and serving HTTP. No dependencies, so it stays green
/// even when downstream systems (DB) are down.
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// Readiness: the process can serve real traffic — here, the database is reachable.
async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(e) => {
            // Never leak the raw DB error to clients; log it, return a generic status.
            tracing::warn!(error = %e, "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "unavailable" })),
            )
        }
    }
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
