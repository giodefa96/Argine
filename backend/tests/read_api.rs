//! Integration tests for the read API, via `oneshot` against a seeded ephemeral DB.
#![cfg(feature = "integration")]

use argine_backend::config::{Config, Environment};
use argine_backend::domain::{self, AlertLevel, Metric, NewStation, StationKind};
use argine_backend::{router, AppState};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use sqlx::PgPool;
use time::macros::datetime;
use tower::ServiceExt;

fn test_config() -> Config {
    Config {
        environment: Environment::Local,
        bind_addr: "127.0.0.1:0".to_string(),
        secret_key: "test-secret".to_string(),
        database_url: "unused".to_string(),
        cors_origins: vec![],
    }
}

fn station(external_id: &str, name: &str) -> NewStation {
    NewStation {
        source: "arpa_lombardia".to_string(),
        external_id: external_id.to_string(),
        name: name.to_string(),
        river: "Seveso".to_string(),
        kind: StationKind::Hydrometric,
        lat: 45.5,
        lon: 9.2,
    }
}

async fn get(pool: &PgPool, uri: &str) -> (StatusCode, Value) {
    let app = router(AppState { pool: pool.clone() }, &test_config());
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

#[sqlx::test]
async fn list_stations_returns_seeded(pool: PgPool) {
    domain::upsert_station(&pool, &station("3118", "Milano Niguarda"))
        .await
        .unwrap();
    domain::upsert_station(&pool, &station("8121", "Paderno Dugnano"))
        .await
        .unwrap();

    let (status, body) = get(&pool, "/stations").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["external_id"], "3118");
    assert_eq!(arr[0]["river"], "Seveso");
    assert_eq!(arr[0]["kind"], "hydrometric");
}

#[sqlx::test]
async fn get_station_includes_thresholds(pool: PgPool) {
    let s = domain::upsert_station(&pool, &station("3118", "Milano Niguarda"))
        .await
        .unwrap();
    domain::upsert_threshold(&pool, s.id, AlertLevel::Yellow, 1.5)
        .await
        .unwrap();

    let (status, body) = get(&pool, &format!("/stations/{}", s.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["external_id"], "3118");
    let thr = body["thresholds"].as_array().unwrap();
    assert_eq!(thr.len(), 1);
    assert_eq!(thr[0]["level"], "yellow");
    assert_eq!(thr[0]["value_m"], 1.5);
}

#[sqlx::test]
async fn get_unknown_station_is_404(pool: PgPool) {
    let (status, _) = get(&pool, "/stations/999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn observations_in_range_and_limit(pool: PgPool) {
    let s = domain::upsert_station(&pool, &station("3118", "Milano Niguarda"))
        .await
        .unwrap();
    for (ts, v) in [
        (datetime!(2026-06-01 00:00 UTC), 0.5),
        (datetime!(2026-06-01 01:00 UTC), 0.7),
        (datetime!(2026-06-01 02:00 UTC), 0.9),
    ] {
        domain::upsert_observation(&pool, s.id, ts, Metric::LevelM, v)
            .await
            .unwrap();
    }

    // Full range → all three, ascending.
    let (status, body) = get(
        &pool,
        &format!(
            "/stations/{}/observations?from=2026-05-01T00:00:00Z&to=2026-07-01T00:00:00Z",
            s.id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["value"], 0.5);
    assert_eq!(arr[0]["metric"], "level_m");

    // limit caps the result.
    let (_, body) = get(
        &pool,
        &format!(
            "/stations/{}/observations?from=2026-05-01T00:00:00Z&to=2026-07-01T00:00:00Z&limit=2",
            s.id
        ),
    )
    .await;
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[sqlx::test]
async fn observations_rejects_bad_params(pool: PgPool) {
    let s = domain::upsert_station(&pool, &station("3118", "Niguarda"))
        .await
        .unwrap();

    // from > to
    let (status, _) = get(
        &pool,
        &format!(
            "/stations/{}/observations?from=2026-07-01T00:00:00Z&to=2026-06-01T00:00:00Z",
            s.id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // unknown metric
    let (status, _) = get(
        &pool,
        &format!("/stations/{}/observations?metric=flow", s.id),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // bad timestamp
    let (status, _) = get(
        &pool,
        &format!("/stations/{}/observations?from=not-a-date", s.id),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn observations_bucketed_averages_level_and_sums_rain(pool: PgPool) {
    let s = domain::upsert_station(&pool, &station("3118", "Milano Niguarda"))
        .await
        .unwrap();
    // Two points in hour 00 and one in hour 01, for both metrics.
    for (ts, level, rain) in [
        (datetime!(2026-06-01 00:10 UTC), 0.4, 2.0),
        (datetime!(2026-06-01 00:50 UTC), 0.6, 3.0),
        (datetime!(2026-06-01 01:10 UTC), 1.0, 1.0),
    ] {
        domain::upsert_observation(&pool, s.id, ts, Metric::LevelM, level)
            .await
            .unwrap();
        domain::upsert_observation(&pool, s.id, ts, Metric::RainMm, rain)
            .await
            .unwrap();
    }
    let range = "from=2026-06-01T00:00:00Z&to=2026-06-01T02:00:00Z";

    // Level is averaged per bucket: hour 00 → (0.4+0.6)/2 = 0.5.
    let (status, body) = get(
        &pool,
        &format!("/stations/{}/observations?{range}&bucket=1h", s.id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["ts"], "2026-06-01T00:00:00Z"); // bucket start
    assert_eq!(arr[0]["value"], 0.5);
    assert_eq!(arr[1]["value"], 1.0);

    // Rain is summed per bucket: hour 00 → 2+3 = 5 mm.
    let (_, body) = get(
        &pool,
        &format!(
            "/stations/{}/observations?{range}&bucket=1h&metric=rain_mm",
            s.id
        ),
    )
    .await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr[0]["value"], 5.0);
    assert_eq!(arr[1]["value"], 1.0);

    // Unknown bucket → 400.
    let (status, _) = get(&pool, &format!("/stations/{}/observations?bucket=5m", s.id)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
