//! Integration test for the HTTP router, exercised in-process via `oneshot`
//! (no real network bind).

use argine_backend::config::{Config, Environment};
use argine_backend::router;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        environment: Environment::Local,
        bind_addr: "127.0.0.1:0".to_string(),
        secret_key: "test-secret".to_string(),
        cors_origins: vec![],
    }
}

#[tokio::test]
async fn health_returns_ok_json() {
    let app = router(&test_config());

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
    let app = router(&test_config());

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
