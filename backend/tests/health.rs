//! Integration test for DB-independent routes, exercised in-process via `oneshot`.
//! Uses a *lazy* pool so `/health` is tested without contacting a database.

use argine_backend::config::{Config, Environment};
use argine_backend::{router, AppState};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        environment: Environment::Local,
        bind_addr: "127.0.0.1:0".to_string(),
        secret_key: "test-secret".to_string(),
        database_url: "postgres://argine:test@localhost:5432/argine".to_string(),
        cors_origins: vec![],
    }
}

/// A pool that never actually connects (lazy) — fine for routes that issue no queries.
fn lazy_state() -> AppState {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://argine:test@localhost:5432/argine")
        .expect("build lazy pool");
    AppState { pool }
}

#[tokio::test]
async fn health_returns_ok_json() {
    let app = router(lazy_state(), &test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], br#"{"status":"ok"}"#);
}

#[tokio::test]
async fn unknown_route_is_404() {
    let app = router(lazy_state(), &test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
