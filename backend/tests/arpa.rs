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

#[sqlx::test]
async fn poll_once_seeds_stations_and_stores_observations(pool: PgPool) {
    let server = MockServer::start().await;
    // The recent dataset returns one row per Seveso sensor (100 cm).
    for s in arpa::SEVESO_STATIONS {
        Mock::given(method("GET"))
            .and(path("/647i-nhxk.json"))
            .and(query_param("idsensore", s.sensor_id))
            .respond_with(ResponseTemplate::new(200).set_body_json(one_row(
                s.sensor_id,
                "2026-06-03T00:30:00.000",
                "100",
            )))
            .mount(&server)
            .await;
    }
    let client = ArpaClient::new(server.uri());

    let stored = arpa::poll_once(&pool, &client).await.unwrap();
    assert_eq!(stored, 3);

    // All three Seveso stations are seeded.
    let stations = domain::list_stations(&pool).await.unwrap();
    assert_eq!(stations.len(), 3);
    assert!(stations
        .iter()
        .all(|s| s.source == arpa::SOURCE && s.river == "Seveso"));

    // Niguarda's observation is stored, 100 cm → 1.0 m.
    let niguarda = stations.iter().find(|s| s.external_id == "3118").unwrap();
    let obs = domain::observations_in_range(&pool, niguarda.id, Metric::LevelM, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].value, 1.0);

    // A second poll upserts the same point — no duplicate row.
    let stored2 = arpa::poll_once(&pool, &client).await.unwrap();
    assert_eq!(stored2, 3);
    let obs2 = domain::observations_in_range(&pool, niguarda.id, Metric::LevelM, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(obs2.len(), 1, "idempotent upsert, not a duplicate");
}

#[sqlx::test]
async fn backfill_pages_past_sentinel_only_page_until_raw_empty(pool: PgPool) {
    let server = MockServer::start().await;
    // Per sensor: page 0 is a full page that normalizes to nothing (all-sentinel outage),
    // page 1000 has a valid row, page 2000 is the real (raw-empty) end. Backfill must NOT
    // stop at page 0 just because it normalized empty — it terminates on the raw count.
    for s in arpa::SEVESO_STATIONS {
        Mock::given(method("GET"))
            .and(path("/3e8b-w7ay.json"))
            .and(query_param("idsensore", s.sensor_id))
            .and(query_param("$offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(one_row(
                s.sensor_id,
                "2021-01-01T00:00:00.000",
                "-9999", // sentinel → dropped by normalize, but raw count is 1
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/3e8b-w7ay.json"))
            .and(query_param("idsensore", s.sensor_id))
            .and(query_param("$offset", "1000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(one_row(
                s.sensor_id,
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

    let stored = arpa::backfill(&pool, &client).await.unwrap();
    assert_eq!(
        stored, 3,
        "the valid row on page 1000 must survive the empty page 0"
    );

    let stations = domain::list_stations(&pool).await.unwrap();
    let cantu = stations.iter().find(|s| s.external_id == "8119").unwrap();
    let obs = domain::observations_in_range(&pool, cantu.id, Metric::LevelM, WIDE.0, WIDE.1)
        .await
        .unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].value, 0.5); // 50 cm
}
