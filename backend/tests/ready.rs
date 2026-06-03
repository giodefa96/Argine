//! DB-backed integration test for the `/ready` readiness endpoint.
//!
//! Gated behind the `integration` feature so the default `cargo test` runs DB-free.
//! CI enables it via `--all-features` with a TimescaleDB service container.
//! `#[sqlx::test]` provisions an ephemeral database per test and runs `migrations/`.
#![cfg(feature = "integration")]

use argine_backend::config::{Config, Environment};
use argine_backend::{router, AppState};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        environment: Environment::Local,
        bind_addr: "127.0.0.1:0".to_string(),
        secret_key: "test-secret".to_string(),
        database_url: "unused-in-test".to_string(),
        cors_origins: vec![],
    }
}

#[sqlx::test]
async fn ready_returns_ok_when_db_reachable(pool: sqlx::PgPool) {
    let app = router(AppState { pool }, &test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], br#"{"status":"ready"}"#);
}

#[sqlx::test]
async fn migrations_created_the_smoke_table(pool: sqlx::PgPool) {
    // The bootstrap migration must have run, so the smoke table exists and is queryable.
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('public.schema_smoke') IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(exists, "schema_smoke table should exist after migrations");
}
