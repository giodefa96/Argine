//! DB + HTTP integration tests for ARPA ingestion. `wiremock` stands in for the Socrata
//! API (never hit the real endpoint in CI); `#[sqlx::test]` provides an ephemeral DB.
#![cfg(feature = "integration")]

use argine_backend::arpa::{self, ArpaClient};
use argine_backend::domain::{self, Metric};
use sqlx::PgPool;
use time::macros::datetime;
use time::OffsetDateTime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A time window wide enough to capture any test observation.
const WIDE: (OffsetDateTime, OffsetDateTime) =
    (OffsetDateTime::UNIX_EPOCH, datetime!(2030-01-01 0:00 UTC));

fn one_row(id: &str, data: &str, valore: &str) -> serde_json::Value {
    serde_json::json!([{ "idsensore": id, "data": data, "valore": valore, "stato": "VA" }])
}

/// Every sensor the seed list polls: the level sensor plus its co-located rain gauge.
fn all_sensor_ids() -> Vec<&'static str> {
    arpa::SEVESO_STATIONS
        .iter()
        .flat_map(|s| std::iter::once(s.sensor_id).chain(s.rain_sensor_id))
        .collect()
}

#[sqlx::test]
async fn poll_once_stores_level_and_rain_on_the_same_station(pool: PgPool) {
    let server = MockServer::start().await;
    // Each sensor returns one row of "100" (level → 1.0 m; rain → 100 mm).
    for id in all_sensor_ids() {
        Mock::given(method("GET"))
            .and(path("/647i-nhxk.json"))
            .and(query_param("idsensore", id))
            .respond_with(ResponseTemplate::new(200).set_body_json(one_row(
                id,
                "2026-06-03T00:30:00.000",
                "100",
            )))
            .mount(&server)
            .await;
    }
    let client = ArpaClient::new(server.uri());

    // 3 stations × (level + rain) = 6 observations.
    let stored = arpa::poll_once(&pool, &client).await.unwrap();
    assert_eq!(stored, 6);

    let stations = domain::list_stations(&pool).await.unwrap();
    assert_eq!(stations.len(), 3);

    // Niguarda has both a level (100 cm → 1.0 m) and rain (100 mm) series on the same station.
    let niguarda = stations.iter().find(|s| s.external_id == "3118").unwrap();
    let level = domain::observations_in_range(&pool, niguarda.id, Metric::LevelM, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(level.len(), 1);
    assert_eq!(level[0].value, 1.0);
    let rain = domain::observations_in_range(&pool, niguarda.id, Metric::RainMm, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(rain.len(), 1);
    assert_eq!(rain[0].value, 100.0);

    // A second poll upserts the same points — no duplicates.
    let stored2 = arpa::poll_once(&pool, &client).await.unwrap();
    assert_eq!(stored2, 6);
    let level2 = domain::observations_in_range(&pool, niguarda.id, Metric::LevelM, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(level2.len(), 1, "idempotent upsert, not a duplicate");
}

#[sqlx::test]
async fn backfill_pages_past_sentinel_only_page_until_raw_empty(pool: PgPool) {
    let server = MockServer::start().await;
    // For every sensor (level + rain): page 0 normalizes to nothing (all-sentinel outage),
    // page 1000 has a valid row, page 2000 is the real (raw-empty) end. Backfill must NOT
    // stop at page 0 just because it normalized empty — it terminates on the raw count.
    for id in all_sensor_ids() {
        Mock::given(method("GET"))
            .and(path("/3e8b-w7ay.json"))
            .and(query_param("idsensore", id))
            .and(query_param("$offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(one_row(
                id,
                "2021-01-01T00:00:00.000",
                "-9999", // sentinel → dropped by normalize, but raw count is 1
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/3e8b-w7ay.json"))
            .and(query_param("idsensore", id))
            .and(query_param("$offset", "1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(one_row(
                id,
                "2021-01-01T00:10:00.000",
                "50",
            )))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/3e8b-w7ay.json"))
        .and(query_param("$offset", "2000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;
    let client = ArpaClient::new(server.uri());

    // 3 stations × (level + rain), one valid row each = 6.
    let stored = arpa::backfill(&pool, &client).await.unwrap();
    assert_eq!(
        stored, 6,
        "the valid row on page 1000 must survive the empty page 0, for both metrics"
    );

    let stations = domain::list_stations(&pool).await.unwrap();
    let cantu = stations.iter().find(|s| s.external_id == "8119").unwrap();
    let level = domain::observations_in_range(&pool, cantu.id, Metric::LevelM, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(level.len(), 1);
    assert_eq!(level[0].value, 0.5); // 50 cm
    let rain = domain::observations_in_range(&pool, cantu.id, Metric::RainMm, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(rain.len(), 1);
    assert_eq!(rain[0].value, 50.0); // 50 mm
}
