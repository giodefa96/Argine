//! DB + HTTP integration tests for Open-Meteo forecast ingestion. `wiremock` stands in
//! for the API (never hit the real endpoint in CI); `#[sqlx::test]` provides an ephemeral DB.
#![cfg(feature = "integration")]

use argine_backend::domain;
use argine_backend::open_meteo::{self, OpenMeteoClient};
use sqlx::PgPool;
use time::macros::datetime;

/// An Open-Meteo response body with two forecast hours (the second is `null` → dropped).
fn body() -> serde_json::Value {
    serde_json::json!({
        "hourly": {
            "time": ["2026-06-04T00:00", "2026-06-04T01:00", "2026-06-04T02:00"],
            "precipitation": [0.0, null, 2.5]
        }
    })
}

async fn mock_forecast(server: &wiremock::MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/forecast"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body()))
        .mount(server)
        .await;
}

#[sqlx::test]
async fn poll_once_stores_one_run_per_station(pool: PgPool) {
    let server = wiremock::MockServer::start().await;
    mock_forecast(&server).await;
    let client = OpenMeteoClient::new(server.uri());
    let run = datetime!(2026-06-04 06:00 UTC);

    // 3 stations × 2 valid hours (the null is dropped) = 6 points.
    let stored = open_meteo::poll_once(&pool, &client, run).await.unwrap();
    assert_eq!(stored, 6);

    let stations = domain::list_stations(&pool).await.unwrap();
    assert_eq!(stations.len(), 3);
    let niguarda = stations.iter().find(|s| s.external_id == "3118").unwrap();
    let fc = domain::latest_weather_forecast(&pool, niguarda.id)
        .await
        .unwrap();
    assert_eq!(fc.len(), 2);
    assert_eq!(fc[0].ts, datetime!(2026-06-04 00:00 UTC));
    assert_eq!(fc[0].rain_mm, 0.0);
    assert_eq!(fc[1].ts, datetime!(2026-06-04 02:00 UTC));
    assert_eq!(fc[1].rain_mm, 2.5);
    assert_eq!(fc[0].run_ts, run);
    assert_eq!(fc[0].model, open_meteo::MODEL);

    // Re-polling the same run upserts in place — no duplicates.
    let stored2 = open_meteo::poll_once(&pool, &client, run).await.unwrap();
    assert_eq!(stored2, 6);
    let fc2 = domain::latest_weather_forecast(&pool, niguarda.id)
        .await
        .unwrap();
    assert_eq!(fc2.len(), 2, "idempotent upsert, not a duplicate");
}

#[sqlx::test]
async fn a_new_run_is_kept_alongside_the_old_and_becomes_latest(pool: PgPool) {
    let server = wiremock::MockServer::start().await;
    mock_forecast(&server).await;
    let client = OpenMeteoClient::new(server.uri());
    let run1 = datetime!(2026-06-04 06:00 UTC);
    let run2 = datetime!(2026-06-04 07:00 UTC);

    open_meteo::poll_once(&pool, &client, run1).await.unwrap();
    open_meteo::poll_once(&pool, &client, run2).await.unwrap();

    let stations = domain::list_stations(&pool).await.unwrap();
    let cantu = stations.iter().find(|s| s.external_id == "8119").unwrap();

    // Both runs persist (backtesting needs the history of what was forecast when) …
    let runs: (i64,) =
        sqlx::query_as("SELECT count(DISTINCT run_ts) FROM weather_forecast WHERE station_id = $1")
            .bind(cantu.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(runs.0, 2);

    // … and "latest" returns only the newest one.
    let latest = domain::latest_weather_forecast(&pool, cantu.id)
        .await
        .unwrap();
    assert_eq!(latest.len(), 2);
    assert!(latest.iter().all(|f| f.run_ts == run2));
}
