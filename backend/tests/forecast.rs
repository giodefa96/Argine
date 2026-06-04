//! Integration tests for `GET /stations/{id}/forecast`: router via `oneshot` against a
//! seeded ephemeral DB (`#[sqlx::test]`). No HTTP mocking needed — the forecast is computed
//! from data already in the DB.
#![cfg(feature = "integration")]

use argine_backend::domain::{self, Metric, NewStation, StationKind};
use argine_backend::{config::Config, router, AppState};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;

fn app(pool: PgPool) -> axum::Router {
    let config = Config::from_env().unwrap();
    router(AppState { pool }, &config)
}

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn seed_station(pool: &PgPool) -> i64 {
    domain::upsert_station(
        pool,
        &NewStation {
            source: "test".into(),
            external_id: "fc-1".into(),
            name: "Test".into(),
            river: "Seveso".into(),
            kind: StationKind::Hydrometric,
            lat: 45.5,
            lon: 9.2,
        },
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn forecast_applies_the_baseline_to_level_and_rain(pool: PgPool) {
    let id = seed_station(&pool).await;
    let level_ts = datetime!(2026-06-04 00:00 UTC);
    domain::upsert_observation(&pool, id, level_ts, Metric::LevelM, 1.0)
        .await
        .unwrap();
    // Two future hours of forecast rain: 10 mm then 0 mm (run far in the future so the
    // handler's `now` filter keeps them).
    let run = OffsetDateTime::now_utc();
    let h1 = run + Duration::hours(1);
    let h2 = run + Duration::hours(2);
    domain::upsert_weather_forecasts(&pool, id, "best_match", run, &[(h1, 10.0), (h2, 0.0)])
        .await
        .unwrap();

    let (status, body) = get_json(app(pool), &format!("/stations/{id}/forecast")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["model"], "baseline_v0");
    assert_eq!(body["based_on"]["level_m"], 1.0);
    let points = body["points"].as_array().unwrap();
    assert_eq!(points.len(), 2);
    // α=0.02, β=0.01: h1 = 1 + 0.2 − 0.01 = 1.19; h2 = 1 + 0.2 − 0.02 = 1.18.
    assert!((points[0]["value_m"].as_f64().unwrap() - 1.19).abs() < 1e-9);
    assert!((points[1]["value_m"].as_f64().unwrap() - 1.18).abs() < 1e-9);
}

#[sqlx::test]
async fn forecast_is_empty_without_data(pool: PgPool) {
    let id = seed_station(&pool).await;
    let (status, body) = get_json(app(pool), &format!("/stations/{id}/forecast")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["points"].as_array().unwrap().len(), 0);
    assert!(body["based_on"].is_null());
}

#[sqlx::test]
async fn forecast_caps_horizon_and_validates_params(pool: PgPool) {
    let id = seed_station(&pool).await;
    domain::upsert_observation(
        &pool,
        id,
        datetime!(2026-06-04 00:00 UTC),
        Metric::LevelM,
        1.0,
    )
    .await
    .unwrap();
    // 6 future hours stored, but only hours=3 requested.
    let run = OffsetDateTime::now_utc();
    let points: Vec<_> = (1..=6).map(|i| (run + Duration::hours(i), 1.0)).collect();
    domain::upsert_weather_forecasts(&pool, id, "best_match", run, &points)
        .await
        .unwrap();

    let (status, body) = get_json(
        app(pool.clone()),
        &format!("/stations/{id}/forecast?hours=3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["points"].as_array().unwrap().len(), 3);

    let (status, _) = get_json(
        app(pool.clone()),
        &format!("/stations/{id}/forecast?hours=0"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get_json(
        app(pool.clone()),
        &format!("/stations/{id}/forecast?hours=49"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get_json(app(pool), "/stations/999999/forecast").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
