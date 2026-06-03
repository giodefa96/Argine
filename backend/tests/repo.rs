//! DB-backed CRUD tests for the domain repository (`src/domain.rs`).
//!
//! Gated behind the `integration` feature; `#[sqlx::test]` provisions an ephemeral DB
//! per test and runs `migrations/`. CI enables it with a TimescaleDB service container.
#![cfg(feature = "integration")]

use argine_backend::domain::{self, AlertLevel, Metric, NewStation, StationKind};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

fn seveso_station() -> NewStation {
    NewStation {
        source: "arpa_lombardia".to_string(),
        external_id: "SEVESO-01".to_string(),
        name: "Milano via Valfurva".to_string(),
        river: "Seveso".to_string(),
        kind: StationKind::Hydrometric,
        lat: 45.49,
        lon: 9.20,
    }
}

#[sqlx::test]
async fn upsert_station_is_idempotent_on_source_external_id(pool: PgPool) {
    let first = domain::upsert_station(&pool, &seveso_station())
        .await
        .unwrap();

    // Same (source, external_id), changed name → updates in place, no new row.
    let mut changed = seveso_station();
    changed.name = "Milano Niguarda".to_string();
    let second = domain::upsert_station(&pool, &changed).await.unwrap();

    assert_eq!(first.id, second.id, "upsert must reuse the same row");
    assert_eq!(second.name, "Milano Niguarda");
    assert_eq!(domain::list_stations(&pool).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn get_station_roundtrip_and_missing(pool: PgPool) {
    let saved = domain::upsert_station(&pool, &seveso_station())
        .await
        .unwrap();

    let got = domain::get_station(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(got.kind, StationKind::Hydrometric);
    assert_eq!(got.river, "Seveso");
    assert_eq!(got.lat, 45.49);

    assert!(domain::get_station(&pool, 999_999).await.unwrap().is_none());
}

#[sqlx::test]
async fn thresholds_upsert_and_list(pool: PgPool) {
    let s = domain::upsert_station(&pool, &seveso_station())
        .await
        .unwrap();

    domain::upsert_threshold(&pool, s.id, AlertLevel::Yellow, 1.0)
        .await
        .unwrap();
    domain::upsert_threshold(&pool, s.id, AlertLevel::Orange, 2.0)
        .await
        .unwrap();
    // Re-set yellow → updates value, does not add a row.
    domain::upsert_threshold(&pool, s.id, AlertLevel::Yellow, 1.5)
        .await
        .unwrap();

    let thresholds = domain::thresholds_for_station(&pool, s.id).await.unwrap();
    assert_eq!(thresholds.len(), 2);
    let yellow = thresholds
        .iter()
        .find(|t| t.level == AlertLevel::Yellow)
        .unwrap();
    assert_eq!(yellow.value_m, 1.5);
}

#[sqlx::test]
async fn observations_upsert_is_idempotent_and_range_filters(pool: PgPool) {
    let s = domain::upsert_station(&pool, &seveso_station())
        .await
        .unwrap();
    let t0 = OffsetDateTime::UNIX_EPOCH;
    let t1 = t0 + Duration::hours(1);
    let t2 = t0 + Duration::hours(2);

    domain::upsert_observation(&pool, s.id, t0, Metric::LevelM, 0.5)
        .await
        .unwrap();
    domain::upsert_observation(&pool, s.id, t1, Metric::LevelM, 0.7)
        .await
        .unwrap();
    // Same (station, metric, ts) → overwrites, no duplicate.
    domain::upsert_observation(&pool, s.id, t1, Metric::LevelM, 0.9)
        .await
        .unwrap();
    // Same ts, different metric → distinct row.
    domain::upsert_observation(&pool, s.id, t1, Metric::RainMm, 3.0)
        .await
        .unwrap();

    let levels = domain::observations_in_range(&pool, s.id, Metric::LevelM, t0, t2)
        .await
        .unwrap();
    assert_eq!(levels.len(), 2, "t1 was updated in place, not duplicated");
    assert_eq!(levels[0].value, 0.5);
    assert_eq!(levels[1].value, 0.9);

    // Narrowing the window drops t0.
    let narrowed = domain::observations_in_range(&pool, s.id, Metric::LevelM, t1, t2)
        .await
        .unwrap();
    assert_eq!(narrowed.len(), 1);
    assert_eq!(narrowed[0].ts, t1);
}
